"""The LuST subsection of the VeReMi Extension replica (WP-24 P24.1): the edge list of a box
cut, and the fixed header that makes netconvert's output byte-reproducible.

  python3 -I tools/realworld/lust_cut.py edges <lust.net.xml> <x1,y1,x2,y2> <edges.txt>
  python3 -I tools/realworld/lust_cut.py stamp <netconvert out> <LICENSE.md> <out.net.xml>
  python3 -I tools/realworld/lust_cut.py measure <net.xml>

`edges` keeps a non-internal edge only when its from and to junctions and every point of
every lane shape lie inside the box ("fully within"). netconvert's own
`--keep-edges.in-boundary` keeps an edge that is only partly inside, which on LuST pulls in
30 long boundary edges and doubles the extent (3.2 km² of junctions against the published
"approximately 1.61 km²"); the fully-within rule gives 1.57 km² (provenance file).
`stamp` replaces netconvert's header comment, which carries the run time and local paths,
with a fixed provenance comment and the MIT licence text the LuST licence requires in copies.
"""
import sys
import xml.etree.ElementTree as ET

SOURCE_COMMIT = "5edb7ecb9ad196c39172b6eb95d19ed789f4b1a6"
SOURCE_SHA256 = "6f5d76223cf14b797ae6267f13b23eb6c872d76adec1fb22a8569a806dc09341"
BOX = (6035.21, 4144.26, 7378.53, 5480.06)


def fully_within(path, box):
    x1, y1, x2, y2 = box
    inside = lambda x, y: x1 <= x <= x2 and y1 <= y <= y2  # noqa: E731
    junctions, edges = {}, []
    for _, el in ET.iterparse(path):
        if el.tag == "junction":
            junctions[el.get("id")] = (float(el.get("x")), float(el.get("y")))
            el.clear()
        elif el.tag == "edge":
            if el.get("function") != "internal":
                shape = [tuple(map(float, p.split(","))) for ln in el.findall("lane")
                         for p in ln.get("shape").split()]
                edges.append((el.get("id"), el.get("from"), el.get("to"), shape))
            el.clear()
    return sorted(e for e, f, t, shape in edges
                  if inside(*junctions[f]) and inside(*junctions[t]) and all(inside(x, y) for x, y in shape))


def header(box):
    return ("A subsection of the Luxembourg SUMO Traffic (LuST) Scenario network, cut for the VeReMi\n"
            "Extension replica (WP-24 P24.1). Provenance: worlds/sumo/lust-veremi-sub.PROVENANCE.md.\n\n"
            "Source: https://github.com/lcodeca/LuSTScenario at %s\n"
            "(2023-10-23), scenario/lust.net.xml (sha256 %s).\n"
            "Cut: the non-internal edges whose end junctions and every lane-shape point lie inside the box\n"
            "x %.2f..%.2f, y %.2f..%.2f m (LuST network coordinates), kept with\n"
            "netconvert 1.28.0 with the options keep-edges.input-file, offset.disable-normalization true\n"
            "and output.street-names true. Coordinates are LuST's (netOffset unchanged).\n"
            "The LuST topology was built from OpenStreetMap data (the repository ships it as\n"
            "misc/lust.legacy.scenario.osm): (c) OpenStreetMap contributors, ODbL 1.0, kept as a\n"
            "precaution beside the MIT licence below (provenance file).\n\n"
            % (SOURCE_COMMIT, SOURCE_SHA256, box[0], box[2], box[1], box[3]))


def stamp(src, lic, out, box=BOX):
    text = open(src, encoding="utf-8").read()
    head = '<?xml version="1.0" encoding="UTF-8"?>\n'
    if not text.startswith(head):
        raise SystemExit("not a netconvert output: " + src)
    a = text.index("<!--")
    b = text.index("-->", a) + 3
    body = text[b:].lstrip("\n")
    lic_text = open(lic, encoding="utf-8").read().strip()
    if "MIT License" not in lic_text:
        raise SystemExit("the licence is not the MIT text: " + lic)
    if "--" in header(box) + lic_text:
        raise SystemExit("an XML comment cannot hold a double hyphen")
    with open(out, "w", encoding="utf-8") as f:
        f.write(head + "\n<!--\n" + header(box) + lic_text + "\n-->\n\n" + body)


def measure(path):
    js, edges, lane_m, tls = [], 0, 0.0, 0
    for _, el in ET.iterparse(path):
        if el.tag == "junction":
            if el.get("type") != "internal":
                js.append((float(el.get("x")), float(el.get("y"))))
                tls += el.get("type", "").startswith("traffic_light")
            el.clear()
        elif el.tag == "edge":
            if el.get("function") != "internal":
                edges += 1
                lane_m += sum(float(ln.get("length")) for ln in el.findall("lane"))
            el.clear()
    xs, ys = [p[0] for p in js], [p[1] for p in js]
    return {"junctions": len(js), "signalised": tls, "edges": edges, "lane_km": round(lane_m / 1000, 2),
            "junction_extent_km2": round((max(xs) - min(xs)) * (max(ys) - min(ys)) / 1e6, 3)}


def main(argv):
    if argv[:1] == ["edges"]:
        keep = fully_within(argv[1], [float(v) for v in argv[2].split(",")])
        open(argv[3], "w").write("\n".join(keep) + "\n")
        print("kept", len(keep), "edges")
    elif argv[:1] == ["stamp"]:
        stamp(argv[1], argv[2], argv[3])
    elif argv[:1] == ["measure"]:
        print(measure(argv[1]))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
