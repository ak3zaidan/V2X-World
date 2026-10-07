# `scenarios/` — scenario files (YAML, schema `v2xw/scenario/1`)

A scenario file is the whole input to a run: the world, the fleet, the radio stack, the
security stack, the threats, the metrics and — optionally — the sweep that turns one file
into an experiment. There is no configuration outside it except the command line's output
paths, which is what makes a run reproducible from one artefact.

Every file here is meant to be **copied and edited**, so every one is commented like a
document rather than like a config: each states at the top what it measures, what it does
*not* measure, and why each non-obvious number is that number.

## The ready-made scenarios (what the Studio's picker offers)

A scenario says **what happens** — the traffic, the radio, the network, the security stack,
the attacks. A city says **where**. The two are chosen separately: the Studio's picker offers a
scenario first and then a city of the library ([`../worlds/cities.yaml`](../worlds/cities.yaml):
the largest city of every state and DC, the most-studied connected-vehicle cities, Berlin), and
the command line takes `v2xw run <scenario> --city <id>` (`v2xw cities` lists the ids). A
scenario file's `city` is only the city it runs on when none is chosen.

[`ready-made.txt`](ready-made.txt) is the picker's list, grouped and in order; the names and
one-line descriptions the page shows are each file's `meta.name` and `meta.description`. Every
other file in this folder still loads by path (`scenario.load`, `v2xw run`) and is simply not
offered. Each ready-made file is a short overlay (`meta.base`) on the deployment it changes, so
the deployment is written once: [`base/city.yaml`](base/city.yaml) (a downtown at midday, the
US SCMS, eight roadside units at the city's busiest signalised intersections, detection, demand
stated per kilometre of driving lane) and [`base/radio-faceoff.yaml`](base/radio-faceoff.yaml)
(rush-hour density with no backend), or an existing study file below.

What makes a scenario fit any city (`crates/v2xw-engine/src/city.rs`):

* `actors.vehicles.demand.rate_veh_per_h_per_lane_km` — demand as a density; the engine
  multiplies it by the city's driving-lane length once the map is imported;
* `actors.rsu_placement: {count, roles}` — roadside units at the city's busiest signalised
  intersections, at least `min_spacing_m` apart;
* `target: auto` on a `closure` — a side street of whatever city it is;
* `at: busiest-junction` on a jammer — the city's busiest signalised intersection;
* the clock keeps its local time of day on another city (noon is noon in Seattle too).

A scenario that embeds its own world still runs as written, and `--city` moves it: explicit
roadside-unit positions become a placement of the same count and roles, a fixed rate becomes a
density, a named-street closure becomes `auto`.

| File | Name in the picker | What it shows |
|---|---|---|
| [`midtown-normal.yaml`](midtown-normal.yaml) | Normal traffic | the honest baseline; start the server with this one |
| [`midtown-rush-hour.yaml`](midtown-rush-hour.yaml) | Evening rush hour | rising rush-hour demand, a lane-blocking breakdown, a side street closed |
| [`midtown-pedestrians.yaml`](midtown-pedestrians.yaml) | Pedestrians, jaywalkers and motorcycles | `midtown-street-life.yaml` at a size the page draws smoothly |
| [`midtown-rain.yaml`](midtown-rain.yaml) | Heavy rain | slower, more cautious traffic; the radio loses almost nothing (ITU-R P.838-3) |
| [`highway-congestion.yaml`](highway-congestion.yaml) | Highway - heavy traffic and breakdowns | a busy open-road channel on its own synthetic highway (or on a city, if one is picked) |
| [`radio-dsrc.yaml`](radio-dsrc.yaml), [`radio-lte.yaml`](radio-lte.yaml), [`radio-nr.yaml`](radio-nr.yaml) | Radio comparison - DSRC / LTE / NR | identical traffic, only the radio differs |
| [`berlin.yaml`](berlin.yaml) | European stack - ITS-G5 radios and CAMs | the European stack end to end; Berlin-Mitte by default |
| [`attack-fake-positions.yaml`](attack-fake-positions.yaml) | Attack - fake positions at rush hour | ConstPos and RandomPos attackers detected, reported and revoked |
| [`attack-replay.yaml`](attack-replay.yaml) | Attack - replayed and delayed messages | validly signed lies only plausibility checks can catch |
| [`attack-jamming.yaml`](attack-jamming.yaml) | Attack - radio jamming at a busy junction | a constant jammer for three minutes, the drop and the recovery |
| [`privacy-pseudonyms.yaml`](privacy-pseudonyms.yaml) | Certificates and pseudonym privacy | `credential-lifecycle.yaml` with a roadside eavesdropper |
| [`europe-credentials.yaml`](europe-credentials.yaml) | European credential system (ETSI CCMS) | `ccms-lifecycle.yaml` by day; Berlin-Mitte by default |
| [`post-quantum.yaml`](post-quantum.yaml) | Post-quantum security - the cost on the air | Falcon-512 hybrid signatures, fragmented on the air |
| [`emergency-priority.yaml`](emergency-priority.yaml) | Emergency vehicles get green lights | `connected-intersections.yaml`: SPaT, MAP, SRM and SSM |

[`portland.yaml`](portland.yaml) (downtown Portland by day) stays as a file but is no longer in
the picker: Portland is a city, chosen in the city step like any other.

Traffic volumes: the demand presets' *ratios* (night, midday, rush hour) are cited in
`crates/v2xw-engine/src/scenario/options.rs`; their *level* is scaled, because NYC DOT's Midtown
counts would need about forty times the interactive rate on the extract (thousands of vehicles
at once). The rush-hour congestion comes from the incident and the closure concentrating
traffic, as it does in the city.

## The study library

| File | Question it answers | Sweep | Runs |
|---|---|---|---|
| [`pdr-vs-distance.yaml`](pdr-vs-distance.yaml) | How does delivery fall off with distance, and what does the propagation tier do to the curve? | propagation tier × 5 seeds | 10 |
| [`density-sweep-congestion.yaml`](density-sweep-congestion.yaml) | At what vehicle density does the channel congest, and where does the load land first? | 5 demand rates × 3 seeds | 15 |
| [`pseudonym-privacy.yaml`](pseudonym-privacy.yaml) | What does a pseudonym-change period cost in bytes, verification and detection? | 4 periods × 3 seeds | 12 |
| [`pseudonym-strategies.yaml`](pseudonym-strategies.yaml) | Which pseudonym-change strategy stops a roadside eavesdropper — J2945/1, the NYC pilot, C2C-CC, CMIX mix zones or none — and at what cost? | 5 strategies | 5 |
| [`revocation-latency.yaml`](revocation-latency.yaml) | How long does each stage of a revocation take, and how big is the list? | 3 demand rates × 2 attacker fractions × 3 seeds | 18 |
| [`rat-comparison.yaml`](rat-comparison.yaml) | DSRC vs LTE-V2X vs NR-V2X: who delivers further, and what each loses frames to | radio technology × 5 seeds | 15 |

## The vertical slices and the scaling ladder

| File | What it is |
|---|---|
| [`phase1-manhattan.yaml`](phase1-manhattan.yaml) | one vehicle, the real Manhattan import, signed BSMs, one metric |
| [`phase1-grid.yaml`](phase1-grid.yaml) | the same on a procedural Midtown-shaped grid, so determinism cannot be blamed on the extract |
| [`phase2-manhattan.yaml`](phase2-manhattan.yaml) | the Phase 2 *path*: a liar, a detector, a report, a roadside unit, the SCMS backend, a revocation |
| [`portland-downtown.yaml`](portland-downtown.yaml) | a second city: downtown Portland, Oregon, with its own speed limits and its slow green wave — the file to copy for your own city (`worlds/fetch.sh`, then `world_report --baseline`) |
| [`credential-lifecycle.yaml`](credential-lifecycle.yaml) | every US SCMS entity in the loop with the vehicles on a small grid — bootstrap, batches and top-ups through the LOP, renewal, reports, revocation, CRL — with the calendar compressed to five minutes; open the page's Backend view to watch it |
| [`manhattan-5min.yaml`](manhattan-5min.yaml) | the default: five minutes of Midtown with a fleet, the SCMS in the loop through eight roadside units and every car's modem, and misbehaviour detection reporting to the corroborating authority; open the Backend view |
| [`ccms-lifecycle.yaml`](ccms-lifecycle.yaml) | the European counterpart: CAMs over GeoNetworking, the ECTL and CA-CRL from the Distribution Centre, butterfly tickets the AA cannot link to the enrolment, TS 103 759 reports and passive revocation at the EA |
| [`midtown-street-life.yaml`](midtown-street-life.yaml) | Midtown with its road users as they behave: pedestrians by age group who cross on DON'T WALK into a gap and jaywalk mid-block (more beside stopped traffic), groups, cyclists on conventional and e-bikes, motorcycles and delivery mopeds among the cars, vans, trucks and buses; the auditor counts conflicts and near misses, the `ped_*` metrics measure them |
| [`scale/`](scale/) | the wall-clock ladder, 2 to 10,000 vehicles, with what the bulk spawn costs in realism stated in `scale/base.yaml` |

## Running one

```sh
v2xw run scenarios/pdr-vs-distance.yaml --out runs/one          # a single run
v2xw experiment run scenarios/pdr-vs-distance.yaml --out runs/s # the whole sweep
v2xw experiment status scenarios/pdr-vs-distance.yaml --out runs/s
v2xw experiment resume scenarios/pdr-vs-distance.yaml --out runs/s
```

A sweep is **serial by default** (`v2xw_experiment::runner::DEFAULT_CONCURRENCY` is 1).
Raising it multiplies peak memory by the concurrency, because the engine's own loop
already uses the machine's cores inside one run; this project has lost a wave of work to
exactly that. The resume journal is what makes an interrupted sweep survivable, not what
makes over-subscription acceptable.

Three of the five studies need `worlds/cache/manhattan.osm.xml`, which is git-ignored: it
is 30 MB of OSM XML for the build decision D7 bounding box (`-73.9900, 40.7440, -73.9680,
40.7620`, 13,553 ways), downloaded from any OSM mirror and saved at that path. Replace the
`world` block with the procedural one from `pdr-vs-distance.yaml` to run without it.

## Rules the scenario validator enforces, that a new file trips over first

The validator returns **every** problem at once, each naming the field and the conflict.
These are the ones that catch people:

| Rule | Why |
|---|---|
| `exporters` must be empty | the engine has no exporter stage; exporting is `v2xw run --record` and `v2xw export`. A list here would be silently ignored, so it is refused instead |
| `net.layer` must be `wsmp` | nothing composes a network layer: a frame goes from the signer to the MAC with no header between them, so `gn-btp` would not be the thing that ran |
| `messages.sets` ⊆ {`bsm`, `cam`} | `ServiceSet` has a flag for each and nothing else, so any other set names a generator that does not exist |
| `messages.codec_tier` must be `uper` | the node runtime encodes real UPER unconditionally; nothing selects the size model |
| `actors.vru` must be all zeros | nothing spawns a pedestrian or a cyclist yet, so the three fields would select a population that never exists |
| `radio.tiers.focus` must be absent | one tier runs for the whole world; a focus region would be quietly ignored |
| `phy: high` requires `mac: high` | a frame-level PHY decides an outcome per frame, and a coarser MAC does not schedule frames at that granularity |
| `propagation: abstract` requires `phy: abstract` | an abstract propagation model returns a probability, and a link-budget PHY needs a power |
| `time.mobility_step_ms` ∈ [10, 100] | no mobility provider is calibrated below 10 ms, and the constant-velocity extrapolation between steps stops being accurate above 100 ms |
| exactly one of `fraction`, `count`, `actor_ids` per attacker | two would need a rule for which wins |
| `pseudonym_change.period_s` required when `strategy: time` | there is no default period, and a defaulted one would set a whole run's unlinkability invisibly |
| every `experiment.sweep` path must resolve in the **serialised** scenario | a sweep can only replace a value that is already there. A field that is `Option` and `None` is not serialised, so set it in the base before sweeping it |

That last one is the one that costs an afternoon. `security.pseudonym_change.period_s`,
`actors.vehicles.demand.rate_veh_per_h` and `threats.attackers[0].fraction` are all
optional fields: a sweep over any of them needs the base file to set it, which is why each
study file sets its swept fields to a real base value rather than leaving them out.

## Overlays

`meta.base` names a sibling file to inherit from, and the overlay is merged field by field
(`v2xw_engine::scenario::merge`). `scale/10.yaml` is three lines on top of
`scale/base.yaml`. Use it for a ladder of one parameter; use `experiment.sweep` for a
factorial. An overlay changes the scenario hash, a sweep cell changes the run seed as
well — the plan derives one seed per (slot, replication) and reuses it across cells, which
is common random numbers.

## Writing a new one

Start from the study whose shape matches yours, then:

1. **Say what it does not measure.** Every file here has that section, and in three of the
   five it is the longest one. A scenario that does not state its omissions produces a
   figure whose reader supplies their own.
2. **Justify every number that is not a default**, in place. "60 s" is not a duration, it
   is a claim that 60 s is long enough for the mechanism under study to happen.
3. **Hold everything but the swept axis fixed**, and say so where it would be tempting not
   to.
4. **Check that your sweep can differ.** A swept field the engine does not read produces
   identical cells and reads as a null result. `rat-comparison.yaml` was that failure
   mode until `radio.rat` was wired; the page's key-status table says which keys are.
