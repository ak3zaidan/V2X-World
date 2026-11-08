# Traffic demand

Demand says when vehicles enter the world, where they start and end, and which way they
go. It fills one slot, [`actors.vehicles.demand`](../configuration/reference/actors.md#s-actors-vehicles-demand-kind):

```yaml
actors:
  vehicles:
    equipped_fraction: 1.0
    demand:
      kind: mobility/demand/poisson       # which demand model
      rate_veh_per_h: 1500.0              # or rate_veh_per_h_per_lane_km, or fleet_size
      params: {}                          # the model's own parameters
```

| Field | Meaning |
|---|---|
| [`kind`](../configuration/reference/actors.md#s-actors-vehicles-demand-kind) | the demand model's id; default `mobility/demand/none` (nothing arrives) |
| [`rate_veh_per_h`](../configuration/reference/actors.md#s-actors-vehicles-demand-rate-veh-per-h) | vehicles per hour offered to the network |
| [`rate_veh_per_h_per_lane_km`](../configuration/reference/actors.md#s-actors-vehicles-demand-rate-veh-per-h-per-lane-km) | the same as a density, multiplied by the built world's driving-lane length; wins over `rate_veh_per_h`, and is what makes a scenario fit any city |
| [`fleet_size`](../configuration/reference/actors.md#s-actors-vehicles-demand-fleet-size) | hold exactly this many vehicles on the road: a trip that ends is replaced, and the rate is not used |
| [`params`](../configuration/reference/actors.md#s-actors-vehicles-demand-params) | the model's parameter struct; unknown keys are refused at load |

The vehicle mix, driver behaviour, GNSS and the other vehicle settings are in the
[`actors` reference](../configuration/reference/actors.md); pedestrians and cyclists are
[`actors.vru`](../configuration/reference/actors.md#s-actors-vru-pedestrians).

## The demand kinds

| `kind` | Arrivals | `params` |
|---|---|---|
| `mobility/demand/poisson` (alias `…/poisson-thinned`) | a thinned Poisson process with a time-of-day shape | the Poisson struct below, plus an optional `od` object |
| `mobility/demand/tr36885-drop` | one placement of vehicles along every lane, spaced by 3GPP TR 36.885 Table A.1.2-1 or TR 37.885 §6.1.2 | `variant`, `mean_gap_time`, `speeds`, … |
| `mobility/demand/sumo-rou` | a SUMO `.rou.xml`: vTypes, routes, flows | `{path, probability_flows: bernoulli \| equal-spacing}` |
| `mobility/demand/routes` | a `v2xw/routes/1` file (an assignment's or a count sampler's output) | `{path}` |
| `mobility/demand/trips` | a `v2xw/trips/1` file, optionally assigned at load | `{path, assign?}` |
| `mobility/demand/od-matrix` | an OD matrix turned into trips | `{matrix \| cells \| gravity, zones, od2trips?, assign?}` |
| `mobility/demand/counts` | routes and trips that reproduce observed link counts | `{counts, sampler?, candidates?}` |
| `mobility/demand/gtfs` | timetabled buses and street-level rail | the transit struct below |
| `mobility/demand/purposes` | a day of home-based tours by trip purpose | `{daily_trips, purposes?, run_start_s_of_day?, seed?}` |
| `mobility/demand/composite` | several of the above at once | `{parts: [{kind, params, rate_veh_per_h?}], utc_offset_s?}` |
| `mobility/demand/none` | nothing | |

Paths are read relative to the working directory, as `world.source.path` is. Times of day
are local: `utc_offset_s` (for example `-28800` for Pacific standard time) turns the
scenario's UTC [`time.t0`](../configuration/reference/time.md#s-time-t0) into the local
start time that timetables, matrices and profiles are written in.

## Poisson arrivals

`mobility/demand/poisson` is an exact simulation of an inhomogeneous Poisson process by
thinning: candidates arrive at a boosted constant rate and each is kept with the
probability the time-of-day shape gives.

$$
\Delta t \sim \mathrm{Exp}(\lambda\, b), \qquad
P(\text{keep at } t) = \frac{m(f)}{b}, \qquad f = \frac{t}{T}
$$

with $\lambda$ the base rate, $b \ge \sup m$ the candidate boost (`candidate_boost`) and
$T$ the run length. Shapes (`profile`):

| `profile` | $m(f)$ |
|---|---|
| `uniform` (default) | $1$ |
| `rush` | $0.2 + 0.8\,\min\!\left(1,\ e^{-((f-0.25)/0.09)^2} + e^{-((f-0.75)/0.09)^2}\right)$: two peaks, a quiet midday |
| `night` | $0.15 + 0.25 f$ |

These shapes follow the fraction of the run, whatever its length. A run of hours or days
uses `daily` instead: a factor for each hour of the local civil day, interpolated between
hour centres, with weekday and weekend curves. The shipped curve (`nyc-dot-manhattan`, or
`-weekday`/`-weekend` on every day) is NYC DOT's Automated Traffic Volume Counts for
Manhattan, 2015 to 2026, normalised so the weekday factors average 1 (night lull 0.26 at
03:00, morning peak 1.30 at 08:00, evening plateau 1.33). `rate_veh_per_h` is then the mean
weekday rate. The curve is a borough average, not the counts at the streets of a world, and
the clock is standard time from a fixed UTC offset.

```yaml title="scenarios/manhattan-day.yaml (excerpt)"
demand:
  kind: mobility/demand/poisson
  rate_veh_per_h: 300.0
  params:
    daily: {curve: nyc-dot-manhattan}     # utc_offset_h defaults from the world's longitude
```

Other Poisson parameters: `max_total_vehicles` (a cap, `0` for none), `fleet` (the class
mix), `speed` (how a trip's desired speed is drawn; default SUMO-style class speed
deviation), `fleet_size`, and `od` (where trips start and end: `uniform` or gravity
destinations with `w(h) = e^{−h/scale}` over junction hops, optional perimeter origins).
Every parameter with its source:
[`mobility/demand/poisson-thinned`](../configuration/reference/models.md#mobilitydemandpoisson-thinned).

The Studio's named rates for Midtown (`crates/v2xw-engine/src/scenario/options.rs`): Night
500, Light 1,500, Normal (midday) 3,000, Rush hour 4,500, Gridlock 9,000 veh/h. Their
ratios are cited (HCM 6th ed. design-hour factor about 9 % of daily traffic; FHWA Traffic
Monitoring Guide 2016, about 6 % at midday and 1 % in the small hours); their level is
scaled to what the Midtown extract runs interactively.

Demand can also change during a run through the timeline:
`{t: 60.0, until: 240.0, type: demand.multiplier, value: 1.3}`
([`events`](../configuration/reference/events.md)).

## OD matrices

```yaml
demand:
  kind: mobility/demand/od-matrix
  params:
    zones: {kind: grid, cols: 2, rows: 2}
    cells:
      - {from: r0c0, to: r1c1, count: 120}
      - {from: r1c1, to: r0c0, count: 120}
    assign: {max_iterations: 4}
```

- **Formats** (`matrix: path`): PTV VISUM/VISSIM O-format (`$O`, one `from to count` line
  per cell) and V-format (`$V`, a full square matrix), the two SUMO's `od2trips` reads, and
  a plain CSV `from,to,count[,begin_s,end_s]`. Or `cells` inline.
- **Synthesis** (`gravity`): a doubly-constrained gravity model
  $T_{ij} = a_i b_j P_i A_j f(c_{ij})$ balanced by Furness's method (Furness 1965;
  Ortúzar and Willumsen, *Modelling Transport* 4th ed., ch. 5), with exponential, power or
  combined deterrence. β is uncalibrated; the card gives Hyman's method (1969) to fit it.
- **Zones**: a grid of the world (`grid:CxR` on the command line), weighted by building
  floor area.
- **Matrix to trips**: each cell's count is scaled and rounded stochastically; departures
  are uniform in the interval or follow a departure profile; origin and destination lanes
  are drawn by zone weights. Draws are keyed by the cell, so changing one cell changes no
  other cell's trips.

## Link counts

A count is vehicles past a place in an interval. The place is an edge, a lane, or a real
station's coordinate (with direction), snapped to the nearest road of matching heading:

```text
id,kind,ref,begin_s,end_s,count
5th-ave-n,point,45.5190 -122.6765 0,0,3600,612
e17,edge,17,0,900,140
```

`mobility/demand/counts` builds routes and trips that reproduce them (max-entropy or
greedy sampling). Any routed demand is checked against counts with the GEH statistic and
the acceptance criterion of UK DfT TAG Unit M3.1 (2020) Table 2: GEH < 5 on at least 85 %
of counts.

## Route choice and dynamic traffic assignment

`assign` (on `od-matrix` and `trips`, and the `v2xw-demand assign` command) runs an
iterative dynamic traffic assignment in the manner of SUMO's `duaIterate` (Gawron 1998):

1. every trip takes its free-flow time-dependent shortest path, or a saved assignment's;
2. a network loader drives the routes and measures lane travel times per interval;
3. the relative gap $\sum (c_\text{used} - c_\text{best}) / \sum c_\text{used}$ is measured
   (Chiu et al., *Dynamic Traffic Assignment: A Primer*, TRB E-C153, 2011, §4);
4. stop when the gap is below `gap_tolerance` (after `min_iterations`) or at
   `max_iterations`;
5. otherwise the route-choice model updates each trip's probabilities, each trip draws its
   next route, and the loop repeats.

| Route choice (`assign.choice.model`) | Source |
|---|---|
| `gawron` (default) | Gawron 1998, pairwise update with smoothed costs; `duarouter`'s default |
| `logit` | multinomial logit (Ben-Akiva and Lerman 1985) with the C-logit commonality correction (Cascetta et al. 1996) |
| `msa` | method of successive averages, `1/k` onto the best route at iteration `k` (Sheffi 1985, ch. 12) |

| Network loader (`assign.loader.model`) | What it is |
|---|---|
| `queue` (default) | event-driven point queues with signal gating (Vickrey 1969), no spill-back |
| `bpr` | static link performance $t = t_0 (1 + \alpha (v/c)^\beta)$ (Bureau of Public Roads 1964) |
| `micro` | the engine's own microscopic mobility driving the routes; most faithful, slowest |

The result is a `v2xw/routes/1` file: every trip with its alternatives, each route's cost
and probability, the chosen one, the world hash it was made for, the parameters and the
iteration log. `mobility/demand/routes` runs it. During a run,
[`mobility/routing/dynamic-reroute`](../configuration/reference/models.md#mobilityroutingdynamic-reroute)
re-plans at the next junction on a closure.

## Trip purposes

`mobility/demand/purposes` generates a day as tours: a home-based tour leaves home at a
time drawn from its purpose's departure profile, stays for a drawn duration and returns;
non-home-based and commercial trips are single legs (purposes after NCHRP Report 716,
ch. 4). Activities are drawn with distance decay $w_j e^{-d_{ij}/d_0}$. The shipped
purpose shares, profiles and durations are uncalibrated shapes, marked `todo-calibrate`
with the plan of fitting the 2017 NHTS; a study passes its own per-purpose specification in `purposes`. Days are
generated one at a time, keyed by `(seed, day, purpose, tour)`, so a week costs one day's
memory and a resumed run regenerates the same day.

## Public transport (GTFS)

```yaml title="scenarios/portland-transit.yaml (excerpt)"
demand:
  kind: mobility/demand/composite
  params:
    utc_offset_s: -25200                  # 15:00 UTC is 08:00 Pacific daylight time
    parts:
      - kind: mobility/demand/poisson
        rate_veh_per_h: 1000.0
      - kind: mobility/demand/gtfs
        params:
          feed: worlds/cache/portland.gtfs
          date: 20261007
          dwell: {model: scheduled, min_s: 4.0}
          hold_early: true
```

The GTFS kind places each stop on the network, runs buses, trolleybuses and street-level
rail (route types 0, 3, 5, 11, 700–799, 900–999 unless `route_types` says otherwise), with
a dwell model and optional holding of early vehicles. Fetch and clip a feed, then check
what the stop matcher made of it before running:

```sh
worlds/fetch.sh portland
worlds/fetch-gtfs.sh portland 20261007            # worlds/cache/portland.gtfs/
v2xw-demand gtfs --scenario scenarios/portland-transit.yaml \
    --feed worlds/cache/portland.gtfs --date 20261007 --start 08:00
```

## The `v2xw-demand` tool

Precomputes demand outside a run, against exactly the world a scenario builds, so the
files' world hash matches when the scenario loads them. Every command and flag:
[Command-line tool](../getting-started/cli.md#v2xw-demand).

```sh
v2xw-demand od2trips --scenario S.yaml --matrix city.od --zones grid:4x4 --start 07:00 --out trips.json
v2xw-demand assign   --scenario S.yaml --trips trips.json --choice logit --loader queue --out routes.json
v2xw-demand check    --scenario S.yaml --counts counts.csv --routes routes.json
```

`assign` prints one line per iteration (relative gap, mean travel time, arrivals, share
switched, mean alternatives) and whether it converged. `check` prints observed and modelled
counts with GEH per row and the TAG M3.1 verdict.

## Adding a demand model

Implement `v2xw_mobility::traits::Demand`, add its id to `v2xw_demand::scenario::KINDS`
and an arm to `build`; the engine forwards every kind in `KINDS` to that crate, so nothing
in the engine changes. Each family (route choice, loader, dwell, deterrence, departure
profile) has its own trait and spec enum, and each module ends with a "How to add another"
note. See [Extending: models](../dev/extending/models.md).
