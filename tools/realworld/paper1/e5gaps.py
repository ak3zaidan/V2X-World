#!/usr/bin/env python3
"""P1-E5: receiver-side BSM gaps at density, against Tampa's RSU-received gaps.

    python3 -I tools/realworld/paper1/e5gaps.py tampa <tampa.csv>      # the reference
    python3 -I tools/realworld/paper1/e5gaps.py run <run-dir> [--tampa <tampa.csv>]

A *stream* is one transmitter pseudonym as one receiver decoded it (Tampa: one temporary
ID in the RSU log; the 12 public columns do not name the RSU). A *gap* is the time between
two consecutive decoded BSMs of a stream on the receiver's clock (Tampa:
`metadata_recordgeneratedat`; engine: `node.rx` `t_rx_done`). A gap over SESSION_S = 10 s
closes the stream (the sender left coverage), so both sides compare gaps inside a
contact. Pre-registered measures (research/synthesis/p1-experiments.md, P1-E5):

  share_1int   share of gaps in [50, 150) ms (one 100 ms BSM interval)
  share_le500  share of gaps <= 500 ms (inside the US profile's 0.5 s pairing limit)
  p50_ms, p90_ms, ks_tampa (two-sample KS distance against Tampa's gaps)

`run` also writes the judged-pair share: of the OBU receivers' *verified* BSMs that follow
an earlier verified BSM of the same pseudonym, the share whose secMark dt is in (0, 0.5] s,
which is the pair the US profile (time_base secmark, max_gap_s 0.5) judges; the BSM
reception ratio within 300 m; and the live window's mean and p90 channel busy ratio. The
measures are merged into the run's realism.json as x.e5.*. Ground truth (tx node) is used
only to name the stream, as the RSU log's temp ID does; nothing here feeds a detector.
"""
import bisect
import csv
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SESSION_S = 10.0


def q(xs, f):
    return xs[min(len(xs) - 1, int(f * len(xs)))] if xs else None


def ks(a, b):
    a, b = sorted(a), sorted(b)
    if not a or not b:
        return None
    m = 0.0
    for x in a + b:
        m = max(m, abs(bisect.bisect_right(a, x) / len(a) - bisect.bisect_right(b, x) / len(b)))
    return m


def summary(gaps_ms, ref=None):
    g = sorted(gaps_ms)
    n = len(g)
    d = {"gaps": n,
         "share_1int": (sum(1 for x in g if 50 <= x < 150) / n) if n else None,
         "share_le500": (sum(1 for x in g if x <= 500) / n) if n else None,
         "share_lt50": (sum(1 for x in g if x < 50) / n) if n else None,
         "share_2int": (sum(1 for x in g if 150 <= x < 250) / n) if n else None,
         "share_3to5": (sum(1 for x in g if 250 <= x <= 500) / n) if n else None,
         "p50_ms": q(g, 0.5), "p90_ms": q(g, 0.9), "p99_ms": q(g, 0.99)}
    if ref is not None:
        d["ks_tampa"] = ks(g, ref)
    return d


def tampa_gaps(path):
    sys.path.insert(0, os.path.join(HERE, "..", "..", "realdata"))
    import normalise  # noqa: E402
    gaps, streams = [], 0
    for _, bsms in normalise.stream_ids(path, "tampa"):
        ts = sorted(b.t_rx for b in bsms if b.t_rx is not None)
        streams += 1
        for a, b in zip(ts, ts[1:]):
            dt = (b - a) * 1000.0
            if dt <= SESSION_S * 1000.0:
                gaps.append(round(dt, 3))
    return gaps, streams


def run(rd, tampa):
    ref = tampa_gaps(tampa)[0] if tampa else None
    J = os.path.join(rd, "jsonl")
    res = yaml_load(os.path.join(rd, "scenario.resolved.yaml"))
    warm = float(((res.get("time") or {}).get("warmup_s")) or 0.0)
    w0 = int(warm * 1e9)
    # node.tx: BSM secMark and pseudonym by message id; RSU ids are the nodes that never send a BSM
    secmark, pseud, senders = {}, {}, set()
    seq = {}  # sender -> its BSMs in send order: (t_generated, msg, secMark)
    for line in open(os.path.join(J, "node_tx.jsonl")):
        r = json.loads(line)
        if r.get("msg_type") != "bsm":
            continue
        c = r.get("content") or {}
        secmark[r["msg"]] = (r["t_generated"], c.get("sec_mark_ms"))
        pseud[r["msg"]] = r.get("pseudonym")
        senders.add(r["node"])
        seq.setdefault(r["node"], []).append((r["t_generated"], r["msg"], c.get("sec_mark_ms")))
    at = {}
    for node, lst in seq.items():
        lst.sort()
        for i, (_, m, _sm) in enumerate(lst):
            at[m] = (node, i)
    # P1-E5 gap shape: the BSMs an RSU missed between two it decoded on one stream, by the
    # missed BSM's secMark phase (the msgCnt method on Tampa: 0.51 lost at 10-30 ms, 0.22 at
    # 50-99 ms, free devices)
    ph = {"bump": [0, 0], "rest": [0, 0]}
    lastm = {}
    last = {}
    lastv = {}
    gaps = {"rsu": [], "obu": []}
    judged = [0, 0]
    cnt, last2, drop2 = {}, {}, []
    pdr = [0, 0]
    for line in open(os.path.join(J, "node_rx.jsonl")):
        r = json.loads(line)
        if r.get("msg_type") != "bsm" or r["t"] < w0:
            continue
        if r.get("dist_m") is not None and r["dist_m"] <= 300.0:
            pdr[1] += 1
            pdr[0] += r["outcome"] == "delivered"
        if r["outcome"] != "delivered":
            continue
        rx = r["rx"]
        cls = "obu" if rx in senders else "rsu"
        k = (rx, pseud.get(r["msg"]))
        t = r.get("t_rx_done") or r["t"]
        p = last.get(k)
        if p is not None:
            dt = (t - p) / 1e6
            if dt <= SESSION_S * 1000.0:
                gaps[cls].append(dt)
        last[k] = t
        if cls == "rsu":
            pm = lastm.get(k)
            cur = at.get(r["msg"])
            if pm is not None and cur is not None and pm[0] == cur[0] and p is not None \
                    and (t - p) / 1e6 <= SESSION_S * 1000.0:
                lst = seq[cur[0]]
                for j in range(pm[1] + 1, cur[1] + 1):
                    sm = lst[j][2]
                    if sm is None:
                        continue
                    w = "bump" if 10 <= sm % 100 < 30 else "rest" if sm % 100 >= 50 else None
                    if w:
                        ph[w][0] += 1
                        ph[w][1] += j < cur[1]
            if cur is not None:
                lastm[k] = cur
        # the injected fault (fault proof of the KS row): every second decoded BSM dropped
        n = cnt[k] = cnt.get(k, 0) + 1
        if cls == "rsu" and n % 2 == 0:
            p2 = last2.get(k)
            if p2 is not None and (t - p2) / 1e6 <= SESSION_S * 1000.0:
                drop2.append((t - p2) / 1e6)
            last2[k] = t
        if cls == "obu" and r.get("verification") == "verified":
            gen, sm = secmark.get(r["msg"], (None, None))
            pv = lastv.get(k)
            if pv is not None and sm is not None and pv[1] is not None:
                # secMark dt, unwrapped on the generation clock (a minute wrap)
                dsm = (sm - pv[1]) % 60000
                judged[1] += 1
                judged[0] += 0 < dsm <= 500
            lastv[k] = (gen, sm)
    d = {}
    for cls in ("rsu", "obu"):
        for kk, v in summary(gaps[cls], ref).items():
            d[f"x.e5.{cls}.{kk}"] = v
    for w in ("bump", "rest"):
        d[f"x.e5.rsu.loss_{w}"] = ph[w][1] / ph[w][0] if ph[w][0] else None
    lb, lr = d["x.e5.rsu.loss_bump"], d["x.e5.rsu.loss_rest"]
    d["x.e5.rsu.loss_bump_excess"] = lb - lr if lb is not None and lr is not None else None
    d["x.e5.rsu.ks_tampa_drop2"] = ks(drop2, ref) if ref is not None else None
    d["x.e5.judged_share"] = judged[0] / judged[1] if judged[1] else None
    d["x.e5.judged_pairs"] = judged[0]
    d["x.e5.pdr300"] = pdr[0] / pdr[1] if pdr[1] else None
    cb = []
    for line in open(os.path.join(J, "mac_cbr.jsonl")):
        r = json.loads(line)
        if r["t"] > w0:
            cb.append(r["cbr"])
    cb.sort()
    d["x.e5.cbr_mean"] = sum(cb) / len(cb) if cb else None
    d["x.e5.cbr_p90"] = q(cb, 0.9)
    d["x.e5.senders"] = len(senders)
    p = os.path.join(rd, "realism.json")
    base = json.load(open(p)) if os.path.exists(p) else {}
    base.update(d)
    json.dump(base, open(p, "w"), indent=1, sort_keys=True, default=str)
    print(os.path.basename(rd), json.dumps({k: (round(v, 4) if isinstance(v, float) else v) for k, v in d.items()}))


def yaml_load(p):
    import yaml
    return yaml.safe_load(open(p))


def tampa_tol(path, splits=400, seed=20261026):
    """The KS tolerance, fixed before any E5 run: the 95th percentile of the KS distance
    between the gaps of two random halves of Tampa's temporary IDs (the E4 method)."""
    import random
    sys.path.insert(0, os.path.join(HERE, "..", "..", "realdata"))
    import normalise  # noqa: E402
    per = []
    for _, bsms in normalise.stream_ids(path, "tampa"):
        ts = sorted(b.t_rx for b in bsms if b.t_rx is not None)
        per.append([round((b - a) * 1000.0, 3) for a, b in zip(ts, ts[1:]) if b - a <= SESSION_S])
    rng = random.Random(seed)
    ds = []
    for _ in range(splits):
        idx = list(range(len(per)))
        rng.shuffle(idx)
        h = len(idx) // 2
        ds.append(ks([g for i in idx[:h] for g in per[i]], [g for i in idx[h:] for g in per[i]]))
    ds.sort()
    return q(ds, 0.95)


if __name__ == "__main__":
    if sys.argv[1] == "tol":
        print(json.dumps({"ks_tol_p95": tampa_tol(sys.argv[2])}))
    elif sys.argv[1] == "tampa":
        g, n = tampa_gaps(sys.argv[2])
        print(json.dumps({"streams": n, **summary(g)}))
    elif sys.argv[1] == "run":
        t = sys.argv[sys.argv.index("--tampa") + 1] if "--tampa" in sys.argv else None
        run(sys.argv[2], t)
