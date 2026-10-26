"""Stage 7 (outputs), part 1: the fitted BSM timing model (v2xw.bsm_timing.v1) and the frozen
classifier. Derived statistics only; no raw row leaves the scratch directory."""
import json
import math
from collections import Counter

import bootci
import devclass
import mbrchecks as mc
import pipeline


def r6(x):
    if x is None or (isinstance(x, float) and math.isnan(x)):
        return None
    return float("%.6g" % x)


def headline_units(ds_list, params, classes):
    units = []
    for ds in ds_list:
        units += pipeline.cell_units(ds, 0, 2, (3, 4), 2, classes, params)
    return units


def _class_stats(sets, params, c, key):
    u = headline_units(sets, params, (c,))
    rate, lo, hi = bootci.ci([(t, m) for t, m, _, _ in u], key)
    return {"ids": len(u), "pairs": sum(x[2] for x in u), "triggers": int(sum(x[0] for x in u)),
            "rate_per_id_min": r6(rate), "ci95": [r6(lo), r6(hi)]}


def frozen_doc(fit_sets, params, version, fit_files):
    """The classifier and its out-of-sample predictions, frozen before Wyoming is scored.

    The free class is heterogeneous across deployments (Tampa's free devices trip far less
    often than Orem's), so the bounds are per fit dataset, not pooled: a held-out locked class
    must trip no more than the highest locked upper bound of any fit set, and a free class no
    less than the lowest free lower bound."""
    pooled = {c: _class_stats(fit_sets, params, c, "frozen|" + c) for c in devclass.CLASSES}
    per = {ds.name: {c: _class_stats([ds], params, c, "frozen|%s|%s" % (ds.name, c)) for c in devclass.CLASSES}
           for ds in fit_sets}
    lk = [v["locked"]["ci95"][1] for v in per.values() if v["locked"]["ids"]]
    fr = [v["free"]["ci95"][0] for v in per.values() if v["free"]["ids"]]
    return {
        "schema": "v2xw.realdata_classifier.v1",
        "tool": "tools/realdata " + version,
        "params": params,
        "fitted_on": sorted(per),
        "fit_files_sha256": fit_files,
        "headline_cell": {"check": "RandomPosition v1.0", "gps_drift_m": 1.0, "time_base": "secmark",
                          "gap_limit_s": 0.5, "speed": ">= 10 m/s", "unit": "triggers per ID-minute"},
        "fit": pooled,
        "fit_by_dataset": per,
        "predictions": {
            "locked": {"rate_at_most": max(lk) if lk else None},
            "free": {"rate_at_least": min(fr) if fr else None},
        },
        "rule": ("Out of sample, a class passes when its point rate at the headline cell lies on the "
                 "predicted side of the bound frozen here (locked: at most the highest locked upper 95% "
                 "bound of any fit set; free: at least the lowest free lower 95% bound of any fit set). "
                 "Where both classes are present the free rate must exceed the locked rate."),
    }


def dump_json(doc):
    return json.dumps(doc, indent=1, sort_keys=True) + "\n"


def timing_stats(datasets, params):
    agg = {}
    for ds in datasets:
        for r in ds.ids.values():
            c = devclass.classify(r.frac, r.nsm, params)
            a = agg.setdefault(c, {"ids": 0, "phase100": Counter(), "wrapped": Counter(), "at_risk": 0,
                                   "at_risk_fast": 0, "frozen": 0, "slip": 0, "sizes": Counter(), "hdr": Counter(),
                                   "excess": Counter(), "ret": 0, "ret_sizes": Counter(), "drift": [],
                                   "dev": Counter(), "body_al": Counter(), "body_cr": Counter(), "body_n": 0,
                                   "vdt2": 0.0, "q2": 0.0, "stamp": Counter(), "pairs": [], "dv": Counter(),
                                   "dh": Counter(), "hop": Counter()})
            a["ids"] += 1
            a["phase100"].update(r.phase100)
            # The secMark offsets weigh each ID once too (the stamp jitter of a random device):
            # by message, Orem's 3 locked IDs, a third of whose pairs are 99 or 101 ms apart,
            # outweighed Tampa's 41 (one in seven).
            nw = sum(r.wrapped.values())
            if nw:
                a["wrapped"].update({x: m / nw for x, m in r.wrapped.items()})
            a["at_risk"] += r.at_risk
            a["at_risk_fast"] += r.at_risk_fast
            a["frozen"] += r.ent_frozen
            a["slip"] += r.ent_slip
            a["sizes"].update(r.slip_sizes)
            a["hdr"].update(r.hdr_off_ms)
            a["excess"].update(r.period_excess_ms)
            a["ret"] += r.ent_slip_ret
            a["ret_sizes"].update(r.slip_sizes_ret)
            if r.drift_med is not None:
                a["drift"].append(r.drift_med)
            a["dev"].update(r.period_dev_ms)
            a["body_al"].update(r.body_al)
            a["body_cr"].update(r.body_cr)
            a["body_n"] += r.body_n
            a["vdt2"] += r.body_vdt2
            a["q2"] += r.body_q2
            a["stamp"].update(r.stamp_al)
            # Each ID's pairs together weigh one: the engine's fleet is devices, each sending
            # about as many BSMs as the next, so the fit is the distribution of a random
            # device, not of a random pair (Orem's 3 locked IDs held 28 % of the pairs).
            nb = len(r.body_pairs)
            a["pairs"].extend(x + (1.0 / nb,) for x in r.body_pairs)
            a["dv"].update(r.body_dv)
            a["dh"].update(r.body_dh)
            a["hop"].update(r.hop)
    return agg


def _robust_sd_mm(c):
    """IQR / 1.349 of a 1 mm histogram, metres: the spread of the body, not of the tail."""
    q1, q3 = counter_q(c, 0.25), counter_q(c, 0.75)
    return None if q1 is None else (q3 - q1) / 1.349 / 1000.0


def output_noise(a):
    """The receiver's per-BSM output noise from the exactly-100 ms pairs of one class at
    v >= 10 m/s (secMark jitter excluded by construction).

    The residual of such a pair is, along track, the reported speed's error times Δt and,
    across track, the reported heading's error times v·Δt, each on top of the J2735 position
    quantisation of both BSMs (variance (LSB_n² + LSB_e²) / 12 per component). Consecutive
    residuals are uncorrelated in the data (lag-1 ≈ 0, research/synthesis/p1-realdata-baseline.md
    §4), which rules out white position noise (−0.5) and leaves velocity-domain noise.
    Returns (speed sd m/s, heading sd deg, pairs, along sd m, cross sd m, quantisation sd m)."""
    n = a.get("body_n", 0)
    if n < 50:
        return None
    sa, sc = _robust_sd_mm(a["body_al"]), _robust_sd_mm(a["body_cr"])
    q2 = a["q2"] / n
    vdt2 = a["vdt2"] / n
    sp = math.sqrt(max(0.0, sa * sa - q2)) / 0.1
    hd = math.degrees(math.sqrt(max(0.0, sc * sc - q2) / vdt2))
    return sp, hd, n, sa, sc, math.sqrt(q2)


def _lsb_sd(c, lsb):
    """IQR / 1.349 of a histogram counted in LSBs, in the LSB's unit."""
    q1, q3 = counter_q(c, 0.25), counter_q(c, 0.75)
    return None if q1 is None else (q3 - q1) * lsb / 1.349


def output_noise_v3(a):
    """The receiver's white speed and heading output noise, bounded by how much the
    transmitted speed and heading change between consecutive exactly-100 ms BSMs: a white
    noise of sd s gives a change of sd s·√2, and the true change over 100 ms only adds to it,
    so s = sd(change)/√2 is an upper bound. The v2 fit read the whole residual body as
    velocity noise (0.103 m/s, 0.30 deg), which predicts a heading change of sd 0.43 deg
    between consecutive BSMs; the fit sets' is 0.07 deg (research/synthesis/p1-realdata-baseline.md §4).
    Returns (speed sd m/s, heading sd deg, pairs, change sds) or None."""
    n = sum(a.get("dv", Counter()).values())
    if n < 50:
        return None
    sdv = _lsb_sd(a["dv"], mc.SPEED_RES_MPS)
    sdh = _lsb_sd(a["dh"], mc.HEADING_LSB_DEG)
    return sdv / math.sqrt(2.0), sdh / math.sqrt(2.0), n, sdv, sdh


def stamp_diff_sd_s(mu_ms, sd_ms):
    """sd of the difference of two generation instants N(mu, sd) ms about their epochs, given
    that both truncate to the same whole ms (secMark Δt exactly 100 ms), seconds."""
    num = den = 0.0
    for k in range(-4, 5):
        xs = [k + (i + 0.5) / 400.0 for i in range(400)]
        w = [math.exp(-0.5 * ((x - mu_ms) / sd_ms) ** 2) for x in xs]
        pk = sum(w)
        if pk <= 0:
            continue
        m = sum(a * b for a, b in zip(w, xs)) / pk
        var = sum(a * (b - m) ** 2 for a, b in zip(w, xs)) / pk
        num += pk * pk * 2.0 * var
        den += pk * pk
    return math.sqrt(num / den) * 1e-3


# The position-noise mixture: components, EM iterations, and the floor of a component sd (m).
PN_K = 4
PN_ITERS = 400
PN_FLOOR_M = 1e-4


def position_noise(a, speed_sd, heading_sd_deg, stamp_sd_s, k=PN_K, iters=PN_ITERS):
    """The receiver's reported-position error increment per fix interval, a zero-mean
    Gaussian scale mixture (shared component for the along- and cross-track parts), fitted by
    EM on the exactly-100 ms pairs of one class at v >= 10 m/s after their own kinematics are
    removed. Each pair's known nuisance is deconvolved: the J2735 position quantisation of
    both BSMs, the jitter of both generation instants (stamp_sd_s × v along track), and the
    white speed (× Δt) and heading (× v·Δt) output noise. Consecutive residuals are
    uncorrelated (lag-1 -0.02 along, +0.05 across on Tampa), so the increments are white:
    the error is a random walk at 100 ms, not white position noise (lag-1 -0.5).
    A pair's optional fifth field is its weight (1 / the pairs of its ID, timing_stats: each
    ID weighs one); without it every pair weighs one.
    Returns (weights, along sds m, cross sds m, pairs, log-likelihood) or None."""
    X = a.get("pairs", [])
    if len(X) < 200:
        return None
    hd = math.radians(heading_sd_deg)
    data = []
    wt = []
    for x in X:
        al, cr, v1, q2 = x[:4]
        na = q2 + (v1 * stamp_sd_s) ** 2 + (speed_sd * mc.FIX_INTERVAL_S) ** 2
        nc = q2 + (v1 * mc.FIX_INTERVAL_S * hd) ** 2
        data.append((al, cr, na, nc))
        wt.append(x[4] if len(x) > 4 else 1.0)
    wsum = sum(wt)
    sa = [0.002 * 3 ** j for j in range(k)]
    sc = list(sa)
    pi = [1.0 / k] * k
    ll = 0.0
    for _ in range(iters):
        R = []
        ll = 0.0
        for al, cr, na, nc in data:
            lw = []
            for j in range(k):
                va, vc = sa[j] ** 2 + na, sc[j] ** 2 + nc
                lw.append(math.log(pi[j]) - 0.5 * (al * al / va + cr * cr / vc) - 0.5 * math.log(va * vc)
                          - math.log(2 * math.pi))
            m = max(lw)
            t = sum(math.exp(x - m) for x in lw)
            ll += m + math.log(t)
            R.append([math.exp(x - m) / t for x in lw])
        for j in range(k):
            w = sum(r[j] * u for r, u in zip(R, wt))
            pi[j] = max(w / wsum, 1e-12)
            sa[j] = math.sqrt(max(PN_FLOOR_M ** 2, sum(r[j] * u * (x[0] ** 2 - x[2])
                                                        for r, u, x in zip(R, wt, data)) / max(w, 1e-300)))
            sc[j] = math.sqrt(max(PN_FLOOR_M ** 2, sum(r[j] * u * (x[1] ** 2 - x[3])
                                                        for r, u, x in zip(R, wt, data)) / max(w, 1e-300)))
    order = sorted(range(k), key=lambda j: sa[j] ** 2 + sc[j] ** 2)
    return [pi[j] for j in order], [sa[j] for j in order], [sc[j] for j in order], len(data), ll


# The position walk's mean-reversion time constants tried (s). The cap is the receiver's
# carrier-smoothing time constant (Hatch 1982; RTCA DO-229: 100 s): its position error does not
# wander for longer than the filter remembers.
PN_TAU_GRID_S = (1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0)
# A tau whose in-sample KS is within this of the best one's fits as well (the sampling noise of
# a KS over about 40 IDs); the shortest such tau keeps the walk's own absolute error smallest.
PN_TAU_KS_TOL = 0.01
PN_TAU_REPS = 10


def _wks(data, sim):
    """KS distance between a weighted sample [(x, w)] and an unweighted one [x]."""
    data = sorted(data)
    sim = sorted(sim)
    tw = sum(w for _, w in data)
    d, cw, j = 0.0, 0.0, 0
    for x, w in data:
        cw += w
        while j < len(sim) and sim[j] <= x:
            j += 1
        d = max(d, abs(cw / tw - j / len(sim)), abs((cw - w) / tw - j / len(sim)))
    return d


def position_tau(a, speed_sd, heading_sd_deg, stamp_sd_s, pn, grid=PN_TAU_GRID_S, seed=1):
    """The walk's mean-reversion tau, fitted in sample: the EM above reads an exactly-100 ms
    residual as the increment w alone, but a walk reverting with rho = exp(-T/tau) adds
    (rho - 1)·e to every pair, e the walk's own error (sd sigma / sqrt(1 - rho^2)): at tau 1 s
    and the fitted tail that is about 1 cm per axis on every pair, a body the data do not show.
    For each tau the model's residual (the walk, plus the nuisance the EM deconvolved) is
    simulated at each fit pair's own speed and quantisation and compared with the pairs'
    residuals, each ID weighing one (KS). Returns (tau, [(tau, ks)])."""
    import random
    X = a.get("pairs", [])
    w, sa, sc = pn[0], pn[1], pn[2]
    hd = math.radians(heading_sd_deg)
    data = [(math.hypot(x[0], x[1]), x[4] if len(x) > 4 else 1.0) for x in X]
    cum = [sum(w[:i + 1]) for i in range(len(w))]
    out = []
    for tau in grid:
        rho = math.exp(-mc.FIX_INTERVAL_S / tau)
        rnd = random.Random(seed)
        ea = ec = 0.0
        sim = []
        for rep in range(PN_TAU_REPS + 1):
            for x in X:
                v1, q2 = x[2], x[3]
                u = rnd.random()
                j = next((i for i, c in enumerate(cum) if u < c), len(w) - 1)
                na, nc = rho * ea + rnd.gauss(0.0, sa[j]), rho * ec + rnd.gauss(0.0, sc[j])
                ra, rc = na - ea, nc - ec
                ea, ec = na, nc
                # the first pass is the burn-in: the walk reaches its stationary spread
                if rep:
                    sda = math.sqrt(q2 + (v1 * stamp_sd_s) ** 2 + (speed_sd * mc.FIX_INTERVAL_S) ** 2)
                    sdc = math.sqrt(q2 + (v1 * mc.FIX_INTERVAL_S * hd) ** 2)
                    sim.append(math.hypot(ra + rnd.gauss(0.0, sda), rc + rnd.gauss(0.0, sdc)))
        out.append((tau, _wks(data, sim)))
    best = min(k for _, k in out)
    return next(t for t, k in out if k <= best + PN_TAU_KS_TOL), out


def grid_hop(a):
    """The locked class's :00/:50 grid process, per BSM: P(leave the home grid) and
    P(return to it), from consecutive-msgCnt transitions. Returns (leave, return, counts)."""
    h = a.get("hop", Counter())
    hh, ha, ah, aa = (h.get(("home", "home"), 0), h.get(("home", "away"), 0), h.get(("away", "home"), 0),
                      h.get(("away", "away"), 0))
    if hh + ha == 0 or ah + aa == 0:
        return None
    return ha / (hh + ha), ah / (ah + aa), (hh, ha, ah, aa)


# The mean-reversion time of the position error increments' random walk is fitted
# (position_tau): the lag-1 bound (an AR(1) with step coefficient rho gives consecutive
# residuals a lag-1 correlation of about -(1 - rho) / 2, within 0.05 of 0 on Tampa) only says
# tau >= 1 s, and 1 s itself put a centimetre body on every pair (1.2.0).


def _stamp_note(a):
    out = []
    for e in (-1, 0, 1):
        n = a["stamp"].get((e, "n"), 0)
        if n:
            out.append("%+d ms: %.2f ms (%d)" % (e, a["stamp"][(e, "sum")] / n, n))
    return ", ".join(out)


def _point_pmf(wrapped, core_ms=5):
    """The locked class's secMark offset from its epoch as a pmf over whole ms (|x| <= core)."""
    xs = sorted(x for x in wrapped if abs(x) <= core_ms)
    n = sum(wrapped[x] for x in xs)
    return xs, sum_to_one([wrapped[x] / n for x in xs]), n / sum(wrapped.values())


def _floor_normal(xs, ps):
    """The normal N(mu, sd) whose 1 ms truncation puts the observed shares below 0 and at or
    above 1 ms: P(x < 0) = p(< 0), P(x >= 1) = p(>= 1)."""
    from statistics import NormalDist
    lo = sum(p for x, p in zip(xs, ps) if x < 0)
    hi = sum(p for x, p in zip(xs, ps) if x >= 1)
    z0 = NormalDist().inv_cdf(min(max(lo, 1e-6), 1 - 1e-6))
    z1 = NormalDist().inv_cdf(min(max(1.0 - hi, 1e-6), 1 - 1e-6))
    sd = 1.0 / (z1 - z0)
    return -z0 * sd, sd


def _binned(values, lo, hi, width):
    """Empirical edges and shares of `values` (a Counter or a list), clipped to [lo, hi)."""
    c = Counter()
    items = values.items() if hasattr(values, "items") else ((v, 1) for v in values)
    nb = int((hi - lo) // width)
    for x, n in items:
        c[max(0, min(nb - 1, int((x - lo) // width)))] += n
    tot = sum(c.values())
    edges = [lo + k * width for k in range(nb + 1)]
    return edges, sum_to_one([c[k] / tot for k in range(nb)])


def _slip_dist(sizes):
    d = Counter()
    for s, n in sizes.items():
        d["-1" if s <= -1 else ("1" if s == 1 else "2")] += n
    tot = sum(d.values())
    return {k: (r6(d[k] / tot) if tot else None) for k in ("-1", "1", "2")}, tot


def _normal_core(wrapped, core_ms=5):
    xs = [(x, n) for x, n in wrapped.items() if abs(x) <= core_ms]
    n = sum(w for _, w in xs)
    tot = sum(wrapped.values())
    if not n:
        return None, None, 0.0
    mean = sum(x * w for x, w in xs) / n
    var = sum(w * (x - mean) ** 2 for x, w in xs) / n
    return mean, math.sqrt(var), n / tot


def _empirical(phase100, width=10):
    bins = list(range(0, 101, width))
    c = Counter()
    for x, n in phase100.items():
        c[min(int(x) // width, len(bins) - 2)] += n
    tot = sum(c.values())
    return bins, sum_to_one([c[i] / tot for i in range(len(bins) - 1)])


def sum_to_one(ps, nd=6):
    """Round to nd decimals so the rounded values sum to exactly 1 (the largest absorbs it)."""
    out = [round(p, nd) for p in ps]
    i = max(range(len(out)), key=lambda k: out[k])
    out[i] = round(out[i] + 1.0 - sum(out), nd)
    return out


def counter_moments(c):
    n = sum(c.values())
    if n < 10:
        return None, None, n
    mean = sum(k * v for k, v in c.items()) / n
    return mean, math.sqrt(sum(v * (k - mean) ** 2 for k, v in c.items()) / n), n


def counter_q(c, q):
    n = sum(c.values())
    acc = 0
    for k in sorted(c):
        acc += c[k]
        if acc >= q * n:
            return k
    return None


def header_sd(offsets):
    """Per-message header jitter: sd of (header time - secMark time) over messages (ms).
    The locked class's secMark sits on the fix epoch, so it stands in for true time."""
    return counter_moments(offsets)


def timing_yaml(fit_agg, hdr_agg, prov, version, notes=()):
    nl = fit_agg.get("locked", {}).get("ids", 0)
    nf = fit_agg.get("free", {}).get("ids", 0)
    nm = fit_agg.get("mixed", {}).get("ids", 0)
    hdr = Counter()
    for a in hdr_agg.values():
        hdr.update(a["hdr"])
    hmean, hsd, hn = header_sd(hdr)
    pooled = Counter()
    for c in ("locked", "free"):
        pooled.update(fit_agg.get(c, {}).get("sizes", Counter()))
    L = ["# BSM device-timing model fitted on public CV-pilot BSMs (P1-E1/P1-E2).",
         "# GENERATED by `python3 tools/realdata/rdtool.py all` - do not edit by hand.",
         "# Derived statistics only. Shares are over locked+free IDs of the fit sets; %d mixed and" % nm,
         "# unclassified IDs are left out of the shares. Phases are secMark mod 100 ms (fix epochs",
         "# assumed on the 100 ms grid); locked devices sit on either the :00 or the :50 grid, so the",
         "# locked phase is the distance to the nearest 50 ms epoch.",
         *["# " + n for n in notes],
         "schema: v2xw.bsm_timing.v3",
         "provenance:",
         "  sources:"]
    for s in prov["sources"]:
        L.append('    - {dataset: %s, rows: %d, query: "%s"}' % (s["dataset"], s["rows"], s["query"]))
    L += ["  manifest_sha256: %s" % prov["manifest_sha256"],
          "  fitted_on: [%s]" % ", ".join('"%s"' % x for x in prov["fitted_on"]),
          "  held_out: [%s]" % ", ".join('"%s"' % x for x in prov["held_out"]),
          "  tool: tools/realdata %s" % version,
          "fix_interval_ms: 100",
          "classes:"]
    for c, n in (("locked", nl), ("free", nf)):
        a = fit_agg.get(c)
        if not a:
            continue
        L.append("  - name: %s" % c)
        L.append("    share: %s" % sum_to_one([nl / (nl + nf), nf / (nl + nf)])[0 if c == "locked" else 1])
        if c == "locked":
            mean, sd, core = _normal_core(a["wrapped"])
            xs, ps, core = _point_pmf(a["wrapped"])
            mu, sg = _floor_normal(xs, ps)
            L.append("    # the %.1f%% of locked BSMs within 5 ms of a 50 ms epoch; whole-ms offsets %s with" % (
                100 * core, ", ".join("%+d" % x for x in xs)))
            L.append("    # shares %s. Mean along-track residual / v by secMark Δt - 100 ms (v >= 10 m/s):" % (
                ", ".join(str(p) for p in ps)))
            L.append("    # " + _stamp_note(a))
            L.append("    # That is the 1 ms truncation of a generation instant that jitters about the epoch, with")
            L.append("    # the position at that instant (+-0.5 ms expected; +-1 ms if the position stayed on the")
            L.append("    # epoch). The instant is the normal whose truncation gives the shares of -1 and >= +1:")
            L.append("    secmark_phase_ms: {dist: normal, mean: %s, sd: %s}" % (r6(mu), r6(sg)))
            L.append("    phase_scope: message")
        else:
            bins, p = _empirical(a["phase100"])
            L.append("    # bins_ms are the %d edges of %d bins; p[i] is the share in [bins_ms[i], bins_ms[i+1])"
                     % (len(bins), len(p)))
            L.append("    secmark_phase_ms: {dist: empirical, bins_ms: [%s], p: [%s]}" % (
                ", ".join(str(b) for b in bins), ", ".join(str(x) for x in p)))
        if a["excess"]:
            L.append("    # secMark Δt minus 100 ms on single-interval pairs: p10 %s, p50 %s, p90 %s ms" % (
                counter_q(a["excess"], 0.1), counter_q(a["excess"], 0.5), counter_q(a["excess"], 0.9)))
        if c == "free":
            L.append("    # the free BSM period exceeds the fix interval, so the phase drifts across the fix grid:")
            L.append("    # the phase above is the device's first; bsm_clock moves it. drift_ms is the per-ID")
            L.append("    # median of secMark Δt - 100 ms over consecutive-msgCnt single-interval pairs (%d IDs" % len(a["drift"]))
            L.append("    # with >= %d pairs), period_dev_ms each BSM's period minus its ID's median; its tail" % pipeline.DRIFT_MIN_PAIRS)
            dev_n = sum(a["dev"].values())
            tail = sum(n for x, n in a["dev"].items() if abs(x) > 25)
            L.append("    # beyond +-25 ms (%.1f%% of %d periods) is the clock's resets" % (100.0 * tail / dev_n if dev_n else 0.0, dev_n))
            dedges, dps = _binned(a["drift"], -10, 50, 2)
            vedges, vps = _binned(a["dev"], -50, 51, 5)
            L.append("    bsm_clock:")
            L.append("      drift_ms: {dist: empirical, bins_ms: [%s], p: [%s]}" % (
                ", ".join(str(x) for x in dedges), ", ".join(str(x) for x in dps)))
            L.append("      period_dev_ms: {dist: empirical, bins_ms: [%s], p: [%s]}" % (
                ", ".join(str(x) for x in vedges), ", ".join(str(x) for x in vps)))
        risk = a["at_risk"]
        fast = a["at_risk_fast"]
        L.append("    # slip onsets per single-interval pair at v >= 10 m/s after a normal pair (Δt-corrected")
        L.append("    # displacement): %d at risk; frozen repeats below are per pair at v >= 5 m/s, %d at risk" % (fast, risk))
        L.append("    # %d onsets, of which %d return at the next pair (a one-message slip, counted here); the" % (
            a["slip"], a["ret"]))
        L.append("    # %d that do not return are persistent phase steps, which bsm_clock reproduces" % (a["slip"] - a["ret"]))
        L.append("    epoch_slip_prob: %s" % r6(a["ret"] / fast if fast else 0.0))
        dist, tot = _slip_dist(a["ret_sizes"])
        if not tot:
            dist, _ = _slip_dist(pooled)
            L.append("    # no slips observed in this class; slip sizes are the pooled locked+free ones")
        L.append('    slip_epochs: {"-1": %s, "1": %s, "2": %s}' % (dist["-1"], dist["1"], dist["2"]))
        L.append("    frozen_repeat_prob: %s" % r6(a["frozen"] / risk if risk else 0.0))
        if c == "locked":
            L.append("    # from %d Wyoming BSMs (the only source with a header time; its class and rate results" % hn)
            L.append("    # stay held out). Header minus secMark also has a mean of %.1f ms, and it is" % hmean)
            L.append("    # persistent: it changes in steps of about 50 ms rather than per message. PROVISIONAL")
            L.append("    # until P1-E3: if metadata_generatedat is ODE log time, not the signed 1609.2")
            L.append("    # generationTime, this is not a header jitter.")
        else:
            L.append("    # no free-class source has a header time: borrowed from the locked Wyoming fit")
        L.append("    header_jitter_ms: {sd: %s}" % r6(hsd))
        # Fitted on the locked class only: a free device's exactly-100 ms pairs carry its
        # drifting stamp phase as well, so the free class borrows the locked receiver's noise.
        lk = fit_agg.get("locked", {})
        src = "the locked class" if c == "locked" else "the locked class (borrowed)"
        on = output_noise_v3(lk)
        v2 = output_noise(lk)
        if on:
            L.append("    # receiver output noise, white, bounded by the change of the transmitted speed and heading")
            L.append("    # between consecutive exactly-100 ms BSMs of %s at v >= 10 m/s (%d pairs):" % (src, on[2]))
            L.append("    # change sd %.4f m/s and %.4f deg (IQR/1.349), so noise <= change/sqrt(2). The v2 fit" % (
                on[3], on[4]))
            if v2:
                L.append("    # read the whole residual body (along %.4f m, cross %.4f m) as this noise" % (v2[3], v2[4]))
            L.append("    output_noise: {speed_mps: %s, heading_deg: %s}" % (r6(on[0]), r6(on[1])))
            mu, sg = _floor_normal(*_point_pmf(lk["wrapped"])[:2])
            ssd = stamp_diff_sd_s(mu, sg)
            pn = position_noise(lk, on[0], on[1], ssd)
            if pn:
                w, sa, sc, npairs, ll = pn
                L.append("    # the rest of the body and its tail: the reported position's error increment per fix")
                L.append("    # interval, a Gaussian scale mixture (EM, %d components, log-lik %.1f) on %d exactly-100 ms" % (
                    len(w), ll, npairs))
                L.append("    # pairs of %s at v >= 10 m/s with their own speed and heading change removed" % src)
                L.append("    # and the quantisation, the stamp jitter (%.3f ms x v) and the output noise above" % (ssd * 1e3))
                L.append("    # deconvolved. Increments are white (lag-1 ~ 0), so the error is a random walk over the")
                L.append("    # fix epochs, mean-reverting with tau_s: the shortest tau (of %s s, capped at the receiver's" % (
                    "/".join("%g" % t for t in PN_TAU_GRID_S)))
                if "tau" not in lk:
                    lk["tau"] = position_tau(lk, on[0], on[1], ssd, pn)
                tau, tks = lk["tau"]
                L.append("    # 100 s smoothing) whose simulated exactly-100 ms residual matches these pairs, each ID")
                L.append("    # weighing one, within KS %g of the best; KS by tau: %s" % (
                    PN_TAU_KS_TOL, ", ".join("%g s %.3f" % (t, k) for t, k in tks)))
                L.append("    position_noise: {dist: normal_mixture, tau_s: %s, p: [%s], along_m: [%s], cross_m: [%s]}" % (
                    r6(tau), ", ".join(str(x) for x in sum_to_one(w)), ", ".join(str(r6(x)) for x in sa),
                    ", ".join(str(r6(x)) for x in sc)))
        if c == "locked":
            gh = grid_hop(lk)
            if gh:
                hh, ha, ah, aa = gh[2]
                L.append("    # :00/:50 grid hops: consecutive-msgCnt BSMs (Δt 50/100/150 ms) home->home %d, home->away %d," % (hh, ha))
                L.append("    # away->home %d, away->away %d; a hop is Δt 50 then 150 ms (or 150 then 50), the position" % (ah, aa))
                L.append("    # at the new instant. The engine puts the away grid half an interval late")
                L.append("    grid_hop: {leave_prob: %s, return_prob: %s}" % (r6(gh[0]), r6(gh[1])))
    return "\n".join(L) + "\n"
