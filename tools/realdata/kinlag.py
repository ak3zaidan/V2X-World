"""Latency of the transmitted speed and heading behind the transmitted position (P1-E4 tail).

  python3 -I tools/realdata/kinlag.py <kind> <csv> [<csv> ...]     kind: wyoming | tampa | orem

Runs: maximal stretches of one temporary ID whose consecutive BSMs are msgCnt +1 with a
secMark dt of 40-160 ms (so :00/:50 grid hops and free-clock periods stay in), v >= 10 m/s
throughout, >= 3 s long. Inside a run, time is the cumulative secMark dt and S(t) the
cumulative path length. The position-derived speed at t is (S(t+W/2) - S(t-W/2)) / W with
W = 1 s (linear interpolation; symmetric, so it adds no lag, and it averages the 1e-7 deg
position quantisation down to about 0.01 m/s); the bearing is that of the chord over the same
window. The latency L is the shift that minimises the mean squared difference between the
speed reported at t and the position-derived speed at t - L (grid -0.5..+2.0 s, 25 ms),
pooled and per ID (IDs with >= 20 s of runs). L > 0: the reported speed is older than the
position. A latency L makes the RandomPosition along-track residual about
a * (L + dt/2) * dt for a vehicle accelerating at a; the published fit (pipeline.py) removes
only the 0.5 * dv * dt part. Also printed: the RandomPosition residual quantiles of the run
intervals as sent and with the reported speed / heading taken L later (latency removed).
"""
import bisect
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import normalise  # noqa: E402
import pairing  # noqa: E402

W = 1.0
GRID = [x * 0.025 for x in range(-20, 81)]


def runs(bs):
    cur = [bs[0]] if bs else []
    for a, b in zip(bs, bs[1:]):
        ok = (a.msgcnt is not None and b.msgcnt is not None and (b.msgcnt - a.msgcnt) % 128 == 1
              and a.secmark is not None and b.secmark is not None
              and 40 <= (b.secmark - a.secmark) % 60000 <= 160
              and b.speed is not None and b.speed >= 10 and b.heading is not None
              and a.speed is not None and a.speed >= 10 and a.heading is not None)
        if ok:
            cur.append(b)
        else:
            if len(cur) >= 31:
                yield cur
            cur = [b]
    if len(cur) >= 31:
        yield cur


class Run:
    def __init__(self, r):
        self.t, self.S, self.x, self.y = [0.0], [0.0], [0.0], [0.0]
        self.v = [r[0].speed]
        self.h = [r[0].heading]
        self.d = []
        for a, b in zip(r, r[1:]):
            dx, dy = pairing.displacement(a, b)
            dt = ((b.secmark - a.secmark) % 60000) / 1000.0
            self.t.append(self.t[-1] + dt)
            self.S.append(self.S[-1] + math.hypot(dx, dy))
            self.x.append(self.x[-1] + dx)
            self.y.append(self.y[-1] + dy)
            self.v.append(b.speed)
            self.h.append(b.heading)
            self.d.append((dt, dx, dy))

    def interp(self, arr, t):
        i = bisect.bisect_right(self.t, t) - 1
        if i < 0 or i >= len(self.t) - 1:
            return None
        f = (t - self.t[i]) / (self.t[i + 1] - self.t[i])
        return arr[i] + f * (arr[i + 1] - arr[i])

    def pos_kin(self, t):
        a = [self.interp(z, t - W / 2) for z in (self.S, self.x, self.y)]
        b = [self.interp(z, t + W / 2) for z in (self.S, self.x, self.y)]
        if a[0] is None or b[0] is None:
            return None
        return (b[0] - a[0]) / W, math.degrees(math.atan2(b[1] - a[1], b[2] - a[2])) % 360

    def at(self, arr, t, ang=False):
        i = bisect.bisect_right(self.t, t) - 1
        if i < 0 or i >= len(self.t) - 1:
            return None
        f = (t - self.t[i]) / (self.t[i + 1] - self.t[i])
        if ang:
            return (arr[i] + f * dang(arr[i + 1], arr[i])) % 360
        return arr[i] + f * (arr[i + 1] - arr[i])


def dang(a, b):
    return (a - b + 180) % 360 - 180


class Acc:
    def __init__(self):
        self.sv = [0.0] * len(GRID)
        self.sh = [0.0] * len(GRID)
        self.n = [0] * len(GRID)
        self.secs = 0.0

    def add(self, run):
        self.secs += run.t[-1]
        for k, tk in enumerate(run.t):
            for g, L in enumerate(GRID):
                pk = run.pos_kin(tk - L)
                if pk is None:
                    continue
                self.sv[g] += (run.v[k] - pk[0]) ** 2
                self.sh[g] += dang(run.h[k], pk[1]) ** 2
                self.n[g] += 1

    def best(self):
        ok = [g for g in range(len(GRID)) if self.n[g] >= 50]
        if not ok:
            return None
        gv = min(ok, key=lambda g: self.sv[g] / self.n[g])
        gh = min(ok, key=lambda g: self.sh[g] / self.n[g])
        z = GRID.index(0.0)
        return (GRID[gv], (self.sv[gv] / self.n[gv]) ** .5, (self.sv[z] / self.n[z]) ** .5,
                GRID[gh], (self.sh[gh] / self.n[gh]) ** .5, (self.sh[z] / self.n[z]) ** .5)


def q(xs, fs=(0.5, 0.9, 0.99)):
    s = sorted(xs)
    return [s[min(len(s) - 1, int(f * len(s)))] for f in fs]


def resid(run, Lv, Lh):
    out = []
    for m, (dt, dx, dy) in enumerate(run.d):
        v = run.at(run.v, run.t[m] + Lv)
        h = run.at(run.h, run.t[m] + Lh, True)
        if v is None or h is None:
            continue
        hd = math.radians(h)
        out.append(math.hypot(dx - v * dt * math.sin(hd), dy - v * dt * math.cos(hd)))
    return out


def main():
    kind, paths = sys.argv[1], sys.argv[2:]
    pool, per, keep = Acc(), {}, []
    for path in paths:
        for k, bsms in normalise.stream_ids(path, kind):
            bs = pairing.dedupe_sort(bsms)
            for r in runs(bs):
                ru = Run(r)
                if ru.t[-1] < 3.0:
                    continue
                pool.add(ru)
                per.setdefault(k, Acc()).add(ru)
                if len(keep) < 3000:
                    keep.append(ru)
    b = pool.best()
    if b is None:
        print(kind, "no runs")
        return
    print("%s: %.0f s of runs, %d IDs" % (kind, pool.secs, len(per)))
    print("speed latency %.3f s (rms diff %.3f m/s; at L=0 %.3f)" % b[:3])
    print("heading latency %.3f s (rms diff %.3f deg; at L=0 %.3f)" % b[3:])
    ids = []
    for k, a in per.items():
        if a.secs >= 20:
            bb = a.best()
            if bb:
                ids.append((k, bb[0], bb[3], a.secs))
    if ids:
        fs = (0.1, 0.25, 0.5, 0.75, 0.9)
        print("per-ID (%d IDs >= 20 s) speed latency p10/p25/p50/p75/p90 s: %s" % (
            len(ids), " / ".join("%.3f" % x for x in q([x[1] for x in ids], fs))))
        print("per-ID heading latency p10/p25/p50/p75/p90 s: %s" % " / ".join("%.3f" % x for x in q([x[2] for x in ids], fs)))
        if len(ids) <= 16:
            print("per-ID:", " ".join("%s:%.3f/%.3f(%.0fs)" % x for x in sorted(ids)))
    r0 = [x for ru in keep for x in resid(ru, 0.0, 0.0)]
    r1 = [x for ru in keep for x in resid(ru, b[0], b[3])]
    a0, a1 = q([x * 100 for x in r0]), q([x * 100 for x in r1])
    print("RandomPosition residual of run intervals, cm p50/p90/p99: as sent %.2f / %.2f / %.1f (n=%d); "
          "latency removed %.2f / %.2f / %.1f (n=%d)" % (a0[0], a0[1], a0[2], len(r0), a1[0], a1[1], a1[2], len(r1)))


if __name__ == "__main__":
    main()
