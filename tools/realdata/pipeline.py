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
                 "at_risk_fast", "ent_slip", "slip_sizes", "hdr_off_ms", "period_excess_ms", "res_hist", "dt_near100", "dt_all", "cnt_ok", "cnt_n",
                 "ent_slip_ret", "slip_sizes_ret", "drift_med", "period_dev_ms", "body_al", "body_cr", "body_n", "body_vdt2",
                 "body_q2", "stamp_al", "body_pairs", "body_dv", "body_dh", "hop")

    def __init__(self, key):
        self.key = key
        self.acc = {}
        self.phase100 = Counter()
        self.wrapped = Counter()
        self.at_risk = self.ent_frozen = self.at_risk_fast = self.ent_slip = 0
        self.slip_sizes = Counter()
        self.hdr_off_ms = Counter()
        self.period_excess_ms = Counter()
        self.res_hist = Counter()
        self.dt_near100 = self.dt_all = self.cnt_ok = self.cnt_n = 0
        # v2 timing model (schema v2xw.bsm_timing.v2): slips that return at the next pair, the
        # device's BSM-clock drift (median period excess) and its per-BSM deviation, and the
        # residual body of exactly-100 ms pairs (receiver output noise), mm bins.
        self.ent_slip_ret = 0
        self.slip_sizes_ret = Counter()
        self.drift_med = None
        self.period_dev_ms = Counter()
        self.body_al = Counter()
        self.body_cr = Counter()
        self.body_n = 0
        self.body_vdt2 = self.body_q2 = 0.0
        self.stamp_al = Counter()
        # v3 (schema v2xw.bsm_timing.v3): the exactly-100 ms pairs with the pair's own
        # kinematics removed (along: half the speed change times Δt; across: half the heading
        # change times v·Δt), per pair (along m, cross m, v1 m/s, quantisation variance m²);
        # the speed and heading changes of the same pairs in J2735 LSBs; and the :00/:50 grid
        # state transitions of consecutive BSMs (home or away of the ID's majority grid).
        self.body_pairs = []
        self.body_dv = Counter()
        self.body_dh = Counter()
        self.hop = Counter()


# A slip onset is counted only where a one-epoch slip moves the reported position by at
# least gpsDrift (v * 100 ms >= 1 m): the band RandomPosition judges and the model is used
# in. Below it, locked devices show sub-metre near-stale positions (Tampa: 9 onsets at
# 5-10 m/s, residuals 0.01-0.75 m, none over 1 m; 0 of 1,152 fast pairs) that a
# speed-independent slip probability would carry to highway speed as metre-scale triggers.
SLIP_MIN_SPEED_MPS = 10.0


def slip_size(p, dt):
    """The displacement beyond what the pair's own secMark Δt explains, in fix intervals.

    Measured against v̄·Δt, not against Δt rounded to whole fix intervals: a locked device that
    hops between the :00 and :50 grids gives Δt 0.05 s or 0.15 s with a consistent position,
    and rounding Δt/0.1 s (round(1.49) = 1) while the displacement rounds the other way
    (1.474 m at 9.26 m/s is 1.59 intervals) used to count that hop as a position-epoch slip."""
    vbar = 0.5 * (p.v1 + p.v2)
    return round((math.hypot(p.dx, p.dy) - vbar * dt) / (vbar * mc.FIX_INTERVAL_S))


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
    hop_states(res, bs)
    prev_state = None  # entry estimator: state of the previous pair ('n', 'a' or None)
    pending = None  # the size of a slip onset at the previous pair, awaiting the return test
    excess = []
    for i, p in enumerate(pairing.pairs(bs, xy, nyc)):
        dsm = p.dt[0]
        if dsm is not None and 0.05 < dsm < 0.15 and p.dcnt == 1:
            excess.append(int(round(dsm * 1000.0)) - 100)
        if (not xy and dsm is not None and p.dcnt == 1 and p.v1 is not None and p.v1 >= SLIP_MIN_SPEED_MPS
                and p.h1 is not None and 0.0985 <= dsm <= 0.1015):
            ex, ey = mc.residual(p, dsm)
            al, cr = mc.along_cross(p, ex, ey)
            e_ms = int(round(dsm * 1000.0)) - 100
            res.stamp_al[(e_ms, "sum")] += al / p.v1 * 1000.0
            res.stamp_al[(e_ms, "n")] += 1
            if e_ms == 0:
                res.body_al[int(round(al * 1000.0))] += 1
                res.body_cr[int(round(cr * 1000.0))] += 1
                res.body_n += 1
                res.body_vdt2 += (p.v1 * dsm) ** 2
                lsb_n = math.radians(mc.POS_LSB_DEG) * pairing.R_EARTH
                lsb_e = lsb_n * math.cos(math.radians(bs[i].lat))
                res.body_q2 += (lsb_n ** 2 + lsb_e ** 2) / 12.0
                if p.v2 is not None and p.h2 is not None:
                    dh = ((p.h2 - p.h1 + 180.0) % 360.0) - 180.0
                    alc = al - 0.5 * (p.v2 - p.v1) * dsm
                    crc = cr + 0.5 * p.v1 * dsm * math.radians(dh)
                    res.body_pairs.append((round(alc, 5), round(crc, 5), round(p.v1, 3),
                                           (lsb_n ** 2 + lsb_e ** 2) / 12.0))
                    res.body_dv[int(round((p.v2 - p.v1) / mc.SPEED_RES_MPS))] += 1
                    res.body_dh[int(round(dh / mc.HEADING_LSB_DEG))] += 1
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
            s_raw = slip_size(p, dsm)
            s = 0 if frozen else s_raw
            state = "a" if frozen or s != 0 else "n"
            if pending is not None and s_raw == -pending:
                # the slipped BSM's position came back to its own epoch: a one-message slip.
                # An onset that does not return is a persistent phase step (a BSM-clock wrap).
                res.ent_slip_ret += 1
                res.slip_sizes_ret[pending] += 1
            pending = None
            if prev_state == "n":
                res.at_risk += 1
                fast = p.v1 >= SLIP_MIN_SPEED_MPS
                res.at_risk_fast += fast
                if frozen:
                    res.ent_frozen += 1
                elif s != 0 and fast:
                    res.ent_slip += 1
                    res.slip_sizes[s] += 1
                    pending = s
        else:
            pending = None
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
    if len(excess) >= DRIFT_MIN_PAIRS:
        xs = sorted(excess)
        med = xs[len(xs) // 2] if len(xs) % 2 else 0.5 * (xs[len(xs) // 2 - 1] + xs[len(xs) // 2])
        res.drift_med = med
        for e in excess:
            res.period_dev_ms[max(-50, min(50, int(round(e - med))))] += 1
    return res


# Grid-state tolerance: secMark within this many ms of the :00 or :50 epoch (the locked class
# sits within 5 ms of a 50 ms epoch, data/calibration/bsm-timing-cvpilot.yaml).
HOP_TOL_MS = 5

def grid_state(secmark, tol_ms=HOP_TOL_MS):
    """0 on the :00 grid, 1 on the :50 grid (secMark mod 100 ms within tol_ms), else None."""
    s = secmark % 100
    if min(s, 100 - s) <= tol_ms:
        return 0
    if abs(s - 50) <= tol_ms:
        return 1
    return None


def hop_states(res, bs):
    """The :00/:50 grid process of one ID: its home grid is the one most of its BSMs sit on,
    and each consecutive-msgCnt pair whose secMark Δt is 50, 100 or 150 ms (± tol) counts one
    transition (home or away -> home or away). A locked device that hops shows Δt 50 then
    150 ms (or 150 then 50) with a position consistent with Δt (testing/paper1-validation.md §3)."""
    st = [None if b.secmark is None else grid_state(b.secmark) for b in bs]
    n0, n1 = st.count(0), st.count(1)
    if n0 + n1 == 0:
        return
    home = 0 if n0 >= n1 else 1
    for a, b, sa, sb in zip(bs, bs[1:], st, st[1:]):
        if sa is None or sb is None or a.msgcnt is None or b.msgcnt is None or (b.msgcnt - a.msgcnt) % 128 != 1:
            continue
        d = (b.secmark - a.secmark) % 60000
        if not any(abs(d - x) <= HOP_TOL_MS for x in (50, 100, 150)):
            continue
        res.hop[("home" if sa == home else "away", "home" if sb == home else "away")] += 1


# An ID's BSM-clock drift is the median of its single-interval secMark Δt minus 100 ms over
# consecutive msgCnt pairs; fewer pairs than this leave the ID out of the drift fit.
DRIFT_MIN_PAIRS = 20


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
