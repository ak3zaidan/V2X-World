#!/bin/bash
# worlds/fetch-gtfs.sh — download a city's GTFS feed and clip it to the city's box and one
# service day, for the `mobility/demand/gtfs` demand kind.
#
#   worlds/fetch-gtfs.sh portland [YYYYMMDD]     # TriMet: buses, MAX, Portland Streetcar
#   worlds/fetch-gtfs.sh <name> <date> <url> S,W,N,E
#
# Writes worlds/cache/<name>.gtfs/ (git-ignored, like the OSM extracts: the feeds are
# large and their licences ask for attribution rather than redistribution — TriMet's
# developer terms, developer.trimet.org/terms_of_use.shtml). The full feed is unpacked in
# a temporary directory, clipped with `v2xw-demand gtfs-clip`, and deleted: a 43 MB
# TriMet zip becomes a downtown weekday of a few hundred kilobytes.
#
# The date must be one the feed covers (feeds are published for the next few months);
# the default is today. A scenario that runs the clipped feed names the same date in
# `params.date`, or has `time.t0` on that date.
set -euo pipefail

name=${1:?usage: worlds/fetch-gtfs.sh <name> [YYYYMMDD] [url S,W,N,E]}
date=${2:-$(date +%Y%m%d)}
url=${3:-}
box=${4:-}
if [ -z "$url" ]; then
  case "$name" in
    # The boxes are worlds/fetch.sh's, so the clipped stops are the world's.
    portland) url=https://developer.trimet.org/schedule/gtfs.zip; box=45.5120,-122.6860,45.5250,-122.6700 ;;
    *) echo "no feed known for '$name'; give a date, a URL and a box" >&2; exit 2 ;;
  esac
fi
here="$(cd "$(dirname "$0")" && pwd)"
out="$here/cache/$name.gtfs"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/zip" "$tmp/feed"
curl -sS --fail --max-time 600 -L -A "v2xw-demand (research traffic simulator)" -o "$tmp/zip/feed.zip" "$url"
unzip -q -o "$tmp/zip/feed.zip" -d "$tmp/feed" \
  stops.txt routes.txt trips.txt stop_times.txt 'calendar*.txt' 'frequencies.txt' 2>/dev/null \
  || unzip -q -o "$tmp/zip/feed.zip" -d "$tmp/feed"
rm -f "$tmp/zip/feed.zip"
${V2XW_DEMAND:-cargo run -q -p v2xw-demand-cli --} gtfs-clip --feed "$tmp/feed" --bbox "$box" --date "$date" --out "$out"
echo "$url" > "$out/SOURCE"
echo "$out: $(du -sh "$out" | cut -f1), service date $date, box $box"
