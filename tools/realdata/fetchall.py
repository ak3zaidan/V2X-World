"""Stage 1 driver: fetch every pinned query not already on disk, then write the manifest."""
import os
import sys

import manifest
import soda
import sources


def run(raw_dir, only=None, log=print):
    os.makedirs(raw_dir, exist_ok=True)
    old = {}
    if os.path.exists(manifest.MANIFEST):
        old = {e["name"]: e for e in manifest.load()["files"]}
    entries = []
    for q in sources.queries():
        if only and not any(q["name"].startswith(o) for o in only):
            if q["name"] in old:
                entries.append(old[q["name"]])
            continue
        path = os.path.join(raw_dir, q["file"])
        if os.path.exists(path) and q["name"] in old and soda.sha256(path) == old[q["name"]]["sha256"]:
            entries.append(old[q["name"]])
            log(f"{q['name']}: on disk, matches manifest")
            continue
        rows, star = soda.fetch(q, path, limit_rows=q.get("limit_rows"))
        e = manifest.entry(q, raw_dir, rows, star)
        if q["name"] in old and old[q["name"]]["sha256"] != e["sha256"]:
            log(f"WARNING {q['name']}: sha256 changed from the committed manifest (server-side change?)")
        entries.append(e)
        log(f"{q['name']}: {rows} rows (count(*) {star}), {e['bytes']} bytes")
    manifest.write(entries)


if __name__ == "__main__":
    run(sys.argv[1], sys.argv[2:] or None)
