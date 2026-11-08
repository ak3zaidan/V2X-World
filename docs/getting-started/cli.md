# Command-line tool

`cargo build --release -p v2xw-cli` builds two programs into `target/release/`:

| Program | What it is |
|---|---|
| `v2xw` | the command-line tool: every subcommand below |
| `v2xw-server` | the engine server with the assistant's endpoints always on; the same program as `v2xw serve`, with the server's own flags ([below](#v2xw-server)) |

The grammar is in `crates/v2xw-cli/src/cli.rs`; `v2xw <command> --help` prints the same
text. Two conventions hold everywhere: units are in flag names (`--keyframe-ms`,
`--duration-s`), and a flag that overrides a scenario field changes the scenario hash and
is recorded in the run's resolved scenario.

**Exit codes.** `0` success; `1` the command failed (a scenario error also names the
offending key on stderr); `2` the arguments were wrong. Commands that check something
(`check-traffic`, `sensitivity`) exit `1` when the check fails and still print their table.

**Durations.** Flags typed `DURATION` take a number with units: `90s`, `15m`, `6h`, `2d`,
`1d12h`.

## Commands

| Command | Does |
|---|---|
| [`run`](#run) | run a scenario and write its recording, metrics, report and manifest |
| [`resume`](#resume) | carry on a checkpointed run, verified to be the same run |
| [`validate`](#validate) | load and check a scenario without running it |
| [`info`](#info) | a recording's manifest, channels and verification report |
| [`cities`](#cities) | the city library and which extracts are downloaded |
| [`import-osm`](#import-osm) | import an OpenStreetMap extract into the three world formats |
| [`export-sumo`](#export-sumo) | write a scenario's network and demand in SUMO formats |
| [`experiment`](#experiment) | run, inspect or resume a scenario's sweep |
| [`compare`](#compare) | the settings and headline metrics that differ between two runs |
| [`compare-signatures`](#compare-signatures) | one scenario and seed under several signature schemes |
| [`sensitivity`](#sensitivity) | one-at-a-time perturbations checked against expected directions |
| [`ma-replay`](#ma-replay) | replay recorded misbehaviour reports through MA decision rules |
| [`check-traffic`](#check-traffic) | audit a finished run's traffic from its recording |
| [`energy`](#energy) | score a recording for emissions, fuel and EV energy |
| [`serve`](#serve) | the engine server with the assistant's endpoints |
| [`agent`](#agent) | the assistant: build a scenario from a prompt, run it, report |

---

## `run`

`v2xw run [OPTIONS] <SCENARIO>`. Guide: [Running and recording](../guide/running-and-recording.md).

| Argument or flag | Type | Default | Meaning |
|---|---|---|---|
| `<SCENARIO>` | path | required | the scenario file, YAML or JSON |
| `-o`, `--out` | path | `runs/<meta.name>` (`runs/<meta.name>-<city>` with `--city`) | output directory |
| `--build-utc` | ISO 8601 | this machine's clock | the manifest's build timestamp; pass it to compare two manifests field by field |
| `--keyframe-ms` | integer, ms | `1000` | the recording's keyframe period; a whole number of mobility steps |
| `--node-only` | flag | off | record the NODE-only profile, which refuses every ground-truth record |
| `--no-recording` | flag | off | run into a counting sink with no recording (scaling measurements) |
| `--attach-world` | flag | off | also attach the world payload to the recording (tens of MB for a city) |
| `--no-verify` | flag | off | skip reading the recording back (digest and chunk checksums) |
| `--digest` | flag | off | with `--no-recording`: stream a SHA-256 of every record and report it as `stream_digest` |
| `--duration-s` | number, s | scenario | override [`time.duration_s`](../configuration/reference/time.md#s-time-duration-s) |
| `--duration` | DURATION | scenario | the same in units (`15m`, `6h`); wins over `--duration-s` |
| `--rate-veh-per-h` | number, veh/h | scenario | override [`actors.vehicles.demand.rate_veh_per_h`](../configuration/reference/actors.md#s-actors-vehicles-demand-rate-veh-per-h) |
| `--veremi` | `veremi-ext`, `veremi-1` | none | add the `veremi` exporter with this attack codebook, or set the codebook of the one the scenario names |
| `--city` | city id | scenario's | run on this city of the library (`v2xw cities`) instead of the file's world |
| `--progress-every` | DURATION | 15 min for runs of 1 h or more, else none | write a progress line (`progress.jsonl` and stderr): wall clock per simulated hour, memory, fleet; `0` turns it off |
| `--quiet` | flag | off | keep `progress.jsonl`, print no progress on stderr |
| `--checkpoint-every` | DURATION | none | write `checkpoint.json` this often and record in segments, so `v2xw resume` can carry the run on |
| `--record-level` | `full`, `receptions`, `events`, `metrics` | `full` | how much reaches the recording ([levels](../guide/running-and-recording.md#recording-levels)) |
| `--json` | flag | off | print the outcome as JSON |

!!! warning "Rate override and per-lane-km demand"
    A scenario that states
    [`rate_veh_per_h_per_lane_km`](../configuration/reference/actors.md#s-actors-vehicles-demand-rate-veh-per-h-per-lane-km)
    recomputes `rate_veh_per_h` from the world's lane length after import, so
    `--rate-veh-per-h` has no effect on it. Change the density in an overlay instead.

A scenario that names `exporters` refuses `--no-recording` (every exporter reads the
recording) and `--checkpoint-every` (exporters read one `recording.mcap`, a checkpointed
run records in segments).

```sh
v2xw run scenarios/manhattan-day.yaml --out runs/day --record-level metrics --checkpoint-every 1h
```

## `resume`

`v2xw resume [OPTIONS] <CHECKPOINT>`: rebuild the run from `scenario.resolved.yaml` beside
the checkpoint, replay it to the checkpoint's instant with outputs off, refuse unless the
record digest there matches, then continue writing the outputs.

| Argument or flag | Type | Default | Meaning |
|---|---|---|---|
| `<CHECKPOINT>` | path | required | `checkpoint.json` in the interrupted run's output directory |
| `--quiet` | flag | off | no progress lines on stderr |
| `--json` | flag | off | print the outcome as JSON |

```sh
v2xw resume runs/day/checkpoint.json
```

## `validate`

`v2xw validate [--json] <SCENARIO>`: load, merge overlays, migrate and validate; print
every error with its dotted field, and advisory notes. Exit 1 on any error.

```sh
v2xw validate scenarios/attack-fake-positions.yaml
```

## `info`

`v2xw info [--json] <RECORDING>`: the recording's size, profile, cadence, content digest,
channels with their encoding, visibility class and message counts, attachments, the
verification walk (frames, records, chunk checksums) and the manifest.

```sh
v2xw info runs/highway/recording.mcap
```

## `cities`

`v2xw cities [--json] [FILTER]`: the city library (`worlds/cities.yaml`, compiled into the
binary), grouped by state, with each city's id, name, highway preset, description and
`[not fetched]` when its extract is missing. `FILTER` keeps cities whose name, id, state
or state code contains it, ignoring case.

```sh
v2xw cities texas
```

## `import-osm`

`v2xw import-osm [OPTIONS] --speed-preset <NAME> <EXTRACT> <OUT>`: write `world.vwb`
(the browser's `vwp-world/1` payload), `world.json`, `world.v2xw` (the engine's native
binary) and `report.txt` (the importer's report) into `OUT`.

| Argument or flag | Type | Default | Meaning |
|---|---|---|---|
| `<EXTRACT>` | path | required | the `.osm` or `.osm.xml` extract |
| `<OUT>` | path | required | output directory |
| `--speed-preset` | name | **required, no default** | the `highway=*` class-default speed table: `urban-us-nyc`, `urban-us-portland`, `urban-de`, `sumo-german`, or an `urban-us-<state>` preset |
| `--bbox` | `MIN_LON,MIN_LAT,MAX_LON,MAX_LAT` | the extract's bounds | box to keep; its south-west corner is the world origin |
| `--imported-at` | ISO 8601 | this machine's clock | provenance date; excluded from the world hash |
| `--json` | flag | off | print the outcome as JSON instead of the report |

!!! note "Box order"
    `import-osm --bbox` is longitude first (`min_lon,min_lat,max_lon,max_lat`);
    `worlds/fetch.sh` and `worlds/cities.yaml` are south, west, north, east. An import
    with no drivable lane is refused with the box and the argument order in the message.

```sh
v2xw import-osm worlds/cache/manhattan.osm.xml out/manhattan --speed-preset urban-us-nyc \
    --bbox -73.9900,40.7440,-73.9680,40.7620
```

## `export-sumo`

`v2xw export-sumo [-o <OUT>] [--json] <SCENARIO>`: the scenario's world as SUMO plain XML
(`.nod.xml`, `.edg.xml`, `.con.xml`), a `.net.xml` when `netconvert` is found, and a
`.rou.xml` of every trip the scenario's demand draws, with this engine's routes.
`-o`/`--out` defaults to `sumo-export`. Guide: [SUMO co-simulation](../guide/sumo-cosimulation.md).

```sh
v2xw export-sumo scenarios/phase1-grid.yaml --out out/sumo
```

## `experiment`

Three subcommands over a scenario's `experiment` block. Guide:
[Experiments and sweeps](../guide/experiments-and-sweeps.md).

| Subcommand | Does |
|---|---|
| `run <SCENARIO>` | expand the sweep, run every outstanding run, write the results table; finished runs are skipped (the journal decides) |
| `status <SCENARIO>` | how far along the sweep is and which runs are next; runs nothing |
| `resume <SCENARIO>` | as `run`, but refuse if no journal for this sweep exists in `--out` |

`run` and `resume` flags:

| Flag | Type | Default | Meaning |
|---|---|---|---|
| `-o`, `--out` | path | `runs/<meta.name>-sweep` | the sweep's output directory |
| `--build-utc` | ISO 8601 | the clock, read once | the manifest timestamp every run shares |
| `--concurrency` | integer | `1` | simulations at once; each holds its own world and buffers, so `k` runs cost `k` times the memory |
| `--ci` | `90`, `95`, `99` | `95` | confidence level of the aggregate, percent |
| `--keyframe-ms` | integer, ms | `1000` | recording keyframe period per run |
| `--no-recording` | flag | off | no recording per run; metrics, report and manifest are written anyway |
| `--node-only` | flag | off | NODE-only profile in every run |
| `--no-verify` | flag | off | skip reading each recording back |
| `--format` | `parquet`, `arrow`, `jsonl` | none | also write the results table in this format beside `results.json` |
| `--json` | flag | off | print the outcome as JSON |

`status` takes `-o`/`--out` and `--json`.

```sh
v2xw experiment run scenarios/pdr-vs-distance.yaml --out runs/pdr --format parquet
```

## `compare`

`v2xw compare [OPTIONS] <A> <B>`. Each side is a `v2xw run --out` directory, a run digest
JSON (`v2xw/run-digest/1`, the Studio's export or `runs.digest`), an experiment output
directory, or comma-separated replications. Guide: [Comparing runs](../guide/comparing-runs.md).

| Flag | Meaning |
|---|---|
| `--all` | show every metric, not only the meaningful changes |
| `--json` | JSON (`v2xw/run-comparison/1`) |
| `--markdown` | Markdown |
| `--csv` | CSV |

```sh
v2xw compare runs/a1,runs/a2,runs/a3 runs/b1,runs/b2,runs/b3 --markdown
```

## `compare-signatures`

`v2xw compare-signatures [OPTIONS] <SCENARIO>`: the same scenario and seed under each
scheme, with the signing-overhead metrics on; writes `comparison.json`, `.csv` and `.md`.

| Flag | Type | Default | Meaning |
|---|---|---|---|
| `--schemes` | comma list | `ecdsa-p256,ml-dsa-44,falcon-512,hybrid-mldsa44-ecdsa-p256` | scheme ids; ECDSA P-256 is added as the baseline when missing |
| `-o`, `--out` | path | `runs/<meta.name>-signatures` | output directory |
| `--duration` | number, s | scenario | simulated seconds per run |
| `--build-utc` | ISO 8601 | the clock | manifest timestamp every run uses |
| `--record` | flag | off | write a recording per run |
| `--json` | flag | off | print the comparison as JSON |

```sh
v2xw compare-signatures scenarios/compare-signature-schemes.yaml --duration 60
```

## `sensitivity`

`v2xw sensitivity [OPTIONS] [SPEC]`. A spec file, or `--catalogue`, or `--expect`. Guide:
[Sensitivity analysis](../guide/sensitivity.md).

| Argument or flag | Type | Default | Meaning |
|---|---|---|---|
| `[SPEC]` | path | required unless `--catalogue` or `--expect` | a spec file: a base and one-at-a-time rows |
| `--catalogue` | flag | off | sweep every published numeric and choice setting, low and high end, against a same-seed base |
| `--expect` | flag | off | run only the expectation rows and the interaction checks; exit 1 on a failed row (conflicts with `--catalogue`) |
| `--families` | path | `scenarios/sensitivity/catalogue.yaml` | families, routes and value overrides |
| `--expectations` | comma list | `crates/v2xw-engine/tests/expectations` | expectation files or directories |
| `--interactions` | comma list | `crates/v2xw-engine/tests/expectations/interactions` | interaction files for `--expect` |
| `--only` | comma list | all | keep settings whose path starts with one of these prefixes |
| `--jobs` | integer | `2` | parallel runs (`--catalogue`, `--expect`) |
| `--plan` | flag | off | print the plan and run nothing |
| `--cache` | path | none | JSON-lines cache of finished perturbations; a rerun reuses matching entries |
| `--strict` | flag | off | `--catalogue`: exit 1 when an expectation is contradicted |
| `--markdown` | path | none | also write the results as a Markdown table |
| `--json` | flag | off | print JSON instead of the table |
| `--base-only` | flag | off | spec mode: run only the base and print every metric it measured |

```sh
v2xw sensitivity scenarios/sensitivity/radio-dsrc.yaml
v2xw sensitivity --catalogue --only radio.phy --plan
```

## `ma-replay`

`v2xw ma-replay [--policies <SCENARIO>] <DIR>`: replay the run's recorded misbehaviour
reports through a family of MA decision rules, without re-simulating; writes
`ma-replay.csv` in the run directory. The run needs
[`security.scms.report_records: true`](../configuration/reference/security.md#s-security-scms-report-records).
`--policies` replays the scenario's MA policy family instead of the built-in rules and
writes `ma-replay-policies.csv`.

```sh
v2xw ma-replay runs/attack
```

## `check-traffic`

`v2xw check-traffic [OPTIONS] <DIR>`: audit a finished run's traffic from its recording
(needs `recording.mcap` at `--record-level full` and `scenario.resolved.yaml`). Exit 1
when an invariant fails.

| Flag | Type | Default | Meaning |
|---|---|---|---|
| `--green-stop-s` | number, s | `6.0` | how long a queue head may stand on its own protected green |
| `--examples` | integer | `8` | examples printed per row |
| `--trace` | actor id | none | print this actor's every step on stderr |
| `--json` | flag | off | print the check as JSON |

```sh
v2xw check-traffic runs/midtown-normal
```

## `energy`

`v2xw energy [OPTIONS] <RECORDING>`: score a full-profile recording (it reads
`gt.kinematics`) for CO2, fuel, NOx, PM and EV energy, with health bands, without
re-running it.

| Flag | Type | Default | Meaning |
|---|---|---|---|
| `--spec` | path | the recording's own `energy` section | a JSON or YAML `energy:` section body that replaces it |
| `--emission-model` | id | section's | `hbefa3`, `phemlight`, `moves-opmode`, or a full id |
| `--ev-model` | id | section's | `kurczveil`, `vt-cpem`, or a full id |
| `--vehicles-csv` | path | none | one CSV row per vehicle |
| `--opmode-csv` | path | none | the MOVES operating-mode distribution per lane |
| `--json` | flag | off | print the outcome as JSON |

```sh
v2xw energy runs/glosa/recording.mcap --emission-model phemlight --vehicles-csv out/veh.csv
```

## `serve`

`v2xw serve [OPTIONS]`: the engine server with the assistant's endpoints on the same
origin. Guides: [Studio tour](studio-tour.md), [Assistant](../guide/assistant.md).

| Flag | Type | Default | Meaning |
|---|---|---|---|
| `--scenario` | path | none: the synthetic fixture | the scenario to serve |
| `--attach` | URL | none | serve only the assistant's endpoints in front of an engine already running there |
| `--host` | IP | `127.0.0.1` | bind address; a non-loopback bind needs `--token` |
| `--port` | integer | `8787` | TCP port |
| `--token` | string | none | bearer token required on every request |
| `--paused` | flag | off | start paused at t = 0 |
| `--speed` | number | `1` | multiple of real time; `0` is unthrottled |
| `--record` | path | none | write the MCAP recording here |

```sh
v2xw serve --scenario scenarios/midtown-normal.yaml --paused
```

## `agent`

`v2xw agent [OPTIONS] [PROMPT]`: the assistant builds a scenario from the prompt, runs
it, analyses it and prints a Markdown report on stdout (progress on stderr). Guide:
[Assistant](../guide/assistant.md).

| Argument or flag | Type | Default | Meaning |
|---|---|---|---|
| `[PROMPT]` | text | empty | what to simulate and find out |
| `--engine` | URL | none: start one in-process | a running engine to drive |
| `--scenario` | path | `scenarios/phase1-grid.yaml` | the scenario an in-process engine starts with |
| `--no-model` | flag | off | run without a language model: the scenario runs as shipped and the report is the analyst's alone |
| `--analyse` | flag | off | only analyse the run the engine is serving |
| `--base` | `current` or a preset id | `current` | the scenario a model-less run uses |
| `--token` | string | none | the engine's bearer token |
| `--json` | flag | off | print the report as JSON |

```sh
v2xw agent --no-model > report.md
```

---

## `v2xw-server`

The server program, with its own flag parser (`v2xw-server --help`). It runs the real
engine with `--scenario`, or a synthetic fixture stream without one, and serves the
assistant unless `--no-agent`. Endpoints and methods: [Engine server and RPC](../dev/server.md).

| Flag | Default | Meaning |
|---|---|---|
| `--scenario <p>` | none | scenario to run; without it the synthetic fixture runs |
| `--record <p>` | none | write the MCAP recording here |
| `--build-utc <t>` | the clock | manifest build timestamp |
| `--retain <n>` | `36000` | mobility steps of history kept for `run.seek` |
| `--retain-mb <n>` | `1024` | most memory that history may take, MB |
| `--lookahead <n>` | `64` | steps the kernel may run ahead of the stream |
| `--resume <p>` | none | open a `v2xw run --checkpoint-every` run at its `checkpoint.json`, computed to that instant and served paused |
| `--actors <n>` | `120` | fixture: actors to drive |
| `--grid <n>` | `8` | fixture: junctions per side |
| `--block <m>` | `150` | fixture: junction spacing, metres |
| `--duration <s>` | `600` | fixture: simulated seconds |
| `--seed <n>` | `20260918` | fixture: phase seed |
| `--port <n>` | `8787` | TCP port; `0` for an ephemeral one |
| `--host <addr>` | `127.0.0.1` | bind address; a non-loopback bind needs `--token` |
| `--token <t>` | none | bearer token on the upgrade and every HTTP request |
| `--speed <x>` | `1` | multiple of real time; `0` is unthrottled |
| `--paused` | off | start paused at t = 0 |
| `--label <s>` | none | human label for the run (`Hello.str_run_label`) |
| `--quiet` | off | no banner |
| `--no-agent` | off | serve the engine without the assistant's endpoints and watcher |
| `--env-file <p>` | `./.env` when present | read the assistant's provider variables from this file |

```sh
v2xw-server --actors 200 --port 8787          # a conforming stream with no map, for UI work
```

## `run-sim.sh`

`./run-sim.sh [scenario] [extra v2xw-server flags]`: builds `target/release/v2xw-server`
when it is missing (with `-j 1`), then serves the scenario
(default `scenarios/manhattan-5min.yaml`) on `$V2XW_PORT` (default `8787`), paused and
unthrottled, reading the assistant's variables from `.env`:

```sh
./run-sim.sh scenarios/midtown-pedestrians.yaml
# = v2xw-server --scenario scenarios/midtown-pedestrians.yaml --port 8787 --paused --speed 0 --env-file .env
```

## `v2xw-demand`

The demand and route-choice tool (`cargo build --release -p v2xw-demand-cli`, binary
`v2xw-demand`). Every command takes `--key value` pairs; the world is `--scenario FILE`
(exactly the world a run of that scenario builds, so route files match it) or
`--world grid` (the procedural test grid, the default). Every output is deterministic;
any error prints `v2xw-demand: <message>` and exits 2.
Guide: [Traffic demand](../guide/traffic-demand.md).

| Command | Required | Optional (default) | Writes |
|---|---|---|---|
| `od2trips` | `--matrix M`, `--out F` | `--zones grid:CxR` (`grid:3x3`), `--start HH:MM` (`00:00`), `--scale` (`1`) | trips (`v2xw/trips/1`) |
| `purposes` | `--out F` | `--daily N` (`20000`), `--day D` (`0`) | trips for one day |
| `assign` | `--trips F`, `--out F` | `--choice gawron\|logit\|msa` (`gawron`), `--loader queue\|bpr\|micro` (`queue`), `--iterations` (`25`), `--gap` (`0.02`), `--warm routes.json` | routes (`v2xw/routes/1`) and an iteration log on stdout |
| `sample` | `--counts F`, `--out F` | `--method max-entropy\|greedy` (`max-entropy`) | routes reproducing link counts, and a GEH check |
| `check` | `--counts F`, `--routes F` | `--loader` (`queue`) | GEH per count, share under 5, RMSE, R² |
| `gtfs-clip` | `--feed DIR`, `--bbox S,W,N,E`, `--date YYYYMMDD`, `--out DIR` | | a clipped GTFS feed |
| `gtfs` | `--feed DIR`, `--date YYYYMMDD` | `--start HH:MM` (`08:00`), `--run S` (`900`) | a summary of the transit trips and stops placed |

```sh
v2xw-demand gtfs --scenario scenarios/portland-transit.yaml \
    --feed worlds/cache/portland.gtfs --date 20261007 --start 08:00
```

## Other programs

| Program | Built from | Does |
|---|---|---|
| `cross_validate` | `v2xw-cli` (`just cross-validate`) | traffic and radio cross-validation against SUMO, Veins and OpenCV2X; needs SUMO |
| `city_audit` | `v2xw-cli` | import a city and run it under the traffic auditor; measures `drive_lane_km` and `signals` for `worlds/cities.yaml` |
| `traffic_validation`, `traffic_calibration` | `v2xw-trafficval` | the traffic validation and calibration suites |

What these suites check: [Validation and conformance tools](../dev/tools.md).
