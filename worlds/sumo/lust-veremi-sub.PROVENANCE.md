# `lust-veremi-sub.net.xml`: the LuST subsection of the VeReMi Extension replica

WP-24 P24.1, 2026-10-28. Rebuild: `worlds/fetch-lust.sh <netconvert>`. Checks:
`python3 -I tools/realworld/test_lust_cut.py`.

## Source and licence (re-checked 2026-10-28)

- Repository: <https://github.com/lcodeca/LuSTScenario>, branch `master`, commit
  `5edb7ecb9ad196c39172b6eb95d19ed789f4b1a6` (2023-10-23, the last push).
- Licence: **MIT**. Read at the pinned commit:
  - `LICENSE.md` is the MIT text, "Copyright (c) 2016 Lara CODECA, VehicularLab". sha256
    `9a16c868…d64837b`.
  - `README.md` line 5: "This project is licensed under the terms of the MIT license."
  - The GitHub licence API names it `MIT`.
  - `scenario/*.sumocfg` and `lust.net.xml` carry the same MIT line in their headers.
  - The "GPLv3+" in `docs/LuSTFiles.md` is SUMO 0.27's start-up banner in a pasted log. It
    is about SUMO, not LuST.
  - The MIT text asks to be included in copies, so it is in the net's header comment.
- OpenStreetMap: the repository ships the network in OSM form (`misc/lust.legacy.scenario.osm`,
  `docs/LuSTFiles.md` line 113). As a precaution the header also carries
  "(c) OpenStreetMap contributors, ODbL 1.0". [verify] Codeca et al., "LuST: a 24-hour
  scenario of Luxembourg City for SUMO traffic simulations" (SUMO 2015) is where the
  topology's OSM origin would be stated. The paper is not cached.
- Not used: F2MD and veins-f2md have no licence (`docs/design/frameworks/f2md.md`). Their files
  were read only to take numbers (see "Box"). Nothing from them is in this file.
- MoST is GPL-3.0 and was not used (WP-15).

| file | sha256 |
|---|---|
| LuST `scenario/lust.net.xml` (input) | `6f5d76223cf14b797ae6267f13b23eb6c872d76adec1fb22a8569a806dc09341` |
| `worlds/sumo/lust-veremi-sub.net.xml` (output) | `46eea2b0b70676c223077cae3223dc448538fbd3ef8e8615e173dbe72788c066` |

## Box

The VeReMi Extension site (<https://veremi-dataset.github.io/veremi-extension>, read
2026-10-28) says: "a subsection of the LuST network with an area of approximately
1.61 km²". It also gives 07:00–09:00 and 14:00–16:00 as the windows and 30 % attackers in
all simulations. It does not give a box. Neither does the paper's abstract.

F2MD ships two LuST cuts, both made by hand in netedit 1.0.1. Neither has a box.
The cuts are in `josephkamel/veins-f2md` at `340e3cb`, under `f2md-networks/`:

| F2MD cut | junction extent | hull | edges | lane km |
|---|---|---|---|---|
| `LuSTNanoScenario/lust.net.xml` | 1.79 km² | 1.36 km² | 199 | 31.2 |
| `LuSTMiniScenario/lust.net.xml` | 5.0 km² | | | |

Nano is the one of the published size. The box is therefore the **extent of Nano's
junctions**, in LuST network coordinates (the netOffset is the same):
x 6035.21..7378.53, y 4144.26..5480.06 m.

How the edges are kept: an edge stays only when its two end junctions and every
lane-shape point are inside the box (`tools/realworld/lust_cut.py edges`). The
alternatives were measured with SUMO 1.28.0:

| rule | edges | in Nano | not in Nano | lane km | junction extent |
|---|---|---|---|---|---|
| netconvert `--keep-edges.in-boundary` (partly inside) | 229 | 199 / 199 | 30 | 49.2 | 3.20 km² |
| **fully within (chosen)** | **205** | 197 / 199 | 8 | **30.9** | **1.57 km²** |

The chosen rule matches the published 1.61 km² and Nano's road length (31.2 km). The rule
SUMO uses by default doubles both, because 30 long boundary edges reach out of the box.
The two Nano edges it drops (`--31340#0` and `-31340#0`, a lane shape leaving the box) and
the 8 edges it adds sit at the box's edge.

## Contents

- 77 junctions and 205 edges.
- 5 signalised junctions. All 5 `tlLogic` are `type="actuated"`, as in LuST. netconvert
  warns that they "do not define variable phase length". WP-23 imports these programmes
  as they are in the file.
- netconvert 1.28.0 options: `--keep-edges.input-file`, `--offset.disable-normalization true`
  and `--output.street-names true`.
- netconvert's own header comment carries the run time and local paths. It is replaced
  by a fixed comment, so the file is byte-reproducible: two cuts gave the same sha256.

The release binary at `ac00603c` imports it. This was a 20 s smoke run of 5 vehicles under
internal Poisson demand (`world.source.kind: sumo-net`), and it exited 0. The replica
scenario and routes are P24.2.
