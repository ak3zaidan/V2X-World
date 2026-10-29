# `portland-downtown.asc`: measured terrain under downtown Portland (provenance)

**What it is.** An ESRI/USGS ASCII grid (`v2xw_world::dem::read_ascii_grid`, geodetic
header with `xllcenter`/`yllcenter`) of 81 × 70 posts at 1 arc-second (1/3600°; about
21.7 m east-west and 30.9 m north-south at 45.5° N). It covers latitude 45.50889° to
45.52806° and longitude −122.68917° to −122.66694°: the `portland` map box of
`worlds/cities.yaml` (45.5120..45.5250, −122.6860..−122.6700) with a margin of about
0.003° on every side. Heights are whole metres, 2 m to 115 m, no voids.

**Source.** The 1° tile `N45W123.hgt.gz` of the AWS open-data "Terrain Tiles" set,
`skadi` layout, fetched without a login on 2026-10-28 from
`https://s3.amazonaws.com/elevation-tiles-prod/skadi/N45/N45W123.hgt.gz`.

| File | Bytes | SHA-256 |
|---|---|---|
| `N45W123.hgt.gz` (as fetched) | 10,591,287 | `3f244a6f8e658f5ca30b8a39ce7242838a14ba46fa97b160648fbfbfd6523efc` |
| `N45W123.hgt` (gunzipped) | 25,934,402 = 3601² × 2 | `8c5ed48a863162276100c3646294934eac8bfe9ebe04a078abb79d12b591c96e` |
| `portland-downtown.asc` (this cut) | 16,033 | `2c7e8710a351fe5ce576f2782da62dee86a2c620b0e7868e270015277f39e053` |

The tile is not vendored (26 MB); the cut is.

**Format and datum** (Joerd `docs/formats.md`, read 2026-10-28): skadi tiles are "1°x1°
tiles", "Essentially... the SRTMGL1 format tiles but with global coverage and compressed
using gzip": big-endian 16-bit integers, −32768 for a void, and "All elevations are in
meters referenced to the WGS84/EGM96 geoid". The tile size, 3601 × 3601 samples, is the
SRTMGL1 one-arc-second lattice. So the vertical datum is **EGM96**, not NAVD88 (they differ
by about 1 m at Portland; only differences of height reach the engine, through grade and
the terrain diffraction term).

**Which survey.** Joerd's `docs/data-sources.md` (read 2026-10-28) lists, for US land,
"`SRTM`, `NED/3DEP` 1/3 arcsec" and "10 meters outside of Alaska". It does not say which
of them fills the skadi product, so this file claims only "SRTM or 3DEP, resampled to
1 arc-second by Joerd". The 3DEP 1/3 arc-second GeoTIFF (USGS, public domain) is the
better source and needs an offline GeoTIFF-to-ASCII conversion (`dem.rs` reads no
GeoTIFF on purpose); that is left as an upgrade.

**Licence and attribution** (Joerd `docs/attribution.md`, read 2026-10-28): "All 3DEP
products available through The National Map are in the public domain and may be used
without restriction." The attribution line the tiles ask for in the United States is:
"United States 3DEP (formerly NED) and global GMTED2010 and SRTM terrain data courtesy of
the U.S. Geological Survey." The engine's card reuses `dem::SRTM_LICENCE`
(`NASA-EOSDIS-open`) and `SRTM_ATTRIBUTION`, as the build plan says (WP-26 decision 3).

**How it was cut.** `demcut.py` (a 100-line Python script kept outside the repo, run with
`python3 -I`): rows `floor((46 − 45.528)·3600)` = 1699 to `ceil((46 − 45.509)·3600)` =
1768 and columns `floor((−122.689 + 123)·3600)` = 1119 to 1199 of the tile, written
north row first, unchanged. `xllcenter` is column 1119's longitude and `yllcenter` row
1768's latitude, so each value stands at its post, as in the `.hgt`.

**Grades on the map** (the same script, 2026-10-28): every OSM `highway` way of
`worlds/cache/portland.osm.xml` from `motorway` to `service`, bridges and tunnels left
out, inside the map box, 49.8 km of road; grade = |Δz| over a 10 m chord from a bilinear
read of the tile.

| Grade | 0-1 % | 1-2 % | 2-3 % | 3-4 % | 4-5 % | 5-6 % | 6-8 % | 8-10 % | 10-15 % | ≥ 15 % |
|---|---|---|---|---|---|---|---|---|---|---|
| Share of road length | 39.5 % | 16.3 % | 11.1 % | 13.5 % | 11.3 % | 2.8 % | 3.4 % | 1.1 % | 0.7 % | 0.4 % |

19.6 % of the road length is above 4 % on 10 m chords and 15.6 % between OSM nodes (one
block is about 60 m). The steepest blocks longer than 40 m are on SW Columbia Street
(7.2 % and 6.5 %), SW Jefferson Street (6.8 %) and SW Clay Street (6.5 %), between SW 4th
Avenue and I-405: the rise towards the West Hills that the build plan named. The plan's condition, a grade above
4 %, holds, so the site stays Portland downtown. The 10-15 % and ≥ 15 % bins (1.1 %
of the length) are short chords next to the I-405 cut and ramps, where a 30 m post
spacing cannot resolve the cut wall; they are a limit of a 1 arc-second DEM, not
roads that steep.

**Limits.** The drape (`DrapeOptions::all`) lays every lane on the ground, including
any bridge deck and the I-405 trench on the map's west edge; 1 arc-second posts smooth
grades shorter than about 30 m.
