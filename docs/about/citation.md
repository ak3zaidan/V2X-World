# Citation and licence

## How to cite

V2X World is written by **Ahmed Zaidan**
([ORCID 0009-0009-4590-7996](https://orcid.org/0009-0009-4590-7996)). The repository's
[`CITATION.cff`](https://github.com/ak3zaidan/V2X-World/blob/main/CITATION.cff) is the
citation record (GitHub's "Cite this repository" button reads it). Its current values:

| Field | Value |
|---|---|
| title | V2X World |
| author | Ahmed Zaidan ([0009-0009-4590-7996](https://orcid.org/0009-0009-4590-7996)) |
| version | 0.1.0 |
| licence | Apache-2.0 |
| type | software |
| keywords | V2X, connected vehicles, SCMS, IEEE 1609.2, misbehavior detection, misbehavior authority, vehicular PKI, simulation |
| repository | <https://github.com/ak3zaidan/V2X-World> |
| website | <https://ak3zaidan.github.io/V2X-World/> |

To cite the software:

```bibtex
@software{zaidan_v2xworld,
  title   = {{V2X World}},
  author  = {Zaidan, Ahmed},
  version = {0.1.0},
  year    = {2026},
  license = {Apache-2.0},
  url     = {https://github.com/ak3zaidan/V2X-World}
}
```

A paper describing the simulator is in preparation.

The [V2X-SCMS-MBD dataset](https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini) on Hugging Face
is licensed CC BY 4.0, which requires attribution: cite it with the software entry above
(or the paper, once it is out) and name the dataset release you used.

For a result, also report what makes the run reproducible: the scenario file (or its
`scenario_hash`), the seed, the engine's `build_hash` and `git_commit`, and the
`content_digest` of the recording, all in the run's `manifest.json` and `run-report.json`
([Recording format](../dev/recording-format.md#the-manifest)). Cite the models' sources as
their cards give them ([Standards and references](standards.md)).

## Licence

| Part | Licence |
|---|---|
| the code: the engine, the Studio, the Python package and the tools | **Apache License 2.0** ([`LICENSE`](https://github.com/ak3zaidan/V2X-World/blob/main/LICENSE); `license = "Apache-2.0"` in every crate) |
| the [V2X-SCMS-MBD dataset](https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini) | **Creative Commons Attribution 4.0 International** ([CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)) |

The code licence has a consequence for plug-ins, which the model registry enforces:
Apache-2.0, MIT, BSD and MPL-2.0 code may be linked into the engine; GPL, LGPL and unknown
licences are refused in process ([Plugins](../dev/extending/plugins.md#registration-and-licences)).

## Third-party components in the repository

| Path | What | Source | Licence |
|---|---|---|---|
| `third_party/asn1/etsi/cdd_ts102894_2` | ETSI TS 102 894-2 Common Data Dictionary (Release 2) | ETSI ITS ASN.1 forge | BSD-3-Clause, © 2019 ETSI (`LICENSE.txt` beside it) |
| `third_party/asn1/etsi/cam_ts103900` | CAM, ETSI TS 103 900 (Release 2) | ETSI forge | BSD-3-Clause, © 2024 ETSI |
| `third_party/asn1/etsi/denm_ts103831` | DENM, ETSI TS 103 831 (Release 2) | ETSI forge | BSD-3-Clause, © 2022 ETSI |
| `third_party/asn1/etsi/cpm_ts103324` | CPM, ETSI TS 103 324 V2.1.1 | ETSI forge | BSD-3-Clause, © 2019 ETSI |
| `third_party/asn1/etsi/vam_ts103300_3` | VAM, ETSI TS 103 300-3 | ETSI forge | BSD-3-Clause, © 2020 ETSI |
| `third_party/asn1/etsi/sec_ts103097` | ETSI TS 103 097 modules | ETSI forge | BSD-3-Clause, © 2020 ETSI |
| `third_party/asn1/etsi/ieee1609.2`, `ieee1609.2-2022`, `ieee1609.2.1` | IEEE 1609.2 and 1609.2.1-2022 ASN.1 modules | ETSI forge mirror | module text IEEE copyright, republished by ETSI; no separate licence file was captured (see `PROVENANCE.md`) |
| `third_party/asn1/etsi/normalized-utf8/` | byte-exact CP1252-to-UTF-8 transcodes of three of the modules above, which the build reads | derived | as their originals |
| `third_party/asn1/ieee1609.3` | IEEE 1609.3 WSA, WEE, WSM modules and ISO 17419 identifiers | Certification Operating Council TCI ASN.1 repository, pinned commit | none stated; module text IEEE copyright (see `PROVENANCE.md`) |
| `tools/oracle/asn1/` | byte-identical copies of the IEEE 1609.2-2022, 1609.2.1-2022 and 1609.3 modules for the independent oracle | as above | as above |
| `worlds/sumo/lust-veremi-sub.net.xml` | a cut of the LuST scenario network | LuST repository, pinned commit (`worlds/fetch-lust.sh`) | MIT, © 2016 Lara Codecá, VehicularLab |
| `tests/fixtures/midtown-6block.osm.xml` | a small Manhattan extract for the determinism gate | OpenStreetMap | ODbL 1.0, © OpenStreetMap contributors |
| `worlds/dem/portland-downtown.asc` | a 1 arc-second elevation cut | USGS 3DEP / SRTM via Joerd tiles | public domain (3DEP); attribution in `portland-downtown.md` |

Each `PROVENANCE.md` pins the upstream commit or tag, the fetch date and every file's
sha256. The build reads the files as committed; none may be edited.

!!! warning "Not in the repository"
    - **SAE J2735 ASN.1**: its licence forbids redistribution; the engine's J2735 codecs
      are hand-written. `third_party/asn1/j2735/` is git-ignored.
    - **SCMS Manager CTL, chain and elector files**: "All Rights Reserved" with no
      licence, so `tools/scms-artefacts/fetch.py` downloads them into the git-ignored
      `third_party/scms-artefacts/files/` and checks their published fingerprints.
    - **Licensed test vectors** (IEEE 1609.2a, CAMP, NIST CAVP, RFC 4231): only the values
      needed for interoperability testing are transcribed into
      `crates/v2xw-sec/tests/vectors`, each with its citation and the source's sha256.

## Data the engine reads but does not ship

| Data | Fetched by | Licence | Committed |
|---|---|---|---|
| OpenStreetMap city extracts | `worlds/fetch.sh` (Overpass API) | ODbL 1.0, "© OpenStreetMap contributors"; a world built from one carries the attribution | no (`worlds/cache/`, git-ignored: large and share-alike) |
| GTFS transit feeds (for example TriMet) | `worlds/fetch-gtfs.sh` | the operator's terms (TriMet asks for attribution rather than redistribution) | no (`worlds/cache/<name>.gtfs/`) |
| SRTM 1 arc-second DEM | downloaded by the user, read by `v2xw_world::dem` | NASA EOSDIS open data (`NASA-EOSDIS-open`); the requested citation is recorded with the world | no |
| Copernicus GLO-30 DEM | downloaded by the user, read by `v2xw_world::dem` | Copernicus DEM licence; the importer records the "produced using Copernicus WorldDEM-30 ..." attribution for resampled data | no |
| US connected-vehicle pilot BSM samples (Tampa, Wyoming, NYC) and UDOT/Panasonic Orem BSMs | `tools/realdata/rdtool.py fetch` (data.transportation.gov SODA) | CC BY-SA 4.0 (Tampa, Wyoming), NYC terms of use with CC BY 4.0, none stated (Orem) | no: raw rows stay outside the repository; only derived statistics (`data/calibration/`) are committed |
| NYC DOT Automated Traffic Volume Counts (NYC Open Data `7ym2-wayt`) | `tools/calibration/nyc-dot-hourly-profile.py` | the NYC Open Data terms | no: only the hourly factors in `crates/v2xw-mobility/src/demand/daily.rs` |
| Natural Earth admin-0; US Census cartographic boundaries | `tools/regions/build.py` | public domain | only the derived `crates/v2xw-node/data/regions.bin` |
