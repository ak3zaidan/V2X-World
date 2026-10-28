#!/usr/bin/env python3
"""Measures for crates/v2xw-engine/tests/expectations/radio-fix4.yaml.

    python3 -I tools/realworld/radiofix4.py shares <G20 run dir> [--acir 12.98]
    python3 -I tools/realworld/radiofix4.py digest <run dir>

`shares` predicts the CBR on 172 when SPaT and MAP move to 174 (R4.1.CBR-*), from the
single-channel base run's receptions in the live window: B_all is the airtime of every
arrival at or above -85 dBm (the CBR busy level), B_moved that of the SPaT, MAP, SRM and
SSM among them (SRM and SSM go too: a single-radio OBU never hears a MAP on 174), and
B_leak(L) that of the SPaT and MAP arrivals that still count on 172 from 174, i.e. whose
power less the 174->172 ACIR reaches L (-85 dBm before radio-fix4, the energy-detect level
-65 dBm after it under SAE J2945/1). The predicted ratio is 1 - (B_moved - B_leak) / B_all.

`digest` is the SHA-256 of the run's node_tx, node_rx and mac_cbr jsonl files: the
recorded behaviour, without the manifest's scenario hash.
"""
import hashlib
import json
import os
import re
import sys


def live_start_ns(rd):
    txt = open(os.path.join(rd, "scenario.resolved.yaml")).read()
    m = re.search(r"warmup_s:\s*([0-9.]+)", txt)
    return int(float(m.group(1)) * 1e9) if m else 0


def shares(rd, acir):
    w0 = live_start_ns(rd)
    b_all = b_moved = 0
    leak = {-85.0: 0, -65.0: 0}
    for line in open(os.path.join(rd, "jsonl", "node_rx.jsonl")):
        r = json.loads(line)
        p, us = r.get("rssi_dbm"), r.get("airtime_us")
        if p is None or us is None or r["t"] < w0 or p < -85.0:
            continue
        b_all += us
        t = r.get("msg_type")
        if t in ("spat", "map", "srm", "ssm"):
            b_moved += us
        if t in ("spat", "map"):
            for lvl in leak:
                if p - acir >= lvl:
                    leak[lvl] += us
    out = {"B_all_us": b_all, "B_moved": round(b_moved / b_all, 4)}
    for lvl, v in leak.items():
        out[f"B_leak{int(-lvl)}"] = round(v / b_all, 4)
        out[f"p{int(-lvl)}"] = round(1 - (b_moved - v) / b_all, 4)
    print(json.dumps(out, sort_keys=True))


def digest(rd):
    h = hashlib.sha256()
    for f in ("node_tx", "node_rx", "mac_cbr"):
        path = os.path.join(rd, "jsonl", f + ".jsonl")
        if os.path.exists(path):
            h.update(f.encode())
            h.update(open(path, "rb").read())
    print(h.hexdigest()[:16])


if __name__ == "__main__":
    if sys.argv[1] == "shares":
        a = float(sys.argv[sys.argv.index("--acir") + 1]) if "--acir" in sys.argv else 12.98
        shares(sys.argv[2], a)
    elif sys.argv[1] == "digest":
        digest(sys.argv[2])
