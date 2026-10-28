#!/bin/bash
# worlds/fetch-lust.sh — rebuild worlds/sumo/lust-veremi-sub.net.xml, the LuST subsection of the
# VeReMi Extension replica (WP-24 P24.1), from the LuST repository at a pinned commit.
#
#   worlds/fetch-lust.sh [netconvert]      # default: netconvert on PATH (SUMO 1.28.0 made the file)
#
# LuST is MIT (LICENSE.md at the pinned commit), so the cut is committed; this script exists so
# the cut can be checked. It downloads scenario/lust.net.xml (11 MB) into a new temporary
# directory, checks its sha256, lists the edges fully inside the box (tools/realworld/lust_cut.py),
# keeps them with netconvert, and writes the fixed provenance header. The output's sha256 is
# printed; with netconvert 1.28.0 it matches worlds/sumo/lust-veremi-sub.PROVENANCE.md.
set -euo pipefail

nc=${1:-netconvert}
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"
commit=5edb7ecb9ad196c39172b6eb95d19ed789f4b1a6
want=6f5d76223cf14b797ae6267f13b23eb6c872d76adec1fb22a8569a806dc09341
box=6035.21,4144.26,7378.53,5480.06
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
base="https://raw.githubusercontent.com/lcodeca/LuSTScenario/$commit"
curl -sfL -o "$tmp/lust.net.xml" "$base/scenario/lust.net.xml"
curl -sfL -o "$tmp/LICENSE.md" "$base/LICENSE.md"
got=$(shasum -a 256 "$tmp/lust.net.xml" | cut -d' ' -f1)
[ "$got" = "$want" ] || { echo "lust.net.xml sha256 $got, expected $want" >&2; exit 1; }
python3 -I "$repo/tools/realworld/lust_cut.py" edges "$tmp/lust.net.xml" "$box" "$tmp/edges.txt"
(cd "$tmp" && "$nc" -s lust.net.xml --keep-edges.input-file edges.txt --offset.disable-normalization true \
  --output.street-names true -o cut.net.xml > netconvert.log 2>&1)
python3 -I "$repo/tools/realworld/lust_cut.py" stamp "$tmp/cut.net.xml" "$tmp/LICENSE.md" \
  "$repo/worlds/sumo/lust-veremi-sub.net.xml"
shasum -a 256 "$repo/worlds/sumo/lust-veremi-sub.net.xml"
