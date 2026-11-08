# Scenarios

A scenario file is the whole input of a run: world, fleet, radio, network, messages,
security, credential system, threats, detection, metrics and, optionally, a parameter
sweep. Apart from output paths and the explicit CLI overrides (which are recorded), there
is no configuration outside it, so a run is reproducible from one file and a seed.

Schema: `crates/v2xw-engine/src/scenario/schema.rs` (`Scenario`), current version
`v2xw/scenario/1`. Settings, ranges and their status: [configuration.md](configuration.md);
every field: the [generated reference](configuration/reference/index.md).

## Loading pipeline

```text
bytes -> parse (YAML or JSON) -> migrate to the current schema -> merge meta.base -> deserialise -> validate
```

Modules under `crates/v2xw-engine/src/scenario/`: `migrate.rs` (one migrator per schema
step), `merge.rs`, `validate.rs`. The loader reads files only (no environment, clock or
network), so two loads of one file give the same `Scenario`, and
`Scenario::content_hash` over its canonical JSON is the scenario hash in the manifest.
`validate` returns every problem at once, each as a dotted field plus the conflict, for
example `radio.tiers.phy: 'high' requires mac 'high' (mac is 'medium')`.

## Overlays (`meta.base`)

A scenario may name a base file, resolved relative to the including file. The merge
runs on the untyped document, so "not written" stays distinct from "written as the
default":

- mappings merge key by key, recursively;
- anything else, including a sequence, is replaced by the overlay;
- `null` deletes the base's key.

Most shipped scenarios are short overlays on `scenarios/base/city.yaml` (a downtown at
midday: connected fleet, the US SCMS, eight roadside units at the busiest signalised
intersections, detection on every vehicle) or `scenarios/base/radio-faceoff.yaml`.

## Top-level sections

| Section | Configures |
|---|---|
| `schema`, `meta` | version; name, description, tags, `base` |
| `seed` | the master seed; every RNG stream derives from it (hex or decimal) |
| `time` | `t0`, `duration_s`, `mobility_step_ms`, `des_resolution`, `warmup_s`, credential `calendar` (`real` weekly i-periods from the US epoch, or `compressed`), `fast_forward` windows |
| `city`, `world` | a city of the library, or an explicit world source (`kind: osm-xml`, `sumo-net` or `procedural` with a grid, radial or random generator), buildings, terrain |
| `actors` | vehicles (demand, equipped fraction, driving models, classes), VRUs, roadside units and their placement and roles, the backend deployment |
| `weather` | initial condition and intensity; fronts arrive through `events` |
| `radio` | `rat` (`dsrc-80211p`, `lte-v2x-pc5`, `nr-v2x-pc5`, `hybrid`), tiers per family, model choices, region, channel, MAC and DCC settings |
| `net` | `layer` (`wsmp`, `gn-btp`), GeoNetworking forwarding, fragmenter, backhaul, cellular Uu, backend network, channel access |
| `messages` | `sets` (`bsm`, `cam`, `denm`, `spat`, `map`, `srm`, `ssm`, `psm`, `vam`, `cpm`), `codec_tier` (`uper`, `size-model`), rates and intervals |
| `security` | `envelope` (`ieee1609.2`, `etsi103097`), `signature`, `kem`, `crypto_mode` (`modeled`, `real`), `verification_policy`, signer-id policy, `pseudonym_change`, `protocol` and its parameters, `scms` options |
| `nodes` | hardware profiles for OBUs, RSUs and backend entities; compute tiers |
| `threats` | `attackers`, `jammers`, `compromised_rsus` and their attack, `eavesdropper` |
| `detection` | local detector suites and the MA pipeline |
| `metrics` | metric ids, or `[all]` |
| `exporters` | output exporters ([Datasets and exports](guide/datasets.md)) |
| `events` | the timeline |
| `apps` | V2X applications: `fcw`, `eebl`, `ima`, `lta`, `bsw`, `pcw`, `rlvw`, `glosa` |
| `energy` | emission, fuel and EV energy models and fleet |
| `mobility` | `provider: native` (default) or `sumo` (TraCI) |
| `traffic` | junction and jam behaviour (right turn on red, rerouting, keep-clear, gridlock), signal control (fixed-time, actuated, NEMA, green wave) |
| `perception` | sensor suites for CPM senders and the occlusion rule |
| `scale` | city scale: a microscopic focus region in a mesoscopic city |
| `experiment` | `sweep` (dotted path to list of values), `seeds`, `replications` |

Every optional section, and every optional field at its default, serialises as absent,
so adding a field to the schema does not move existing scenario hashes.

## The timeline (`events`)

Each item has `t`, optional `until`, a `type` and its parameters; an item may instead
fire on a ground-truth condition (`when`, `max_fires`, `cooldown_s`). Types:
`demand.multiplier`, `weather.front`, `attack.wave`, `outage`, `param.change`, `closure`,
`safety.hard-brake`, `safety.breakdown`, `safety.cut-in`.

```yaml
events:
  - {t: 60.0, until: 240.0, type: demand.multiplier, value: 1.3}
  - {t: 90.0, until: 270.0, type: safety.breakdown, target: auto}
  - {t: 0.0, type: closure, target: auto}
```

## Cities

A scenario says what happens; a city says where. `worlds/cities.yaml` (compiled into the
binary; `v2xw cities` lists it) names each city's extract, downtown box and jurisdiction
(speed preset, right turn on red and similar rules, `v2xw_world::jurisdiction`).
`v2xw run <scenario> --city <id>`, the server's `run.start`/`scenario.load` and the
Studio's city picker replace the scenario's world with the city's. City-independent
forms make that work (`crates/v2xw-engine/src/city.rs`):

- `actors.vehicles.demand.rate_veh_per_h_per_lane_km`: demand as a density, multiplied by
  the imported network's driving-lane length;
- `actors.rsu_placement: {count, roles}`: units at the busiest signalised intersections;
- `target: auto` on a closure or breakdown; `at: busiest-junction` on a jammer;
- the clock keeps its local time of day.

Extracts are fetched with `worlds/fetch.sh <id>` (Overpass API) into `worlds/cache/`.
More: [Worlds and maps](guide/worlds-and-maps.md).

## Sweeps (`experiment`)

```yaml
experiment:
  sweep:
    security.protocol.id: [protocol/scms/camp, protocol/etsi/ts102941]
    actors.vehicles.demand.rate_veh_per_h: [1500.0, 15000.0]
  seeds: [1, 2, 3]
  replications: 2
```

The plan (`v2xw_experiment::plan`) is the cross product of the swept values, seed slots
and replications in a fixed order, each run's seed derived by hashing from the master
seed; the same seed is reused across cells (common random numbers). A swept path must
already exist in the serialised scenario, so set an optional field in the base before
sweeping it. `v2xw experiment run|status|resume` executes, journals and aggregates; runs
are serial by default ([Experiments and sweeps](guide/experiments-and-sweeps.md)).

## Shipped scenarios

`scenarios/ready-made.txt` lists the scenarios the Studio offers, in order; each file's
`meta.name` and `meta.description` are what the picker shows. Each file states at the top
what it measures and what it does not.

| File | Shows |
|---|---|
| `midtown-normal.yaml` | the honest baseline: signed BSMs at 10 Hz, the US SCMS, roadside units, detection |
| `midtown-rush-hour.yaml` | rising demand, a lane-blocking breakdown, a closed side street |
| `midtown-peak-load.yaml` | peak demand with pedestrians and cyclists: the channel and verification budget |
| `midtown-pedestrians.yaml`, `midtown-street-life.yaml` | pedestrians, jaywalking, groups, cyclists, motorcycles |
| `midtown-rain.yaml` | rain's effect on driving and on the radio (ITU-R P.838-3) |
| `highway-congestion.yaml` | an open-road channel on a procedural highway |
| `radio-dsrc.yaml`, `radio-lte.yaml`, `radio-nr.yaml` | identical traffic, only the radio access technology differs |
| `berlin.yaml` | the European stack: ITS-G5, GeoNetworking, CAMs |
| `attack-fake-positions.yaml` | position-falsification attackers detected, reported and revoked |
| `attack-replay.yaml` | replayed and delayed, validly signed messages |
| `attack-jamming.yaml` | a constant jammer at the busiest junction, the drop and the recovery |
| `privacy-pseudonyms.yaml` | the credential lifecycle with a roadside eavesdropper |
| `europe-credentials.yaml` | the ETSI credential system (enrolment, authorization tickets, ECTL, CA-CRL) |
| `post-quantum.yaml`, `compare-signature-schemes.yaml` | post-quantum and hybrid signatures and their cost on the air |
| `emergency-priority.yaml` | SPaT, MAP, SRM and SSM: signal priority for emergency vehicles |
| `manhattan-5min.yaml`, `manhattan-day.yaml` | five minutes, or a whole day on the deployment calendar, of Midtown |
| `connected-intersections.yaml` | four roadside units broadcasting SPaT and MAP from the engine's signal plans, with emergency vehicles requesting priority (SRM) and the units answering (SSM) |
| `phase1-grid.yaml`, `phase1-manhattan.yaml` | one equipped vehicle sending signed BSMs at 10 Hz on a Midtown-shaped procedural grid, or on the real Manhattan network: packet delivery ratio |
| `phase2-manhattan.yaml` | an equipped fleet on Manhattan with one ConstPos attacker, 55 roadside units, the CAMP SCMS backend over cellular Uu, the legacy twelve-check detectors, the corroborating authority and a revocation |
| `vru-grid.yaml` | `phase1-grid.yaml` on a smaller grid with a walk network: 150 pedestrians at the crosswalks, 15 cyclists, 30 % of the people sending signed PSMs |
| `portland-downtown.yaml`, `portland.yaml` | downtown Portland, Oregon, on its real one-way grid with bus, cycle and parking lanes and a slow green wave |
| `portland-transit.yaml` | downtown Portland at the morning peak with TriMet's timetabled buses, MAX and Streetcar (GTFS) |
| `credential-weeks.yaml` | the US SCMS lifecycle over one week of the real calendar, quiet stretches fast-forwarded |
| `scms-conformance.yaml` | the SCMS conformance run: enrolment, provisioning, a week rollover, RSU certificate renewal, a misbehaviour report through to CRL and enforcement, a CTL change; every PDU captured for the oracle |
| `scms-audit-mbpipe.yaml` | the conformance deployment with report-fate records: every misbehaviour report followed from detection to enforcement |
| `scms-oracle-mbr.yaml`, `scms-oracle-repro.yaml`, `scms-pq-relay-repro.yaml` | inputs to the SCMS oracle and reproduction tests (two providers with automatic MA over two weeks; the conformance run's first 640 s; hybrid Falcon-512 + P-256 frames above the WSMP MTU) |
| `threshold-revocation-2of2.yaml`, `threshold-revocation-3of5.yaml` | revocation with CAMP's two Linkage Authorities (one down from 90 s), and with a 3-of-5 linkage committee instead |

Study files with an `experiment` block include `pdr-vs-distance.yaml`,
`density-sweep-congestion.yaml`, `pseudonym-privacy.yaml`, `pseudonym-strategies.yaml`,
`revocation-latency.yaml`, `glosa-energy.yaml`, `rat-comparison.yaml`, `pq-signatures.yaml`,
`pq-full-stack.yaml` and `pq-certificate-cycle.yaml`; `credential-lifecycle.yaml`,
`ccms-lifecycle.yaml` and the `threshold-*.yaml` files exercise the credential systems.
The folders:

| Folder | Holds |
|---|---|
| `scenarios/base/` | the bases other files overlay: `city.yaml`, `midtown.yaml`, `radio-faceoff.yaml` |
| `scenarios/edge/` | boundary cases: clock skew, long offline periods, CRL Generator outages, holidays, a 10,000-entry standing CRL, no coverage |
| `scenarios/scale/` | a scaling ladder (2, 10, 100, 1,000 and 10,000 vehicles) |
| `scenarios/real-world/` | the realistic SCMS benchmarks on Midtown (DSRC, LTE-V2X and a quarter-fleet version) and Portland, the Midtown realism check, Portland on its measured terrain |
| `scenarios/interop/` | SUMO interoperation: a SUMO network and a `.rou.xml` demand ([SUMO co-simulation](guide/sumo-cosimulation.md)) |
| `scenarios/benchmark/` | the VeReMi Extension-style legacy-attack export on three seeds ([Datasets and exports](guide/datasets.md)) |
| `scenarios/sensitivity/` | the family files and the catalogue the sensitivity sweep runs ([Sensitivity analysis](guide/sensitivity.md)) |
| `scenarios/platform/` | sensitivity specs for platform checks: BSM inter-transmission time, the CBR window, J2945/1 congestion control, RSU application rates, and seed spread |
| `scenarios/tests/` | fixtures the Rust integration tests and the Studio's end-to-end specs load |

Scenarios with a procedural world (for example `highway-congestion.yaml`,
`connected-intersections.yaml`, `credential-lifecycle.yaml`, `pdr-vs-distance.yaml`) run
without any download.
