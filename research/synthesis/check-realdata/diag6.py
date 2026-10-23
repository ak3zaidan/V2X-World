"""Speed range, ID lifetimes, and long-gap share per dataset. Usage: python3 -I diag6.py <dir>"""
import sys, os
from collections import defaultdict
d0 = sys.argv[1]
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'mbr_residual.py')).read().split('\nfor name, fn in')[0]
g = {}; sys.argv = ['x', d0]; exec(compile(src, 'm', 'exec'), g)
P = g['pct']
for ds in ('tampa', 'wyoming', 'orem', 'pooled'):
    recs = g['load_' + ds]()[0]
    by = defaultdict(list)
    for r in recs: by[r[0]].append(r)
    sp = [r[5] for r in recs if r[5] is not None]
    life = [max(r[1] for r in rs) - min(r[1] for r in rs) for rs in by.values() if len(rs) > 20]
    rate = [len(rs) / (max(r[1] for r in rs) - min(r[1] for r in rs) + 1e-9) for rs in by.values() if len(rs) > 20]
    print(f"{ds}: speed min {min(sp):.2f} p5 {P(sp,.05):.2f} p50 {P(sp,.5):.2f} max {max(sp):.1f} m/s; ID span p50 {P(life,.5):.0f} s max {max(life):.0f} s; BSMs/s per ID p50 {P(rate,.5):.2f}")
