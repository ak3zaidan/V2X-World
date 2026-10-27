"""secMark stamp jitter against the position, and the position-correction coupling (P1-E4 body).

  python3 -I tools/realdata/stampjit.py <kind> <csv> [locked]

For consecutive msgCnt +1 pairs at v >= 10 m/s with secMark dt within 3 ms of 50 / 100 / 150 ms:
share of each whole-ms offset e = dt - nominal, and the mean along-track residual / v (ms)
per offset. A position taken at the stamped instant gives mean ~0 at every e; a position held
on the fix epoch while the stamp jitters gives mean ~ -e (the stamp says e ms more than the
position moved). Then the along-track residual vs the speed change over the same pair
(slope, corr), and its persistence (the share of |along| > 5 cm steps followed by an
opposite step of at least half the size: ~1 for a spike, ~0 for a step / walk).
"""
import collections
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mbrchecks as mc  # noqa: E402
import normalise  # noqa: E402
import pairing  # noqa: E402


def main():
    kind, path = sys.argv[1], sys.argv[2]
    only_locked = len(sys.argv) > 3
    st = collections.defaultdict(lambda: [0, 0.0, 0.0])
    tot = collections.Counter()
    offs = collections.Counter()  # (grid, offset ms) per BSM
    trans = collections.Counter()  # (o_k, o_k+1) over msgCnt +1 pairs
    r_pub, r_epoch, r_cls = [], [], collections.defaultdict(list)
    for k, bsms in normalise.stream_ids(path, kind):
        bs = pairing.dedupe_sort(bsms)
        sms = [b.secmark for b in bs if b.secmark is not None]
        if not sms:
            continue
        lk = sum(min(s % 50, 50 - s % 50) <= 1 for s in sms) / len(sms)
        if only_locked and lk < 0.7:
            continue
        for b in bs:
            if b.secmark is not None:
                o = (b.secmark + 25) % 50 - 25
                if abs(o) <= 3:
                    offs[(":00" if (b.secmark + 25) % 100 < 50 else ":50", o)] += 1
        for p in pairing.pairs(bs):
            dt = p.dt[0]
            if p.dcnt == 1 and p.sm1 is not None and p.sm2 is not None:
                o1, o2 = (p.sm1 + 25) % 50 - 25, (p.sm2 + 25) % 50 - 25
                if abs(o1) <= 3 and abs(o2) <= 3:
                    trans[(o1, o2)] += 1
            if p.dcnt != 1 or dt is None or p.v1 is None or p.v1 < 10 or p.h1 is None:
                continue
            ms = round(dt * 1000)
            nom = min((50, 100, 150), key=lambda n: abs(ms - n))
            e = ms - nom
            if abs(e) > 3:
                continue
            ex, ey = mc.residual(p, dt)
            al, _ = mc.along_cross(p, ex, ey)
            tau = 1000 * al / p.v1
            if nom == 100 or nom in (50, 150):
                _, r0, _ = mc.run(p, dt)
                _, r1, _ = mc.run(p, nom / 1000.0)
                r_pub.append(r0)
                r_epoch.append(r1)
                r_cls[e].append(r0)
                r_cls["n:%d" % nom].append(r0)
                o1, o2 = (p.sm1 + 25) % 50 - 25, (p.sm2 + 25) % 50 - 25
                r_cls["o%+d>%+d" % (o1, o2)].append(r0)
                if r0 > 0.05:
                    r_cls["big:o%+d>%+d" % (o1, o2)].append(r0)
            s = st[(nom, e)]
            s[0] += 1
            s[1] += tau
            s[2] += tau * tau
            tot[nom] += 1
    print("| nominal dt | offset e ms | pairs | share of nominal | mean along/v ms | sd along/v ms |")
    print("|---|---|---|---|---|---|")
    for (nom, e) in sorted(st):
        n, s1, s2 = st[(nom, e)]
        if n < 5:
            continue
        m = s1 / n
        print("| %d | %+d | %d | %.3f | %+.2f | %.2f |" % (nom, e, n, n / tot[nom], m, math.sqrt(max(0, s2 / n - m * m))))
    print("\nsecMark offset from the nearest 50 ms epoch, per BSM, by grid:")
    for g in (":00", ":50"):
        n = sum(v for (gg, _), v in offs.items() if gg == g)
        if n:
            print(" %s n=%d " % (g, n) + " ".join("%+d:%.3f" % (o, offs[(g, o)] / n) for o in range(-3, 4) if offs[(g, o)]))
    print("offset transition o_k -> o_k+1 (msgCnt +1 pairs), row-normalised:")
    for o1 in range(-3, 4):
        n = sum(trans[(o1, o2)] for o2 in range(-3, 4))
        if n >= 20:
            print(" from %+d (n=%d): " % (o1, n) + " ".join("%+d:%.3f" % (o2, trans[(o1, o2)] / n) for o2 in range(-3, 4) if trans[(o1, o2)]))
    def qs(x):
        x = sorted(x)
        if not x:
            return "none"
        return " / ".join("%.2f" % (100 * x[min(len(x) - 1, int(f * len(x)))]) for f in (0.5, 0.75, 0.9, 0.99))
    import bisect
    def ks(a, b):
        a, b = sorted(a), sorted(b)
        return max(abs(bisect.bisect_right(a, x) / len(a) - bisect.bisect_right(b, x) / len(b)) for x in a + b)
    print("RandomPosition residual cm p50/p75/p90/p99 (msgCnt +1, |e| <= 3):")
    print(" secMark dt (published): %s (n=%d)" % (qs(r_pub), len(r_pub)))
    if r_pub:
        print(" nominal-epoch dt      : %s; KS vs published %.3f" % (qs(r_epoch), ks(r_pub, r_epoch)))
    for e in sorted(r_cls, key=str):
        if len(r_cls[e]) >= 20:
            print(" published, offset %s: %s (n=%d)" % (e if isinstance(e, str) else "%+d" % e, qs(r_cls[e]), len(r_cls[e])))


if __name__ == "__main__":
    main()
