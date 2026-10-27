#!/usr/bin/env python3
"""Per-run measures for crates/v2xw-engine/tests/expectations/radio-fix3.yaml.

    python3 -I tools/realworld/radiofix3.py <run dir> [--wall <seconds>]

Prints one JSON object: reception within 300 m of the live window's BSM receptions
(`pdr300`, the P1-E5 definition of e5gaps.py), reception by 50 m bin and the first bin
under 0.5 (`cross50_m`, its lower edge plus half a bin), frames on air and BSMs generated,
the run report's sidelink block, the live window's mean and p90 CBR per channel, and the
DENM / CPM counts by cause and sender when the run has them. Reads node_rx/node_tx/mac_cbr
from the jsonl exporter.
"""
import collections
import json
import os
import re
import sys


def live_start_ns(rd):
    txt = open(os.path.join(rd, "scenario.resolved.yaml")).read()
    m = re.search(r"warmup_s:\s*([0-9.]+)", txt)
    return int(float(m.group(1)) * 1e9) if m else 0


def q(v, p):
    v = sorted(v)
    return v[int(p * (len(v) - 1))] if v else None


def main():
    rd = sys.argv[1]
    J = os.path.join(rd, "jsonl")
    w0 = live_start_ns(rd)
    out = {}
    bsm_gen = 0
    by_type = collections.Counter()
    denm_cause = collections.Counter()
    denm_senders = collections.defaultdict(set)
    cpm_by_node = collections.Counter()
    tx_path = os.path.join(J, "node_tx.jsonl")
    if os.path.exists(tx_path):
        for line in open(tx_path):
            r = json.loads(line)
            t = r.get("msg_type")
            by_type[t] += 1
            if t == "bsm" and r["t_generated"] >= w0:
                bsm_gen += 1
            if t == "denm":
                c = r.get("content") or {}
                k = c.get("cause") or c.get("cause_code") or c.get("event") or "?"
                denm_cause[str(k)] += 1
                denm_senders[str(k)].add(r["node"])
            if t == "cpm":
                cpm_by_node[r["node"]] += 1
    out["tx_by_type"] = dict(by_type)
    out["bsm_generated_live"] = bsm_gen
    if denm_cause:
        out["denm_by_cause"] = dict(denm_cause)
        out["denm_senders_by_cause"] = {k: len(v) for k, v in denm_senders.items()}
    if cpm_by_node:
        out["cpm_per_sender_mean"] = sum(cpm_by_node.values()) / len(cpm_by_node)
    bins = collections.defaultdict(lambda: [0, 0])
    pdr = [0, 0]
    rx_path = os.path.join(J, "node_rx.jsonl")
    if os.path.exists(rx_path):
        for line in open(rx_path):
            r = json.loads(line)
            if r.get("msg_type") != "bsm" or r["t"] < w0 or r.get("dist_m") is None:
                continue
            ok = r["outcome"] == "delivered"
            b = int(r["dist_m"] // 50) * 50
            bins[b][0] += ok
            bins[b][1] += 1
            if r["dist_m"] <= 300.0:
                pdr[0] += ok
                pdr[1] += 1
    out["pdr300"] = round(pdr[0] / pdr[1], 4) if pdr[1] else None
    out["bins"] = {b: round(v[0] / v[1], 3) for b, v in sorted(bins.items()) if v[1] >= 50}
    out["cross50_m"] = next((b + 25 for b, v in sorted(bins.items()) if v[1] >= 50 and v[0] / v[1] < 0.5), None)
    cb = collections.defaultdict(list)
    cbr_path = os.path.join(J, "mac_cbr.jsonl")
    if os.path.exists(cbr_path):
        for line in open(cbr_path):
            r = json.loads(line)
            if r["t"] > w0:
                cb[r.get("channel", "run")].append(r["cbr"])
    out["cbr"] = {str(k): {"mean": round(sum(v) / len(v), 5), "p90": q(v, 0.9), "n": len(v)} for k, v in cb.items()}
    rep = json.load(open(os.path.join(rd, "run-report.json")))
    r = rep.get("report") or {}
    out["frames_transmitted"] = r.get("frames_transmitted")
    sl = r.get("sidelink") or {}
    out["sidelink"] = {k: v for k, v in sl.items() if not isinstance(v, (dict, list))}
    out["content_digest"] = rep.get("content_digest")
    out["record_digest"] = rep.get("record_digest")
    if "--wall" in sys.argv:
        out["wall_s"] = float(sys.argv[sys.argv.index("--wall") + 1])
    print(json.dumps(out, sort_keys=True))


if __name__ == "__main__":
    main()
