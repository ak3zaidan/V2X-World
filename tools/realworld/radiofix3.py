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
    air_by_type = collections.Counter()
    denm_t = collections.defaultdict(list)
    tx_path = os.path.join(J, "node_tx.jsonl")
    if os.path.exists(tx_path):
        for line in open(tx_path):
            r = json.loads(line)
            t = r.get("msg_type")
            by_type[t] += 1
            if r["t"] >= w0 and r.get("airtime_us") is not None:
                air_by_type[f'{t}@{r.get("channel")}'] += r["airtime_us"]
            if t == "bsm" and r["t_generated"] >= w0:
                bsm_gen += 1
            if t == "denm":
                c = r.get("content") or {}
                k = c.get("denm_cause") or "?"
                denm_cause[str(k)] += 1
                denm_senders[str(k)].add(r["node"])
                denm_t[(str(k), r["node"])].append(r["t"] / 1e9)
            if t == "cpm":
                cpm_by_node[r["node"]] += 1
    out["tx_by_type"] = dict(by_type)
    out["bsm_generated_live"] = bsm_gen
    out["air_us_by_type_live"] = dict(air_by_type)
    if denm_cause:
        out["denm_by_cause"] = dict(denm_cause)
        out["denm_senders_by_cause"] = {k: len(v) for k, v in denm_senders.items()}
        # Per (cause, sender): first frame time, frames, and the median repetition gap.
        out["denm_series"] = {
            f"{k}/{n}": [round(ts[0], 2), len(ts), round(sorted(b - a for a, b in zip(ts, ts[1:]))[len(ts) // 2 - 1], 3) if len(ts) > 1 else None]
            for (k, n), ts in sorted(denm_t.items())
        }
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


def _ratio(a, b):
    return a / b if a is not None and b else None


# Each judged row: its measure from the runs' summaries (`<runs>/<RUN>.json`, written by
# the per-run mode above). `run` is the row's run; `base` its base run.
MEASURES = {
    "R3.1.DSRC-NLOS-EDGE": lambda s, run, base: s[run]["cross50_m"],
    "R3.1.LTE-NLOS-EDGE": lambda s, run, base: s[run]["cross50_m"],
    "R3.1.NR-NLOS-EDGE": lambda s, run, base: s[run]["cross50_m"],
    "R3.1.LTE-RETX-DEFAULT": lambda s, run, base: _ratio(
        s[run]["frames_transmitted"], (s[run]["sidelink"] or {}).get("grants")),
    "R3.1.RETX1-FRAMES": lambda s, run, base: _ratio(
        s[run]["frames_transmitted"], s[base]["frames_transmitted"]),
    "R3.1.RETX1-PDR": lambda s, run, base: s[run]["pdr300"],
    "R3.1.RETX1-WALL": lambda s, run, base: _ratio(s[run].get("wall_s"), s[base].get("wall_s")),
    "R3.1.EQP-PDR": lambda s, run, base: s[run]["pdr300"],
    "R3.1.EQP-UDOT": lambda s, run, base: _ratio(s[run]["cross50_m"], s["D1"]["cross50_m"]),
}


def judge(spec_path, runs_dir, swaps):
    """Judges every row with a measure; `swaps` (`RUN=OTHER`) is the fault proof: the row is
    judged on another real run, one without the mechanism the row is about."""
    import yaml

    spec = yaml.safe_load(open(spec_path))
    names = {r: r for r in spec.get("runs", {})}
    names.update(dict(s.split("=", 1) for s in swaps))
    summaries = {}
    for r in spec.get("runs", {}):
        p = os.path.join(runs_dir, names[r] + ".json")
        if os.path.exists(p):
            summaries[r] = json.load(open(p))
    bad = 0
    for row in spec.get("rows", []):
        f = MEASURES.get(row["id"])
        if f is None:
            continue
        run = row.get("run")
        base = (spec["runs"].get(run) or {}).get("base")
        try:
            v = f(summaries, run, base)
        except KeyError:
            v = None
        lo, hi = row["band"]
        ok = v is not None and lo <= v <= hi
        bad += not ok
        print(f"{'PASS' if ok else 'FAIL'} {row['id']}: {v if v is None else round(v, 4)} in [{lo}, {hi}]")
    return bad


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "judge":
        sys.exit(1 if judge(sys.argv[2], sys.argv[3], sys.argv[4:]) else 0)
    main()
