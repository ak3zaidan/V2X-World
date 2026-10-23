"""Wyoming: compare the 1609.2 header time (metadata_generatedat) with secMark. Usage: python3 -I diag4.py <dir>"""
import sys, os, math
from collections import defaultdict, Counter
d0 = sys.argv[1]
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'mbr_residual.py')).read().split('\nfor name, fn in')[0]
g = {}; sys.argv = ['x', d0]; exec(compile(src, 'm', 'exec'), g)
recs = g['load_wyoming']()[0]
off = []; dd = []; gaps = Counter()
by = defaultdict(list)
for r in recs:
    by[r[0]].append(r)
    if r[1] is not None and r[2] is not None:
        ms_in_min = (r[1] % 60) * 1000
        off.append(((ms_in_min - r[2] + 30000) % 60000) - 30000)
for vid, rs in by.items():
    rs.sort(key=lambda r: r[1])
    for a, b in zip(rs, rs[1:]):
        tg = b[1] - a[1]; sd = g['secmark_dt'](a[2], b[2], tg)
        if sd is not None and sd <= 0.5:
            dd.append(round((tg - sd) * 1000))
            gaps[round(tg * 1000, -1)] += 1
P = g['pct']
print('header time minus secMark (ms within minute): p1 %.0f p50 %.0f p99 %.0f' % (P(off, .01), P(off, .5), P(off, .99)))
print('per-pair (header dt - secMark dt) ms: p1 %.0f p10 %.0f p50 %.0f p90 %.0f p99 %.0f' % tuple(P(dd, q) for q in (.01, .1, .5, .9, .99)))
print('header-time gaps (ms, rounded 10) most common:', gaps.most_common(8))
