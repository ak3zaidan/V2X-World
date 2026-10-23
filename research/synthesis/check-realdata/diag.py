"""Diagnostics for artefacts: displacement/(v*dt) ratio, repeated positions, along/cross-track split,
repeated-position runs (GNSS update rate below BSM rate). Usage: python3 -I diag.py <data_dir>"""
import math, sys, os
from collections import defaultdict, Counter
sys.argv = [sys.argv[0], sys.argv[1]]
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'mbr_residual.py')).read()
src = src.split('\nfor name, fn in')[0]
g = {}
exec(compile(src, 'mbr_residual', 'exec'), g)


def diag(name, recs):
    by = defaultdict(list)
    for r in recs:
        by[r[0]].append(r)
    ratios, along, cross, rep, mov, runs = [], [], [], 0, 0, Counter()
    big = []
    for vid, rs in by.items():
        rs.sort(key=lambda r: (r[1] if r[1] is not None else 0))
        run = 0
        for a, b in zip(rs, rs[1:]):
            tgap = None if a[1] is None or b[1] is None else b[1] - a[1]
            dt = g['secmark_dt'](a[2], b[2], tgap) if a[2] is not None else tgap
            if dt is None or dt <= 0 or dt > 0.5 or a[5] is None or a[6] is None or a[5] < 5:
                continue
            dx, dy = b[3] - a[3], b[4] - a[4]
            d = math.hypot(dx, dy)
            mov += 1
            if d < 0.01:
                rep += 1; run += 1
            else:
                if run:
                    runs[run] += 1
                run = 0
            ratios.append(d / (a[5] * dt))
            h = math.radians(a[6]); ux, uy = math.sin(h), math.cos(h)
            px, py = a[5] * dt * ux, a[5] * dt * uy
            ex, ey = dx - px, dy - py
            along.append(ex * ux + ey * uy); cross.append(-ex * uy + ey * ux)
            if math.hypot(ex, ey) > 1:
                big.append((vid, round(dt, 3), round(a[5], 2), round(d, 2), round(a[6], 1), round(math.degrees(math.atan2(dx, dy)) % 360, 1)))
    P = g['pct']
    print(f"\n=== {name}: moving pairs (v>=5 m/s, dt<=0.5) {mov}")
    if not mov:
        return
    print(f" |dp|/(v dt): p10 {P(ratios,.1):.3f} p50 {P(ratios,.5):.3f} p90 {P(ratios,.9):.3f}")
    print(f" repeated position (<1 cm) while v>=5: {rep} ({100*rep/mov:.1f}%); run lengths {dict(sorted(runs.items())[:8])}")
    aa = [abs(x) for x in along]; cc = [abs(x) for x in cross]
    print(f" |along| p50 {P(aa,.5):.3f} p99 {P(aa,.99):.3f}; |cross| p50 {P(cc,.5):.3f} p99 {P(cc,.99):.3f}")
    print(' sample >1 m (id, dt, v, |dp|, heading, bearing of dp):', big[:6])


for name, fn in (('Tampa', g['load_tampa']), ('Wyoming', g['load_wyoming']), ('Orem', g['load_orem']),
                 ('Pooled', g['load_pooled']), ('NYC', g['load_nyc'])):
    diag(name, fn()[0])
