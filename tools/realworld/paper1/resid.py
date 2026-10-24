"""RandomPosition residuals of a BSM source, through tools/realdata's own stages 2-4.

  python3 -I resid.py wyoming <wyoming.csv> [--vmin 10] [--json out.json]
  python3 -I resid.py engine <run>/jsonl/node_tx.jsonl [--vmin 10] [--json out.json]

The engine source turns each transmitted BSM (node.tx, the J2735 fields the OBU sent) into
the realdata Bsm tuple, keyed by temp_id, sequenced by its own secMark (as Wyoming is), so
both go through the same dedupe/pair/check code. Residual: the v1.0 RandomPosition
prediction error |p1 + v1*dt*u(h1) - p2|, secMark dt, pairs judged at the 0.5 s gap limit,
v1 >= vmin. Prints n, quantiles and the trigger share (r > 1 m)."""
import json
import math
import os
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
sys.path.insert(0, os.path.join(REPO, "tools", "realdata"))
import mbrchecks as mc  # noqa: E402
import normalise  # noqa: E402
import pairing  # noqa: E402


def engine_ids(path):
    by = {}
    with open(path) as f:
        for line in f:
            r = json.loads(line)
            if r.get("msg_type") != "bsm":
                continue
            c = r.get("content")
            if isinstance(c, str):
                c = json.loads(c)
            if not c or c.get("sec_mark_ms") is None:
                continue
            # the transmit instant (s) sequences one ID like Wyoming's header time does
            t = r["t"] / 1e9
            sm = c["sec_mark_ms"]
            if sm >= 60000:
                sm = None
            hd = c.get("heading_deg")
            b = normalise.Bsm(c["temp_id"], sm, c.get("msg_count"), normalise.unwrap_secmark(t, sm), t, None,
                              c["lat_deg"], c["lon_deg"], c.get("speed_mps"),
                              None if hd is None or hd < 0 or hd >= 360 else hd,
                              c.get("accel_long_mps2"), None, None, None)
            by.setdefault(c["temp_id"], []).append(b)
    for k in sorted(by):
        yield k, by[k]


def residuals(ids, vmin):
    out = []
    nid = 0
    for _, bsms in ids:
        bs = pairing.dedupe_sort(bsms)
        used = False
        for p in pairing.pairs(bs):
            dt = p.dt[0]
            if p.v1 is None or p.h1 is None or dt is None or dt <= 0 or p.v1 < vmin:
                continue
            if not mc.judged(dt, 0.5):
                continue
            _, r10, _ = mc.run(p, dt)
            out.append((r10, dt, p.v1))
            used = True
        nid += used
    return out, nid


def q(xs, f):
    if not xs:
        return None
    s = sorted(xs)
    return s[min(len(s) - 1, int(f * len(s)))]


def main():
    kind, path = sys.argv[1], sys.argv[2]
    vmin = float(sys.argv[sys.argv.index("--vmin") + 1]) if "--vmin" in sys.argv else 10.0
    ids = normalise.stream_ids(path, "wyoming") if kind == "wyoming" else engine_ids(path)
    rows, nid = residuals(ids, vmin)
    rs = [r for r, _, _ in rows]
    trig = sum(r > mc.GPS_DRIFT_M for r in rs)
    summ = {"source": kind, "vmin": vmin, "ids": nid, "pairs": len(rs), "triggers": trig,
            "trigger_share": trig / len(rs) if rs else None,
            "q": {str(f): q(rs, f) for f in (0.1, 0.25, 0.5, 0.75, 0.9, 0.99)}}
    print(json.dumps(summ))
    if "--json" in sys.argv:
        with open(sys.argv[sys.argv.index("--json") + 1], "w") as f:
            json.dump({"summary": summ, "residuals": [round(r, 5) for r in rs]}, f)


if __name__ == "__main__":
    main()
