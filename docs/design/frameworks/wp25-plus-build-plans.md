# Build plans for WP-25 to WP-43

Status: prep, 2026-10-28. This is read-only on code: nothing was built or run. It follows
`wp16-24-build-plans.md` in form and turns three lists into work a build agent can start on:

- **Part A**: what adoption waves 6-7 and fix round 5 left open (BUILD-STATUS 2026-10-28
  sections, the status notes under WP-16..24 in `00-adoption-plan.md`, the tail of
  `.claude/pm/queue.md`).
- **Part B**: the SCMS realism gaps that code can close (`scms-real/06-validation-summary.md`
  §5 and §9, and the open rows of `tests/expectations/scms-matrix.yaml`).
- **Part C**: the deferred items of `00-adoption-plan.md` §3 with the most realism value
  (Doppler-aware 802.11p PER, driver distraction, zipper merge, parking, overtaking in the
  opposing lane).

Each package gives what the code is today (`file:line` at `ac00603c`), the decision to build
against with its sources, the files to change, the settings, the tests (each with an injected
fault), the expectation rows, the goldens, and pieces of about three agent-hours each.

Citation marks are those of `wp16-24-build-plans.md`:
- **[cached]**: the source text is in the repo or its cache.
- **[repo]**: the value is already cited in a repo document, which is named.
- **[verify]**: the source is real and named, but its text is not cached. The piece that
  uses the value caches the source first and pins the clause and page. Until then, no number
  from it goes on a card.

No clause below was guessed. Where the clause is not known, only the document is named.

**Not planned here:**
- WP-07 (F2MD attacks), REDV2X and P24.4-P24.5. These are parked owner decisions.
- Items adopt-7 is building (run `wf_6212b431-de5`, base `ac00603c`):
  - the J2945/1 RV count at reception;
  - WP-19 P19.1-P19.5, which include M11 and U5 (SCMS over the RSU service channel, the WSA
    encoder and relayed reports off channel 172);
  - the NEMA concurrent-greens defect, then WP-23 P23.1-P23.3;
  - the kerbside lane-change overlaps, then WP-20 P20.1-P20.5;
  - side work with no builds: P24.1 (the LuST licence and cut), P24.3 (the ML recipe), and
    the P1-E4 GNSS body refit.

  If adopt-7 stops before P19.5, P20.4-P20.5 or P23.3, those pieces stay in their own build
  plan and come before any round below that touches the same files. They are not re-planned
  here.

## 0. Rules shared by every piece

Rules 1-7 of `wp16-24-build-plans.md` §0 apply unchanged:
1. 3 build agents at most, one heavy command at a time through `heavy.sh`.
2. Every new check is shown red under its named fault (`fault: <what> -> <test> red` in the
   commit).
3. New behaviour is off by default, and the default path stays byte-identical.
4. The six-file settings checklist (`schema.rs`, `validate.rs`, `options.rs`, `publish.rs`,
   `guide.rs`, `configuration-reference.md`).
5. Keyed draws, never a new position in a shared stream.
6. Belief only for anything V2X triggers.
7. The goldens G-SCMS, G-ML, G-LEG, G-TV and G-SENS.

Two rules are added:

8. **Wait on adopt-7.** WP-28, WP-30 and WP-31 start only after adopt-7 has merged. WP-28 also
   needs `invariants/nema-safety` green on main. Every other package also starts after that
   merge, because adopt-7 holds all three build slots.
9. **Source gates.** A piece marked *gated* starts only when its source piece has cached the
   numbers it needs. If the source piece finds none, the build piece is dropped from its round
   and the round's spare item takes the slot (§0.3).

### 0.1 Priority across the three parts (realism value per effort)

| Rank | Package | Size | Why it ranks here |
|---|---|---|---|
| 1 | WP-34 SPS-aligned BSM generation (LTE-V2X) | M, 2 pieces | Closes the matrix row `lte-latency-budget`: every LTE-V2X run is about 50 ms late today |
| 2 | WP-39 driver distraction episodes | M, 3 | Gives FCW and EEBL a benefit that can be measured through reaction time; public sources |
| 3 | WP-25 power rows for truck, bus, coach, van | S-M, 2 | `power_limit` binds only cars and trailers today |
| 4 | WP-27 R-tree for `DensityTerm` | S, 1 | The only thing between GEMV²'s density σ (measured) and a default |
| 5 | WP-28 NEMA completion | M, 4 | Needed before US presets can run `nema` |
| 6 | WP-26 measured DEM and a real hill | S, 2 | No shipped scenario has measured terrain |
| 7 | WP-29 single-bounce building reflection | M, 2 | NLOSb reception in street canyons; GEMV² models it |
| 8 | WP-40 Doppler-aware 802.11p PER | M, 3 (gated) | High-speed links fail above the AWGN SNR; the source is not cached |
| 9 | WP-35 `infoStatus` SPDU (A16) | M, 2 | Poll bytes and a two-step fetch, as deployed |
| 10 | WP-41 zipper merge | M, 3 | Lane drops and work zones |
| 11 | WP-42 parking | M, 3 | Downtown manoeuvre delay; parked-car occlusion |
| 12 | WP-36 governance calendar | M, 2 | Runs of a year or more only |
| 13 | WP-30 per-device stamp rates and fleet mix | S, 2 | P1-E4 heterogeneity; after the GNSS refit |
| 14 | WP-31 P24.2 `veremi-ext` preset and replica | S, 1 | Benchmark comparability |
| 15 | WP-37 vehicle-side SCMS checks | S, 1 | C6, R10 and the D1 censoring measure |
| 16 | WP-38 SCMS run cost | S-M, 2 | Wake scans and the QD +9 % memory |
| 17 | WP-43 overtaking in the opposing lane | L, 4 | DNPW and rural roads; large |
| 18 | WP-32 small leftovers | S, 1 | The list sweep and the world-box polygon check |
| 19 | WP-33 1609.4 sync tolerance | S, 1 | `alternating` is off and its constants are UNVERIFIED |

**Dropped:**
- **The `SlottedMac` 1609.4 gate.** No tier selects `SlottedMac` (`wiring.rs:4179-4181`;
  P16.3 note), so a gate on it changes no run. Proposal: leave it ungated, with a card line
  saying so.
- **The post-quantum RSU certificate cycle.** No deployment uses PQ.
- **The SLH-DSA sign-queue drop policy.** No published policy was found (06 §9 item 12).
- **The late-stamp hold and the RA/PCA retention.** Both are owner decisions (§9 below).
- **Online calibrators (S-P11).** Real value, but they fix no open check and are not on this
  task's list.

### 0.2 Shared-file map (merge planning)

| Hot file | Packages (piece) |
|---|---|
| `v2xw-engine/src/run.rs` | 29 (P29.2, light), 33 (P33.1, light), 34 (P34.2), 35 (P35.2), 38 (P38.1), 40 (P40.3) |
| `v2xw-engine/src/run/sidelink.rs` | 34 (P34.2) |
| `v2xw-engine/src/wiring.rs` | 27 (P27.1), 29 (P29.2), 37 (P37.1) |
| `v2xw-engine/src/phase2.rs` | 35 (P35.2), 37 (P37.1, metric), 38 (P38.1) |
| `v2xw-engine/src/nema.rs`, `nema/*.rs`, `infra.rs`, `priority.rs` | 28 (all) |
| `v2xw-world/src/model.rs` | 28 (P28.1 `SignalState`, P28.2 groups), 32 (P32.1 bounding box, light) |
| `v2xw-world/src/osm.rs` | 42 (P42.2) |
| `v2xw-world/src/sumo.rs` | 41 (P41.2) |
| `v2xw-mobility/src/engine.rs` | 39 (P39.3), 41 (P41.3), 42 (P42.3), 43 (P43.2, P43.3) |
| `v2xw-mobility/src/drivers.rs` | 39 (P39.2) |
| `v2xw-mobility/src/classes.rs` | 25 (P25.2) |
| `v2xw-mobility/src/intersection/` | 41 (P41.2), 28 (P28.1, the new state's reading) |
| `v2xw-radio/src/obstacle.rs`, `prop.rs` | 29 (P29.1), 27 (P27.1, light) |
| `v2xw-radio/src/per.rs`, `phy.rs` | 40 (P40.2, P40.3) |
| `v2xw-radio/src/sps.rs`, `v2xw-node/src/generate.rs` | 34 (P34.1) |
| `v2xw-node/src/bsm_timing.rs` | 30 (P30.1) |
| `v2xw-proto/src/scms/governance.rs`, `run/crl.rs`, `v2xw-sec/src/ctl.rs` | 36 (P36.1, P36.2) |
| `v2xw-msg` (1609.2.1 `CertManagementPdu`) | 35 (P35.1) |
| scenario `schema/validate/options/publish/guide` | almost every piece (light, appended blocks) |

Each piece appends its own block to the scenario files, and the integrator merges those
blocks, as in waves 1-7.

### 0.3 Spare items for a free slot

Use these when a gated piece is dropped, or when the side agent has no source piece:
- **Build:** P32.1.
- **Side (no build):** the owed tiny-slot SCMS re-runs of 06 §9 "Testing still owed" (the
  pending live rows, MAL1, MWP), and the owner-decision brief (§ "Owner decisions").

---

# Part A. Leftovers from adoption waves 6-7 and fix round 5

## WP-25 Power rows for the rigid truck, bus, coach and van

**Today.**
- `VehicleClass::dynamics` (`v2xw-mobility/src/classes.rs:459-490`) has two rows:
  - passenger: PHEMlight `PC_D_EU4`;
  - trailer: the Green Book 120 kg/kW design truck.

  Every other class returns `None`, so `power_grade_bound` (`engine.rs:883`, applied at
  `:7238`) gives it the grade term only.
- `grade-dynamics.md:41-44` explains why there is no rigid-truck row: the design ratio would
  cap SUMO's 4.5 t truck near 70 km/h on the flat, and "NCHRP 505 has field data; not read".
- PHEMlight in `v2xw-energy/src/phemlight.rs:315-319` resolves passenger cars only.

**Decision.**
1. **Rows from measured or certified figures, never from the design ratio.**
   - **Rigid truck:** weight-to-power from NCHRP Report 505 (Harwood et al. 2003, *Review of
     Truck Characteristics as Factors in Roadway Design*) [verify: public on TRB; the
     single-unit table and page]. The mass is SUMO's `truck` vType default [repo:
     `classes.rs` sumo-vtypes card].
   - **Bus and coach:** rated power and test weight from FTA "Altoona" bus test reports for
     a 40 ft transit bus and a 45 ft coach [verify: public, Penn State LTI Bus Research and
     Testing Center; pin one report per row]. Alternatively, the PHEMlight heavy-duty `.veh`
     files in SUMO's `data/emissions/PHEMlight` [verify: the file names; EPL-2.0, like the
     `PC_D_EU4` transcription].
   - **Delivery van:** the EPA *Test Car List* data file (rated horsepower, equivalent test
     weight, road-load target coefficients A/B/C) for one high-volume cargo van [verify:
     public, fueleconomy.gov / EPA data].
   - **C_dA and c_rr:** C_dA from the same source when it gives one. Otherwise convert the
     EPA A/B/C target coefficients (B gives rolling, C gives aero). For heavy rows, the 40 CFR
     1037.520 bins already used for the trailer.
2. **A row's mass is its source's mass.** The class's own `spec().mass_kg` changes only if
   the owner accepts it, because it moves G-TV (`cities/*`).
3. **The crawl band.** The P22.3 band is an ASSUMPTION. P25.1 also reads the NCHRP 505
   speed-distance curves for a 5 % grade. If those curves are usable, the band becomes cited
   [verify].

**Files and functions.**
- `v2xw-mobility/src/classes.rs` `dynamics()`: new rows `Truck`, `Bus`, `Coach` and
  `Delivery`, using the variant names `classes.rs` already has. Each `source` string names
  the table and page.
- `docs/design/research/grade-dynamics.md`: the new rows and how each was derived.
- `v2xw-trafficval`: `calibration/truck-grade-crawl` gains a rigid-truck case when NCHRP 505
  gives a curve.

**Settings.** None. `traffic.vehicle_dynamics.power_limit` already gates the rows.

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Each row reproduces its source's own flat top speed or 0-60 mph time within the stated tolerance | `classes::tests::heavy_rows_reproduce_their_sources` (the figure each source gives) | power in hp read as kW: red |
| A rigid truck is not held to the trailer's crawl | `vehicle_dynamics.rs::a_rigid_truck_outclimbs_the_design_trailer` on `graded_ring` 5 % | the trailer row reused for the truck: red |
| Flat city runs are unchanged with `power_limit` off | the existing grade-only identity test | n/a |

**Expectation rows** (`expectations/traffic.yaml`, family `grade-truck`):
`{setting: traffic.vehicle_dynamics.power_limit, change: true, metric: traffic.speed_mean_mps,
expect: down, source: "<NCHRP 505 table> on a 5 % draped climb with 30 % rigid trucks"}`.

**Goldens.** None on the default path (`power_limit` off).

**Pieces.**
- **P25.1** (3 h, no cargo): cache NCHRP 505, one Altoona report per bus class, one EPA Test
  Car List row, and the PHEMlight HDV file names. Write the table into `grade-dynamics.md`
  with page numbers.
- **P25.2** (3 h): the rows, the two tests, the trafficval case, the expectation row and the
  faults.

**Sources (2026-10-28, P25.1, done).** All four rows have read sources, with pages, in
`docs/design/research/grade-dynamics.md` § "Sources for the rigid truck, bus, coach and van":
- **Rigid truck:** NCHRP 505 Table 30 (p. 55, Gillespie 1985): 26 straight trucks, pooled
  82.9 lb/hp. App. D has no single-unit split. Air and rolling come from eqs. 20 and 23
  (pp. 54-55).
- **Bus and coach:** Altoona 1211-P (New Flyer XD40, ISL9 280, SLW 33,320 lb) and 1209
  (Prevost X3-45, D13H435, SLW 47,110 lb), with their interval times. At η 0.9 the transit
  bus is twice as fast as measured, so η is fitted on each run's last interval
  (0.54, 0.84) and checked on the whole run from rest (+2 %, −9 %).
- **Van:** EPA Test Car List MY2020, Ford Transit RWD Wagon Med Roof (275 hp, ETW
  6,500 lb, A/B/C). No full-size cargo van is listed in MY2023 or MY2025.
- **PHEMlight HDV files:** not open, so dropped.
- **Crawl band:** App. E's TWOPAS equations, checked against the report's own example,
  give the design truck 12.04 m/s on 5 %. The balance gives 12.90 m/s (+7 %), so the band
  stays the balance, with TWOPAS reported beside it.
- **Rigid-truck crawl case:** its power crawl on 5 % is 22.95 m/s (TWOPAS 22.58 m/s),
  which is close to a 25 m/s road's desired speed. Whether a held row can go red with the
  power term off is settled in P25.2.

**P25.2 (2026-10-28, done; `power_limit` stays off by default, no golden moved).**
- **Rows:** `VehicleClass::dynamics` gains Truck, Bus, Coach and Delivery, plus a
  `c_rr_per_mps` speed term (eq. 20, the EPA B). It is zero on the old rows, so their bound
  is bit-identical.
- **Tests:**
  - `classes::tests::heavy_rows_reproduce_their_sources`: the Altoona runs from rest, bus
    +3 % and coach −9 % against a ±12 % band; Table 30's 82.94 lb/hp; the EPA road load at
    50 mph.
  - `v2xw-mobility/tests/vehicle_dynamics.rs::a_rigid_truck_outclimbs_the_design_trailer`:
    22.96 against 12.94 m/s, 2 km into the 5 % climb.
  - `calib::tests::twopas_reproduces_the_reports_own_example`: 41.9 mph.
- **The check:** `calibration/truck-grade-crawl` now holds the rigid truck to its TWOPAS
  crawl ±2 % (22.96 in [22.13, 23.03]). With the power term off it stays at 23.60, which
  is red, so a held row can fail. It also reports TWOPAS for the trailer, at 12.04 m/s.
- **Expectation row: not built.** A setting's rows run on the first family that accepts it
  (`catalogue.yaml` routes), and `power_limit` is already judged on `grade-truck`, which has
  no rigid trucks or buses. A second family needs a per-row family route, which the sweep
  does not have.

---

## WP-26 A measured DEM and a real hill

**Today.**
- `dem.rs` reads SRTM `.hgt` and ESRI ASCII grids (`dem.rs:11`, `:339-346`). It does not
  read GeoTIFF on purpose (`:34`).
- `wiring::attach_terrain` (`wiring.rs:196-211`) drapes only under `world.terrain.drape`.
- The only DEM in the repo is the synthetic 5 % plane `scenarios/sensitivity/grade-5pct.asc`
  (P22.4). Every shipped city has `terrain.dem: null`, for example
  `phase2-manhattan.yaml:68`.

**Decision.**
1. **Source.** 1-arc-second SRTM-format tiles from the public AWS "Terrain Tiles" open dataset
   (`skadi` layout, `.hgt.gz`), which need no login [verify: the dataset's licence and
   attribution text]. USGS 3DEP 1/3 arc-second data is public domain but is GeoTIFF. It can
   be used only after an offline conversion to ASCII grid by the source agent, recorded in
   the provenance file.
2. **Site.** The Portland downtown map already shipped (`scenarios/portland-downtown.yaml`),
   because its west edge climbs into the West Hills. If the cut has no grade above 4 %, the
   source agent picks the shipped map with the steepest measured grade and says why.
3. **The vertical datum and resolution go on the card.** The `dem.rs` licence constant for
   SRTM (`:76-79`) is reused.

**Files and functions.**
- `worlds/dem/<site>.hgt` (or `.asc`) and `worlds/dem/<site>.md` (provenance, licence,
  datum).
- A new `scenarios/real-world/portland-hills.yaml` with `terrain.dem` set, `drape: true`, and
  `grade` and `power_limit` on.
- `tests/vehicle_dynamics.rs`: a measured-terrain test.

**Settings.** None new.

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Draped lane grades match the DEM | `a_measured_dem_drapes_the_lanes_to_its_heights`: 20 sampled lane points within one DEM cell's vertical accuracy of a direct bilinear read | the DEM read with rows flipped north/south: red |
| The hill changes traffic | rows below | the drape ignored: rows DEAD |

**Expectation rows** (`expectations/traffic.yaml`, new family `hills`):
- `grade: true` -> `traffic.speed_mean_mps` down;
- `world.terrain.drape: true` -> `prr` changes. Terrain diffraction now sees the hills the
  roads climb. The direction is not predicted, so the row is `expect: changes`.

**Goldens.** None. The new scenario is not in a golden.

**Pieces.**
- **P26.1** (2 h, no cargo): fetch the tiles into their own empty directory, cut them, write
  the provenance file, and measure the grade histogram over the map's lanes.
- **P26.2** (3 h): the scenario, the test, the rows and the faults.

---

## WP-27 An R-tree for `DensityTerm`

**Today.**
- `wiring::DensityTerm` (`wiring.rs:2872-2930`) keeps `statics: Vec<(Vec3, f64)>` for every
  building footprint. Its doc says "behind a grid", but it has none.
- `DensitySigma::link_density` (`v2xw-radio/src/prop.rs:624-650`) scans every footprint on
  every link, so the cost is O(buildings) per reception.
- Vehicles already come from `snapshot.actors_within`.
- P18.5 left this open "before it can be a default".
- `rstar` is already a `v2xw-radio` dependency (`foliage.rs:33`).

**Decision.**
1. **The index.** An `RTree` of footprint centroids, built once per world. A query takes the
   ellipse's bounding box, then applies `in_link_ellipse` as today.
2. **Determinism.** The hits are sorted by building index before
   `math::sum_ordered`, so the sum is bit-identical to today's scan.
3. **Fix the doc line.**

**Files.**
- `wiring.rs` `DensityTerm::{new, sigma_db}`.
- `prop.rs`: a `link_density_indexed` variant, or a `statics` iterator built from the query.

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Same σ as the scan | `wiring::tests::density_index_equals_the_scan`: 2,000 keyed-random links on the Manhattan world, bit-equal σ | hits summed in R-tree order: red on at least one link |
| It is faster | `gemv2_off.rs`: the density run's wall time recorded, as a note (not asserted) | n/a |
| Off is unchanged | `gemv2_off` (4) | n/a |

**Goldens.** None. Proposal to the owner: `density_sigma: true` (Boban 2014 measured).
That moves every channel-model digest.

**Pieces.** **P27.1** (3 h): the index, the test, the fault, and a note on Manhattan wall time
with `density_sigma` on, before and after.

---

## WP-28 NEMA completion (after adopt-7's concurrency fix and WP-23)

**Today.**
- WP-17 runs off by default (`traffic.signals.control: nema`).
- `invariants/nema-safety` is red: 189 foe pairs show green together. The cause is
  `NemaPlan::vector` (`nema/control.rs:243-271`). Adopt-7 is fixing it.
- Gaps left by the P17.5-P17.9 notes:
  - **Permissive clearance.** A permissive left's yellow maps to `ProtectedClearance`. The
    world has no state for the end of a permissive interval (`infra.rs:95-104`;
    `SignalState`, `v2xw-world/src/model.rs:2386-2402`). J2735 code 7
    `PermissiveClearance` exists in the codec (`v2xw-msg/src/j2735/spat.rs:219, 237, 253`)
    and is never sent.
  - **Per-left-lane signal groups.** `nema/assign.rs` gives one group per movement. A
    two-lane exclusive left with one lane shared shows one face for both lanes.
  - **Offset transition.** A new cycle or offset (from coordination or WP-23 programmes) is
    applied at once (`nema/control.rs:345` re-bases `plan.offset_s`). Real controllers
    transition by shortway or dwell.
  - **Priority omission.** Phases between a priority cut and the requested phase are not
    omitted (P17.9).

**Decision.**
1. **`SignalState::GreenYieldClearance`.** A new variant, the permissive yellow. It maps to
   `MovementPhaseState::PermissiveClearance` (7). Mobility reads it as `Amber` for the
   stop-or-go decision and as `GreenYield` for whom it yields to. Sources: MUTCD 2009 §4D.05
   (yellow meaning), §4D.17-4D.20 (FYA) [repo: wp16-24 sources table, public], SAE J2735
   `MovementPhaseState` [cached: codec].

   The variant is used only by `nema` plans, so fixed-time hashes do not move. The world
   hash covers plans only when they are serialised; P28.1 checks this with the
   `hash::tests` pattern.
2. **Per-lane groups.** `assign.rs` gives an exclusive left lane its own group when the
   approach has more than one left lane and one of them is shared. This is US practice in
   STM 2e ch. 4 (lane-use and display) [repo: STM read 2026-10-27, P17.1 note; pin the
   exhibit].
3. **Offset transition.** Shortway (lengthen or shorten each cycle by at most a set share of
   the cycle) and dwell (hold the coordinated phase), from STM 2e §7.4 [verify: the section
   number and the shortway limits in the read copy]. The default `shortway` cap is the STM's
   value if it gives one. Otherwise it is an ASSUMPTION setting, labelled.
4. **Omission.** During a priority cut, phases with no call between the cut and the
   requested phase are skipped. This is the NTCIP 1211 behaviour already cited in
   `priority.rs` [repo].

**Files.**
- `model.rs` `SignalState`;
- `infra.rs:95` `movement_phase`;
- `nema/control.rs` `vector`;
- `v2xw-mobility/src/intersection/` (reading the new state);
- `nema/assign.rs`;
- `nema.rs` `Coordination` (`:243-300`) and `step` (`:711`);
- `priority.rs`.

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `traffic.signals.nema.offset_transition` | `shortway` | choice `shortway`/`dwell`/`immediate` | STM 2e [verify §] |
| `traffic.signals.nema.shortway_max_share` | STM value or ASSUMPTION | fraction | STM 2e [verify] |
| `traffic.signals.nema.per_lane_left_groups` | `true` under `nema` | bool | STM 2e ch. 4 |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| A permissive left's yellow is sent as code 7 and read as clearance | `nema::control::tests::permissive_yellow_is_permissive_clearance`; `tests/nema_spat.rs` counts code 7 > 0 | mapped back to `ProtectedClearance`: red |
| Two left lanes, one shared, get two faces | `nema::assign::tests::a_shared_left_lane_has_its_own_group` | groups merged: red |
| A new offset is reached by shortway within the cap, never by a jump | `nema::tests::offset_transition_is_shortway` (largest per-cycle change ≤ cap) | `immediate` forced: red |
| Omitted phases | `nema::tests::priority_cut_omits_uncalled_phases` | omission off: red |
| nema-safety stays green | G-TV `invariants/nema-safety` | n/a |

**Expectation rows** (`traffic.yaml`, `traffic.signals.nema` context): `offset_transition:
dwell` against `shortway` -> `traffic.delay_mean_s` up during the transition window.

**Goldens.** None (`nema` is off). After WP-28 the default proposal for US suburban and
arterial presets (P17.10) can go to the owner.

**Pieces.**
- **P28.1** (3 h): permissive clearance, end to end.
- **P28.2** (3 h): per-lane left groups.
- **P28.3** (3 h): offset transition.
- **P28.4** (3 h): omission, the rows, the Manhattan nema run recorded, and the
  default-proposal note.

---

## WP-29 GEMV² single-bounce building reflection

**Today.**
- `ObstacleStack` (`wiring.rs:2509-2522`) has the building, corner, vehicle, terrain,
  foliage and density terms.
- P18.5 did not build the reflection term, and it has no key.
- GEMV² Table V gives NLOSb σ 4.1 dB "0 when reflections are computed"
  (`gemv2-p833-constants.md:44`, Boban 2014 [cached: constants read with pages]).
- `CornerTracer` handles diffraction round street corners, not wall reflections.

**Decision.**
1. **The path.** For an NLOSb link, find the best single specular reflection off a building
   wall within the link's ellipse (GEMV² Table VI's NLOSb axis). The wall is a footprint edge
   whose mirror image of the transmitter sees the receiver, and the path is clear of other
   footprints.
2. **The loss.** Free-space loss over the unfolded length plus a reflection loss from the
   Fresnel coefficient with GEMV²'s relative permittivity for buildings [verify: the
   paper's ε_r value and page]. The term is combined with the shadowed direct path by power
   sum. When it is on, the NLOSb σ becomes 0 dB, as Table V says.
3. **Cost.** Footprint edges go behind the WP-27 R-tree, so WP-27 comes first.

**Files.**
- `v2xw-radio/src/obstacle.rs`: a new `reflection.rs`, `ReflectionTracer`.
- `wiring.rs`: `ObstacleStack.reflection`.
- `run.rs`: where `ObstacleStack` is summed (light).

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `radio.channel_model.reflection` | `false` | bool | Boban 2014 §III [cached] |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| A canyon link gains power over the shadowed path | `reflection::tests::a_mirror_wall_lifts_an_nlosb_link` (two parallel walls) | reflection loss sign flipped: red |
| No reflection through another building | `reflection::tests::a_blocked_bounce_is_not_used` | occlusion check skipped: red |
| Off is byte-identical | `gemv2_off.rs` gains a case | n/a |

**Expectation rows** (`phy-channel.yaml`, family `test-networking-dsrc` on Manhattan):
reflection on -> `prr` up; `report.loss_cause.shadowed` down.

**Goldens.** None.

**Pieces.**
- **P29.1** (3 h): the tracer and its unit tests.
- **P29.2** (3 h): wiring, the key, the rows, `gemv2_off`, and the faults.

---

## WP-30 Per-device stamp rates and the chain/fixed fleet mix (after adopt-7's GNSS refit)

**Today.**
- The `tampa_chain` profile (candidate E) is in `v2xw-node/src/bsm_timing.rs`:
  `StampChain` at `:298`, `TransitionStep` at `:312`, `profile` at `:408`.
- Candidate F (E with the rates of each of 12 chain IDs) is not in the engine. It was
  predicted at KS 0.098 at run speeds and 0.083 speed-matched, against E's 0.099 and 0.082
  (`research/synthesis/p1-realdata-baseline.md:433`).
- The same section proposes a deployment-realistic locked class: chain devices (22 of 50
  Tampa locked IDs) mixed with fixed-stamp devices (`tampa_fixed`).
- Adopt-7's side agent is refitting the GNSS body that keeps E above 0.092.

**Decision.**
1. **F.** A `StampChain` may carry `devices: Vec<DeviceRates>`. Each OBU draws one by a keyed
   draw on (seed, node), never from a shared stream. The numbers come from the candidates
   file, as for E (pinned by `the_stamp_profiles_are_the_candidates_files_numbers`).
2. **Mix.** A profile `tampa_mixed` with a chain share of 22/50 (Tampa's own count, cited in
   the baseline doc) and the rest `tampa_fixed`.
3. **Order.** Build after the GNSS refit lands, so the KS rows are judged on the refit body.

**Files.**
- `bsm_timing.rs`;
- `bsm-stamp-steps-candidates.yaml`;
- `tests/bsm_timing.rs`;
- `scenarios/.../p1-e4-cf.yaml`;
- rows in `p1-stamp-e.yaml`.

**Settings.** New profile names under the existing stamp profile key (no new key).

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Each device keeps its rates for the whole run | `bsm_timing::tests::a_device_keeps_its_drawn_rates` | redraw per BSM: red |
| The mix share holds | `tampa_mixed` share of chain devices within binomial 2σ of 22/50 over 500 OBUs | draw keyed per message: red |
| P1-E4 | `p1-stamp-e.yaml` rows for F and the mix on the speed-matched `p1-e4-u` | n/a (judged) |

**Goldens.** None. The default stamp profile is the owner's choice.

**Pieces.**
- **P30.1** (3 h): F and the mix in `bsm_timing.rs`, with the unit tests.
- **P30.2** (3 h): the scenarios, the engine test, two runs under `runtiny.sh`, and the
  rows.

---

## WP-31 WP-24 P24.2: the `veremi-ext` GNSS preset and the replica scenario

**Today.**
- The P24.2 plan is in `wp16-24-build-plans.md` §WP-24.
- `GnssSettings` is at `schema.rs:1655`.
- `messages.rate_hz` exists.
- The LuST cut (P24.1) and WP-23 import come from adopt-7.

**Decision.** As in that plan. The preset carries F2MD's `veremiConf` constants [repo:
`f2md.md` §4.2] and is never a default. The replica uses the P24.1 cut with the WP-23
programmes "as in the file".

**Tests.** As in that plan: `gnss::tests::veremi_ext_confidences_sit_in_their_ranges`, where
U(3,5) drawn as U(0,5) turns it red, and a 120 s smoke run.

**Goldens.** None.

**Pieces.** **P31.1** (3 h). It waits for adopt-7's P24.1 and WP-23 merge.

---

## WP-32 Small leftovers: list-valued sweeps and the world-box polygon check

**Today.**
- **The max-recall row.** It "cannot be swept by `--expect`", because the setting is a list.
  The sweep reads scalar changes only (`sensitivity/sweep.rs:17`).
- **The polygon check.** WP-21's "polygon inside the world box" check is open, because the
  world publishes no local bounding box (P21.2 note; `timeline/trigger.rs`, validation at
  `validate.rs:3865`).

**Decision.**
1. A row's `change` may be a YAML list. It is applied as one value.
2. `World::local_bounds()` returns the min/max of the lane geometry, computed when needed and
   not hashed. The loader warns, and does not refuse, when every corner of an `in_polygon`
   lies outside it.

**Tests.**
- `R5.nema.max-recall` judged by `--expect`. Fault: the list is applied as its first element
  -> red.
- `validate::tests::an_off_map_polygon_warns`. Fault: the bounds are inverted -> red.

**Pieces.** **P32.1** (3 h).

---

## WP-33 1609.4 sync tolerance (low priority)

**Today.**
- The gate `gate_step` (`v2xw-radio/src/mac.rs:352`) uses the global schedule
  (`wave_schedule.rs:19-25`, 100/50/4 ms, UNVERIFIED).
- Per-node clock offsets exist (`v2xw-mobility/src/clock/tcxo_ocxo.rs:201` `offset_s`).
- The P16.2 card says no sync tolerance or clock error is modelled.

**Decision.**
1. With `alternating` on, each single-radio unit times its interval boundaries on its local
   clock (UTC plus `offset_s`).
2. The guard interval is the device's margin. A frame from a unit whose offset exceeds the
   guard can land in another unit's switch.
3. The guard's make-up (SyncTolerance/2 + MaxChSwitchTime) is in IEEE 1609.4 [verify:
   paywalled], so it stays one constant, and the card says so.
4. Value is low while `alternating` is off and its constants are UNVERIFIED. The piece runs
   last.

**Tests.** `mac::tests::a_late_clock_lands_in_the_peer_guard`: two units, one offset by more
than the guard; that unit's frames are lost to the switch. Fault: the global clock is used
-> red.

**Pieces.** **P33.1** (3 h).

---

# Part B. SCMS realism gaps that code can close

The 06 summary's §5 lists 22 deviations, 8 of which matter at runtime. M11 and U5 are
adopt-7's (WP-19). The remaining runtime ones that code can close are A16, H10, G-10, L-8,
R10 and C6. The no-effect rows C-9, C-10 and C-11 are cheap to close alongside them. The
matrix has 3 open rows:
- `lte-latency-budget` (`scms-matrix.yaml:769`): code, WP-34;
- `QD25-wall` (`:1572`): a re-measure on an idle machine; a run, not code;
- `QD25-memory` (`:1586`): a bisect, WP-38.

## WP-34 SPS-aligned BSM generation for LTE-V2X

**Today.**
- On an LTE-V2X sidelink the BSM is generated on its own 100 ms clock and then waits for the
  UE's SPS reservation. The matrix row `lte-latency-budget` is open: p95 113.0 ms against
  the 100 ms J3161/1 budget, mean 66.4 ms against DSRC's 12.3 ms (`scms-matrix.yaml:752-781`).
  Its note: "the BSM is generated without regard to the SPS reservation, which J3161/1 has
  the application align with".
- Generation is `MessageSchedule::due` (`v2xw-node/src/generate.rs:121`, `:212`).
- The SPS state is in `SpsEngine` (`v2xw-radio/src/sps.rs:601`): `last_grant` at `:770` and
  `latency_budget_slots` at `:686`. It is driven by `on_sidelink_timer`
  (`run/sidelink.rs:596`).
- `SpsParams::sae_j3161` (`sps.rs:175`) says that J3161/1 §7.3.3's "SPS + one-shot"
  interleaving is not modelled.

**Decision.**
1. **Align the generation instant.** When a UE holds an SPS reservation, the node's next
   periodic BSM is generated `lead` before the reserved subframe. `lead` is the node's own
   signing and stack time as the engine already models them, plus a margin setting.
2. **Follow reselection.** On each reselection (counter expiry, or a size or latency
   trigger), the generation phase moves to the new reservation. The one interval that
   changes is clamped to J2945/1's interval bounds, which the engine already holds.
3. **Event-driven BSMs are not aligned.** A hard-braking BSM goes out at once, and the MAC
   handles it as today (one-shot or wait). The J3161/1 §7.3.3 interleave stays open and is
   counted.
4. **Sources.**
   - TS 36.213 V14.4.0 §14.1.1.6 (resource selection, reservation) [cached: R2-cv2x.md:23];
   - TS 36.321 §5.14.1.1 (SPS reselection counter) [verify];
   - SAE J3161/1 (the alignment and the 100 ms budget) [verify: paywalled; second-hand via
     Abrar et al. 2026, arXiv 2608.05087, repo `sps.rs:170`];
   - Molina-Masegosa & Gozalvez 2017 (the generation/reservation mismatch) [repo: matrix
     row source].

   The margin default is an ASSUMPTION until J3161/1 is read.

**Files.**
- `v2xw-radio/src/sps.rs`: `SpsEngine::next_reserved_at(node) -> Option<slot>` and a
  reselection event.
- `v2xw-node/src/generate.rs`: `MessageSchedule::align_to(at)`.
- `run/sidelink.rs`: hand the reservation to the node at each grant and reselection.
- `run.rs`: the node wake for the aligned instant (light).

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `radio.sidelink.align_generation` | `false` (proposed `true` for LTE-V2X) | bool | J3161/1 [verify] |
| `radio.sidelink.align_margin_ms` | ASSUMPTION, labelled | ms | none until J3161/1 is read |
| report `bsm_aligned`, `bsm_realigned_on_reselection`, `bsm_unaligned_event` | n/a | count | |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| The aligned BSM meets the budget | the matrix row `lte-latency-budget` (p95 ≤ 100 ms) on `lte`, and `tests/sidelink_align.rs::aligned_bsm_waits_at_most_the_margin` (every periodic BSM's MAC wait ≤ margin + one slot) | alignment off: red |
| Reselection is followed | `sidelink_align.rs::alignment_follows_a_reselection`: a forced reselection, then the next BSM within margin of the new reservation | phase kept after reselection: red |
| J2945/1 bounds hold | the realigned interval stays inside the bounds | clamp removed: red |
| DSRC and off are unchanged | `bsm_timing` (4), `phase2` (15) digests | n/a |

**Expectation rows** (`scms-matrix.yaml` re-judged, plus `networking.yaml`, family `lte`):
`align_generation: true` -> `m.e2e_latency.bsm.mean` down; `prr` changes. The sign is not
predicted: aligned UEs keep their resources, so collisions are those of SPS.

**Goldens.** None while the key is off. The proposal to turn it on for LTE-V2X profiles moves
every LTE digest (owner).

**Pieces.**
- **P34.1** (3 h): the SPS API, `align_to`, and unit tests in `v2xw-radio` and `v2xw-node`.
- **P34.2** (3 h): engine wiring, report fields, `tests/sidelink_align.rs`, the matrix row
  re-run on `lte` under `runtiny.sh` (or the 1.2 GB slot), and the faults.

---

## WP-35 The `infoStatus` SPDU (A16)

**Today.**
- A16 (06 §5): the `infoStatus` SPDU body and its RA signature are not encoded. The device
  polls the composite CRL conditionally, so a change costs one exchange instead of two.
- The ASN.1 is cached: `CertManagementPdu.infoStatus` and `CertificateManagementInfoStatus`
  (crl, ctl, caCcf, ma, ra) are in
  `third_party/asn1/etsi/ieee1609.2.1/Ieee1609Dot2Dot1CertManagement.asn:70-80, 341-410`.
- `CrlIssuance` names the structure (`v2xw-proto/src/scms/params.rs:325-340`).
- Device polls: `phase_crl_poll` (`run.rs:4053`) and `crl_polls_due` (`run.rs:9360`).

**Decision.**
1. **Encode the SPDU.** Encode `CertificateManagementInfoStatus` with the existing codec forge
   (COER, as the other 1609.2.1 PDUs), signed by the RA certificate the device already
   holds.
2. **The device's poll.** The device fetches `infoStatus` first. It then downloads only the
   lists, the CTL or the MA certificates whose dates are newer than the ones it holds. The
   flow follows IEEE 1609.2.1 §6.3.5 [verify: paywalled body text; the ASN.1 is cached] and
   [CAMP122] §6.2.3 `download-crl` [repo: params.rs].
3. **Option.** The conditional composite poll stays available as `crl_poll: conditional`
   (today's behaviour). `info-status` is proposed as the default to the owner, because it
   moves G-SCMS.

**Files.**
- `v2xw-msg`: the `CertManagementPdu` choice;
- `v2xw-proto/src/scms`: the RA endpoint and the device flow;
- `phase2.rs`: the poll;
- `run.rs:9300-9365`: the poll result handling;
- the oracle rule for the new SPDU.

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `security.scms.crl_poll` | `conditional` | choice `conditional`/`info-status` | 1609.2.1 §6.3.5 [verify] |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| The SPDU round-trips through the cached ASN.1 | `v2xw-msg` codec test against a forge vector | `ra` field encoded as mandatory: red |
| A device fetches only what changed | `phase2::tests::info_status_fetches_only_newer_lists` | the date comparison is `>=`: red (refetch) |
| Poll bytes | `scms-fix5.yaml`-style row: Uu bytes per poll with `info-status` > `conditional` when nothing changed (one signed SPDU) | n/a |

**Goldens.** None while the setting is `conditional`.

**Pieces.**
- **P35.1** (3 h): the codec, the RA endpoint and the oracle rule.
- **P35.2** (3 h): the device flow, the setting, the tests, and a run of base.yaml under
  `runtiny.sh` for the bytes row.

---

## WP-36 Governance calendar: G-10, C-9, C-10, C-11, L-8 and H10

**Today.**
- `Governance::test_ballot` exists (`v2xw-proto/src/scms/governance.rs:55`, counter at
  `:1164`), but no scenario key schedules it (G-10).
- A CTL's `effectiveDate` is the publication deadline (C-9; `v2xw-sec/src/ctl.rs:226`).
- A run's first CTL has sequence 0 (C-10; `governance.rs:988`).
- A CTL that drops a root does not carry the dropped root's certificate (C-11).
- `remove_root` (`governance.rs:2320`) models removal but not the 365-day termination notice
  (L-8).
- The CRL Generator's `indexWithinI` saturates at 255 (`v2xw-proto/src/scms/run/crl.rs:366`
  `next_index`). A real generator would stop issuing (H10).

**Decision.** Each change follows the clause the 06 summary already cites for its row
[repo: 06 §5 and the governance checklist rows]:
1. **G-10.** `security.scms.governance.test_ballot_month` (off by default) schedules the
   yearly ballot (Elector Policy §1.1 [repo]).
2. **C-9.** `effectiveDate` is the ballot date, behind `ctl_effective_date: ballot | deadline`
   (default `deadline`, today's).
3. **C-10.** `first_ctl_sequence` (default 0; the live production CTL is 1 [repo: gov C-10]).
4. **C-11.** The CTL that drops a root carries its certificate, as gov C-11 cites; bytes
   only.
5. **L-8.** `remove_root` gains a `terminate` path: notice, then removal after 365 days.
6. **H10.** At 255 publications in one i-period the generator stops and counts
   `crl_issuance_stopped`. It never wraps. This is reached only above 127 publications a
   week.

**Files.** `governance.rs`, `ctl.rs`, `run/crl.rs`, and the scenario block
`security.scms.governance`.

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| A two-year run has two test-ballot records | `governance::tests::the_test_ballot_runs_yearly` | month off by one: red |
| The `ballot` effective date | ctl test | deadline used: red |
| Termination: notice, then removal after 365 d | `governance::tests::termination_waits_its_notice` | removal at notice: red |
| H10 | `crl::tests::issuance_stops_at_255_in_one_period` | wrap to 0: red |
| Defaults unchanged | G-SCMS digests | n/a |

**Goldens.** None at defaults. `first_ctl_sequence: 1` would move G-SCMS (owner).

**Pieces.**
- **P36.1** (3 h): G-10, C-9, C-10 and C-11.
- **P36.2** (3 h): L-8 and H10.

---

## WP-37 Vehicle-side SCMS checks: C6, R10 and the D1 censoring measure

**Today.**
- **C6.** "2 km since the last change" is distance travelled (`wiring.rs:3886`), not
  straight-line.
- **R10.** Country regions are counted in `RxStats::region_unchecked`
  (`v2xw-node/src/rxval.rs:69, 339`) and are not checked. `location_in` (`rxval.rs:660`)
  handles circles and rectangles, but not identified (country) regions.
- **D1.** `D1.slow-aca-h2` is open because the mean top-up latency is censored: 17 of 23
  top-ups never finish. `topups_in_flight` exists (`phase2.rs:1801-1804`), but not per
  provider.

**Decision.**
1. **C6.** `pseudonym.distance_metric: travelled | displacement`. The default stays
   `travelled` until the owner reads the C6 source. US pseudonyms are unaffected otherwise.
2. **R10.** Identified regions are checked against country polygons from Natural Earth
   admin-0 (public domain) [verify: version; vendoring is an owner decision]. They act only
   on ETSI certificates that carry a region. US certificates carry none, so
   `region_unchecked` stays 0 on US runs.
3. **D1.** Per-provider `topups_in_flight_at_end` and a Kaplan-Meier style "completed by t"
   share in the report, so the row can be judged without censoring.

**Tests.**
- `pseudonym` displacement on a loop route: no change before 2 km straight-line. Fault: the
  travelled metric -> red.
- `rxval` a German-region certificate refused at a Polish position. Fault: the region check
  skipped -> red.
- The D1 row re-judged.

**Pieces.** **P37.1** (3 h).

---

## WP-38 SCMS run cost: the wake index and the QD memory rise

**Today.**
- `Phase2::next_wake` / `next_wake_cause` (`phase2.rs:6048`, `:6079`) scan per node. 06 §9
  item 3 puts this at 0.6 % of samples ("the event index for the per-node wake scans").
- The `QD25-memory` row is open: 247.5 MB against 227 MB (+9 %), with a bisect
  `fe3757b9..8becf72a` owed (`scms-matrix.yaml:1586-1598`).

**Decision.**
1. A `BTreeMap<(SimTime, NodeId), Cause>` wake index, updated where each wake source
   changes. Ties are kept in node order, so the run is byte-identical.
2. The bisect runs on the QD quarter-fleet scenario with the 1.2 GB slot, one build per step
   (about 7 steps).

**Tests.**
- `phase2::tests::the_wake_index_equals_the_scan` (keyed random wake sets). Fault: stale
  entries not removed -> red.
- G-SCMS digests unchanged.

**Pieces.**
- **P38.1** (3 h): the index.
- **P38.2** (3-4 h): the bisect, with the commit that adds the memory named, and either a fix
  or a corrected row.

---

# Part C. Deferred items with public sources

## WP-39 Driver distraction episodes (S-P15)

**Today.**
- `drivers.rs` has the HDM errors (Treiber 2006), a lognormal parameter spread and a
  reaction delay: `HumanDriverParams` at `:98`, `HumanState` at `:220`, `perceive` at
  `:248`, `delayed` at `:278`.
- Driver settings are `Drivers` (`schema.rs:1749`).
- There are no episodes: attention is constant. SUMO's `MSDriverState` is EQUIVALENT only in
  its steady errors (`sumo.md:73`).

**Decision.**
1. **The process.** A two-state process per driver: attentive, and eyes-off-road. Episode
   onset rates and glance-duration distributions come from naturalistic data. During an
   episode the driver's perception is frozen at its last value, and reaction to a new
   stimulus starts only when the glance ends.
2. **Sources.**
   - Klauer et al. 2006, *The Impact of Driver Inattention on Near-Crash/Crash Risk*, NHTSA
     DOT HS 810 594 [verify: public; it reports glances over 2 s and their risk];
   - Dingus et al. 2016, PNAS 113(10) (SHRP 2 prevalence of distraction) [verify: open
     access];
   - the NHTSA Visual-Manual Driver Distraction Guidelines, 78 FR 24818 (2013; the 2 s
     single-glance and 12 s total criteria) [verify: public];
   - the 100-Car Phase II report DOT HS 810 593 is already read for the braking target
     [repo: `scms-real/testing/honest-density.md:13`].

   The process shape follows SUMO `MSDriverState` awareness (behaviour reference only).
3. **V2X.** A received, verified FCW or EEBL warning ends an episode after a warning reaction
   time [verify: from the same sources or a cited FCW study]. This is how warnings gain a
   measurable benefit (belief only, rule 6).
4. **Keyed draws** on (seed, actor, step).

**Files.**
- `drivers.rs`: `AttentionParams` and `AttentionState`, a pure process.
- `v2xw-mobility/src/engine.rs`: apply it inside the human-driver pass.
- The warning hook where FCW and EEBL already reach the driver.

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `traffic.drivers.distraction` | `false` | bool | as above |
| `traffic.drivers.distraction_rate_per_h` | source value | 1/h | Klauer 2006 / Dingus 2016 [verify] |
| `traffic.drivers.glance_dist` | source fit | choice + params | Klauer 2006 [verify] |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Glance durations match the source distribution | `drivers::tests::glances_follow_the_cited_distribution` (KS against the table) | the mean used as a constant: red |
| A distracted follower brakes later | `vehicle_dynamics`-style lead-brake test: the reaction is longer by the glance remainder | perception not frozen: red |
| A warning cuts the episode | `tests/distraction_warning.rs`: FCW on -> fewer TTC < 1.5 s events than FCW off, with distraction on | the warning hook disconnected: red (equal counts) |
| Off is byte-identical | `phase2-path`, G-TV | n/a |

**Expectation rows** (`traffic.yaml` and `safety.yaml`):
- `distraction: true` -> `traffic.ttc_below_1_5_share` up;
- with distraction on, `apps.fcw: true` -> the same metric down.

**Goldens.** None. The proposal for realistic US presets (`distraction: true`) moves G-TV
(owner).

**Pieces.**
- **P39.1** (3 h, no cargo): cache Klauer 2006, Dingus 2016 and the NHTSA guidelines; fit
  the onset rate and the glance distribution, with pages.
- **P39.2** (3 h): the pure process and its tests.
- **P39.3** (3 h): the engine wiring, the warning hook, the rows and the faults.

---

## WP-40 Doppler-aware 802.11p PER (O-P12), gated on its source

**Today.**
- `per.rs` is NIST with implementation-loss presets: `PerModel` at `:187`, `per` at `:342`.
- C-V2X has table-driven BLER curves from 3GPP R1-160284 (`bler.rs:85-133`).
- The PHY card lists "no Doppler-dependent channel-estimation loss" (`phy.rs:1603`).
- `DopplerCoherence` (`fading.rs:403-431`) time-correlates the fade. Its doc says no cited
  curve gives a per-frame PER loss at high Doppler (`:423`).

**Decision.**
1. **The curves.** Table-driven PER per (MCS, frame length, channel profile, relative speed),
   interpolated like `BlerCurve`.
2. **Sources, in order of preference.** All are [verify] until cached:
   - Anwar, Franchi, Fettweis, "Physical layer evaluation of V2X communications
     technologies: 5G NR-V2X, LTE-V2X, IEEE 802.11bd, and IEEE 802.11p", IEEE VTC2019-Fall
     [verify: an author or arXiv copy];
   - the IEEE 802.11 TGbd link-level submissions for 802.11p in the TR 37.885 / Acosta-Marum
     and Ingram profiles [verify: public on the IEEE 802.11 document server; document
     numbers to pin];
   - 3GPP TR 37.885 (public) supplies the channel profiles, not the PER.

   If no source gives numeric curves for 802.11p at more than one speed, the package is
   dropped (rule 9) and the card line stays.
3. **The relative speed** is the two ends' speed difference projected on the link, read from
   the snapshot (a physical law, so ground truth is allowed, rule 6).

**Files.**
- `per.rs`: `PerTable`, as `bler.rs`;
- `phy.rs`: use the table when the key is on;
- `run.rs`: pass the relative speed into the reception (light).

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `radio.tiers.phy_error` or `radio.channel_model.doppler_per` | `false` | bool | the cached curve |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| The table reproduces the source points | `per::tests::doppler_table_hits_the_source_points` | speed axis in km/h read as m/s: red |
| A long frame at high relative speed fails above the AWGN SNR | `per::tests::long_frames_suffer_at_speed` | speed ignored: red |
| Off is byte-identical | `bsm_timing`, `phase2` | n/a |

**Expectation rows** (`phy-channel.yaml`, family `highway`): `doppler_per: true` -> `prr`
down at 300-500 m.

**Pieces.**
- **P40.1** (3 h, no cargo): find and cache the curves; digitise them with provenance.
- **P40.2** (3 h, gated): `PerTable` and its tests.
- **P40.3** (3 h, gated): the PHY and engine wiring, the row and the faults.

---

## WP-41 Zipper merge (S-P10)

**Today.**
- A SUMO `zipper` junction is imported as plain priority (`v2xw-world/src/sumo.rs:1570`).
- A lane closure removes a lane with no merge rule (`v2xw-mobility/src/engine.rs:9`, `:98`).
- Junction conflicts are in `intersection/two_coloring.rs` and `gap_acceptance.rs`.

**Decision.**
1. **The conflict type.** A new `Zipper` conflict type. The two streams alternate one vehicle
   each at the merge point. A merging driver adapts its speed to its foe's arrival ahead
   (SUMO zipper behaviour, reference only [verify: SUMO "Networks/PlainXML" zipper
   section]).
2. **Driver compliance.** A share of drivers merge early rather than late. That share comes
   from the MnDOT zipper-merge evaluations [verify: MnDOT reports; public].
3. **Where it applies.** It is set on import of `zipper`, and on scenario lane closures and
   lane drops when `traffic.merge.zipper` is on.

**Files.** `two_coloring.rs`, `gap_acceptance.rs`, `sumo.rs:1570`, and `engine.rs` (closure
lane drops).

**Settings.** `traffic.merge.zipper` (false); `traffic.merge.late_merge_share` (source value).

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Alternation at the merge | `gap_acceptance::tests::zipper_alternates` (two saturated streams: order ABAB… within one vehicle) | priority kept: red |
| Throughput at a lane drop | work-zone scenario: the zipper discharge and queue length against the MnDOT figures, within their stated spread | early-merge share 1.0: red |
| Import | `sumo.rs` test: `zipper` maps to `Zipper` | n/a |

**Pieces.**
- **P41.1** (2 h, no cargo): the SUMO zipper page and the MnDOT reports, with figures.
- **P41.2** (3 h): the conflict type and the import.
- **P41.3** (3 h): closures and lane drops, the work-zone scenario and rows.

---

## WP-42 Parking (S-P12)

**Today.**
- `kerbside.rs` models in-lane bus, taxi and delivery stops and double parking.
- OSM parking lanes are counted (`v2xw-world/src/osm.rs:536` `parking_lanes`, built per the
  options at `:1413`), but are not used as places to park.
- Trips end by vanishing.

**Decision.**
1. **Parking supply.** On-street parking lanes and off-street `amenity=parking` areas with
   capacity. Tags are read per the OSM wiki `parking:lane` / `parking:*` scheme [verify:
   public wiki].
2. **Manoeuvres.** Parking-in and parking-out manoeuvres block the lane for a duration drawn
   from a cited distribution [verify: a field study of on-street manoeuvre times, found by
   P42.1]. HCM's parking-manoeuvre saturation-flow adjustment is the check [verify:
   paywalled; used only if a public secondary source quotes it].
3. **Search.** When the area at the destination is full, the driver cruises to the next area
   within a radius (SUMO parking reroute, behaviour only [verify: SUMO docs]).

**Files.**
- `osm.rs`: parking areas into a hash-excluded world layer, as vegetation was;
- `kerbside.rs`: the manoeuvre;
- `v2xw-mobility/src/routing`: the final leg;
- `engine.rs`.

**Settings.** `traffic.parking.enabled` (false); `traffic.parking.manoeuvre_s`
(distribution); `traffic.parking.search_radius_m`.

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| A parking manoeuvre blocks its lane for the drawn time | `kerbside::tests::parking_blocks_its_lane` | no block: red |
| A full area sends drivers searching | `routing` test: arrivals > capacity -> searchers > 0, all end parked or leave | capacity ignored: red |
| Lane saturation flow falls next to parking | an HCM-style check, if a source is cached | n/a |
| Off is byte-identical | G-TV | n/a |

**Pieces.**
- **P42.1** (2 h, no cargo): the OSM tags on the shipped maps, and the manoeuvre and search
  sources.
- **P42.2** (3 h): areas and manoeuvres.
- **P42.3** (3 h): the final leg, search, rows and faults.

---

## WP-43 Overtaking in the opposing lane (S-P13)

**Today.**
- There is no opposite-direction manoeuvre (`sumo.md:84`). LC2013 lists it as left out
  (`v2xw-mobility/src/lanechange/lc2013.rs:40`).
- `v2xw-world/src/los.rs` gives sight distance.

**Decision.**
1. **The manoeuvre.** On a two-lane two-way road, a follower slower than its desired speed
   judges the oncoming gap against the passing sight distance, starts, and aborts by the
   NCHRP 605 rule. Sources:
   - Harwood et al., NCHRP Report 605 (2008), *Passing Sight Distance Criteria* [verify:
     public on TRB];
   - AASHTO Green Book §3.2.4 [verify: paywalled; NCHRP 605 quotes the values].
2. **Sight distance** comes from `los.rs` and the DEM, which beats SUMO's `lcOpposite`.
3. **Markings.** Passing is allowed only where OSM `overtaking=*` [verify: wiki] or the road
   markings allow it.
4. **DNPW.** The Do-Not-Pass Warning (CAMP VSC-A [verify: NHTSA report number]) can then be
   studied. Belief only: the warning reads the received BSMs of oncoming vehicles.

**Files.**
- `lanechange/`: a new `opposite.rs`;
- `engine.rs`: lateral occupancy of the opposing lane;
- `los.rs`: a sight-distance query;
- `osm.rs`: the `overtaking` tag.

**Settings.** `traffic.lane_change.opposite_overtaking` (false); `critical_gap` and abort
parameters from NCHRP 605.

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Accepted gaps sit in the NCHRP 605 distribution | `opposite::tests::accepted_gaps_match_nchrp_605` | gap in metres read as seconds: red |
| No head-on overlap | G-TV `invariants/*` on a rural two-lane scenario with overtaking on | abort rule removed: red (overlap) |
| No passing at a crest without sight | `los` crest test | sight check skipped: red |

**Pieces.**
- **P43.1** (3 h, no cargo): cache NCHRP 605 and the DNPW report; tag counts on the shipped
  rural maps.
- **P43.2** (3 h): gap judgement and the start.
- **P43.3** (3 h): abort, sight distance and the occupancy rules.
- **P43.4** (3 h): the scenario, G-TV checks, rows and faults.

---

## Sources to cache before the matching pieces

| For | Source | Access | Cached? |
|---|---|---|---|
| WP-25 | NCHRP Report 505 (Harwood et al. 2003) | public (TRB) | **read** 2026-10-28 (P25.1): Tables 29-30, eqs. 20-23, App. D, App. E (`grade-dynamics.md`) |
| WP-25 | FTA Altoona bus test reports (40 ft transit, 45 ft coach) | public | **read** 2026-10-28: 1211-P (New Flyer XD40), 1209 (Prevost X3-45) |
| WP-25 | EPA Test Car List data files | public | **read** 2026-10-28: MY2020 Ford Transit RWD Wagon Med Roof |
| WP-25 | PHEMlight HDV/LCV `.veh` files (SUMO `data/emissions/PHEMlight`) | EPL-2.0 | PC files only; none open (checked 2026-10-28), dropped |
| WP-26 | AWS Terrain Tiles (SRTM-format `skadi`); USGS 3DEP | public / public domain | no (P26.1) |
| WP-28 | FHWA Signal Timing Manual 2e (NCHRP 812), offset transition and lane-use sections | public | read 2026-10-27 (P17.1); sections to pin |
| WP-28 | MUTCD 2009 §4D | public | [repo] |
| WP-29 | Boban, Barros, Tonguz 2014 (GEMV²), permittivity and reflection method | IEEE | constants read (`gemv2-p833-constants.md`); ε_r to pin |
| WP-34 | 3GPP TS 36.213 V14.4.0 §14 | public | **cached** (R2-cv2x.md:23) |
| WP-34 | 3GPP TS 36.321 §5.14 | public | no |
| WP-34 | SAE J3161/1 | paywalled | no (second-hand: arXiv 2608.05087) |
| WP-35 | IEEE 1609.2.1 ASN.1 | cached | **cached** (`third_party/asn1/etsi/ieee1609.2.1/`) |
| WP-35 | IEEE 1609.2.1 body text §6.3.5 | paywalled | no |
| WP-37 | Natural Earth admin-0 polygons | public domain | no |
| WP-39 | Klauer et al. 2006, DOT HS 810 594 | public (NHTSA) | no |
| WP-39 | Dingus et al. 2016, PNAS 113(10) | open access | no |
| WP-39 | NHTSA visual-manual guidelines, 78 FR 24818 | public | no |
| WP-39 | 100-Car Phase II, DOT HS 810 593 | public | read ([repo] honest-density.md) |
| WP-40 | Anwar, Franchi, Fettweis, VTC2019-Fall | IEEE; author copy to find | no |
| WP-40 | IEEE 802.11 TGbd link-level submissions | public (document server) | no |
| WP-40 | 3GPP TR 37.885 | public | channel models summarised in R2c |
| WP-41 | SUMO zipper documentation; MnDOT zipper-merge evaluations | public | no |
| WP-42 | OSM wiki `parking:*`; a manoeuvre-time field study; SUMO parking reroute docs | public | no |
| WP-43 | NCHRP Report 605 (2008); CAMP VSC-A DNPW report | public | no |
| WP-33 | IEEE 1609.4-2016 | paywalled | no (constants UNVERIFIED) |

---

## Round schedule (3 build agents and 1 no-build agent per round)

Round 1 starts when adopt-7 has merged. Round 0 runs beside adopt-7, because it builds
nothing. Lanes keep each hot file to one lane:
- **A** holds `run.rs` and the radio and SCMS work;
- **B** holds the NEMA, world and SCMS-library work;
- **C** holds `v2xw-mobility/src/engine.rs`.

Every round's agents share only the scenario settings files (`schema.rs`, `validate.rs`,
`options.rs`, `publish.rs`, `guide.rs`) and `configuration-reference.md`, each appending its
own block. The integrator merges those blocks.

| Round | Agent A | Agent B | Agent C | No-build agent | Shared beyond the settings files |
|---|---|---|---|---|---|
| R0 (beside adopt-7) | n/a | n/a | n/a | P25.1, P26.1 | none |
| R1 | P34.1 (`sps.rs`, `generate.rs`) | P27.1 (`wiring.rs`, `prop.rs`) | P25.2 (`classes.rs`) | P39.1 | `expectations/traffic.yaml` (C only) |
| R2 | P34.2 (`run/sidelink.rs`, `run.rs`) | P28.1 (`nema/control.rs`, `infra.rs`, `model.rs`, `intersection/`) | P39.2 (`drivers.rs`) | P40.1 | none |
| R3 | P35.1 (`v2xw-msg`, `v2xw-proto/scms`) | P28.2 (`nema/assign.rs`, `model.rs`) | P39.3 (`mobility/engine.rs`) | P41.1 | none |
| R4 | P35.2 (`phase2.rs`, `run.rs`) | P28.3 (`nema.rs`) | P26.2 (scenarios, `tests/vehicle_dynamics.rs`) | P42.1 | `expectations/traffic.yaml` (B row, C rows) |
| R5 | P29.1 (`radio/reflection.rs`, `obstacle.rs`) | P28.4 (`nema.rs`, `priority.rs`) | P41.2 (`intersection/`, `world/sumo.rs`) | P43.1 | none |
| R6 | P29.2 (`wiring.rs`, `run.rs` light) | P36.1 (`scms/governance.rs`, `sec/ctl.rs`) | P41.3 (`mobility/engine.rs`) | spare: owed SCMS re-runs | none |
| R7 | P40.2 (`per.rs`; gated, else P32.1) | P36.2 (`scms/run/crl.rs`, `governance.rs`) | P42.2 (`world/osm.rs`, `kerbside.rs`) | spare | none |
| R8 | P40.3 (`phy.rs`, `run.rs`; gated, else spare) | P37.1 (`rxval.rs`, `wiring.rs`, `phase2.rs` metric) | P42.3 (`routing/`, `mobility/engine.rs`) | spare | none |
| R9 | P38.1 (`phase2.rs`, `run.rs`) | P30.1 (`node/bsm_timing.rs`) | P43.2 (`lanechange/`, `mobility/engine.rs`) | spare | none |
| R10 | P38.2 (bisect; release builds of old commits, one at a time) | P30.2 (`tests/bsm_timing.rs`, scenarios) | P43.3 (`world/los.rs`, `mobility/engine.rs`) | spare | none |
| R11 | P33.1 (`mac.rs`, `wave_schedule.rs`, `run.rs` light) | P31.1 (`mobility/gnss`, scenarios) | P43.4 (scenarios, trafficval) | spare | none |
| R12 | P32.1 (`sweep.rs`, `timeline/trigger.rs`, `model.rs` light) | n/a | n/a | n/a | none |

Notes on the schedule:
- **Ordering constraints.**
  - P27.1 (R1) comes before P29 (R5-R6), which reuses its index.
  - P28 needs `invariants/nema-safety` green on main after adopt-7.
  - P30 needs adopt-7's GNSS refit.
  - P31 needs P24.1 and WP-23.
- **Collisions avoided.** In each round no two agents edit `run.rs`, `wiring.rs`,
  `phase2.rs`, `mobility/engine.rs`, `model.rs` or a `nema` file together. Two cases are
  close:
  - P37.1 (R8) touches `phase2.rs` only for a report metric. No other R8 agent edits
    `phase2.rs`.
  - P29.2 and P27.1 both edit `wiring.rs`, but in R6 and R1.
- **Size.** 34 build pieces and 7 no-build source pieces, which is 12 build rounds of about
  3-4 h each.
- **The integrator** merges after every three rounds, as in waves 6-7: after R3, R6, R9 and
  R12.

---

## Owner decisions these plans need

1. **LTE-V2X alignment default (WP-34).** Turn `radio.sidelink.align_generation` on for
   LTE-V2X profiles? J3161/1 has the application align. This moves every LTE digest. Buying
   SAE J3161/1 would pin the margin.
2. **`power_limit: true` (WP-22/25).** Turn it on once the heavy rows exist? This moves G-TV.
   Also: should a class's mass follow its source row?
3. **`drape: true` and the measured-DEM scenario as a shipped preset (WP-26).**
4. **`density_sigma: true` after the R-tree (WP-27), and `reflection` once built (WP-29).**
   Both move the channel digests.
5. **`nema` for US suburban and arterial presets once `nema-safety` is green and WP-28 is
   in.** Fixed-time stays for `urban-us-nyc`. This moves G-TV and the SPaT digests.
6. **`crl_poll: info-status` as default (WP-35).** This moves G-SCMS.
7. **`first_ctl_sequence: 1` and `ctl_effective_date: ballot` (WP-36).** These move G-SCMS;
   they have no trust effect.
8. **C6 metric and R10 polygons (WP-37).** Which reading of "2 km" the source means, and
   whether Natural Earth polygons may be vendored.
9. **Distraction on for realistic presets (WP-39).** This moves G-TV.
10. **Doppler PER default (WP-40), if its source is found.**
11. **The default stamp profile (WP-30)**: `tampa_chain`, `tampa_mixed`, or Wyoming.
12. **Purchases** that would turn [verify] into [cached]: IEEE 1609.4-2016 (WP-33, WP-16),
    SAE J3161/1 (WP-34), IEEE 1609.2.1 body text (WP-35), AASHTO Green Book and HCM 7th ed.
    (WP-25, WP-42, WP-43).
13. **Dropping the `SlottedMac` gate**, since no tier selects it.
14. **Still parked, not planned here:**
    - WP-07 and REDV2X, P24.4-P24.5;
    - the late-stamp hold;
    - RA/PCA retention;
    - R-PQUNSEC;
    - `alternating` access stays `continuous`.
