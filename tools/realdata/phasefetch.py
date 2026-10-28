"""Fetch the two side queries the phase-loss / clock analysis needs (phaseloss.py).

  python3 -I tools/realdata/phasefetch.py <data dir>

The pinned pipeline queries (sources.py) do not select Tampa's metadata_rsuid or Wyoming's
log-file / serial / receivedAt metadata, and changing them would move the frozen manifest.
These two queries are separate files (tampa_rsu.csv, wyoming_meta.csv) with their own
sha256 printed; raw rows stay outside the repository, as for the pinned queries.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import soda  # noqa: E402
from sources import TAMPA, WYOMING  # noqa: E402

QUERIES = [
    {"name": "tampa_rsu", "dataset": TAMPA, "file": "tampa_rsu.csv",
     "select": ["metadata_rsuid", "metadata_recordgeneratedat", "coredata_id", "coredata_secmark",
                "coredata_msgcnt", "coredata_speed"],
     "order": ["coredata_id", "metadata_recordgeneratedat", ":id"]},
    {"name": "wyoming_meta", "dataset": WYOMING, "file": "wyoming_meta.csv",
     "select": ["metadata_logfilename", "metadata_recordtype", "metadata_generatedat",
                "metadata_receivedat", "metadata_serialid_bundleid", "metadata_serialid_recordid",
                "coredata_id", "coredata_secmark", "coredata_msgcnt", "coredata_speed"],
     "order": ["coredata_id", "metadata_generatedat", ":id"]},
]


def main():
    out = sys.argv[1]
    for q in QUERIES:
        p = os.path.join(out, q["file"])
        rows, star = soda.fetch(q, p)
        print(q["name"], rows, star, soda.sha256(p))


if __name__ == "__main__":
    main()
