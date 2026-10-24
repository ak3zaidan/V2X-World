"""Two-sample KS distance, and the tolerance from Wyoming's own between-device spread.

  python3 -I ks.py tol <wyoming.csv>                 # the tolerance, before any engine run
  python3 -I ks.py cmp <a.json> <b.json>             # KS(a, b) of two resid.py --json files
"""
import bisect
import itertools
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import resid  # noqa: E402


def ks(a, b):
    a, b = sorted(a), sorted(b)
    d = 0.0
    for x in a + b:
        fa = bisect.bisect_right(a, x) / len(a)
        fb = bisect.bisect_right(b, x) / len(b)
        d = max(d, abs(fa - fb))
    return d


def tol(path):
    per = {}
    for k, bsms in resid.normalise.stream_ids(path, "wyoming"):
        rs, _ = resid.residuals([(k, bsms)], 10.0)
        rs = [r for r, _, _ in rs]
        if rs:
            per[k] = rs
    ids = sorted(per)
    half = len(ids) // 2
    ds = []
    for left in itertools.combinations(ids, half):
        if ids[0] not in left:
            continue  # each split once
        a = [r for i in left for r in per[i]]
        b = [r for i in ids if i not in left for r in per[i]]
        ds.append(ks(a, b))
    ds.sort()
    print(json.dumps({"ids": len(ids), "pairs": {i: len(per[i]) for i in ids}, "splits": len(ds),
                      "ks_p50": ds[len(ds) // 2], "ks_p95": ds[int(0.95 * len(ds))], "ks_max": ds[-1]}))


if __name__ == "__main__":
    if sys.argv[1] == "tol":
        tol(sys.argv[2])
    else:
        a = json.load(open(sys.argv[2]))["residuals"]
        b = json.load(open(sys.argv[3]))["residuals"]
        print(json.dumps({"ks": ks(a, b), "n_a": len(a), "n_b": len(b)}))
