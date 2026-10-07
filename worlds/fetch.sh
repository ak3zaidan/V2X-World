#!/bin/bash
# worlds/fetch.sh — download a city's OpenStreetMap extract for the importer.
#
#   worlds/fetch.sh <id>                # a city in worlds/cities.yaml (e.g. houston, boston)
#   worlds/fetch.sh <id> <id>...        # several, one after another
#   worlds/fetch.sh --all               # every city in the library whose extract is missing
#   worlds/fetch.sh --all --force       # every city, re-downloading the ones already here
#   worlds/fetch.sh --list              # the ids, with whether each extract is present
#   worlds/fetch.sh <name> S,W,N,E      # any box: south, west, north, east in degrees
#
# Writes worlds/cache/<id>.osm.xml (git-ignored: the data is ODbL and large). The query
# asks the Overpass API for every node and way in the box, the turn restrictions, and
# the building multipolygons, with every node they reference — what the importer reads.
# It deliberately leaves out the other relations (bus routes, administrative boundaries,
# large land-use areas), whose members reach far outside the box: with them, the Berlin
# box was 171 MB instead of 11 MB and the Portland box 32 MB instead of 7 MB. It asks for
# `out body`, not `out meta`: the importer reads no element metadata (version, timestamp,
# user), and on 2026-10-07 the metadata lookup was what made a loaded server time out (504
# from every mirror for `out meta`, the same box in 9 s with `out body`).
#
# The boxes are the registry's (worlds/cities.yaml), so re-fetching cannot move the world
# frame (a city's box fixes the origin). The data changes as OSM is edited: the date of a
# fetch is the registry's `fetched`, which becomes the world's `imported_at`, and
# `world_report --baseline` says what changed.
#
# Politeness: the public Overpass servers are shared and rate-limited. Downloads run one at
# a time with a pause between them (FETCH_PAUSE_S, default 15 s); a refused or failed
# request is retried with exponential backoff, and after the main server fails the next
# mirror is tried (OVERPASS_URL puts your own first). Each download is checked to be an
# OSM document that the server did not cut short (a timeout or out-of-memory answer is a
# well-formed document with a <remark>), and only then moved into place.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
registry="$here/cities.yaml"
cache=${V2XW_WORLDS_CACHE:-$here/cache}
pause=${FETCH_PAUSE_S:-15}
endpoints=(${OVERPASS_URL:-} https://overpass-api.de/api/interpreter
  https://overpass.private.coffee/api/interpreter
  https://overpass.kumi.systems/api/interpreter
  https://maps.mail.ru/osm/tools/overpass/api/interpreter)

usage() { sed -n '3,9p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; }

# The registry's box for an id, as S,W,N,E; empty when the id is not there.
box_of() {
  awk -v id="$1" '
    $1 == "-" && $2 == "id:" { cur = $3 }
    cur == id && $1 == "bbox:" { gsub(/[][ ]/, "", $0); sub(/^bbox:/, "", $0); print; exit }
  ' "$registry"
}

ids() { awk '$1 == "-" && $2 == "id:" { print $3 }' "$registry"; }

fetch_box() {
  local name=$1 box=$2 out="$cache/$1.osm.xml"
  local query="[out:xml][timeout:180];(node($box);way($box);rel($box)[\"type\"=\"restriction\"];rel($box)[\"type\"=\"multipolygon\"][\"building\"];rel($box)[\"type\"=\"multipolygon\"][\"building:part\"];);(._;>;);out body;"
  mkdir -p "$cache"
  local attempt=0 wait=20
  for endpoint in "${endpoints[@]}" "${endpoints[@]}"; do
    attempt=$((attempt + 1))
    if curl -sS --fail --max-time 300 -A "v2xw-world-importer (research traffic simulator)" \
        -H "Accept: */*" --data-urlencode "data=$query" -o "$out.part" "$endpoint"; then
      if head -c 300 "$out.part" | grep -q "<osm" && ! tail -c 2000 "$out.part" | grep -q "<remark"; then
        mv "$out.part" "$out"
        echo "$out: $(wc -c < "$out" | tr -d ' ') bytes, box $box, from $endpoint"
        return 0
      fi
      echo "$name: $endpoint answered with no usable extract ($(tail -c 300 "$out.part" | tr -d '\n' | cut -c1-200))" >&2
    else
      echo "$name: $endpoint failed (attempt $attempt)" >&2
    fi
    rm -f "$out.part"
    echo "$name: waiting ${wait} s before the next try" >&2
    sleep "$wait"
    wait=$((wait * 2 > 240 ? 240 : wait * 2))
  done
  echo "$name: every endpoint failed; try again later or set OVERPASS_URL" >&2
  return 1
}

fetch_id() {
  local box
  box=$(box_of "$1")
  [ -n "$box" ] || { echo "no city '$1' in $registry; worlds/fetch.sh --list shows them, or give a box as south,west,north,east" >&2; return 2; }
  fetch_box "$1" "$box"
}

[ $# -ge 1 ] || usage
case "$1" in
  -h|--help) usage ;;
  --list)
    for id in $(ids); do
      f="$cache/$id.osm.xml"
      if [ -s "$f" ]; then printf '%-18s %s  (%s bytes)\n' "$id" "$(box_of "$id")" "$(wc -c < "$f" | tr -d ' ')"
      else printf '%-18s %s  missing\n' "$id" "$(box_of "$id")"; fi
    done ;;
  --all)
    force=${2:-}
    failed=()
    first=1
    for id in $(ids); do
      if [ "$force" != "--force" ] && [ -s "$cache/$id.osm.xml" ]; then continue; fi
      [ $first -eq 1 ] || sleep "$pause"
      first=0
      fetch_id "$id" || failed+=("$id")
    done
    if [ ${#failed[@]} -gt 0 ]; then echo "not fetched: ${failed[*]}" >&2; exit 1; fi ;;
  *)
    if [ $# -eq 2 ] && [[ "$2" == *,*,*,* ]]; then
      fetch_box "$1" "$2"
    else
      first=1
      for id in "$@"; do
        [ $first -eq 1 ] || sleep "$pause"
        first=0
        fetch_id "$id"
      done
    fi ;;
esac
