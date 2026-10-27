#!/usr/bin/env python3
"""P1-E6 offline store replay: a finished run's report-fate log replayed at a smaller quota.

    python3 -I tools/realworld/paper1/e6replay.py <base-run> <out-run> <quota-bytes>

The base run must hold every report (the default 64 KiB quota evicted nothing). Its
`pki.report_fate` log (jsonl/ or keep/) gives each report's reporter, when it was stored
and when, if ever, it left on an access link. The replay re-runs only the SM-MBR v1.0 §2.2
store on each device at `quota` (capacity floor(quota / 1,119 B), the reference report):
a stored report enters, over capacity the lowest weight goes, older first on ties (every
initial weight is 1.0 and no 05:00 ET decay falls inside a 120 s run); a report the base
uploaded leaves at its upload time if the replay still holds it. Evicted reports are lost;
a kept report keeps the base's final fate.

Approximation, stated: the engine takes reports out of the store while their upload is in
flight (they then do not count against the quota) and a smaller store can change what the
MA revokes and so what is reported later; the replay holds a report until its upload
instant and keeps the base's report stream. Its agreement with the engine at each quota is
the replay's own falsification test (rows P1.E6.replay-*).

Writes `<out-run>/realism.json` with the x.fate.* measures of tools/realworld/paper1/summ.py.
"""
import json
import os
import sys

REPORT_BYTES = 1119
LOST = {"evicted_quota", "decayed_out", "expired"}


def rows(run):
    for sub in ("jsonl", "keep"):
        p = os.path.join(run, sub, "pki_report_fate.jsonl")
        if os.path.exists(p):
            return [json.loads(line) for line in open(p)]
    raise SystemExit(f"{run}: no pki_report_fate.jsonl")


def replay(fates, quota):
    cap = quota // REPORT_BYTES
    first, upload, final, attack, rep = {}, {}, {}, set(), {}
    for i, r in enumerate(fates):
        rid = r["report_id"]
        t = r["t"]
        if rid not in first:
            first[rid] = (t, i)
            rep[rid] = r.get("reporter")
        if r["fate"] == "uploaded" and rid not in upload:
            upload[rid] = (t, i)
        if r["fate"] in LOST:
            raise SystemExit("the base run evicted a report: it must hold every report")
        if rid not in final or t >= final[rid][0]:
            final[rid] = (t, r["fate"])
        tr = r.get("truth")
        tr = json.loads(tr) if isinstance(tr, str) else (tr or {})
        if tr.get("subject_attacker"):
            attack.add(rid)
    ev = [(t, i, 0, rid) for rid, (t, i) in first.items()] + [(t, i, 1, rid) for rid, (t, i) in upload.items()]
    ev.sort()
    store, seq, evicted = {}, 0, set()
    for _, _, kind, rid in ev:
        dev = rep[rid]
        s = store.setdefault(dev, {})
        if kind == 0:
            seq += 1
            s[rid] = seq
            while len(s) > cap:
                old = min(s, key=s.get)
                del s[old]
                evicted.add(old)
        else:
            s.pop(rid, None)
    d = {}
    term = {k: ("evicted_quota" if k in evicted else f) for k, (_, f) in final.items()}
    delivered = {k for k, f in term.items() if f == "delivered"}
    d["x.fate.reports"] = len(term)
    d["x.fate.attack_reports"] = len(attack)
    d["x.fate.honest_reports"] = len(term) - len(attack)
    d["x.fate.attack_lost"] = len(evicted & attack)
    d["x.fate.attack_lost_share"] = len(evicted & attack) / len(attack) if attack else None
    d["x.fate.lost_share"] = len(evicted) / len(term) if term else None
    d["x.fate.delivered"] = len(delivered)
    d["x.fate.attack_delivered"] = len(delivered & attack)
    d["x.fate.evicted_quota"] = len(evicted)
    d["x.e6r.quota"] = quota
    d["x.e6r.capacity"] = cap
    return d


if __name__ == "__main__":
    base, out, quota = sys.argv[1], sys.argv[2], int(sys.argv[3])
    d = replay(rows(base), quota)
    os.makedirs(out, exist_ok=True)
    json.dump(d, open(os.path.join(out, "realism.json"), "w"), indent=1, sort_keys=True)
    print(os.path.basename(out), json.dumps(d))
