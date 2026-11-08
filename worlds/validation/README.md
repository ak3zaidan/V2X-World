# `worlds/validation/` — what each city's import is held to

One JSON file per city extract: the most failures each check of
`v2xw_world::validate` may report on that city's import (a `Baseline`). The
`world_report` example compares a fresh import against it and exits 1, naming every
check that got worse:

```sh
worlds/fetch.sh portland
cargo run -p v2xw-world --example world_report -- worlds/cache/portland.osm.xml \
    --bbox 45.5120,-122.6860,45.5250,-122.6700 --preset urban-us-portland \
    --baseline worlds/validation/portland.json
```

| File | Extract | Box (S,W,N,E) | Preset |
|---|---|---|---|
| `manhattan.json` | `worlds/cache/manhattan.osm.xml` (D7) | 40.7440,-73.9900,40.7620,-73.9680 | `urban-us-nyc` |
| `portland.json` | `worlds/fetch.sh portland` | 45.5120,-122.6860,45.5250,-122.6700 | `urban-us-portland` |
| `berlin.json` | `worlds/fetch.sh berlin` | 52.5140,13.3880,52.5250,13.4080 | `urban-de` |

## The checks

Geometry a car, a cyclist or a pedestrian cannot use — pavements on the roadway, lanes
of two roads laid over each other, connectors a car cannot steer round, stub lanes,
lanes through buildings — and fidelity to the source: lane counts, one-way, `turn:lanes`,
bus lanes, cycle lanes, parking lanes, `width` and `maxspeed` against what was built. The
table of checks and what each counts is the module documentation of
`crates/v2xw-world/src/validate.rs`.

## Updating a baseline

A baseline is lowered when the importer gets better and raised only with a reason: an
OSM edit to the extract, or a deliberate change whose cost the commit message states.
`--write-baseline FILE` writes the current counts; review its diff before committing it.
The extracts change as OSM is edited, so a baseline is only meaningful against the
extract it was written from — the `description` names the file and its size.
