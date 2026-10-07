# v2xw-demand — demand and route choice

Where the traffic comes from and which way it goes. This crate covers the ground SUMO
covers with `od2trips`, `duarouter`/`duaIterate`, `routeSampler`/`flowrouter`, `gtfs2pt`
and `activitygen`, as plug-ins behind traits, with a model card per model, deterministic to
the bit.

| Need | Here | Scenario `actors.vehicles.demand.kind` |
|---|---|---|
| Dynamic traffic assignment (stochastic user equilibrium) | `assign` + `choice` + `loader` | `mobility/demand/trips` or `od-matrix` with `assign`, or `routes` for saved output |
| OD matrices (VISUM O/V-format, CSV), zones, gravity model | `od` | `mobility/demand/od-matrix` |
| Routes from observed link counts, with a GEH check | `counts` | `mobility/demand/counts` |
| Buses and street-level rail from GTFS, with dwell | `transit` | `mobility/demand/gtfs` |
| Trip purposes, tours, departure-time profiles, multi-day | `purpose` | `mobility/demand/purposes` |
| Several of the above at once (with Poisson cars too) | `models` | `mobility/demand/composite` |

The `v2xw-demand` command (crate `v2xw-demand-cli`) runs each step offline on the world a
scenario builds:

```sh
cargo run -p v2xw-demand-cli -- od2trips --scenario scenarios/portland-downtown.yaml \
    --matrix my.od --zones grid:4x4 --start 07:00 --out trips.json
cargo run -p v2xw-demand-cli -- assign --scenario scenarios/portland-downtown.yaml \
    --trips trips.json --choice gawron --loader queue --out routes.json
cargo run -p v2xw-demand-cli -- check --scenario scenarios/portland-downtown.yaml \
    --counts counts.csv --routes routes.json
worlds/fetch-gtfs.sh portland 20261007     # TriMet: buses, MAX, Portland Streetcar
cargo run -p v2xw-demand-cli -- gtfs --scenario scenarios/portland-transit.yaml \
    --feed worlds/cache/portland.gtfs --date 20261007 --start 08:00
```

## Models and sources

| Model id | What | Source |
|---|---|---|
| `demand/assign/iterative-due` | assignment loop, relative-gap stopping | Gawron 1998; SUMO duaIterate; Chiu et al., *DTA Primer*, TRB E-C153, 2011 |
| `demand/choice/gawron` | Gawron's pairwise update, β = 0.3, a = 0.05 | Gawron, IJMPC 9(3) 1998; SUMO duarouter defaults |
| `demand/choice/logit` | multinomial / C-logit, MSA-averaged | Ben-Akiva & Lerman 1985; Cascetta et al. 1996; Daganzo & Sheffi 1977; Sheffi 1985 |
| `demand/choice/msa` | successive averages | Sheffi 1985; Robbins & Monro 1951; Wardrop 1952 |
| `demand/loader/bpr` | static BPR link costs | BPR *Traffic Assignment Manual* 1964; HCM 6th ed. |
| `demand/loader/queue` | event-driven point queue with signal gating | Vickrey 1969; Gawron 1998; HCM 6th ed. ch. 19 (1,900 pc/h/ln); Webster 1958 (zero-flow delay) |
| `demand/loader/micro` | the native engine as the loader | SUMO duaIterate |
| `demand/od/gravity-furness` | doubly-constrained gravity, Furness balancing | Furness 1965; Ortúzar & Willumsen 2011 ch. 5; Hyman 1969 |
| `mobility/demand/od-matrix` | matrix → trips, keyed by cell | SUMO od2trips; PTV VISUM formats |
| `mobility/demand/counts` | candidate routes, max-entropy or greedy flows | van Zuylen & Willumsen 1980; SUMO routeSampler; UK TAG M3.1 (GEH) |
| `mobility/demand/gtfs` | GTFS timetables, HMM stop matching, braking to the stop, dwell, holding | GTFS Reference; Newson & Krumm 2009; TCQSM 3rd ed. (TCRP 165) |
| `mobility/demand/purposes` | tours by purpose, departure profiles, distance decay | NCHRP 716; ITE Trip Generation 11th ed.; SUMO activitygen |

Uncalibrated numbers (logit perception CV, gravity β, purpose shares and profiles, the bus
stopping deceleration, passengers per stop, the gap tolerance) are marked `todo-calibrate`
on their cards with a calibration plan.

## Where this goes past the reference tools

* **Measured convergence.** Every iteration reports the relative gap; the assignment stops
  on it, not on an iteration count, and the gap is written into the route file.
* **Determinism.** Route files are byte-identical across runs, machines and thread counts:
  every draw is keyed by `(seed, trip, iteration)`, so adding a trip changes no other
  trip's choices. SUMO's duaIterate draws from one sequence.
* **Three loaders behind one trait**, from a seconds-fast point queue to the microscopic
  engine itself, so an assignment can be brought close cheaply and finished faithfully.
* **Counts with real coordinates.** A count station is placed from latitude, longitude and
  direction of travel; candidate routes are generated with coverage of every count
  guaranteed (routeSampler needs them supplied), flows are maximum-entropy rather than
  first-drawn, and departures are shifted so vehicles pass the counter inside the counted
  interval.
* **GTFS from the feed alone.** Stops are placed jointly by HMM map matching, which picks
  the right side of a one-way pair or divided road; SUMO's gtfs2pt needs the OSM transit
  relations. Vehicles brake to the stop in traffic, dwell by the timetable or TCQSM, hold
  when early, and every call is logged with its schedule deviation.
* **Days generated one at a time**, keyed by day, so a week-long run costs a day's memory
  and a checkpoint restored on day 3 regenerates day 3 exactly.

## How it plugs into the engine

Two defaulted methods on `v2xw_mobility::traits::Demand` (`demand/hooks.rs`):
`planned_route` (a demand model supplies the route; the engine checks it is drivable and
falls back to its own router) and `wants_progress`/`observe` (a demand model sees its
vehicles each step and answers with commands — how buses stop). Nothing changes for a
demand model that does not use them.

The long-run engine's daily profile plugs in through `Demand::set_multiplier` (every demand
here honours it, thinning or copying by trip id) and through `purpose::TimeOfDayCurve` →
`DepartureProfile::from_curve`.

## How to add another

* a route-choice model: implement `choice::RouteChoice`, add a `ChoiceSpec` variant;
* a network loader (CTM, LTM, …): implement `loader::NetworkLoader`, add a `LoaderSpec` variant;
* a dwell model: add a `transit::dwell::DwellSpec` variant;
* a deterrence function: add a `od::gravity::Deterrence` variant;
* an OD file format: a `parse_*` in `od` and a branch in `OdMatrix::parse`;
* a demand kind: implement `Demand`, add it to `scenario::KINDS` and `scenario::build` —
  the engine forwards every kind in `KINDS`.

## Limitations

* The queue loader has no spill-back; the micro loader has it (it is the engine).
* Street-level rail runs as the bus class on the road lanes it shares.
* Zone weights come from building floor area: the world does not say which buildings are
  homes, so purposes draw homes and activities from the same weights.
* No LP solver: the counts sampler's optimiser is maximum entropy (IPF), not
  routeSampler's `--optimize` linear program.
