"""The committed sha256 manifest of the raw rows (the rows themselves are never committed)."""
import json
import os

import soda
import sources
from version import VERSION

MANIFEST = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "data", "calibration",
                        "cvpilot-raw-manifest.json")


def load():
    with open(MANIFEST) as f:
        return json.load(f)


def entry(q, raw_dir, rows, star):
    path = os.path.join(raw_dir, q["file"])
    return {"name": q["name"], "dataset": q["dataset"], "file": q["file"],
            "query": soda.url(q["dataset"], soda.query_params(q)),
            "rows": rows, "count_star": star, "bytes": os.path.getsize(path), "sha256": soda.sha256(path)}


def write(entries):
    doc = {"schema": "v2xw.realdata_manifest.v1", "tool": "tools/realdata " + VERSION,
           "note": "Raw rows are not in the repository (CC BY-SA / unstated licences). "
                   "Rebuild with: python3 tools/realdata/rdtool.py fetch --raw <dir>",
           "files": sorted(entries, key=lambda e: e["name"])}
    os.makedirs(os.path.dirname(MANIFEST), exist_ok=True)
    with open(MANIFEST, "w") as f:
        json.dump(doc, f, indent=1, sort_keys=True)
        f.write("\n")


def verify(raw_dir, names=None):
    """Check every raw file against the committed manifest. Returns a list of problems."""
    bad = []
    for e in load()["files"]:
        if names and e["name"] not in names:
            continue
        p = os.path.join(raw_dir, e["file"])
        if not os.path.exists(p):
            bad.append(e["file"] + ": missing")
        elif soda.sha256(p) != e["sha256"]:
            bad.append(e["file"] + ": sha256 differs from the manifest")
    return bad


def manifest_sha256():
    return soda.sha256(MANIFEST)


__all__ = ["load", "write", "verify", "entry", "manifest_sha256", "sources"]
