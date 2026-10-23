"""For RandomPosition triggers (>1 m, dt<=0.5): share explained by an epoch slip (|dp|/(v dt) off by
>= 0.4, i.e. stale or doubled fix), msgCnt delta, cross-track share. Usage: python3 -I diag2.py <dir>"""
import math, sys, os
from collections import defaultdict, Counter
d0 = sys.argv[1]
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'mbr_residual.py')).read().split('\nfor name, fn in')[0]
g = {}; sys.argv = ['x', d0]; exec(compile(src, 'm', 'exec'), g)
for name in ('tampa', 'wyoming', 'orem', 'pooled', 'nyc'):
    recs = g['load_' + name]()[0]
    by = defaultdict(list)
    for r in recs: by[r[0]].append(r)
    trig = 0; slip = 0; cross_dom = 0; dcnt = Counter(); stat = []; rpos_after_freeze = 0
    for vid, rs in by.items():
        rs.sort(key=lambda r: (r[1] if r[1] is not None else 0))
        prev_frozen = False
        for a, b in zip(rs, rs[1:]):
            tgap = None if a[1] is None or b[1] is None else b[1] - a[1]
            dt = g['secmark_dt'](a[2], b[2], tgap) if a[2] is not None else tgap
            if dt is None or dt <= 0 or dt > 0.5 or a[5] is None or a[6] is None: prev_frozen=False; continue
            h = math.radians(a[6]); ux, uy = math.sin(h), math.cos(h)
            dx, dy = b[3]-a[3], b[4]-a[4]
            ex, ey = dx - a[5]*dt*ux, dy - a[5]*dt*uy
            res = math.hypot(ex, ey)
            frozen = math.hypot(dx, dy) < 0.01 and a[5] >= 0.02
            if res > 1:
                trig += 1
                if prev_frozen: rpos_after_freeze += 1
                if a[5] > 0.5:
                    ratio = math.hypot(dx, dy)/(a[5]*dt)
                    if abs(ratio-1) >= 0.4: slip += 1
                else:
                    stat.append((vid, round(dt,3), round(a[5],2), round(math.hypot(dx,dy),2)))
                if abs(-ex*uy+ey*ux) > abs(ex*ux+ey*uy): cross_dom += 1
                if a[8] is not None and b[8] is not None: dcnt[int((b[8]-a[8]) % 128)] += 1
            prev_frozen = frozen
    if trig:
        print(f"{name}: triggers {trig}; epoch-slip-like (|ratio-1|>=0.4, v>0.5) {slip} ({100*slip/trig:.0f}%); "
              f"right after a frozen pair {rpos_after_freeze}; cross-track dominant {cross_dom}; msgCnt delta {dict(dcnt.most_common(5))}; "
              f"stationary(v<=0.5) {len(stat)} e.g. {stat[:4]}")
    else:
        print(f"{name}: no triggers")
