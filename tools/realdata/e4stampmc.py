"""Which stamp model reproduces Wyoming's P1-E4 residual body? (decomposition, not a fit)

  python3 -I tools/realdata/e4stampmc.py <wyoming.csv> <tampa.csv>

Pairs: msgCnt +1, secMark dt within 3 ms of 50/100/150 ms, v1 >= 10 m/s (resid.py's set
minus the 1.2 % multi-interval / irregular pairs). Every synthetic pair keeps a real Wyoming
pair's speed and nominal dt and draws three parts:
  base   the (along, cross) residual of a random Wyoming pair whose two stamps have the same
         offset (stamp term 0) and whose residual is <= 5 cm (quantisation + small noise)
  step   with probability P_STEP the (along, cross) of a random Wyoming 0 -> -1 pair with its
         +1 ms x v stamp term removed and residual > 5 cm (the persistent position steps)
  stamp  model EPOCH: position on the fix epoch, secMark = epoch + o, o in {-1, 0} a Markov
         chain (stay(-1), stay(0)) -> along -= v * e * 1 ms, e = o2 - o1.
         model INSTANT (engine v3, bsm-timing-cvpilot.yaml): instant j ~ N(0.286, 0.254) ms,
         position at the instant, secMark = epoch + floor(j) -> along += v * ((j2 - j1) - e).
The chain is taken from Tampa's locked IDs (a fit set) and, for reference, Wyoming's own.
The base and step pools come from Wyoming itself, so the KS isolates the stamp model only.
"""
import bisect
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mbrchecks as mc  # noqa: E402
import normalise  # noqa: E402
import pairing  # noqa: E402

N = 20000


def pairs(path, kind, locked_only):
    return pairs_from(normalise.stream_ids(path, kind), locked_only)


def pairs_from(ids, locked_only):
    """pairs() on any (id, [Bsm]) stream, e.g. an engine run's (paper1/resid.engine_ids)."""
    out = []
    for _, bsms in ids:
        bs = pairing.dedupe_sort(bsms)
        sms = [b.secmark for b in bs if b.secmark is not None]
        if not sms:
            continue
        if locked_only and sum(min(s % 50, 50 - s % 50) <= 1 for s in sms) / len(sms) < 0.7:
            continue
        for p in pairing.pairs(bs):
            dt = p.dt[0]
            if p.dcnt != 1 or dt is None or p.v1 is None or p.v1 < 10 or p.h1 is None:
                continue
            ms = round(dt * 1000)
            nom = min((50, 100, 150), key=lambda n: abs(ms - n))
            if abs(ms - nom) > 3:
                continue
            o1, o2 = (p.sm1 + 25) % 50 - 25, (p.sm2 + 25) % 50 - 25
            ex, ey = mc.residual(p, dt)
            al, cr = mc.along_cross(p, ex, ey)
            out.append({"v": p.v1, "nom": nom, "o1": o1, "o2": o2, "al": al, "cr": cr, "r": math.hypot(al, cr)})
    return out


def chain(ps):
    c = {(a, b): 0 for a in (-1, 0) for b in (-1, 0)}
    for x in ps:
        if (x["o1"], x["o2"]) in c:
            c[(x["o1"], x["o2"])] += 1
    s_m = c[(-1, -1)] / max(1, c[(-1, -1)] + c[(-1, 0)])
    s_0 = c[(0, 0)] / max(1, c[(0, 0)] + c[(0, -1)])
    return s_m, s_0


def ks(a, b):
    a, b = sorted(a), sorted(b)
    return max(abs(bisect.bisect_right(a, x) / len(a) - bisect.bisect_right(b, x) / len(b)) for x in a + b)


def q(xs):
    s = sorted(xs)
    return " / ".join("%.2f" % (100 * s[int(f * (len(s) - 1))]) for f in (0.25, 0.5, 0.75, 0.9, 0.99))


def main():
    wy = pairs(sys.argv[1], "wyoming", False)
    ta = pairs(sys.argv[2], "tampa", True)
    base = [(x["al"], x["cr"]) for x in wy if x["o1"] == x["o2"] and x["r"] <= 0.05]
    # 0 -> -1: e = -1, the stamp term is -v*e*1ms = +v*1ms; remove it
    step = [(x["al"] - x["v"] * 0.001, x["cr"]) for x in wy if (x["o1"], x["o2"]) == (0, -1) and x["r"] > 0.05]
    p_step = len(step) / len(wy)
    real = [x["r"] for x in wy]
    print("Wyoming pairs %d; base pool %d; step pool %d; P_STEP %.3f; Tampa locked pairs %d" % (
        len(wy), len(base), len(step), p_step, len(ta)))
    print("Wyoming published residual cm p25/p50/p75/p90/p99: %s" % q(real))
    for name, ps in (("Wyoming", wy), ("Tampa locked", ta)):
        tr = [x for x in ps if (x["o1"], x["o2"]) == (0, -1)]
        st = [x for x in tr if x["r"] > 0.05]
        sa = [abs(x["al"] - x["v"] * 0.001) for x in st]
        sc = [abs(x["cr"]) for x in st]
        po = sum(1 for x in ps if x["o2"] == -1) / len(ps)
        print("%s: P(o=-1) %.3f; 0->-1 pairs %d, of them r > 5 cm %.3f; other pairs r > 5 cm %.3f; "
              "step |along| cm p50/p90 %.1f/%.1f, |cross| p50/p90 %.1f/%.1f; v p50 %.1f m/s" % (
                  name, po, len(tr), len(st) / max(1, len(tr)),
                  sum(1 for x in ps if x["r"] > 0.05 and (x["o1"], x["o2"]) != (0, -1)) / max(1, len(ps) - len(tr)),
                  100 * sorted(sa)[len(sa) // 2], 100 * sorted(sa)[int(0.9 * len(sa))],
                  100 * sorted(sc)[len(sc) // 2], 100 * sorted(sc)[int(0.9 * len(sc))],
                  sorted(x["v"] for x in ps)[len(ps) // 2]))
    chains = {"Tampa locked": chain(ta), "Wyoming": chain(wy)}
    rng = random.Random(1)
    rows = []
    for model in ("EPOCH", "INSTANT", "NONE"):
        for cname, (s_m, s_0) in chains.items():
            if model != "EPOCH" and cname != "Tampa locked":
                continue
            for steps in (True, False):
                syn = []
                for _ in range(N):
                    w = rng.choice(wy)
                    v = w["v"]
                    al, cr = rng.choice(base)
                    if steps and rng.random() < p_step:
                        da, dc = rng.choice(step)
                        al, cr = al + da, cr + dc
                    if model == "EPOCH":
                        pm = (1 - s_0) / ((1 - s_0) + (1 - s_m))  # stationary P(o = -1)
                        o1 = -1 if rng.random() < pm else 0
                        o2 = (-1 if rng.random() < s_m else 0) if o1 == -1 else (0 if rng.random() < s_0 else -1)
                        al -= v * 0.001 * (o2 - o1)
                    elif model == "INSTANT":
                        j1, j2 = rng.gauss(0.286, 0.254), rng.gauss(0.286, 0.254)
                        e = math.floor(j2) - math.floor(j1)
                        al += v * 0.001 * ((j2 - j1) - e)
                    syn.append(math.hypot(al, cr))
                label = "%s%s, steps %s" % (model, "" if model != "EPOCH" else " (chain %s %.3f/%.3f)" % (cname, s_m, s_0),
                                           "on" if steps else "off")
                rows.append((label, ks(syn, real), q(syn)))
    print("| model | KS vs Wyoming | residual cm p25/p50/p75/p90/p99 |")
    print("|---|---|---|")
    for r in rows:
        print("| %s | %.3f | %s |" % r)


if __name__ == "__main__":
    main()
