"""Fit radio.devices.rsu.off_channel on Tampa's free-device loss by secMark phase.

usage: fitoff.py tampa.csv node_tx.jsonl
Model: excess(phi) = p * P(from <= phi + L - 0 < from + D), D ~ U[dmin, dmax], L the engine's
own generation->air latency (sampled from a base run's node.tx: t - t_generated, BSMs).
The rest-of-interval loss (phase 50-99) is the baseline the excess is measured against."""
import csv, sys, json, collections, datetime as dt, random

rows = collections.defaultdict(list)
for r in csv.DictReader(open(sys.argv[1])):
    try:
        t = dt.datetime.fromisoformat(r["metadata_recordgeneratedat"]).timestamp()
        rows[r["coredata_id"]].append((t, int(r["coredata_secmark"]), int(float(r["coredata_msgcnt"]))))
    except Exception:
        pass


def near(s):
    m = s % 50
    return min(m, 50 - m) <= 1


b = collections.defaultdict(lambda: [0, 0])
for k, v in rows.items():
    if len(v) < 10 or sum(near(x[1]) for x in v) / len(v) >= 0.3:
        continue  # free devices only
    v.sort()
    for a, c in zip(v, v[1:]):
        dm = (c[2] - a[2]) % 128
        ds = (c[1] - a[1]) % 60000
        if dm == 0 or dm > 30 or abs(ds - 100 * dm) > 60:
            continue
        step = ds / dm
        for j in range(1, dm + 1):
            ph = int(((a[1] + step * j) % 60000) % 100) // 2  # 2 ms bins
            b[ph][0] += 1
            b[ph][1] += j < dm
loss = [b[i][1] / b[i][0] for i in range(50)]
base = sum(b[i][1] for i in range(25, 50)) / sum(b[i][0] for i in range(25, 50))
obs = [max(0.0, (l - base) / (1 - base)) for l in loss]  # conditional on not lost otherwise
lat = []
if sys.argv[2].startswith("U"):
    lo, hi = map(float, sys.argv[2][1:].split(","))
    lat = [lo + (hi - lo) * i / 3999 for i in range(4000)]
for line in (open(sys.argv[2]) if not lat else []):
    r = json.loads(line)
    sm = (r.get("content") or {}).get("sec_mark_ms")
    if r.get("msg_type") == "bsm" and sm is not None:
        # air instant minus the secMark it carries (t0 is a whole UTC second)
        d = ((r["t"] / 1e6) % 60000 - sm) % 60000
        lat.append(d - 60000 if d > 30000 else d)
    if len(lat) >= 4000:
        break
lat.sort()
print("engine gen->air latency ms p10/p50/p90: %.2f %.2f %.2f" % (lat[len(lat) // 10], lat[len(lat) // 2], lat[9 * len(lat) // 10]))
rng = random.Random(1)
Ls = [rng.choice(lat) for _ in range(400)]


def model(p, s, dmin, dmax):
    out = []
    for i in range(50):
        phi = 2 * i + 1
        acc = 0.0
        for L in Ls:
            x = phi + L - s  # time since the unit left
            if x < 0:
                continue
            # P(D > x), D ~ U[dmin, dmax]
            acc += 1.0 if x < dmin else 0.0 if x >= dmax else (dmax - x) / max(dmax - dmin, 1e-9)
        out.append(p * acc / len(Ls))
    return out


best = None
for p in [x / 20 for x in range(4, 21)]:
    for s in range(0, 30, 2):
        for dmin in range(0, 40, 4):
            for dmax in range(dmin, 52, 4):
                if s + dmax > 50:  # back by the end of the 1609.4 control-channel interval
                    continue
                m = model(p, s, dmin, dmax)
                e = sum((m[i] - obs[i]) ** 2 * b[i][0] for i in range(50))
                if best is None or e < best[0]:
                    best = (e, p, s, dmin, dmax)
e, p, s, dmin, dmax = best
m = model(p, s, dmin, dmax)
print("rest-of-interval loss %.3f" % base)
print("fit: probability %.2f from_ms %d min_duration_ms %d max_duration_ms %d (sse %.1f)" % (p, s, dmin, dmax, e))
print("obs   :", " ".join("%.2f" % x for x in obs[:30]))
print("model :", " ".join("%.2f" % x for x in m[:30]))
pred_bump = sum((base + (1 - base) * m[i]) * b[i][0] for i in range(5, 15)) / sum(b[i][0] for i in range(5, 15))
print("predicted Tampa-free loss in [10,30) ms: %.3f (observed %.3f)" % (
    pred_bump, sum(b[i][1] for i in range(5, 15)) / sum(b[i][0] for i in range(5, 15))))
print("mean time away per interval ms: %.1f" % (p * (dmin + dmax) / 2))
