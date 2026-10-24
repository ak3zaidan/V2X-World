"""Stages 3-5 over one dataset, streamed per temporary ID.

For each ID: de-duplicate and pair (stage 3), run every check under every time base and gap
limit (stage 4), classify the ID by secMark phase (stage 5) and collect the timing
statistics the engine's BSM timing model is fitted to. Only per-ID accumulators are kept.
"""
import math
from collections import Counter

import devclass
import mbrchecks as mc
import normalise
import pairing

NB = len(mc.SPEED_BANDS)
NL = len(mc.GAP_LIMITS_S)
NC = len(mc.CHECKS)
# accumulator layout per (base, limit, band): [pairs, seconds, *fired(NC), slip_zero, slip_double, cross_dom]
ACC_LEN = 2 + NC + 3
I_SLIPZ, I_SLIPD, I_CROSS = 2 + NC, 3 + NC, 4 + NC
RES_EDGES = [0.0] + [10 ** (k / 10.0) for k in range(-30, 21)]  # 1 mm .. 100 m, log bins


def res_bin(r):
    lo, hi = 0, len(RES_EDGES)
    while lo < hi - 1:
        mid = (lo + hi) // 2
        if RES_EDGES[mid] <= r:
            lo = mid
        else:
            hi = mid
    return lo


class IdResult:
    __slots__ = ("key", "frac", "nsm", "nbsm", "raw", "acc", "phase100", "wrapped", "at_risk", "ent_frozen",
                 "ent_slip", "slip_sizes", "hdr_off_ms", "period_excess_ms", "res_hist", "dt_near100", "dt_all", "cnt_ok", "cnt_n")

    def __init__(self, key):
        self.key = key
        self.acc = {}
        self.phase100 = Counter()
        self.wrapped = Counter()
        self.at_risk = self.ent_frozen = self.ent_slip = 0
        self.slip_sizes = Counter()
        self.hdr_off_ms = Counter()
        self.period_excess_ms = Counter()
        self.res_hist = Counter()
        self.dt_near100 = self.dt_all = self.cnt_ok = self.cnt_n = 0


def slip_size(p, dt):
    """Displacement in fix intervals minus the expected number, for a moving pair."""
    vbar = 0.5 * (p.v1 + p.v2)
    k_obs = round(math.hypot(p.dx, p.dy) / (vbar * mc.FIX_INTERVAL_S))
    return k_obs - round(dt / mc.FIX_INTERVAL_S)


def analyse_id(key, bsms, xy=False, nyc=False, gps_drift=mc.GPS_DRIFT_M):
    res = IdResult(key)
    res.raw = len(bsms)
    bs = pairing.dedupe_sort(bsms)
    res.nbsm = len(bs)
    sms = [b.secmark for b in bs if b.secmark is not None]
    res.frac, res.nsm = devclass.locked_fraction(sms)
    for b in bs:
        if b.t_hdr is not None and b.secmark is not None:
            # header time minus the BSM's own timestamp (`order` is the unwrapped secMark time)
            res.hdr_off_ms[int(round((b.t_hdr - b.order) * 1000.0))] += 1
    for s in sms:
        res.phase100[devclass.phase100(s)] += 1
        res.wrapped[devclass.phase_wrapped(s)] += 1
    prev_state = None  # entry estimator: state of the previous pair ('n', 'a' or None)
    for p in pairing.pairs(bs, xy, nyc):
        dsm = p.dt[0]
        if dsm is not None and dsm > 0:
            res.dt_all += 1
            res.dt_near100 += 0.09 <= dsm <= 0.11
            if p.dcnt is not None:
                res.cnt_n += 1
                res.cnt_ok += p.dcnt == 1
        if dsm is not None and 0.05 < dsm < 0.15:
            res.period_excess_ms[int(round(dsm * 1000.0)) - 100] += 1
        # timing-model estimator on single-interval moving pairs (secMark base)
        state = None
        if (dsm is not None and 0.05 < dsm < 0.15 and p.v1 is not None and p.v2 is not None
                and p.v1 >= 5.0 and p.v2 >= 5.0):
            frozen = math.hypot(p.dx, p.dy) < mc.POS_RES_M
            s = 0 if frozen else slip_size(p, dsm)
            state = "a" if frozen or s != 0 else "n"
            if prev_state == "n":
                res.at_risk += 1
                if frozen:
                    res.ent_frozen += 1
                elif s != 0:
                    res.ent_slip += 1
                    res.slip_sizes[s] += 1
        prev_state = state
        if p.v1 is None or p.h1 is None:
            continue
        b = mc.band(p.v1)
        for bi, dt in enumerate(p.dt):
            if dt is None or dt <= 0 or dt >= mc.GAP_LIMITS_S[-1] + mc.GAP_TOL_S:
                continue
            fired, r10, (ex, ey) = mc.run(p, dt, gps_drift)
            if bi == 0 and p.v1 >= 10.0 and mc.judged(dt, 0.5):
                res.res_hist[res_bin(r10)] += 1
            sig = mc.slip_signature(p, dt) if fired[0] else None
            cross = False
            if fired[0]:
                al, cr = mc.along_cross(p, ex, ey)
                cross = abs(cr) > abs(al)
            for li, lim in enumerate(mc.GAP_LIMITS_S):
                if not mc.judged(dt, lim):
                    continue
                k = (bi, li, b)
                a = res.acc.get(k)
                if a is None:
                    a = res.acc[k] = [0] * ACC_LEN
                    a[1] = 0.0
                a[0] += 1
                a[1] += dt
                for ci in range(NC):
                    if fired[ci]:
                        a[2 + ci] += 1
                a[I_SLIPZ] += sig == "zero"
                a[I_SLIPD] += sig == "double"
                a[I_CROSS] += cross
    return res


class Dataset:
    def __init__(self, name, role):
        self.name, self.role = name, role
        self.ids = {}
        self.raw_rows = 0

    def add(self, r):
        self.ids[r.key] = r
        self.raw_rows += r.raw

    def cls(self, key, params=None):
        r = self.ids[key]
        return devclass.classify(r.frac, r.nsm, params)


def analyse_files(name, role, files, kind, gps_drift=mc.GPS_DRIFT_M, ds=None):
    ds = ds or Dataset(name, role)
    xy = kind == "nyc"
    for path, pre in files:
        for vid, bsms in normalise.stream_ids(path, kind):
            key = vid if pre is None else pre + ":" + vid
            ds.add(analyse_id(key, bsms, xy=xy, nyc=xy, gps_drift=gps_drift))
    return ds


def cell_units(ds, base, limit_i, bands, check_i, classes=None, params=None):
    """Per-ID (triggers, minutes) for one table cell. check_i indexes the accumulator."""
    out = []
    for key, r in ds.ids.items():
        if classes is not None and devclass.classify(r.frac, r.nsm, params) not in classes:
            continue
        t = m = 0.0
        n = 0
        for b in bands:
            a = r.acc.get((base, limit_i, b))
            if a:
                t += a[check_i]
                m += a[1]
                n += a[0]
        if n:
            out.append((t, m / 60.0, n, key))
    return out
