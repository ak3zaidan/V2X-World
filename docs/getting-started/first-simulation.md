# Your first simulation

This walk-through runs the honest baseline of the ready-made scenarios,
`scenarios/midtown-normal.yaml`: a weekday midday in Midtown Manhattan with every vehicle
signing BSMs ten times a second, the US SCMS in the loop, eight roadside units at the
busiest signalised intersections and misbehaviour detection on every vehicle. Then it
changes one setting and compares the two runs.

It assumes the tool is built ([Install and build](../getting-started.md)); the commands
write `v2xw` for `./target/release/v2xw`. Run them from the repository root: relative
paths in a scenario are read from the directory the engine was started in.

## 1. Fetch the map

The scenario names no world of its own. Its base, `scenarios/base/city.yaml`, says
`city: manhattan`, so the world is the Manhattan entry of the city library
([Worlds and maps](../guide/worlds-and-maps.md)), whose OpenStreetMap extract is not in the
repository (it is ODbL data and large):

```sh
v2xw cities manhattan          # the library entry, and whether its extract is present
worlds/fetch.sh manhattan      # writes worlds/cache/manhattan.osm.xml from the Overpass API
```

!!! tip "No download"
    `scenarios/highway-congestion.yaml`, `scenarios/phase1-grid.yaml` and
    `scenarios/credential-lifecycle.yaml` build a procedural road network and need no
    extract. Every step below works on them unchanged.

## 2. Read and validate the scenario

`midtown-normal.yaml` is four lines of `meta` over `meta.base: base/city.yaml`; open both
to see what the run is. The overlay merge, the sections and the timeline are explained in
[Scenarios](../scenarios.md).

```sh
v2xw validate scenarios/midtown-normal.yaml
```

```text
ok              Normal traffic
schema          v2xw/scenario/1
scenario hash   e04fe072…
seed            0xc0ffee5eed
world source    osm-xml:worlds/cache/manhattan.osm.xml
time            300.000 s at 100 ms steps
metrics         all
note            security.verification_policy: 'prioritized' sheds the oldest and least relevant messages unverified …
```

Validation reports every problem at once, each with its dotted field, and exits 1 if any is
an error. A `note` is advice, not an error: here it says that the `prioritized`
[verification policy](../configuration/reference/security.md#s-security-verification-policy)
leaves detection nearly blind under load, which matters if you study detection.

## 3. Run it

```sh
v2xw run scenarios/midtown-normal.yaml --out runs/midtown-normal --duration 60s
```

`--duration 60s` overrides [`time.duration_s`](../configuration/reference/time.md#s-time-duration-s)
(300 s in the file) to keep the first run short; like every override it changes the
scenario hash and is recorded in the resolved scenario. The command prints a summary:

```text
scenario        Normal traffic
seed            0xc0ffee5eed
scenario hash   …
world hash      …
outputs         runs/midtown-normal

simulated       60.000 s in … s wall (… wall s per simulated s)
  load … s | build … s | run … s | verify … s | total … s

events          …
  MacTimer      …
  MobilityStep  601
  …
actors spawned  …
frames sent     …
receptions      … attempted, … received
records         … written, 0 refused

channels
  gt.kinematics   … records   … json bytes
  node.tx         … records   … json bytes
  …
container       … messages in … chunk(s), largest chunk … uncompressed bytes
recording       … bytes on disk
content digest  …
file sha256     …  (the container's summary section is not reproducible; compare the content digest)
verified        … records, … frames; integrity VERIFIED over … chunk(s)
```

`--json` prints the same as one JSON object. After the run the recording is read back and
its chunk checksums verified (`--no-verify` skips that).

## 4. Read the run directory

```text
runs/midtown-normal/
├── recording.mcap           every record, the VWP frames, the manifest, the scenario attached
├── scenario.resolved.yaml   the scenario after overlay merge, migration and defaults: what ran
├── metrics.json             every metric.sample the run emitted
├── run-report.json          event, frame and record counts per channel, and content_digest
├── manifest.json            build, models and card hashes, world, scenario and seed, crypto mode
└── progress.jsonl           progress lines: empty unless --progress-every is set or the run is an hour or more
```

| To answer | Read |
|---|---|
| is this the same run as another one? | `content_digest` in `run-report.json` (not the file hash of `recording.mcap`) |
| what exactly ran? | `scenario.resolved.yaml` and the hashes in `manifest.json` |
| what did it measure? | `metrics.json`, or `v2xw compare` and the Studio's Metrics panel |
| what happened, message by message? | `recording.mcap`, through `v2xw info`, Python or the Studio |

What each file and channel holds: [Metrics and outputs](../recording-and-metrics.md) and
[Running and recording](../guide/running-and-recording.md).

## 5. Inspect the recording

```sh
v2xw info runs/midtown-normal/recording.mcap
```

```text
recording       runs/midtown-normal/recording.mcap
size            … bytes
profile         full
cadence         keyframe 1000 ms, mobility step 100 ms
content digest  …

channels
  vwp/keyframe              vwp1  mixed         61 messages
  vwp/delta                 vwp1  mixed        540 messages
  record/metric.sample      json  derived        … messages
  record/node.tx            json  node           … messages
  record/gt.kinematics      json  gt             … messages
  …

attachments
  scenario.yaml             … bytes

verify          601 frames (61 keyframes, 540 deltas, 0 unknown), … records
integrity       VERIFIED — … chunk(s) checksummed and matched

manifest
  { "build_hash": "…", "build_utc": "…", "crypto_mode": "modeled", … }
```

Each channel carries a visibility class: `gt` is ground truth no device could know, `node`
is what a device saw, `derived` is computed from records. A `--node-only` run refuses every
ground-truth record, which is what a blind evaluation hands a detector.

## 6. Watch it in the Studio

The Studio is a browser page that drives a live engine. Serve the same scenario and open
the page in a second terminal:

```sh
./run-sim.sh scenarios/midtown-normal.yaml          # the engine on :8787, paused at t = 0
pnpm --dir ui install
pnpm --dir ui --filter @vwp/studio dev              # http://127.0.0.1:5173
```

Press play, click a vehicle to follow it, press **M** for the metrics. The
[Studio tour](studio-tour.md) covers each panel, including opening a finished recording
without an engine.

## 7. Change one setting and compare

Write a three-line overlay beside the scenario. It inherits everything from
`midtown-normal.yaml` and raises the arrival rate by half, from 25 to 37.5 vehicles per
hour per kilometre of driving lane
([`actors.vehicles.demand.rate_veh_per_h_per_lane_km`](../configuration/reference/actors.md#s-actors-vehicles-demand-rate-veh-per-h-per-lane-km)):

```yaml title="scenarios/my-midtown-busier.yaml"
schema: v2xw/scenario/1
meta:
  base: midtown-normal.yaml          # resolved relative to this file
  name: Normal traffic, busier
actors:
  vehicles:
    demand:
      rate_veh_per_h_per_lane_km: 37.5
```

```sh
v2xw validate scenarios/my-midtown-busier.yaml
v2xw run scenarios/my-midtown-busier.yaml --out runs/midtown-busier --duration 60s
v2xw compare runs/midtown-normal runs/midtown-busier
```

!!! warning "`--rate-veh-per-h` does not change this scenario"
    The run flag `--rate-veh-per-h` sets `actors.vehicles.demand.rate_veh_per_h`, and a
    scenario that states `rate_veh_per_h_per_lane_km` (as every ready-made city scenario
    does) recomputes the rate from the city's lane length, which wins. Change the density
    in an overlay, as above.

`v2xw compare` prints the settings that differ and every headline metric whose change is
meaningful, judged by the metric's health direction:

```text
  1 setting changed: … → ….
  … of … metrics changed meaningfully: … better, … worse, … neutral.
  …
  Both runs used seed 828927532781: every random draw the settings did not touch is shared, so the differences are caused by the settings change.

Settings changed:
  …

metric                       A          B       change        %  verdict
offered_load               … bit/s    … bit/s   … bit/s    +… %  changed
…
  (… rows without a meaningful change hidden; --all shows them)
```

Both runs inherit one seed, so they share every random draw the change did not reach
(common random numbers): the differences are the setting's, not noise. How the verdicts
are decided, and the JSON, Markdown and CSV forms: [Comparing runs](../guide/comparing-runs.md).

## Next

- Turn the one change into a sweep with replications and confidence intervals:
  [Experiments and sweeps](../guide/experiments-and-sweeps.md).
- Run the same scenario on another city: `v2xw run scenarios/midtown-normal.yaml --city boston`
  after `worlds/fetch.sh boston`.
- Every flag of every command: [Command-line tool](cli.md).
