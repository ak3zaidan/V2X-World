"""Trigger rate per observed vehicle-minute and concentration across IDs; Wyoming dt from the 1609.2
generation time instead of secMark. Usage: python3 -I diag3.py <dir>"""
import math, sys, os
from collections import defaultdict
d0 = sys.argv[1]
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'mbr_residual.py')).read().split('\nfor name, fn in')[0]
g = {}; sys.argv = ['x', d0]; exec(compile(src, 'm', 'exec'), g)
for name in ('tampa', 'wyoming', 'orem', 'nyc'):
    recs = g['load_' + name]()[0]
    by = defaultdict(list)
    for r in recs: by[r[0]].append(r)
    T = 0.0; per = defaultdict(int); fast_T = 0.0; fast_trig = 0; wy_alt = []
    for vid, rs in by.items():
        rs.sort(key=lambda r: (r[1] if r[1] is not None else 0))
        for a, b in zip(rs, rs[1:]):
            tgap = None if a[1] is None or b[1] is None else b[1] - a[1]
            dt = g['secmark_dt'](a[2], b[2], tgap) if a[2] is not None else tgap
            if dt is None or dt <= 0 or dt > 0.5 or a[5] is None or a[6] is None: continue
            h = math.radians(a[6])
            res = math.hypot(b[3]-a[3]-a[5]*dt*math.sin(h), b[4]-a[4]-a[5]*dt*math.cos(h))
            T += dt
            if a[5] >= 10: fast_T += dt
            if res > 1:
                per[vid] += 1
                if a[5] >= 10: fast_trig += 1
            if name == 'wyoming' and tgap and 0 < tgap <= 0.5:
                wy_alt.append(math.hypot(b[3]-a[3]-a[5]*tgap*math.sin(h), b[4]-a[4]-a[5]*tgap*math.cos(h)))
    tot = sum(per.values()); ids = sorted(per.values(), reverse=True)
    top = sum(ids[:max(1, len(by)//10)])
    print(f"{name}: observed {T/60:.1f} vehicle-min in judged pairs; triggers {tot} = {tot/(T/60) if T else 0:.1f} per vehicle-min; "
          f"at v>=10 m/s: {fast_trig/(fast_T/60) if fast_T else 0:.1f} per vehicle-min over {fast_T/60:.1f} min; "
          f"top-10% IDs hold {100*top/tot if tot else 0:.0f}% of triggers; per-ID max {ids[:5]}")
    if wy_alt:
        print(f"  wyoming dt from 1609.2 generatedAt: pairs {len(wy_alt)}, p99 {g['pct'](wy_alt,.99):.3f} m, >1 m {sum(1 for x in wy_alt if x>1)}")
