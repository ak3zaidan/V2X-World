#!/usr/bin/env python3
"""Build crates/v2xw-node/data/regions.bin (SC-P11.1).

The receiver checks an IEEE 1609.2 `identifiedRegion` (countryOnly, countryAndRegions)
against a signed `generationLocation`. This builder turns two public-domain sources into
the integer rings and the grid that check reads:

- Natural Earth admin-0 countries, 1:50m (ne_50m_admin_0_countries, v5.1.1), field
  ISO_N3: the countries 124 (Canada) and 484 (Mexico);
- US Census cartographic boundary states, 1:20m (cb_2020_us_state_20m), field STATEFP:
  the 2010 FIPS state codes 1609.2 names for countryAndRegions(840, ...).

Country 840 is not a polygon of its own: regions.rs reads countryOnly(840) as the union of
the 50 states and DC (FIPS 72, Puerto Rico, is ISO 3166-1 630, not 840). Natural Earth's
1:50m United States puts parts of Manhattan, Miami, San Francisco, Honolulu, Bridgeport
and Portland ME in the sea (measured with --check), so a countryOnly(840) certificate
would be refused on those streets; the Census 1:20m states hold every one of them, and a
countryAndRegions(840, [s]) acceptance then always implies a countryOnly(840) one.
Run it with `python3 -I` on the unpacked downloads, kept in their own directory:

    python3 -I tools/regions/build.py <ne_50m_admin_0_countries.shp> \
        <cb_2020_us_state_20m.shp> crates/v2xw-node/data/regions.bin \
        [--check worlds/cities.yaml]

It uses only the standard library (a minimal .shp/.dbf reader) and is deterministic: the
same inputs give the same bytes.

The sources' vertices are kept: no simplification. Coordinates are rounded to integer
microdegrees and repeated points dropped, nothing else. The 10-completion-specs SC-P11
design simplified to about 2,000 vertices per country and 300 per state to bound the
point-in-polygon cost; regions.rs instead indexes each ring edge by grid row, so a
lookup in a border cell reads only the edges of its 0.25 degree band and the vertex
count no longer sets the cost. Simplifying to that budget would move Canada's border by
up to 12 km (Douglas-Peucker tolerance 0.107 degree, measured), more than Detroit to
Windsor. Run with --check worlds/cities.yaml to verify that every city box's corners lie
in country 840 and in the city's own state.

File layout (little-endian), read by crates/v2xw-node/src/regions.rs:

    b"V2XRGN01"          magic
    u32 nregions
    per region:
      u8  kind           0 = country (ISO 3166-1 numeric), 1 = US state (2010 FIPS)
      u16 code
      i32 row0, col0     first grid cell (cell = floor((deg + 90|180) / 0.25))
      u16 rows, cols
      u32 nruns          grid, row-major, run-length encoded
      nruns x (u16 len, u8 class)   class 0 outside, 1 inside, 2 border
      u32 nrings
      per ring: u32 npts, npts x (i32 lat_udeg, i32 lon_udeg)   (not closed: last != first)

A cell is "border" when any ring edge meets the closed cell rectangle (exact integer
test); every other cell takes the crossing-number verdict of its centre, which holds for
every point in it because no edge enters it.
"""

import re
import struct
import sys

COUNTRIES = (124, 484)
NOT_840 = (72,)  # FIPS codes in the Census file that are not ISO 3166-1 840
CELL = 250_000  # 0.25 degree in microdegrees
OUT, IN, BORDER = 0, 1, 2


def read_dbf(path):
    with open(path, "rb") as f:
        data = f.read()
    nrec = struct.unpack_from("<I", data, 4)[0]
    hlen, rlen = struct.unpack_from("<HH", data, 8)
    fields = []
    off = 32
    while data[off] != 0x0D:
        name = data[off:off + 11].split(b"\0", 1)[0].decode("ascii")
        flen = data[off + 16]
        fields.append((name, flen))
        off += 32
    rows = []
    for i in range(nrec):
        base = hlen + i * rlen + 1
        row = {}
        for name, flen in fields:
            row[name] = data[base:base + flen].decode("utf-8", "replace").strip()
            base += flen
        rows.append(row)
    return rows


def read_shp(path):
    """Each record's rings, as lists of (x, y) degree tuples."""
    with open(path, "rb") as f:
        data = f.read()
    off = 100
    shapes = []
    while off < len(data):
        _, clen = struct.unpack_from(">ii", data, off)
        off += 8
        rec = data[off:off + clen * 2]
        off += clen * 2
        stype = struct.unpack_from("<i", rec, 0)[0]
        if stype == 0:
            shapes.append([])
            continue
        if stype != 5:
            raise SystemExit(f"shape type {stype} is not a polygon")
        nparts, npts = struct.unpack_from("<ii", rec, 36)
        parts = list(struct.unpack_from(f"<{nparts}i", rec, 44)) + [npts]
        pbase = 44 + 4 * nparts
        pts = [struct.unpack_from("<dd", rec, pbase + 16 * k) for k in range(npts)]
        shapes.append([pts[parts[j]:parts[j + 1]] for j in range(nparts)])
    return shapes


def to_udeg(ring):
    """(lat, lon) integer microdegrees, closing point and repeats removed."""
    out = []
    for x, y in ring:
        p = (round(y * 1e6), round(x * 1e6))
        if not out or out[-1] != p:
            out.append(p)
    if len(out) > 1 and out[0] == out[-1]:
        out.pop()
    return out


def seg_meets_rect(p, q, y0, x0, y1, x1):
    """Exact integer test: does segment p-q meet the closed rectangle [y0,y1] x [x0,x1]?"""
    (py, px), (qy, qx) = p, q
    if max(py, qy) < y0 or min(py, qy) > y1 or max(px, qx) < x0 or min(px, qx) > x1:
        return False
    if y0 <= py <= y1 and x0 <= px <= x1:
        return True
    dy, dx = qy - py, qx - px
    sides = set()
    for cy, cx in ((y0, x0), (y0, x1), (y1, x0), (y1, x1)):
        c = dx * (cy - py) - dy * (cx - px)
        sides.add((c > 0) - (c < 0))
    # Bounding boxes overlap; the segment meets the rectangle unless every corner lies
    # strictly on one side of its line.
    return not (sides == {1} or sides == {-1})


def inside(rings, lat, lon):
    """Crossing number (even-odd) in integer arithmetic; same rule as regions.rs."""
    c = False
    for r in rings:
        n = len(r)
        j = n - 1
        for i in range(n):
            (yi, xi), (yj, xj) = r[i], r[j]
            if (yi > lat) != (yj > lat):
                # lon < xi + (lat - yi) * (xj - xi) / (yj - yi), sign-safe
                num = (lat - yi) * (xj - xi)
                den = yj - yi
                lhs = (lon - xi) * den
                if (lhs < num) if den > 0 else (lhs > num):
                    c = not c
            j = i
    return c


def fdiv(a, b):
    return a // b


def grid(rings):
    lats = [p[0] for r in rings for p in r]
    lons = [p[1] for r in rings for p in r]
    row0 = fdiv(min(lats) + 90_000_000, CELL)
    col0 = fdiv(min(lons) + 180_000_000, CELL)
    rows = fdiv(max(lats) + 90_000_000, CELL) - row0 + 1
    cols = fdiv(max(lons) + 180_000_000, CELL) - col0 + 1
    g = [None] * (rows * cols)

    def rect(rw, cl):
        y0 = (row0 + rw) * CELL - 90_000_000
        x0 = (col0 + cl) * CELL - 180_000_000
        return y0, x0, y0 + CELL, x0 + CELL

    for r in rings:
        n = len(r)
        for i in range(n):
            p, q = r[i], r[(i + 1) % n]
            ra = fdiv(min(p[0], q[0]) + 90_000_000, CELL) - row0
            rb = fdiv(max(p[0], q[0]) + 90_000_000, CELL) - row0
            ca = fdiv(min(p[1], q[1]) + 180_000_000, CELL) - col0
            cb = fdiv(max(p[1], q[1]) + 180_000_000, CELL) - col0
            # A point on a cell line also touches the neighbour's closed rectangle.
            for rw in range(max(ra - 1, 0), min(rb + 1, rows - 1) + 1):
                for cl in range(max(ca - 1, 0), min(cb + 1, cols - 1) + 1):
                    k = rw * cols + cl
                    if g[k] is None and seg_meets_rect(p, q, *rect(rw, cl)):
                        g[k] = BORDER
    for rw in range(rows):
        last = None  # verdict carried along a run of non-border cells
        for cl in range(cols):
            k = rw * cols + cl
            if g[k] == BORDER:
                last = None
                continue
            if last is None:
                y0, x0, _, _ = rect(rw, cl)
                last = IN if inside(rings, y0 + CELL // 2, x0 + CELL // 2) else OUT
            g[k] = last
    return row0, col0, rows, cols, g


def rle(g):
    runs = []
    for v in g:
        if runs and runs[-1][1] == v and runs[-1][0] < 0xFFFF:
            runs[-1][0] += 1
        else:
            runs.append([1, v])
    return runs


def region_bytes(kind, code, rings):
    row0, col0, rows, cols, g = grid(rings)
    runs = rle(g)
    b = bytearray(struct.pack("<BHiiHH", kind, code, row0, col0, rows, cols))
    b += struct.pack("<I", len(runs))
    for n, v in runs:
        b += struct.pack("<HB", n, v)
    b += struct.pack("<I", len(rings))
    for r in rings:
        b += struct.pack("<I", len(r))
        for lat, lon in r:
            b += struct.pack("<ii", lat, lon)
    border = sum(1 for v in g if v == BORDER)
    return bytes(b), (rows, cols, border, len(runs))


def check_cities(path, regions, postal):
    """Every corner and the centre of each US city box lies in 840 and in its state."""
    text = open(path, encoding="utf-8").read()
    bad = 0
    n = 0
    for m in re.finditer(r"- id: (\S+).*?\n\s+state: (\w\w)\n.*?bbox: \[([^\]]+)\]",
                         text, re.S):
        cid, st = m.group(1), m.group(2)
        s, w, nn, e = (round(float(v) * 1e6) for v in m.group(3).split(","))
        fips = postal.get(st)
        if fips is None:
            continue  # not a US city
        for lat, lon in ((s, w), (s, e), (nn, w), (nn, e), ((s + nn) // 2, (w + e) // 2)):
            n += 1
            us = any(inside(r, lat, lon) for (k, c), r in regions.items()
                     if k == 1 and c not in NOT_840)
            if not us:
                bad += 1
                print(f"CHECK FAIL {cid} ({st}) {lat} {lon} not in 840")
            for kind, code in ((1, fips),):
                if not inside(regions[(kind, code)], lat, lon):
                    bad += 1
                    print(f"CHECK FAIL {cid} ({st}) {lat} {lon} not in {kind}/{code}")
    print(f"checked {n} city points: {bad} failures")
    return bad


def main():
    args = sys.argv[1:]
    check = None
    if len(args) == 5 and args[3] == "--check":
        check = args[4]
        args = args[:3]
    if len(args) != 3:
        raise SystemExit(__doc__)
    ne_shp, cb_shp, out = args
    regions = []
    ne_rows = read_dbf(ne_shp[:-4] + ".dbf")
    ne_shapes = read_shp(ne_shp)
    for code in COUNTRIES:
        rings = [to_udeg(r) for row, s in zip(ne_rows, ne_shapes)
                 if row.get("ISO_N3") == str(code) for r in s]
        if not rings:
            raise SystemExit(f"country {code} not found")
        regions.append((0, code, rings))
    cb_rows = read_dbf(cb_shp[:-4] + ".dbf")
    cb_shapes = read_shp(cb_shp)
    postal = {r["STUSPS"]: int(r["STATEFP"]) for r in cb_rows}
    for code in sorted({int(r["STATEFP"]) for r in cb_rows}):
        rings = [to_udeg(r) for row, s in zip(cb_rows, cb_shapes)
                 if int(row["STATEFP"]) == code for r in s]
        regions.append((1, code, rings))
    blob = bytearray(b"V2XRGN01")
    blob += struct.pack("<I", len(regions))
    for kind, code, rings in regions:
        rings = [r for r in rings if len(r) >= 3]
        b, (rows, cols, border, nruns) = region_bytes(kind, code, rings)
        blob += b
        print(f"{'country' if kind == 0 else 'state'} {code}: "
              f"{sum(len(r) for r in rings)} vertices in {len(rings)} rings, "
              f"grid {rows}x{cols}, {border} border cells, {nruns} runs")
    with open(out, "wb") as f:
        f.write(blob)
    print(f"wrote {out}: {len(blob)} bytes")
    if check and check_cities(check, {(k, c): r for k, c, r in regions}, postal):
        raise SystemExit("city check failed")


if __name__ == "__main__":
    main()
