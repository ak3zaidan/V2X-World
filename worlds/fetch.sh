#!/bin/bash
# worlds/fetch.sh — download a city's OpenStreetMap extract for the importer.
#
#   worlds/fetch.sh portland            # a city this repository has a scenario for
#   worlds/fetch.sh <name> S,W,N,E      # any box: south, west, north, east in degrees
#
# Writes worlds/cache/<name>.osm.xml (git-ignored: the data is ODbL and large). The query
# asks the Overpass API for every node and way in the box, the turn restrictions, and
# the building multipolygons, with every node they reference — what the importer reads.
# It deliberately leaves out the other relations (bus routes, administrative boundaries,
# large land-use areas), whose members reach far outside the box: with them, the Berlin
# box below was 171 MB instead of 11 MB and the Portland box 32 MB instead of 7 MB.
#
# The boxes are the ones the scenarios name, so re-fetching cannot move the world frame
# (a scenario's bbox fixes the origin). The data changes as OSM is edited: the date of a
# fetch belongs in the scenario's `world.imported_at`, and `world_report --baseline`
# says what changed.
set -euo pipefail

name=${1:?usage: worlds/fetch.sh <name> [south,west,north,east]}
box=${2:-}
if [ -z "$box" ]; then
  case "$name" in
    manhattan) box=40.7440,-73.9900,40.7620,-73.9680 ;;  # Midtown, 44th-60th, 8th Ave-Park (D7)
    portland)  box=45.5120,-122.6860,45.5250,-122.6700 ;; # downtown Portland, Oregon
    berlin)    box=52.5140,13.3880,52.5250,13.4080 ;;     # Berlin-Mitte, Friedrichstrasse-Museumsinsel
    *) echo "no box known for '$name'; give one as south,west,north,east" >&2; exit 2 ;;
  esac
fi
endpoint=${OVERPASS_URL:-https://overpass-api.de/api/interpreter}
out="$(cd "$(dirname "$0")" && pwd)/cache/$name.osm.xml"
query="[out:xml][timeout:180];(node($box);way($box);rel($box)[\"type\"=\"restriction\"];rel($box)[\"type\"=\"multipolygon\"][\"building\"];rel($box)[\"type\"=\"multipolygon\"][\"building:part\"];);(._;>;);out meta;"
curl -sS --fail --max-time 300 -A "v2xw-world-importer (research traffic simulator)" \
  -H "Accept: */*" --data-urlencode "data=$query" -o "$out.part" "$endpoint"
head -c 200 "$out.part" | grep -q "<osm" || { echo "not an OSM document; see $out.part" >&2; exit 1; }
mv "$out.part" "$out"
echo "$out: $(wc -c < "$out") bytes, box $box"
