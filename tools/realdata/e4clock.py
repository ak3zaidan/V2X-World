"""Device-clock anatomy of the P1-E4 RandomPosition residual (Wyoming, plus Tampa/Orem locked).

  python3 -I tools/realdata/e4clock.py wyoming <wyoming.csv>
  python3 -I tools/realdata/e4clock.py tampa <tampa.csv>

Same pairs as tools/realworld/paper1/resid.py (secMark dt, judged at 0.5 s, v1 >= 10 m/s,
v1.0 residual |p1 + v1 dt u(h1) - p2|). Each pair is split by its clock signature:
  step     secMark dt in ms (100, 50/150 = a :00/:50 grid hop, 200+ = msgCnt-attested loss)
  dcnt     msgCnt step
  hdr-sm   header-time dt minus secMark dt (ms): the log clock against the BSM's own stamp
and the residual is resolved along / across the heading. tau = along / v1 (ms) is the
timing error that would explain the along-track residual alone. Counterfactual "grid dt":
the residual recomputed with dt = round(secMark dt / 100 ms) * 100 ms, i.e. as if the
position were fixed on the 100 ms epoch and only the stamp hopped.
"""
import collections
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mbrchecks as mc  # noqa: E402
import normalise  # noqa: E402
import pairing  # noqa: E402


def q(xs, fs=(0.5, 0.9, 0.99)):
    s = sorted(xs)
    return [s[min(len(s) - 1, int(f * len(s)))] for f in fs] if s else [float("nan")] * len(fs)


def ks(a, b):
    import bisect
    a, b = sorted(a), sorted(b)
    return max(abs(bisect.bisect_right(a, x) / len(a) - bisect.bisect_right(b, x) / len(b)) for x in a + b)


def rows(path, kind):
    for k, bsms in normalise.stream_ids(path, kind):
        bs = pairing.dedupe_sort(bsms)
        locked = sum(b.secmark is not None and min(b.secmark % 50, 50 - b.secmark % 50) <= 1 for b in bs)
        cls = "locked" if bs and locked / len(bs) >= 0.7 else "free" if bs and locked / len(bs) < 0.3 else "mixed"
        for i, p in enumerate(pairing.pairs(bs)):
            dt = p.dt[0]
            if p.v1 is None or p.h1 is None or dt is None or dt <= 0 or p.v1 < 10.0 or not mc.judged(dt, 0.5):
                continue
            _, r, (ex, ey) = mc.run(p, dt)
            a, c = mc.along_cross(p, ex, ey)
            g = max(1, round(dt * 10)) / 10.0
            _, rg, _ = mc.run(p, g)
            hs = None if p.dt[1] is None else round((p.dt[1] - dt) * 1000)
            yield {"i": i, "id": k, "cls": cls, "step": round(dt * 1000), "dcnt": p.dcnt, "r": r, "a": a, "c": c,
                   "tau": 1000 * a / p.v1, "rg": rg, "hs": hs, "v": p.v1, "dv": None if p.v2 is None else p.v2 - p.v1,
                   "dh": None if p.h2 is None else (p.h2 - p.h1 + 180) % 360 - 180, "ph1": p.sm1 % 100, "ph2": p.sm2 % 100}


def cat(x):
    s = x["step"]
    return "100" if 98 <= s <= 102 else "hop 50/150" if 48 <= s <= 52 or 148 <= s <= 152 else \
        "loss 200-500" if s >= 198 and abs(s % 100) <= 2 else "other"


def table(rs, key, title):
    print("\n" + title)
    print("| %s | pairs | share | r p50/p90/p99 cm | along p50/p90 abs cm | cross p90 cm | tau p50/p90 abs ms |" % key)
    print("|---|---|---|---|---|---|---|")
    g = collections.defaultdict(list)
    for x in rs:
        g[x[key] if key != "cat" else cat(x)].append(x)
    for k in sorted(g, key=str):
        v = g[k]
        r = q([x["r"] * 100 for x in v])
        a = q([abs(x["a"]) * 100 for x in v], (0.5, 0.9))
        c = q([abs(x["c"]) * 100 for x in v], (0.9,))
        t = q([abs(x["tau"]) for x in v], (0.5, 0.9))
        print("| %s | %d | %.3f | %.2f / %.2f / %.1f | %.2f / %.2f | %.2f | %.2f / %.2f |" % (
            k, len(v), len(v) / len(rs), r[0], r[1], r[2], a[0], a[1], c[0], t[0], t[1]))


def main():
    kind, path = sys.argv[1], sys.argv[2]
    rs = list(rows(path, kind))
    if kind != "wyoming":
        rs = [x for x in rs if x["cls"] == "locked"]
    print("%s pairs %d, IDs %d" % (kind, len(rs), len({x["id"] for x in rs})))
    table(rs, "cat", "By secMark step")
    table(rs, "dcnt", "By msgCnt step")
    for x in rs:
        x["hsb"] = "none" if x["hs"] is None else "|d|<=5" if abs(x["hs"]) <= 5 else "5<|d|<=20" if abs(x["hs"]) <= 20 \
            else "20<|d|<=50" if abs(x["hs"]) <= 50 else ">50"
        x["phs"] = "%02d->%02d" % (round(x["ph1"] / 50) * 50 % 100, round(x["ph2"] / 50) * 50 % 100)
    table(rs, "hsb", "By header-minus-secMark dt (ms)")
    table(rs, "phs", "By grid of the two secMarks (:00 / :50)")
    table(rs, "id", "By device (temporary ID)")
    r = [x["r"] for x in rs]
    rg = [x["rg"] for x in rs]
    nh = [x["r"] for x in rs if cat(x) == "100"]
    print("\nCounterfactuals (cm p50/p90/p99; KS against all pairs as published):")
    for name, v in (("as published (secMark dt)", r), ("grid dt (hop = stamp only)", rg), ("100 ms pairs only", nh)):
        a = q([y * 100 for y in v])
        print("| %s | %d | %.2f / %.2f / %.1f | KS vs published %.3f |" % (name, len(v), a[0], a[1], a[2], ks(v, r)))
    hop = [x for x in rs if cat(x) == "hop 50/150"]
    if hop:
        sg = collections.Counter(("+" if x["a"] > 0 else "-") + ("50" if x["step"] < 100 else "150") for x in hop)
        print("hop along-sign counts:", dict(sg))
        print("hop |along| / (v*50ms) p50/p90: %.2f / %.2f" % tuple(q([abs(x["a"]) / (x["v"] * 0.05) for x in hop], (0.5, 0.9))))
    t = sorted(x["tau"] for x in rs if cat(x) == "100")
    print("100 ms pairs signed tau ms p1/p10/p50/p90/p99: %s" % " / ".join(
        "%.2f" % t[int(f * (len(t) - 1))] for f in (0.01, 0.1, 0.5, 0.9, 0.99)))
    for x in rs:
        x["vb"] = "10-20" if x["v"] < 20 else "20-27" if x["v"] < 27 else "27-31" if x["v"] < 31 else ">=31"
    table(rs, "vb", "By speed v1 (m/s): timing error predicts tau flat, along growing with v")
    # lag-1: consecutive pairs share their middle BSM. A per-fix time (or position) error e_k gives
    # along_k = e_{k+1} - e_k, so corr(along_k, along_{k+1}) = -0.5; independent per-interval
    # errors (speed noise) give 0; a slow drift gives > 0.
    for lab, sel in (("all", lambda x: True), ("100 ms both", lambda x: cat(x) == "100")):
        xs, ys = [], []
        for u, w in zip(rs, rs[1:]):
            if u["id"] == w["id"] and w["i"] == u["i"] + 1 and sel(u) and sel(w):
                xs.append(u["a"])
                ys.append(w["a"])
        mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
        cv = sum((a - mx) * (b - my) for a, b in zip(xs, ys))
        sx = math.sqrt(sum((a - mx) ** 2 for a in xs))
        sy = math.sqrt(sum((b - my) ** 2 for b in ys))
        tail = [(a, b) for a, b in zip(xs, ys) if abs(a) > 0.05]
        opp = sum(a * b < 0 and abs(b) > 0.5 * abs(a) for a, b in tail) / max(1, len(tail))
        print("lag-1 along corr (%s): %.3f over %d; after |along|>5 cm the next is opposite (>half size) %.3f of %d" % (
            lab, cv / (sx * sy), len(xs), opp, len(tail)))
    # the transmitted speed / heading: white output noise gives lag-1 corr(dv_k, dv_k+1) = -0.5
    def corr(xs, ys):
        mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
        return sum((a - mx) * (b - my) for a, b in zip(xs, ys)) / math.sqrt(
            sum((a - mx) ** 2 for a in xs) * sum((b - my) ** 2 for b in ys))
    for f in ("dv", "dh"):
        pr = [(u[f], w[f]) for u, w in zip(rs, rs[1:]) if u["id"] == w["id"] and w["i"] == u["i"] + 1
              and cat(u) == "100" and cat(w) == "100" and u[f] is not None and w[f] is not None]
        a = q([abs(x) for x, _ in pr], (0.5, 0.9, 0.99))
        print("%s per 100 ms |p50/p90/p99| %.3f / %.3f / %.3f; lag-1 corr %.3f over %d" % (
            f, a[0], a[1], a[2], corr([x for x, _ in pr], [y for _, y in pr]), len(pr)))
    pr = [(x["a"], x["dv"] * x["step"] / 1000) for x in rs if cat(x) == "100" and x["dv"] is not None]
    xs, ys = [b for _, b in pr], [a for a, _ in pr]
    mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
    slope = sum((a - mx) * (b - my) for a, b in zip(xs, ys)) / sum((a - mx) ** 2 for a in xs)
    print("along on dv*dt: slope %.3f, corr %.3f (constant-accel motion predicts slope 0.5)" % (slope, corr(xs, ys)))
    big = [x for x in rs if x["r"] > 0.05]
    print("pairs with r > 5 cm: %d (%.3f); of them hop %.3f, |tau|>1ms %.3f, |cross|>|along| %.3f" % (
        len(big), len(big) / len(rs), sum(cat(x) == "hop 50/150" for x in big) / max(1, len(big)),
        sum(abs(x["tau"]) > 1 for x in big) / max(1, len(big)), sum(abs(x["c"]) > abs(x["a"]) for x in big) / max(1, len(big))))


if __name__ == "__main__":
    main()
