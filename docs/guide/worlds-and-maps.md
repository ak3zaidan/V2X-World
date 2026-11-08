# Worlds and maps

A world is everything that does not move: lanes, junctions with their connectors and
conflict matrices, signal plans, crossings, buildings, terrain, vegetation, roadside
sites, and a provenance record. `v2xw-world` builds it once per run, quantises it, hashes
it (`world_hash` in the manifest) and every other crate reads it. A scenario gets its world
one of three ways:

| Way | Scenario says | Needs |
|---|---|---|
| a city of the library | [`city: <id>`](../configuration/reference/city.md#s-city), or `--city <id>` | the extract in `worlds/cache/<id>.osm.xml` |
| an explicit source | `world.source: {kind: …}` | the file it names, or nothing for a procedural generator |
| a city replacing the file's world | `v2xw run <scenario> --city <id>` | as the first row |

Every world setting: [`world` reference](../configuration/reference/world.md). The import
models and their equations: [Traffic and mobility](../models/traffic.md).

## The city library

`worlds/cities.yaml` (schema `v2xw/cities/1`, compiled into the binary so every interface
lists the same cities) holds the largest city of every US state and of DC, the most-studied
connected-vehicle sites (the New York and Tampa USDOT pilots, Ann Arbor, Columbus,
Pittsburgh, the large Texas and California cores) and Berlin-Mitte for the European stack.
Each entry is a downtown box of about 1.5 km × 1.5 km:

| Field | Meaning |
|---|---|
| `id` | the city's name everywhere; its extract is `worlds/cache/<id>.osm.xml` |
| `bbox` | `[south, west, north, east]`, WGS 84; its south-west corner is the world origin, so re-fetching cannot move a coordinate |
| `highway_preset` | the jurisdiction's default speed limits and rules of the road (`crates/v2xw-world/src/jurisdiction.rs` cites each) |
| `utc_offset_h` | standard-time offset |
| `fetched` | the download date, which becomes `world.imported_at` |
| `drive_lane_km`, `signals` | driving-lane length and signalised intersections, measured by `city_audit`; used to rescale demand |

```sh
v2xw cities                       # every city by state, [not fetched] where the extract is missing
v2xw cities --json                # the registry as JSON
worlds/fetch.sh boston            # one extract
worlds/fetch.sh --all             # every missing one (one at a time, FETCH_PAUSE_S apart)
worlds/fetch.sh --list            # ids and whether each extract is present
worlds/fetch.sh mytown 45.51,-122.69,45.53,-122.66   # any box: S,W,N,E
```

`fetch.sh` asks the Overpass API for every node and way in the box, the turn restrictions
and the building multipolygons, leaves out far-reaching relations, retries with backoff,
falls back to mirrors (`OVERPASS_URL` puts yours first) and rejects a truncated answer.
`V2XW_WORLDS_DIR` points the engine at another extract directory.

### What `--city` changes

A scenario says what happens; a city says where (`crates/v2xw-engine/src/city.rs`). To make
any scenario fit any city:

1. **The world.** The city's extract, box, highway preset and fetch date go into `world`;
   the scenario's building and signal options stay.
2. **What belonged to the old city.** Roadside units at explicit positions become an
   [`actors.rsu_placement`](../configuration/reference/actors.md) of the same count and
   roles; a fixed arrival rate becomes a rate per kilometre of driving lane; the clock
   keeps its local time of day; a closure of a named street becomes `target: auto`; a
   focus box is dropped.
3. **The built world.** Once imported, a per-lane-km rate becomes vehicles per hour and
   placed roadside units stand at the busiest signalised intersections (most approach
   lanes first, at least `min_spacing_m` apart).

So a scenario written with
[`rate_veh_per_h_per_lane_km`](../configuration/reference/actors.md#s-actors-vehicles-demand-rate-veh-per-h-per-lane-km),
`rsu_placement`, `target: auto` and `at: busiest-junction` runs on every city unchanged.
Before a city is listed it is imported and run under the traffic auditor; a city that
fails is listed under `dropped:` with the reason.

## World sources

`world.source.kind` selects the importer:

| `kind` | Reads | Importer |
|---|---|---|
| `osm-xml` | `path` to a `.osm`/`.osm.xml` extract, optional `bbox` (`min_lat_deg`, `min_lon_deg`, `max_lat_deg`, `max_lon_deg`) | `world/source/osm` |
| `sumo-net` | `path` to a SUMO `net.xml` | `world/source/sumo-net` |
| `open-drive` | `path` to an ASAM OpenDRIVE `.xodr`, converted through `netconvert` | `world/source/opendrive` |
| `procedural` | `generator` and `params` | `world/source/procedural-grid`, `-radial`, `-random` |

`osm-bbox` (fetch at load) and `legacy-json` are accepted by the type but have no importer
in this build; a scenario naming them is refused with `UnsupportedSource`.

=== "OpenStreetMap"

    ```yaml
    world:
      source:
        kind: osm-xml
        path: worlds/cache/manhattan.osm.xml
        bbox: {min_lat_deg: 40.744, min_lon_deg: -73.990, max_lat_deg: 40.762, max_lon_deg: -73.968}
      highway_preset: urban-us-nyc
      imported_at: "2026-09-18T00:00:00Z"
    ```

=== "SUMO network"

    ```yaml
    world:
      source: {kind: sumo-net, path: worlds/sumo/grid3.net.xml}
      imported_at: "2026-10-07T00:00:00Z"
    ```

=== "Procedural grid"

    ```yaml
    world:
      source:
        kind: procedural
        generator: world/source/procedural-grid
        params: {cols: 13, rows: 34, block_x_m: 274.0, block_y_m: 80.0,
                 lanes_per_direction: 2, signalised: true, cycle_s: 60.0,
                 crossings: true, block_buildings: true}
      imported_at: "2026-09-18T00:00:00Z"
    ```

### OpenStreetMap

The importer builds drivable, pedestrian and cycle networks, junctions with connectors and
conflict matrices, synthesised fixed-time signals, buildings with sourced heights,
land-use zones and a provenance record. It needs a speed preset for roads with no
`maxspeed` tag, and has no default: a fallback speed limit is a statement about a
jurisdiction. Presets: `urban-us-nyc`, `urban-us-portland`, `urban-de`, `sumo-german`
(netconvert's table, for comparability with SUMO), and an `urban-us-<state>` preset for
every state ([`world.highway_preset`](../configuration/reference/world.md#s-world-highway-preset)).

OpenStreetMap carries no signal timing, so plans are synthesised; the
[`world.signals`](../configuration/reference/world.md#s-world-signals-cycle-s) block sets
the cycle (default 90 s), all-red, pedestrian walk (7 s, MUTCD 2009 §4E.06) and walking
speed (1.067 m/s), green-wave coordination and the joining of close junctions
(netconvert's `--tls.join`). These apply to imported worlds only.
[`world.service_roads`](../configuration/reference/world.md#s-world-service-roads) keeps or
drops `highway=service` alleys and driveways.

`v2xw import-osm` runs the same importer outside a run and writes the three world formats:

```sh
v2xw import-osm worlds/cache/portland.osm.xml out/portland --speed-preset urban-us-portland
```

| File | Format |
|---|---|
| `world.vwb` | the browser's binary `vwp-world/1` payload ([protocol §4](../protocol/vwp-v1.md)) |
| `world.json` | the same payload as JSON |
| `world.v2xw` | the engine's native binary, an exact round trip (same content hash) |
| `report.txt` | what the importer kept, dropped and inferred, with every anomaly counted |

The world validation report (geometry a car or pedestrian cannot use, every imported
attribute against its source tags) runs as
`cargo run -p v2xw-world --example world_report -- <extract> --preset <p> [--baseline worlds/validation/<city>.json]`;
with `--baseline` it fails when a check's count rises above the committed ceiling.

### SUMO `net.xml` and OpenDRIVE

`sumo-net` reads edges, lanes, junctions, connections, internal connectors, right-of-way
matrices, traffic-light programmes and crossings into the same lane-level world, counting
an anomaly wherever the formats disagree. Each SUMO edge id is kept as the edge's name, so a
SUMO lane id is always a world lane, which is what the TraCI co-simulation relies on
([SUMO co-simulation](sumo-cosimulation.md)).
[`world.sumo.signal_programmes`](../configuration/reference/world.md#s-world-sumo-signal-programmes)
runs actuated, delay-based and NEMA `tlLogic` as live controllers (`as-file`, the default)
or flattens them (`static`). OpenDRIVE goes through `netconvert`, then the same reader.

### Procedural networks

| Generator | Shape | Main parameters (defaults) |
|---|---|---|
| `world/source/procedural-grid` | a rectangular lattice of two-way streets; the reference world for tests and the TR 36.885 urban grid | `cols` 6, `rows` 6, `block_x_m`/`block_y_m` 120, `lanes_per_direction` 1, `signalised` false, `cycle_s` 60, `amber_s` 3, `ew_green_share` 0.5, `crossings`, `block_buildings`, `rsu_at_junctions`, `corner_radius_m` |
| `world/source/procedural-radial` | spokes from a central plaza crossed by ring roads | `arms` 6, `rings` 6, `ring_spacing_m` 120, `dropout` 0, `signalised` |
| `world/source/procedural-random` | an irregular network grown by random attachment, reproducible from its seed | `iterations` 150, `min_distance_m` 80, `max_distance_m` 250, `seed` 1, `min_angle_deg` 45 |

Every parameter with its source: [model parameters](../configuration/reference/models.md#world).
The random generator is the only one that draws numbers, from one declared stream.

## Buildings, terrain and vegetation

| Layer | Settings | Notes |
|---|---|---|
| Buildings | [`world.buildings.enabled`](../configuration/reference/world.md#s-world-buildings-enabled), `keep_holes`, `metres_per_level` | footprints from OSM with heights from tags; `metres_per_level` (the `building:levels` rule) is marked `todo-calibrate`. Buildings obstruct radio links |
| Terrain | [`world.terrain.dem`](../configuration/reference/world.md#s-world-terrain-dem), [`world.terrain.drape`](../configuration/reference/world.md#s-world-terrain-drape) | a DEM raster resampled bilinearly onto the world grid, voids filled from the nearest sample and counted. `drape: true` lifts every lane point, junction, building, site and signal head onto it (and changes the world hash) |
| Vegetation | [`world.vegetation`](../configuration/reference/world.md#s-world-vegetation) | keeps OSM woods, tree rows and single trees as a layer; [`radio.channel_model.foliage`](../configuration/reference/radio.md#s-radio-channel-model-foliage) charges the ITU-R P.833 excess loss through it |

DEM formats (`crates/v2xw-world/src/dem.rs`): SRTM `.hgt` (NASA SRTMGL1 one-degree tiles,
corner read from the file name) and ESRI/USGS ASCII grid (`.asc`), the path for Copernicus
GLO-30 after `gdal_translate -of AAIGrid`. Without a DEM every height is 0 and existing
world hashes are unchanged. `scenarios/real-world/portland-hills.yaml` is the worked
example, on `worlds/dem/portland-downtown.asc` (provenance in
`worlds/dem/portland-downtown.md`).

## Provenance and the cache

Every world carries a provenance record: the source kind and id, the source box, the import
date (from the scenario, never a clock, and excluded from the world hash), tool versions,
projection and origin, the transformations applied, each layer's licence and counts of
what was dropped. Worlds built from OpenStreetMap carry the ODbL attribution.

[`world.cache`](../configuration/reference/world.md#s-world-cache) names a directory: the
world is read from `<key>.v2xwworld` there when present, imported and written there when
not. The key hashes the `world` section and the source files' contents, so an edited
extract is a different world. The cached form round-trips exactly, so a cached run is the
same run; a corrupt entry is re-imported, never an error.

## Other data under `worlds/`

| Path | What |
|---|---|
| `worlds/fetch-gtfs.sh <city> [YYYYMMDD]` | download a GTFS feed, clip it to the box and one service day with `v2xw-demand gtfs-clip`, into `worlds/cache/<city>.gtfs/` ([Traffic demand](traffic-demand.md#public-transport-gtfs)) |
| `worlds/fetch-lust.sh` | rebuild the committed LuST subsection `worlds/sumo/lust-veremi-sub.net.xml` from a pinned commit and check its digest |
| `worlds/sumo/` | committed SUMO networks and route files for the interop scenarios |
| `worlds/validation/*.json` | world-validation baselines for `world_report --baseline` |
| `worlds/cache/` | git-ignored: extracts, GTFS feeds, cached worlds |
