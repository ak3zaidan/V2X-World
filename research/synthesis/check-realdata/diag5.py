"""Tampa: split IDs by whether secMark is locked to 50 ms GNSS epochs; trigger rate per class.
Usage: python3 -I diag5.py <dir>"""
import sys, os, math
from collections import defaultdict
d0 = sys.argv[1]
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'mbr_residual.py')).read().split('\nfor name, fn in')[0]
g = {}; sys.argv = ['x', d0]; exec(compile(src, 'm', 'exec'), g)
for ds in ('tampa', 'orem', 'wyoming'):
    recs = g['load_' + ds]()[0]
    by = defaultdict(list)
    for r in recs: by[r[0]].append(r)
    cls = {}
    for vid, rs in by.items():
        sm = [int(r[2]) for r in rs if r[2] is not None]
        locked = sum(1 for s in sm if min(s % 50, 50 - s % 50) <= 1) / max(1, len(sm))
        cls[vid] = 'locked' if locked > 0.8 else ('free' if locked < 0.3 else 'mixed')
    stat = defaultdict(lambda: [0, 0, 0.0, set()])
    for vid, rs in by.items():
        rs.sort(key=lambda r: r[1])
        for a, b in zip(rs, rs[1:]):
            tg = None if a[1] is None or b[1] is None else b[1] - a[1]
            dt = g['secmark_dt'](a[2], b[2], tg)
            if dt is None or dt <= 0 or dt > 0.5 or a[5] is None or a[6] is None or a[5] < 10: continue
            h = math.radians(a[6])
            res = math.hypot(b[3]-a[3]-a[5]*dt*math.sin(h), b[4]-a[4]-a[5]*dt*math.cos(h))
            s = stat[cls[vid]]; s[0] += 1; s[1] += res > 1; s[2] += dt; s[3].add(vid)
    for k, (n, t, T, ids) in sorted(stat.items()):
        print(f"{ds} {k}: IDs {len(ids)}, pairs at v>=10 m/s {n}, >1 m {t} ({100*t/n:.2f}%), {t/(T/60):.1f} per vehicle-min")
