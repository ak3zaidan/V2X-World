"""P1-E4 stamp-step refit: device split, chain rates, walk, step rate and size, speed coupling
(Tampa only), and an engine emulator that predicts the E4 KS against Wyoming (held out).

  python3 -I tools/realdata/e4stepfit.py fit <tampa.csv> [--boot N] > fit.json
  python3 -I tools/realdata/e4stepfit.py predict <wyoming.csv> fit.json [--wyoming-speeds]
  python3 -I tools/realdata/e4stepfit.py validate <wyoming.csv>
  python3 -I tools/realdata/e4stepfit.py yaml fit.json [--wyoming <wyoming.csv>]
  python3 -I tools/realdata/e4stepfit.py timing fit.json data/calibration/bsm-timing-cvpilot.yaml

`fit` reads Tampa only (the training set). `predict` and `validate` read Wyoming only as the
KS reference and for its speed distribution (a covariate the engine scenario is matched to,
scaled to the engine's own p50 of 29.8 m/s); nothing in `fit` sees Wyoming.

Pairs: msgCnt +1, secMark dt within 3 ms of 50/100/150 ms, as e4stampmc.py; o = secMark
offset from the nearest 50 ms epoch. Locked IDs: >= 70 % of secMarks within 1 ms of a 50 ms
epoch (e4stampmc.py). Within them:
  chain IDs  P(o = -1) >= CHAIN_MIN: the two-state stamp chain of research/synthesis/
             p1-realdata-baseline.md §7.2 (Wyoming's one OBU is one of these);
  fixed IDs  never o = -1: stamp always on the epoch.
Model of a chain device (the engine's StampChain, v2xw_node::bsm_timing):
  walk       the per-epoch position error increment, a Gaussian scale mixture, fitted with
             calib.position_noise on the chain IDs' exactly-100 ms pairs (o1 = o2: no stamp
             term, no transition) with their own kinematics removed, each ID weighing one;
  chain      stay(-1), stay(0) by maximum likelihood (transition counts), all speeds;
  step       at a 0 -> -1 transition, with probability PI, a lasting position step
             (+-LN(mu_a, s_a) along, +-LN(mu_c, s_c) across, independent signs and sizes),
             fitted by maximum likelihood on the 0 -> -1 pairs at v >= 10 m/s with the walk
             as the base density: each pair's (along, cross) residual after the +v x 1 ms
             stamp term is (walk increment) or (walk increment + step). Every step counts,
             including the ones under 5 cm, which the 2026-10-27 rate (share over 5 cm minus
             the share elsewhere) left out and whose size it took from the over-5-cm pairs;
  speed      the reported speed changes with the step by K x the along step (m/s per m),
             and the change persists (decay time TAU_V), with the position moving at the
             changed speed: later pairs show no residual (§ 2026-10-28 of the baseline).
"""
import bisect
import json
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import calib  # noqa: E402
import mbrchecks as mc  # noqa: E402
import normalise  # noqa: E402
import pairing  # noqa: E402

LOCKED_MIN = 0.7
ID_MIN_PAIRS = 50
# (core components, a second size law for the 0 -> 0 steps); the lowest BIC is the joint model
JOINT_MODELS = ((2, False), (2, True), (3, False))
CHAIN_MIN = 0.05
VMIN = 10.0
Z90 = 1.2815515655446004
# The shipped v3 locked class (data/calibration/bsm-timing-cvpilot.yaml, tools/realdata 1.3.0).
V3_WALK = {"p": [0.633315, 0.169124, 0.113336, 0.084225], "along_m": [0.0001, 0.0001, 0.0518646, 0.132925],
           "cross_m": [0.00323245, 0.0118151, 0.0309238, 0.0982351], "tau_s": 50.0}
V3_OUTPUT_NOISE = (0.0314503, 0.0524171)  # speed m/s, heading deg
V3_HOP = (0.113156, 0.445099)  # grid_hop leave, return
V3_INSTANT_MS = (0.285749, 0.254333)
# The 2026-10-27 profiles (bsm-stamp-epoch-candidates.yaml, StampChain::profile).
OLD_PROFILES = {
    "tampa_locked": {"stay_minus1": 0.235, "stay_zero": 0.918, "step_prob": (0.697 - 0.110) / (1 - 0.110),
                     "along_m": (0.079, 0.174), "cross_m": (0.049, 0.124)},
    "wyoming_obu": {"stay_minus1": 0.566, "stay_zero": 0.630, "step_prob": (0.483 - 0.047) / (1 - 0.047),
                    "along_m": (0.063, 0.130), "cross_m": (0.024, 0.058)},
}
# Engine runs of the stamp track (crates/v2xw-engine/tests/expectations/p1-stamp.yaml):
# x.e4.ks and the residual p50 / p75 / p90 cm on p1-e4-u.yaml, v p50 29.8 m/s.
ENGINE_RUNS = {
    "SB": ("instant", None, False, 0.150, (1.50, 2.70, 7.0)),
    "CT": ("chain", "tampa_locked", False, 0.130, (1.28, 2.93, 6.9)),
    "CTS": ("chain", "tampa_locked", True, 0.118, (1.29, 3.29, 9.4)),
    "CW": ("chain", "wyoming_obu", False, 0.062, (1.85, 3.76, 7.2)),
    "CWS": ("chain", "wyoming_obu", True, 0.074, (1.88, 4.30, 9.6)),
}
ENGINE_V_P50 = 29.8
# The emulator's one nuisance the engine adds that the timing model does not carry (its GNSS
# layer, mobility): a white increment per axis, m. Chosen on the engine runs' p50/p75/p90
# (the least squared log error over 3.5-6 mm in 0.5 mm steps; validate), never on Wyoming's
# KS. Predictions move with it: candidate E speed-matched 0.094 / 0.082 / 0.074 / 0.063 at
# 4 / 4.5 / 5 / 5.5 mm (research/synthesis/p1-realdata-baseline.md section 7.5).
EMU_EXTRA_M = 0.0045
LSB_M = math.radians(mc.POS_LSB_DEG) * pairing.R_EARTH  # the engine world sits at latitude 0
TAMPA_LAT = 27.95


# ---------------------------------------------------------------------------- data

def offset(sm):
    return (sm + 25) % 50 - 25


def sequences(ids):
    """Per locked ID: (id, P(o = -1), [pair record or None, in order])."""
    out = []
    for tid, bsms in ids:
        bs = pairing.dedupe_sort(bsms)
        sms = [b.secmark for b in bs if b.secmark is not None]
        if not sms or sum(min(s % 50, 50 - s % 50) <= 1 for s in sms) / len(sms) < LOCKED_MIN:
            continue
        seq = []
        for i, p in enumerate(pairing.pairs(bs)):
            dt = p.dt[0]
            if p.dcnt != 1 or dt is None or p.v1 is None or p.v2 is None or p.h1 is None:
                seq.append(None)
                continue
            ms = round(dt * 1000)
            nom = min((50, 100, 150), key=lambda n: abs(ms - n))
            if abs(ms - nom) > 3:
                seq.append(None)
                continue
            o1, o2 = offset(p.sm1), offset(p.sm2)
            ex, ey = mc.residual(p, dt)
            al, cr = mc.along_cross(p, ex, ey)
            dh = None if p.h2 is None else ((p.h2 - p.h1 + 180.0) % 360.0) - 180.0
            lsb_n = LSB_M
            lsb_e = lsb_n * math.cos(math.radians(bs[i].lat))
            seq.append({"v": p.v1, "v2": p.v2, "dt": dt, "nom": nom, "o1": o1, "o2": o2,
                        # the stamp term -v * (o2 - o1) * 1 ms removed (position on the epoch)
                        "al": al + p.v1 * 0.001 * (o2 - o1), "cr": cr, "dh": dh, "r": math.hypot(al, cr), "alr": al,
                        "q2": (lsb_n ** 2 + lsb_e ** 2) / 12.0})
        xs = [x for x in seq if x is not None]
        if not xs:
            continue
        pm = sum(x["o2"] == -1 for x in xs) / len(xs)
        out.append((tid, pm, seq))
    return out


def split(seqs):
    chain = [s for s in seqs if s[1] >= CHAIN_MIN]
    fixed = [s for s in seqs if s[1] == 0.0]
    return chain, fixed


def chain_rates(seqs, vmin=0.0):
    c = {(a, b): 0 for a in (-1, 0) for b in (-1, 0)}
    for _, _, seq in seqs:
        for x in seq:
            if x is not None and x["v"] >= vmin and (x["o1"], x["o2"]) in c:
                c[(x["o1"], x["o2"])] += 1
    s_m = c[(-1, -1)] / max(1, c[(-1, -1)] + c[(-1, 0)])
    s_0 = c[(0, 0)] / max(1, c[(0, 0)] + c[(0, -1)])
    return s_m, s_0, c


def walk_pairs(seqs, per_id=True):
    """calib.position_noise input: exactly-100 ms pairs without a stamp transition, own
    kinematics removed (pipeline.py's body pairs), weight 1 / the ID's pairs (per_id, the
    shipped fit's convention) or 1 (every pair weighs one)."""
    out = []
    for _, _, seq in seqs:
        mine = []
        for x in seq:
            if (x is None or x["v"] < VMIN or x["nom"] != 100 or x["o1"] != x["o2"] or x["dh"] is None
                    or round(x["dt"] * 1000) != 100 + 0):
                continue
            alc = x["al"] - 0.5 * (x["v2"] - x["v"]) * x["dt"]
            crc = x["cr"] + 0.5 * x["v"] * x["dt"] * math.radians(x["dh"])
            mine.append((alc, crc, x["v"], x["q2"]))
        out.extend(m + ((1.0 / len(mine)) if per_id else 1.0,) for m in mine)
    return out


def transition_pairs(seqs, vmin=VMIN):
    return [x for _, _, seq in seqs for x in seq
            if x is not None and x["v"] >= vmin and x["nom"] == 100 and (x["o1"], x["o2"]) == (0, -1)]


# ---------------------------------------------------------------------------- step likelihood

T_GRID = [-5.0 + 0.125 * i for i in range(81)]
T_W = [math.exp(-0.5 * t * t) / math.sqrt(2 * math.pi) * 0.125 for t in T_GRID]


def _slog(s, mu, sg):
    """The symmetric lognormal density (half the mass on each sign) at s."""
    a = abs(s)
    if a < 1e-9:
        return 0.0
    z = (math.log(a) - mu) / sg
    return 0.5 * math.exp(-0.5 * z * z) / (a * sg * math.sqrt(2 * math.pi))


def _conv(y, var, mu, sg):
    """(normal(0, var) * symmetric lognormal)(y), by E over the normal: trapezoid on t."""
    sd = math.sqrt(var)
    return sum(w * _slog(y - sd * t, mu, sg) for t, w in zip(T_GRID, T_W))


def _phi(y, var):
    return math.exp(-0.5 * y * y / var) / math.sqrt(2 * math.pi * var)


def nuisance(x, on=V3_OUTPUT_NOISE):
    na = x["q2"] + (on[0] * mc.FIX_INTERVAL_S) ** 2
    nc = x["q2"] + (x["v"] * mc.FIX_INTERVAL_S * math.radians(on[1])) ** 2
    return na, nc


def step_terms(obs, walk):
    """Per pair and walk component: (p_j, var_a, var_c, a, c). Precomputed once."""
    out = []
    for x in obs:
        na, nc = nuisance(x)
        out.append([(p, sa * sa + na, sc * sc + nc, x["al"], x["cr"])
                    for p, sa, sc in zip(walk["p"], walk["along_m"], walk["cross_m"])])
    return out


def loglik(theta, terms):
    """theta = (pi, mu_a, s_a, mu_c, s_c): pi the step probability at a 0 -> -1 transition."""
    pi, mu_a, s_a, mu_c, s_c = theta
    ll = 0.0
    for comps in terms:
        f = 0.0
        for p, va, vc, a, c in comps:
            f += p * ((1 - pi) * _phi(a, va) * _phi(c, vc) + pi * _conv(a, va, mu_a, s_a) * _conv(c, vc, mu_c, s_c))
        ll += math.log(max(f, 1e-300))
    return ll


def _unpack(u):
    return (1 / (1 + math.exp(-u[0])), u[1], math.exp(u[2]), u[3], math.exp(u[4]))


def _pack(t):
    return [math.log(t[0] / (1 - t[0])), t[1], math.log(t[2]), t[3], math.log(t[4])]


def nelder_mead(f, x0, step=0.3, iters=400, tol=1e-6):
    n = len(x0)
    pts = [list(x0)] + [[x0[j] + (step if j == i else 0.0) for j in range(n)] for i in range(n)]
    vals = [f(p) for p in pts]
    for _ in range(iters):
        order = sorted(range(n + 1), key=lambda i: vals[i])
        pts, vals = [pts[i] for i in order], [vals[i] for i in order]
        if abs(vals[-1] - vals[0]) < tol:
            break
        cen = [sum(p[j] for p in pts[:-1]) / n for j in range(n)]
        xr = [cen[j] + (cen[j] - pts[-1][j]) for j in range(n)]
        fr = f(xr)
        if fr < vals[0]:
            xe = [cen[j] + 2 * (cen[j] - pts[-1][j]) for j in range(n)]
            fe = f(xe)
            pts[-1], vals[-1] = (xe, fe) if fe < fr else (xr, fr)
        elif fr < vals[-2]:
            pts[-1], vals[-1] = xr, fr
        else:
            xc = [cen[j] + 0.5 * (pts[-1][j] - cen[j]) for j in range(n)]
            fc = f(xc)
            if fc < vals[-1]:
                pts[-1], vals[-1] = xc, fc
            else:
                pts = [pts[0]] + [[pts[0][j] + 0.5 * (p[j] - pts[0][j]) for j in range(n)] for p in pts[1:]]
                vals = [vals[0]] + [f(p) for p in pts[1:]]
    i = min(range(n + 1), key=lambda i: vals[i])
    return pts[i], vals[i]


def fit_steps(obs, walk, start=(0.6, math.log(0.06), 0.7, math.log(0.04), 0.8)):
    terms = step_terms(obs, walk)
    u, nll = nelder_mead(lambda u: -loglik(_unpack(u), terms), _pack(start))
    u, nll = nelder_mead(lambda u: -loglik(_unpack(u), terms), u, step=0.1)
    th = _unpack(u)
    null = -loglik((1e-9,) + th[1:], terms)
    return th, -nll, -null


def posterior(theta, obs, walk):
    """P(step | pair) for each 0 -> -1 pair."""
    pi, mu_a, s_a, mu_c, s_c = theta
    out = []
    for comps in step_terms(obs, walk):
        f0 = f1 = 0.0
        for p, va, vc, a, c in comps:
            f0 += p * (1 - pi) * _phi(a, va) * _phi(c, vc)
            f1 += p * pi * _conv(a, va, mu_a, s_a) * _conv(c, vc, mu_c, s_c)
        out.append(f1 / max(f0 + f1, 1e-300))
    return out


def lognormal_q(mu, sg):
    return math.exp(mu), math.exp(mu + Z90 * sg)


# ---------------------------------------------------------------------------- state-dependent steps
# Every chain-ID pair at once: the core walk (a K-component Gaussian scale mixture) and a step
# whose probability depends on the stamp states of the pair, P(step | 0 -> 0) = PI_00,
# P(step | 0 -> -1) = PI_T, P(step | from -1) = PI_M, with one lognormal size law (shared) or a
# second one for the 0 -> 0 steps (split). Pairs are kinematics-corrected as the walk's are.

J_GRID = [-4.0 + 0.2 * i for i in range(41)]
J_W = [math.exp(-0.5 * t * t) / math.sqrt(2 * math.pi) * 0.2 for t in J_GRID]
LABEL = {(0, 0): 0, (0, -1): 1, (-1, 0): 2, (-1, -1): 2}


def _jconv(y, var, mu, sg):
    sd = math.sqrt(var)
    return sum(w * _slog(y - sd * t, mu, sg) for t, w in zip(J_GRID, J_W))


def joint_pairs(seqs, vmin=VMIN):
    out = []
    for _, _, seq in seqs:
        for x in seq:
            if (x is None or x["v"] < vmin or x["nom"] != 100 or x["dh"] is None
                    or (x["o1"], x["o2"]) not in LABEL):
                continue
            na, nc = nuisance(x)
            out.append((x["al"] - 0.5 * (x["v2"] - x["v"]) * x["dt"],
                        x["cr"] + 0.5 * x["v"] * x["dt"] * math.radians(x["dh"]),
                        na, nc, LABEL[(x["o1"], x["o2"])]))
    return out


def joint_unpack(u, k, split_sizes):
    ew = [math.exp(min(50.0, v)) for v in [0.0] + list(u[:k - 1])]
    w = [e / sum(ew) for e in ew]
    sa = [math.exp(v) for v in u[k - 1:2 * k - 1]]
    sc = [math.exp(v) for v in u[2 * k - 1:3 * k - 1]]
    r = u[3 * k - 1:]
    pis = [1 / (1 + math.exp(-max(-50.0, min(50.0, v)))) for v in r[:3]]
    size_t = (r[3], math.exp(r[4]), r[5], math.exp(r[6]))
    size_0 = (r[7], math.exp(r[8]), r[9], math.exp(r[10])) if split_sizes else size_t
    return w, sa, sc, pis, size_t, size_0


def joint_nll(u, obs, k, split_sizes, pim_zero=False):
    w, sa, sc, pis, size_t, size_0 = joint_unpack(u, k, split_sizes)
    if pim_zero:
        pis[2] = 0.0
    tot = 0.0
    for a, c, na, nc, lab in obs:
        pi = pis[lab]
        mua, sga, muc, sgc = size_t if lab != 0 else size_0
        f = 0.0
        for j in range(k):
            va, vc = sa[j] ** 2 + na, sc[j] ** 2 + nc
            f1 = _jconv(a, va, mua, sga) * _jconv(c, vc, muc, sgc) if pi > 1e-6 else 0.0
            f += w[j] * ((1 - pi) * _phi(a, va) * _phi(c, vc) + pi * f1)
        tot -= math.log(max(f, 1e-300))
    return tot


def joint_start(k, split_sizes):
    u = [math.log(0.3 / 0.7)] * (k - 1) + [math.log(0.003 * 2 ** j) for j in range(k)] * 2
    u += [math.log(0.06 / 0.94), math.log(0.9 / 0.1), math.log(0.01 / 0.99), math.log(0.065), math.log(0.7),
          math.log(0.035), math.log(0.8)]
    if split_sizes:
        u += [math.log(0.065), math.log(0.7), math.log(0.035), math.log(0.8)]
    return u


def fit_joint(obs, k=2, split_sizes=False, u0=None, iters=3000):
    """Maximum likelihood (Nelder-Mead, two passes). Returns a dict with the core walk, the
    three step probabilities, the size law(s), the log-likelihood and its PI_M = 0 value."""
    u, v = nelder_mead(lambda x: joint_nll(x, obs, k, split_sizes), u0 or joint_start(k, split_sizes),
                       step=0.3, iters=iters)
    u, v = nelder_mead(lambda x: joint_nll(x, obs, k, split_sizes), u, step=0.1, iters=iters)
    w, sa, sc, pis, size_t, size_0 = joint_unpack(u, k, split_sizes)
    v0 = joint_nll(u, obs, k, split_sizes, pim_zero=True)

    def law(s):
        a50, a90 = lognormal_q(s[0], s[1])
        c50, c90 = lognormal_q(s[2], s[3])
        return {"along_p50_m": a50, "along_p90_m": a90, "cross_p50_m": c50, "cross_p90_m": c90,
                "mu_a": s[0], "s_a": s[1], "mu_c": s[2], "s_c": s[3]}
    order = sorted(range(k), key=lambda j: sa[j] ** 2 + sc[j] ** 2)
    return {"k": k, "split_sizes": split_sizes, "pairs": len(obs),
            "pairs_by_label": [sum(o[4] == i for o in obs) for i in range(3)],
            "core": {"p": [w[j] for j in order], "along_m": [sa[j] for j in order],
                     "cross_m": [sc[j] for j in order], "tau_s": V3_WALK["tau_s"]},
            "prob_stay_zero": pis[0], "prob_on_transition": pis[1], "prob_from_minus1": pis[2],
            "size": law(size_t), "size_stay_zero": law(size_0), "loglik": -v, "loglik_pim0": -v0, "u": list(u)}


# ---------------------------------------------------------------------------- speed coupling

def _slope(x, y):
    n = len(x)
    mx, my = sum(x) / n, sum(y) / n
    sxx = sum((a - mx) ** 2 for a in x)
    return sum((a - mx) * (b - my) for a, b in zip(x, y)) / sxx if sxx > 0 else 0.0


def speed_coupling(seqs, theta, walk, lags=(0, 1, 2, 3, 4, 5)):
    """K: slope of the speed change on the along step at the step BSM (pairs with P(step) >
    0.5); the slope of the cumulative speed change n BSMs later, and of the cumulative along
    residual of the next n pairs (0 if the position follows the reported speed)."""
    rec = {n: ([], [], []) for n in lags}
    k0 = ([], [])
    base = ([], [])
    # does the position move at the changed speed? the along residual of each of the next
    # pairs against -(v_i - v_before) * dt: slope 0 if it does, 1 if it keeps the old speed
    fol = ([], [])
    for _, _, seq in seqs:
        idx = [i for i, x in enumerate(seq) if x is not None and x["v"] >= VMIN and x["nom"] == 100
               and (x["o1"], x["o2"]) == (0, -1)]
        if not idx:
            continue
        post = posterior(theta, [seq[i] for i in idx], walk)
        for i, pst in zip(idx, post):
            x = seq[i]
            if pst <= 0.5:
                base[0].append(x["al"])
                base[1].append(x["v2"] - x["v"])
                continue
            k0[0].append(x["al"])
            k0[1].append(x["v2"] - x["v"])
            for y in seq[i + 1:i + 6]:
                if y is None:
                    break
                fol[0].append(-(y["v"] - x["v"]) * y["dt"])
                fol[1].append(y["al"])
            for n in lags:
                ys = seq[i:i + n + 1]
                if len(ys) < n + 1 or any(y is None for y in ys):
                    continue
                rec[n][0].append(x["al"])
                rec[n][1].append(ys[-1]["v2"] - x["v"])
                rec[n][2].append(sum(y["al"] for y in ys[1:]))
    k = _slope(*k0) if len(k0[0]) > 5 else None
    persist = [(n, len(a), _slope(a, dv), _slope(a, ca) if n else 0.0) for n, (a, dv, ca) in rec.items() if len(a) > 5]
    # TAU_V: least squares of log(slope_n / slope_0) = -n * 0.1 / tau over the lags with slope > 0
    tau = None
    if k and k > 0:
        pts = [(n * 0.1, math.log(s / k)) for n, _, s, _ in persist if n > 0 and s > 0]
        if pts:
            sxx = sum(t * t for t, _ in pts)
            b = sum(t * y for t, y in pts) / sxx
            tau = -1.0 / b if b < 0 else float("inf")
    follow = None
    if len(fol[0]) > 10:
        b = _slope(*fol)
        follow = {"slope": b, "pairs": len(fol[0]), "follow_fraction": min(1.0, max(0.0, 1.0 - b))}
    return {"k_mps_per_m": k, "n_steps": len(k0[0]), "persistence": persist, "tau_v_s": tau, "follow": follow,
            "k_nonstep_pairs": _slope(*base) if len(base[0]) > 5 else None, "n_nonstep": len(base[0])}


# ---------------------------------------------------------------------------- emulator

def emulate(speeds, n_pairs, stamp="chain", chain=None, step=None, step0=None, walk=V3_WALK, extra_m=EMU_EXTRA_M,
            k_v=0.0, tau_v=1.0, follow=1.0, seed=1, nbsm=300, hop=V3_HOP, on=V3_OUTPUT_NOISE, lat_deg=0.0,
            heading_deg=90.0, lsb_m=LSB_M):
    """The E4 highway as the engine builds it (bsm_timing.rs report_inner, locked class): a
    straight east-bound device at a constant speed drawn from `speeds`, the walk advancing one
    epoch per BSM (mixture increment, tau), the :00/:50 grid hop (error interpolated half an
    interval), output noise, J2735 quantisation (lat 0), and either the instant stamp
    (normal phase, position at the instant, secMark floored to 1 ms) or the chain (position on
    the epoch, secMark - 1 ms when early). chain = (stay_minus1, stay_zero), or a list of
    them drawn once per device; step = (prob, along p50, along p90, cross p50, cross p90) m at
    a 0 -> -1 transition, step0 the same at a 0 -> 0 BSM. k_v: the step's speed change per m of
    along step, decaying with tau_v, the position moving with it (follow 1) or keeping the old
    speed (follow 0). Returns the v1.0 residuals (m) of pairs at v1 >= 10 m/s and their
    (o1, o2) labels."""
    rng = random.Random(seed)
    out, lab = [], []
    rho = math.exp(-0.1 / walk["tau_s"])
    rho_v = math.exp(-0.1 / tau_v) if tau_v > 0 else 0.0
    al = [math.hypot(a, extra_m) for a in walk["along_m"]]
    cr = [math.hypot(c, extra_m) for c in walk["cross_m"]]
    cum = []
    acc = 0.0
    for p in walk["p"]:
        acc += p
        cum.append(acc)
    hstep = math.radians(mc.HEADING_LSB_DEG)
    lsb_n, lsb_e = lsb_m, lsb_m * math.cos(math.radians(lat_deg))
    while len(out) < n_pairs:
        v = rng.choice(speeds)
        # heading_deg None: a uniform heading per device (real devices); 90: the engine's
        # east-bound straight
        h0 = rng.uniform(0, 2 * math.pi) if heading_deg is None else math.radians(heading_deg)
        sh, ch = math.sin(h0), math.cos(h0)
        if stamp == "chain":
            s_m, s_0 = rng.choice(chain) if isinstance(chain, list) else chain
        ea = ec = 0.0
        dv = 0.0
        away = False
        early = None
        prev = None
        for k in range(nbsm):
            u = rng.random()
            j = min(bisect.bisect_right(cum, u), len(cum) - 1)
            wa, wc = rng.gauss(0, 1) * al[j], rng.gauss(0, 1) * cr[j]
            ua = rng.random()
            away = (ua >= hop[1]) if away else (ua < hop[0])
            cur = (ea, ec)
            dv *= rho_v
            d = None
            if stamp == "chain":
                pe = early
                uu = rng.random()
                if pe is None:
                    early = uu < (1 - s_0) / ((1 - s_0) + (1 - s_m))
                elif pe:
                    early = uu < s_m
                else:
                    early = uu >= s_0
                us, za, zc, sa, sc = rng.random(), rng.gauss(0, 1), rng.gauss(0, 1), rng.random(), rng.random()
                law = (step if early else step0) if pe is False else None
                if law and us < law[0]:
                    da = math.exp(math.log(law[1]) + za * math.log(law[2] / law[1]) / Z90)
                    dc = math.exp(math.log(law[3]) + zc * math.log(law[4] / law[3]) / Z90)
                    d = (-da if sa < 0.5 else da, -dc if sc < 0.5 else dc)
                stamp_ms, at_ms = (-1.0 if early else 0.0), 0.0
                o = -1 if early else 0
            else:
                jj = rng.gauss(*V3_INSTANT_MS)
                stamp_ms = at_ms = jj
                o = None
            if d:
                cur = (cur[0] + d[0], cur[1] + d[1])
                dv += k_v * d[0]
            # the error at the next epoch: walk increment, the step, and the speed offset's
            # displacement (the position moves at the reported speed)
            nxt = (rho * cur[0] + wa + follow * dv * 0.1, rho * cur[1] + wc)
            ea, ec = nxt
            f = 0.5 if away else 0.0
            err = (cur[0] + f * (nxt[0] - cur[0]), cur[1] + f * (nxt[1] - cur[1]))
            t_ms = k * 100 + (50 if away else 0)
            s_al = v * (t_ms + at_ms) * 1e-3 + err[0]
            east, north = s_al * sh - err[1] * ch, s_al * ch + err[1] * sh
            xq, yq = round(east / lsb_e) * lsb_e, round(north / lsb_n) * lsb_n
            sm_ms = math.floor(t_ms + stamp_ms + 1e-9)
            vr = round(max(0.0, v + dv + rng.gauss(0, on[0])) / mc.SPEED_RES_MPS) * mc.SPEED_RES_MPS
            hr = round((h0 + rng.gauss(0, math.radians(on[1]))) / hstep) * hstep
            b = (xq, yq, sm_ms, vr, hr, o)
            if prev is not None and prev[3] >= VMIN:
                dt = (b[2] - prev[2]) * 1e-3
                ex = (b[0] - prev[0]) - prev[3] * dt * math.sin(prev[4])
                ey = (b[1] - prev[1]) - prev[3] * dt * math.cos(prev[4])
                out.append(math.hypot(ex, ey))
                lab.append((prev[5], o))
            prev = b
    return out[:n_pairs], lab[:n_pairs]


def ks_at(a, b):
    a, b = sorted(a), sorted(b)
    best, at = 0.0, None
    for x in a + b:
        d = abs(bisect.bisect_right(a, x) / len(a) - bisect.bisect_right(b, x) / len(b))
        if d > best:
            best, at = d, x
    return best, at


def quant(xs, fs=(0.5, 0.75, 0.9, 0.99)):
    s = sorted(xs)
    return [s[min(len(s) - 1, int(f * len(s)))] for f in fs]


def wyoming_reference(path, v_p50=ENGINE_V_P50):
    """Wyoming's published E4 residual (tools/realworld/paper1/resid.py: v1.0, secMark dt,
    judged at 0.5 s, v1 >= 10 m/s) and its speeds scaled to the engine's p50 (v_p50 None:
    Wyoming's own speeds, a speed-matched scenario)."""
    real, speeds = [], []
    for _, bsms in normalise.stream_ids(path, "wyoming"):
        bs = pairing.dedupe_sort(bsms)
        for p in pairing.pairs(bs):
            dt = p.dt[0]
            if p.v1 is None or p.h1 is None or dt is None or dt <= 0 or p.v1 < VMIN or not mc.judged(dt, 0.5):
                continue
            _, r10, _ = mc.run(p, dt)
            real.append(r10)
            speeds.append(p.v1)
    sp = sorted(speeds)
    scale = v_p50 / sp[len(sp) // 2] if v_p50 else 1.0
    return real, [v * scale for v in speeds]


def over5_on_transition(res, lab):
    tr = [r for r, ll in zip(res, lab) if ll == (0, -1)]
    return sum(r > 0.05 for r in tr) / len(tr) if tr else None


# ---------------------------------------------------------------------------- commands

def step_block(obs, walk):
    """The joint step fit on one walk, with its parametric over-5-cm check."""
    th, ll, ll0 = fit_steps(obs, walk)
    a50, a90 = lognormal_q(th[1], th[2])
    c50, c90 = lognormal_q(th[3], th[4])
    rng = random.Random(7)
    hits = n = 0
    for x in obs * 40:
        j = random_component(rng, walk)
        na, nc = nuisance(x)
        a = rng.gauss(0, math.sqrt(walk["along_m"][j] ** 2 + na))
        c = rng.gauss(0, math.sqrt(walk["cross_m"][j] ** 2 + nc))
        if rng.random() < th[0]:
            a += math.copysign(math.exp(th[1] + th[2] * rng.gauss(0, 1)), rng.random() - 0.5)
            c += math.copysign(math.exp(th[3] + th[4] * rng.gauss(0, 1)), rng.random() - 0.5)
        hits += math.hypot(a, c) > 0.05
        n += 1
    return th, {"pairs": len(obs), "prob": th[0], "mu_a": th[1], "s_a": th[2], "mu_c": th[3], "s_c": th[4],
                "along_p50_m": a50, "along_p90_m": a90, "cross_p50_m": c50, "cross_p90_m": c90,
                "loglik": ll, "loglik_no_step": ll0,
                "over5_observed": sum(math.hypot(x["al"], x["cr"]) > 0.05 for x in obs) / len(obs),
                "over5_model": hits / n}


def in_sample(grp, rates, step, walk, n=20000, step0=None):
    """KS of the emulated device (no hops, no engine extras) against the group's own
    100 ms nominal pairs at v >= 10 m/s, kinematics removed: the training-set check that
    picks the walk."""
    use = [x for _, _, seq in grp for x in seq
           if x is not None and x["v"] >= VMIN and x["nom"] == 100 and x["dh"] is not None]
    # the residual with the pair's own kinematics removed, as the walk was fitted (the
    # emulated device drives straight at a constant speed); the stamp term stays in
    real = [math.hypot(x["alr"] - 0.5 * (x["v2"] - x["v"]) * x["dt"],
                       x["cr"] + 0.5 * x["v"] * x["dt"] * math.radians(x["dh"])) for x in use]
    speeds = [x["v"] for x in use]
    st = (step["prob"], step["along_p50_m"], step["along_p90_m"], step["cross_p50_m"], step["cross_p90_m"]) \
        if step else None
    res, _ = emulate(speeds, n, chain=rates, step=st, step0=step0, walk=walk, extra_m=0.0, hop=(0.0, 1.0),
                     lat_deg=TAMPA_LAT, heading_deg=None)
    return ks_at(res, real)[0], len(real)


def in_sample_labels(grp, rates, step, walk, step0, n=20000):
    """Per stamp transition: (pairs, real p50 / p90 / p99 m, emulated p50 / p90 / p99 m)."""
    use = [x for _, _, seq in grp for x in seq
           if x is not None and x["v"] >= VMIN and x["nom"] == 100 and x["dh"] is not None]
    st = (step["prob"], step["along_p50_m"], step["along_p90_m"], step["cross_p50_m"], step["cross_p90_m"])
    res, lab = emulate([x["v"] for x in use], n, chain=rates, step=st, step0=step0, walk=walk, extra_m=0.0,
                       hop=(0.0, 1.0), lat_deg=TAMPA_LAT, heading_deg=None)
    out = {}
    for lb in ((0, 0), (0, -1), (-1, 0), (-1, -1)):
        rr = [math.hypot(x["alr"] - 0.5 * (x["v2"] - x["v"]) * x["dt"],
                         x["cr"] + 0.5 * x["v"] * x["dt"] * math.radians(x["dh"]))
              for x in use if (x["o1"], x["o2"]) == lb]
        ee = [r for r, ll in zip(res, lab) if ll == lb]
        if rr and ee:
            out["%d%+d" % lb] = [len(rr), quant(rr, (0.5, 0.9, 0.99)), quant(ee, (0.5, 0.9, 0.99))]
    return out


def cmd_fit(path, boot=0):
    seqs = sequences(normalise.stream_ids(path, "tampa"))
    chain, fixed = split(seqs)
    doc = {"source": "nm7w-nvbm (Tampa), locked IDs", "locked_ids": len(seqs),
           "chain_ids": len(chain), "fixed_ids": len(fixed),
           "other_ids": len(seqs) - len(chain) - len(fixed)}
    for name, grp in (("chain", chain), ("fixed", fixed)):
        s_m, s_0, c = chain_rates(grp)
        s_m10, s_010, _ = chain_rates(grp, VMIN)
        g = {"ids": len(grp), "transition_counts": {"%d%+d" % k: n for k, n in c.items()},
             "stay_minus1": s_m, "stay_zero": s_0, "stay_minus1_v10": s_m10, "stay_zero_v10": s_010,
             "pairs_v10": sum(1 for _, _, seq in grp for x in seq if x is not None and x["v"] >= VMIN),
             "speed_p50_v10": quant([x["v"] for _, _, seq in grp for x in seq
                                     if x is not None and x["v"] >= VMIN], (0.5,))[0]}
        for wname, per_id in (("walk", True), ("walk_per_pair", False)):
            wp = walk_pairs(grp, per_id)
            pn = calib.position_noise({"pairs": wp}, V3_OUTPUT_NOISE[0], V3_OUTPUT_NOISE[1], 0.0)
            if pn:
                w, sa, sc, npairs, ll = pn
                g[wname] = {"p": w, "along_m": sa, "cross_m": sc, "tau_s": V3_WALK["tau_s"], "loglik": ll,
                            "pairs": npairs}
        allp = [x for _, _, seq in grp for x in seq if x is not None and x["v"] >= VMIN]
        oth = [x for x in allp if (x["o1"], x["o2"]) != (0, -1)]
        g["over5_other"] = sum(math.hypot(x["al"], x["cr"]) > 0.05 for x in oth) / max(1, len(oth))
        doc[name] = g
    obs = transition_pairs(chain)
    rates = (doc["chain"]["stay_minus1"], doc["chain"]["stay_zero"])
    best = None
    for wname in ("walk", "walk_per_pair"):
        walk = doc["chain"][wname]
        th, blk = step_block(obs, walk)
        blk["in_sample_ks"], blk["in_sample_pairs"] = in_sample(chain, rates, blk, walk)
        blk["in_sample_ks_no_step"], _ = in_sample(chain, rates, None, walk)
        blk["in_sample_ks_v3_walk"], _ = in_sample(chain, rates, blk, V3_WALK)
        blk["walk"] = wname
        doc["step_" + wname] = blk
        if best is None or blk["in_sample_ks"] < best[2]["in_sample_ks"]:
            best = (wname, th, blk)
    wname, th, blk = best
    walk = doc["chain"][wname]
    doc["chosen_walk"] = wname
    doc["step"] = blk
    bands = []
    for lo, hi in ((0.5, 5.0), (5.0, 10.0), (10.0, 20.0), (20.0, 99.0)):
        tr = [x for x in transition_pairs(chain, 0.0) if lo <= x["v"] < hi]
        bands.append((lo, hi, len(tr), sum(math.hypot(x["al"], x["cr"]) > 0.05 for x in tr) / max(1, len(tr))))
    doc["step"]["over5_by_speed"] = bands
    doc["speed"] = speed_coupling(chain, th, walk)
    doc["speed_fixed_ids"] = speed_coupling_fixed(fixed)
    # the chain rates of each chain ID with at least ID_MIN_PAIRS two-state pairs (all speeds)
    per = []
    for tid, pm, seq in chain:
        s_m, s_0, c = chain_rates([(tid, pm, seq)])
        if sum(c.values()) >= ID_MIN_PAIRS and 0 < s_m < 1 and 0 < s_0 < 1:
            per.append({"pairs": sum(c.values()), "stay_minus1": s_m, "stay_zero": s_0,
                        "p_minus1": (1 - s_0) / ((1 - s_0) + (1 - s_m)),
                        "counts": [c[(-1, -1)], c[(-1, 0)], c[(0, 0)], c[(0, -1)]]})
    doc["chain"]["per_id"] = per
    doc["chain"]["heterogeneity"] = heterogeneity([r["counts"] for r in per])
    # state-dependent steps over every chain-ID pair (the joint model)
    jobs = joint_pairs(chain)
    fits = []
    for k, split_sizes in JOINT_MODELS:
        f = fit_joint(jobs, k, split_sizes)
        npar = 3 * k - 1 + 3 + (8 if split_sizes else 4)
        f["bic"] = -2 * f["loglik"] + npar * math.log(len(jobs))
        f["npar"] = npar
        fits.append(f)
    best_j = min(fits, key=lambda f: f["bic"])
    doc["joint_models"] = fits
    doc["joint"] = best_j
    lw = best_j["size"]
    l0 = best_j["size_stay_zero"]
    st = {"prob": best_j["prob_on_transition"], "along_p50_m": lw["along_p50_m"], "along_p90_m": lw["along_p90_m"],
          "cross_p50_m": lw["cross_p50_m"], "cross_p90_m": lw["cross_p90_m"]}
    st0 = (best_j["prob_stay_zero"], l0["along_p50_m"], l0["along_p90_m"], l0["cross_p50_m"], l0["cross_p90_m"])
    best_j["in_sample_ks"], _ = in_sample(chain, rates, st, best_j["core"], step0=st0)
    best_j["in_sample_by_label"] = in_sample_labels(chain, rates, st, best_j["core"], st0)
    if boot:
        reps = bootstrap(chain, best_j, boot)
        doc["joint_boot"] = reps
        doc["joint_ci"] = {name: interval([g(r) for r in reps]) for name, g in (
            ("stay_minus1", lambda r: r["stay_minus1"]), ("stay_zero", lambda r: r["stay_zero"]),
            ("prob_on_transition", lambda r: r["prob_on_transition"]),
            ("prob_stay_zero", lambda r: r["prob_stay_zero"]), ("prob_from_minus1", lambda r: r["prob_from_minus1"]),
            ("along_p50_m", lambda r: r["size"]["along_p50_m"]), ("along_p90_m", lambda r: r["size"]["along_p90_m"]),
            ("cross_p50_m", lambda r: r["size"]["cross_p50_m"]), ("cross_p90_m", lambda r: r["size"]["cross_p90_m"]))}
    return doc


def _chi2_sf(x, df):
    """Wilson-Hilferty: P(chi2_df > x)."""
    if df <= 0:
        return 1.0
    z = ((x / df) ** (1 / 3) - (1 - 2 / (9 * df))) / math.sqrt(2 / (9 * df))
    return 0.5 * math.erfc(z / math.sqrt(2))


def heterogeneity(counts):
    """Do the chain IDs share one pair of rates? Pearson chi-square of the IDs x {stay, leave}
    table from each state ([c(-1,-1), c(-1,0), c(0,0), c(0,-1)] per ID)."""
    out = {}
    for name, (i, j) in (("stay_minus1", (0, 1)), ("stay_zero", (2, 3))):
        rows = [(r[i], r[j]) for r in counts if r[i] + r[j] > 0]
        tot = sum(a + b for a, b in rows)
        pa = sum(a for a, _ in rows) / tot
        x2 = 0.0
        for a, b in rows:
            n = a + b
            ea, eb = n * pa, n * (1 - pa)
            x2 += (a - ea) ** 2 / max(ea, 1e-9) + (b - eb) ** 2 / max(eb, 1e-9)
        out[name] = {"chi2": x2, "df": len(rows) - 1, "p": _chi2_sf(x2, len(rows) - 1)}
    return out


def random_component(rng, walk):
    u, acc = rng.random(), 0.0
    for j, p in enumerate(walk["p"]):
        acc += p
        if u < acc:
            return j
    return len(walk["p"]) - 1


def speed_coupling_fixed(fixed):
    """Fixed-stamp IDs have no marker: K over their pairs with a residual > 5 cm."""
    a, dv = [], []
    for _, _, seq in fixed:
        for x in seq:
            if x is not None and x["v"] >= VMIN and x["nom"] == 100 and math.hypot(x["al"], x["cr"]) > 0.05:
                a.append(x["al"])
                dv.append(x["v2"] - x["v"])
    return {"k_mps_per_m": _slope(a, dv) if len(a) > 5 else None, "n": len(a)}


def bootstrap(chain, joint, n, seed=11):
    """Resampling the chain IDs: the joint model and the chain rates refitted on each
    replicate (started at the full fit), for the parameters' 95 % intervals and, in predict,
    the predicted KS's spread from the training sample alone."""
    rng = random.Random(seed)
    reps = []
    for _ in range(n):
        smp = [rng.choice(chain) for _ in chain]
        obs = joint_pairs(smp)
        if sum(o[4] == 1 for o in obs) < 20:
            continue
        f = fit_joint(obs, joint["k"], joint["split_sizes"], u0=joint["u"], iters=1500)
        s_m, s_0, _ = chain_rates(smp)
        reps.append({"stay_minus1": s_m, "stay_zero": s_0, "core": f["core"],
                     "prob_on_transition": f["prob_on_transition"], "prob_stay_zero": f["prob_stay_zero"],
                     "prob_from_minus1": f["prob_from_minus1"], "size": f["size"],
                     "size_stay_zero": f["size_stay_zero"]})
    return reps


def interval(xs):
    s = sorted(xs)
    return [s[int(0.025 * (len(s) - 1))], s[len(s) // 2], s[int(0.975 * (len(s) - 1))]]


def candidates(fit):
    """The candidate engine configurations, each (name, emulate kwargs, note)."""
    ch, st, sp = fit["chain"], fit["step"], fit["speed"]
    chain_rates_c = (ch["stay_minus1"], ch["stay_zero"])
    step_c = (st["prob"], st["along_p50_m"], st["along_p90_m"], st["cross_p50_m"], st["cross_p90_m"])
    old = OLD_PROFILES["tampa_locked"]
    old_step = (old["step_prob"],) + old["along_m"] + old["cross_m"]
    walk_c = dict(ch[fit["chosen_walk"]])
    k = sp["k_mps_per_m"] or 0.0
    tau = sp["tau_v_s"] if sp["tau_v_s"] and math.isfinite(sp["tau_v_s"]) else 1.0
    fol = sp["follow"]["follow_fraction"] if sp.get("follow") else 1.0
    wy = OLD_PROFILES["wyoming_obu"]
    c = {"chain": chain_rates_c, "step": step_c, "walk": walk_c}
    j = fit["joint"]
    jl, j0 = j["size"], j["size_stay_zero"]
    e = {"chain": chain_rates_c, "walk": dict(j["core"]),
         "step": (j["prob_on_transition"], jl["along_p50_m"], jl["along_p90_m"], jl["cross_p50_m"], jl["cross_p90_m"]),
         "step0": (j["prob_stay_zero"], j0["along_p50_m"], j0["along_p90_m"], j0["cross_p50_m"], j0["cross_p90_m"])}
    per = [(r["stay_minus1"], r["stay_zero"]) for r in ch["per_id"]]
    return [
        ("instant (shipped v3, SB)", {"stamp": "instant"}, "engine 0.150"),
        ("tampa_locked 10-27 (CTS)", {"chain": (old["stay_minus1"], old["stay_zero"]), "step": old_step}, "engine 0.118"),
        ("A chain-ID rates, 10-27 steps, v3 walk", {"chain": chain_rates_c, "step": old_step}, ""),
        ("B chain-ID rates, transition steps (all sizes), v3 walk", {"chain": chain_rates_c, "step": step_c}, ""),
        ("C = B on the chain-ID walk", dict(c), ""),
        ("E chain-ID rates, core walk, steps from state 0 (joint)", dict(e), ""),
        ("E + speed change, position follows %.2f" % fol, dict(e, k_v=k, tau_v=tau, follow=fol), ""),
        ("E + speed change, position follows 1", dict(e, k_v=k, tau_v=tau, follow=1.0), ""),
        ("E + speed change, position does not follow", dict(e, k_v=k, tau_v=tau, follow=0.0), ""),
        ("F = E with per-device rates (%d chain IDs)" % len(per), dict(e, chain=per), ""),
        ("F + speed change, follows %.2f" % fol, dict(e, chain=per, k_v=k, tau_v=tau, follow=fol), ""),
        ("E without steps", {"chain": chain_rates_c, "walk": dict(j["core"])}, ""),
        ("wyoming_obu 10-27 (CWS, in sample)", {"chain": (wy["stay_minus1"], wy["stay_zero"]),
                                                 "step": (wy["step_prob"],) + wy["along_m"] + wy["cross_m"]},
         "engine 0.074"),
    ]


def cmd_predict(wy_path, fit_path, n=20000, seeds=(1, 2, 3), v_p50=ENGINE_V_P50):
    with open(fit_path) as f:
        fit = json.load(f)
    real, speeds = wyoming_reference(wy_path, v_p50)
    rq = quant(real)
    print("speeds: %s" % ("Wyoming's own" if not v_p50 else "Wyoming's scaled to p50 %.1f m/s (the E4 runs)" % v_p50))
    print("Wyoming (held out): %d pairs, residual cm p50/p75/p90/p99 %s" % (
        len(real), " / ".join("%.2f" % (100 * x) for x in rq)))
    print("| candidate | predicted KS (seeds) | at cm | p50 / p75 / p90 / p99 cm | 0->-1 over 5 cm | note |")
    print("|---|---|---|---|---|---|")
    rows = []
    for name, kw, note in candidates(fit):
        ks_s, at_s, qs, o5 = [], [], None, None
        for sd in seeds:
            res, lab = emulate(speeds, n, seed=sd, **kw)
            k, at = ks_at(res, real)
            ks_s.append(k)
            at_s.append(at)
            if qs is None:
                qs, o5 = quant(res), over5_on_transition(res, lab)
        m = sum(ks_s) / len(ks_s)
        rows.append((name, m))
        print("| %s | %.3f (%.3f-%.3f) | %.2f | %s | %s | %s |" % (
            name, m, min(ks_s), max(ks_s), 100 * sorted(at_s)[len(at_s) // 2],
            " / ".join("%.2f" % (100 * x) for x in qs), "-" if o5 is None else "%.2f" % o5, note))
    # held-out structure: P(residual > 5 cm | stamp transition), Wyoming against candidates C, E
    wy = [x for _, _, seq in sequences(normalise.stream_ids(wy_path, "wyoming")) for x in seq
          if x is not None and x["v"] >= VMIN]
    cands = {name: kw for name, kw, _ in candidates(fit)}
    print("| P(r > 5 cm \\| transition) | -1 -> -1 | -1 -> 0 | 0 -> 0 | 0 -> -1 |")
    print("|---|---|---|---|---|")
    labs = ((-1, -1), (-1, 0), (0, 0), (0, -1))

    def share(rs, ls, lb):
        h = [r > 0.05 for r, ll in zip(rs, ls) if ll == lb]
        return "%.3f (%d)" % (sum(h) / len(h), len(h)) if h else "-"
    print("| Wyoming (held out) | %s |" % " | ".join(share([x["r"] for x in wy], [(x["o1"], x["o2"]) for x in wy], lb)
                                                  for lb in labs))
    for name in [c for c in cands if c.startswith(("C ", "E chain"))]:
        res, lab = emulate(speeds, n, seed=1, **cands[name])
        print("| %s | %s |" % (name, " | ".join(share(res, lab, lb).split(" ")[0] for lb in labs)))
    reps = fit.get("joint_boot") or []
    if reps:
        # candidate E on each bootstrap replicate of the Tampa fit (training-sample spread)
        ks_b = []
        for i, r in enumerate(reps):
            lw, l0 = r["size"], r["size_stay_zero"]
            res, _ = emulate(speeds, n // 2, seed=100 + i, chain=(r["stay_minus1"], r["stay_zero"]), walk=r["core"],
                             step=(r["prob_on_transition"], lw["along_p50_m"], lw["along_p90_m"], lw["cross_p50_m"],
                                   lw["cross_p90_m"]),
                             step0=(r["prob_stay_zero"], l0["along_p50_m"], l0["along_p90_m"], l0["cross_p50_m"],
                                    l0["cross_p90_m"]))
            ks_b.append(ks_at(res, real)[0])
        lo, mid, hi = interval(ks_b)
        print("E over %d bootstrap replicates of the Tampa fit: KS %.3f (95 %% %.3f-%.3f); share <= 0.092: %.2f" % (
            len(ks_b), mid, lo, hi, sum(k <= 0.092 for k in ks_b) / len(ks_b)))
        rows.append(("E bootstrap", mid))
    return rows


def cmd_validate(wy_path, n=20000):
    real, speeds = wyoming_reference(wy_path)
    print("| run | engine KS | emulator KS | engine p50/p75/p90 cm | emulator p50/p75/p90 cm |")
    print("|---|---|---|---|---|")
    diffs = []
    for name, (stamp, prof, steps, eks, eq) in ENGINE_RUNS.items():
        kw = {"stamp": stamp}
        if prof:
            o = OLD_PROFILES[prof]
            kw["chain"] = (o["stay_minus1"], o["stay_zero"])
            if steps:
                kw["step"] = (o["step_prob"],) + o["along_m"] + o["cross_m"]
        res, _ = emulate(speeds, n, **kw)
        k, _ = ks_at(res, real)
        diffs.append(k - eks)
        print("| %s | %.3f | %.3f | %s | %s |" % (name, eks, k, " / ".join("%.2f" % x for x in eq),
                                                 " / ".join("%.2f" % (100 * x) for x in quant(res)[:3])))
    print("emulator - engine KS: mean %+.3f, range %+.3f..%+.3f" % (sum(diffs) / len(diffs), min(diffs), max(diffs)))
    return diffs


def _f(x, nd=6):
    return ("%%.%dg" % nd) % x


def _lst(xs, nd=6):
    return "[" + ", ".join(_f(x, nd) for x in xs) + "]"


def _ci(fit, name, scale=1.0, nd=3):
    ci = fit.get("joint_ci", {}).get(name)
    return "" if not ci else "  # 95 %% bootstrap over chain IDs %s-%s" % (_f(scale * ci[0], nd), _f(scale * ci[2], nd))


def _cin(fit, name, what, scale=1.0):
    """A comment: what, then the bootstrap interval."""
    c = _ci(fit, name, scale)
    return "  # " + what + ((";" + c[3:]) if c else "")


def cmd_yaml(fit_path, wy_path=None):
    """The candidates file, data/calibration/bsm-stamp-steps-candidates.yaml."""
    import contextlib
    import io
    with open(fit_path) as f:
        fit = json.load(f)
    ch, fx, j, sp, st = fit["chain"], fit["fixed"], fit["joint"], fit["speed"], fit["step"]
    sz, sz0 = j["size"], j["size_stay_zero"]
    cw = ch[fit["chosen_walk"]]
    pm = (1 - ch["stay_zero"]) / ((1 - ch["stay_zero"]) + (1 - ch["stay_minus1"]))
    het = ch.get("heterogeneity", {})
    L = [
        "# CANDIDATE (2026-10-28): the locked-class stamp chain refitted on Tampa's chain devices,",
        "# with its position steps fitted jointly in rate and size, steps tied to the stamp state,",
        "# and the speed change that comes with a step. Supersedes the step numbers of",
        "# bsm-stamp-epoch-candidates.yaml (2026-10-27), whose rate counted only steps over 5 cm",
        "# and whose size came from the pairs over 5 cm. Analysis: research/synthesis/",
        "# p1-realdata-baseline.md section 7.5. Derived statistics only; no raw rows.",
        "# GENERATED by `python3 -I tools/realdata/e4stepfit.py yaml fit.json --wyoming <wyoming.csv>`",
        "# from `e4stepfit.py fit <tampa.csv> --boot 40` - do not edit by hand.",
        "provenance:",
        "  sources:",
        "    - {dataset: nm7w-nvbm, role: fit, note: \"Tampa, every fitted number; pinned query of tools/realdata/sources.py\"}",
        "    - {dataset: 7qu7-j8rx, role: held_out, note: \"Wyoming, one OBU: read only by predict (KS reference, speed covariate)\"}",
        "  manifest_sha256: ee6eeab3f926d0fefac2e790d0dea9c5411531d7823d39558620a5521189d412",
        "  tool: \"tools/realdata/e4stepfit.py (tests in test_e4stepfit.py)\"",
        "  pairs: \"msgCnt +1, secMark dt within 3 ms of 50/100/150 ms (fits: 100 ms nominal), v1 >= 10 m/s; "
        "o = secMark offset from the nearest 50 ms epoch\"",
        "device_split:  # Tampa locked IDs (>= 70 % of secMarks within 1 ms of a 50 ms epoch)",
        "  locked_ids: %d" % fit["locked_ids"],
        "  chain_ids: %d      # P(o = -1) >= %s: the two-state stamp chain (Wyoming's OBU is one)" % (
            fit["chain_ids"], _f(CHAIN_MIN)),
        "  fixed_ids: %d      # never o = -1" % fit["fixed_ids"],
        "model: epoch_stamp_chain_state_steps",
        "position: fix_epoch",
        "stamp_offset_ms: [-1, 0]",
        "profiles:",
        "  tampa_chain:   # candidate E, recommended: Tampa chain devices, joint model (lowest BIC)",
        "    stay_minus1: %s%s" % (_f(ch["stay_minus1"], 4), _ci(fit, "stay_minus1")),
        "    stay_zero: %s%s" % (_f(ch["stay_zero"], 4), _ci(fit, "stay_zero")),
        "    p_offset_minus1: %s" % _f(pm, 4),
        "    transition_counts: {m1_m1: %d, m1_0: %d, 0_0: %d, 0_m1: %d}   # all speeds" % (
            ch["transition_counts"]["-1-1"], ch["transition_counts"]["-1+0"], ch["transition_counts"]["0+0"],
            ch["transition_counts"]["0-1"]),
        "    # the error walk without the steps (core): Gaussian scale mixture per 100 ms epoch",
        "    walk: {dist: normal_mixture, tau_s: %s, p: %s, along_m: %s, cross_m: %s}" % (
            "%.1f" % j["core"]["tau_s"], _lst(j["core"]["p"]), _lst(j["core"]["along_m"]), _lst(j["core"]["cross_m"])),
        "    step:   # a lasting position correction; independent signs and lognormal sizes",
        "      prob_on_transition: %s%s" % (_f(j["prob_on_transition"], 4),
                                          _cin(fit, "prob_on_transition", "at a 0 -> -1 BSM")),
        "      prob_stay_zero: %s%s" % (_f(j["prob_stay_zero"], 4), _cin(fit, "prob_stay_zero", "at a 0 -> 0 BSM")),
        "      prob_from_minus1: 0.0   # MLE %s; Wyoming (held out): 0 of 1282 -1 -> -1 pairs over 5 cm" % (
            _f(j["prob_from_minus1"], 2)),
        "      along_cm: {p50: %s, p90: %s}%s" % (_f(100 * sz["along_p50_m"], 3), _f(100 * sz["along_p90_m"], 3),
                                              _cin(fit, "along_p50_m", "p50", 100)),
        "      cross_cm: {p50: %s, p90: %s}%s" % (_f(100 * sz["cross_p50_m"], 3), _f(100 * sz["cross_p90_m"], 3),
                                              _cin(fit, "cross_p50_m", "p50", 100)),
        "    speed:   # the reported speed steps with the along step and relaxes; v >= 10 m/s chain IDs",
        "      per_m_along_mps: %s   # dv = this x along step (m), at the step BSM (%d steps)" % (
            _f(sp["k_mps_per_m"], 3), sp["n_steps"]),
        "      tau_s: %s           # decay of the speed offset, from the lag 0-5 slopes" % _f(sp["tau_v_s"], 3),
        "      position_follows: %s  # share of the offset the position moves with (1: no residual later)" % (
            _f(sp["follow"]["follow_fraction"], 3)),
        "    # per-device rates (candidate F): chain IDs with >= %d pairs; one pair drawn per device" % ID_MIN_PAIRS,
        "    per_device_rates: [%s]" % ", ".join("[%s, %s]" % (_f(r["stay_minus1"], 3), _f(r["stay_zero"], 3))
                                                for r in ch["per_id"]),
    ]
    if het:
        L.append("    # one rate pair for all IDs? chi-square stay_minus1 %.1f (df %d, p %.1g), stay_zero %.1f (df %d, p %.1g)" % (
            het["stay_minus1"]["chi2"], het["stay_minus1"]["df"], het["stay_minus1"]["p"],
            het["stay_zero"]["chi2"], het["stay_zero"]["df"], het["stay_zero"]["p"]))
    L += [
        "    speed_p50_mps: %s   # Tampa chain IDs, v >= 10 m/s (Wyoming 34.4)" % _f(ch["speed_p50_v10"], 3),
        "  tampa_chain_transition_only:   # candidate C: steps at 0 -> -1 only, the walk carries the rest",
        "    stay_minus1: %s" % _f(ch["stay_minus1"], 4),
        "    stay_zero: %s" % _f(ch["stay_zero"], 4),
        "    walk: {dist: normal_mixture, tau_s: %s, p: %s, along_m: %s, cross_m: %s}" % (
            "%.1f" % cw["tau_s"], _lst(cw["p"]), _lst(cw["along_m"]), _lst(cw["cross_m"])),
        "    step: {prob_on_transition: %s, along_cm: {p50: %s, p90: %s}, cross_cm: {p50: %s, p90: %s}}" % (
            _f(st["prob"], 4), _f(100 * st["along_p50_m"], 3), _f(100 * st["along_p90_m"], 3),
            _f(100 * st["cross_p50_m"], 3), _f(100 * st["cross_p90_m"], 3)),
        "  tampa_fixed:   # the other Tampa locked devices: stamp always on the epoch, no chain",
        "    walk: {dist: normal_mixture, tau_s: %s, p: %s, along_m: %s, cross_m: %s}" % (
            "%.1f" % fx["walk_per_pair"]["tau_s"], _lst(fx["walk_per_pair"]["p"]), _lst(fx["walk_per_pair"]["along_m"]),
            _lst(fx["walk_per_pair"]["cross_m"])),
        "    pairs_over_5cm: %s" % _f(fx["over5_other"], 3),
        "    speed_per_m_along_mps: %s   # over its pairs > 5 cm (%d)" % (
            _f(fit["speed_fixed_ids"]["k_mps_per_m"], 3), fit["speed_fixed_ids"]["n"]),
        "# Fit checks (Tampa, training): joint model log-lik %.1f, %d pairs (by first state 0->0 / 0->-1 / from -1: %s);" % (
            j["loglik"], j["pairs"], " / ".join(str(n) for n in j["pairs_by_label"])),
        "# emulated chain device against Tampa's own chain pairs: KS %.3f (E), %.3f (C), %.3f (C's steps on the v3 walk)." % (
            j["in_sample_ks"], st["in_sample_ks"], st["in_sample_ks_v3_walk"]),
    ]
    for f in fit["joint_models"]:
        L.append("#   joint K=%d sizes %s: log-lik %.1f, BIC %.1f" % (f["k"], "split" if f["split_sizes"] else "shared",
                                                                      f["loglik"], f["bic"]))
    if wy_path:
        for v in (ENGINE_V_P50, None):
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                cmd_predict(wy_path, fit_path, v_p50=v)
            L.append("# Predicted P1-E4 (held out, emulator; engine runs reproduced within -0.004..+0.018):")
            L += ["#   " + ln for ln in buf.getvalue().splitlines()]
    print("\n".join(L))


def cmd_timing(fit_path, shipped_path):
    """data/calibration/bsm-timing-cvpilot-chain-device.yaml: the shipped v3 file with the
    locked class's position_noise replaced by the chain devices' core walk (candidate E)."""
    with open(fit_path) as f:
        fit = json.load(f)
    with open(shipped_path) as f:
        lines = f.read().splitlines()
    core = fit["joint"]["core"]
    new = "    position_noise: {dist: normal_mixture, tau_s: %s, p: %s, along_m: %s, cross_m: %s}" % (
        "%.1f" % core["tau_s"], _lst(core["p"]), _lst(core["along_m"]), _lst(core["cross_m"]))
    out = [
        "# CANDIDATE (2026-10-28): data/calibration/bsm-timing-cvpilot.yaml (tools/realdata 1.3.0) with ONE",
        "# change: the locked class's position_noise is the core walk of Tampa's chain devices from",
        "# the joint stamp-step model (bsm-stamp-steps-candidates.yaml, profile tampa_chain), whose",
        "# steps come from the stamp chain instead of the walk. Use it only with stamp_chain on and",
        "# the tampa_chain steps (from state 0); without them the locked tail is missing.",
        "# GENERATED by `python3 -I tools/realdata/e4stepfit.py timing fit.json "
        "data/calibration/bsm-timing-cvpilot.yaml` - do not edit by hand.",
    ]
    cls, done = None, False
    for ln in lines:
        s = ln.strip()
        if s.startswith("- name:"):
            cls = s.split(":", 1)[1].strip()
        if cls == "locked" and s.startswith("position_noise:") and not done:
            out.append("    # was: " + s)
            out.append(new)
            done = True
            continue
        out.append(ln)
    if not done:
        raise SystemExit("no locked position_noise in " + shipped_path)
    print("\n".join(out))


def main(argv):
    if len(argv) >= 2 and argv[0] == "fit":
        boot = int(argv[argv.index("--boot") + 1]) if "--boot" in argv else 0
        json.dump(cmd_fit(argv[1], boot), sys.stdout, indent=1)
        print()
    elif len(argv) in (3, 4) and argv[0] == "predict":
        cmd_predict(argv[1], argv[2], v_p50=None if argv[3:] == ["--wyoming-speeds"] else ENGINE_V_P50)
    elif len(argv) == 2 and argv[0] == "validate":
        cmd_validate(argv[1])
    elif len(argv) in (2, 4) and argv[0] == "yaml":
        cmd_yaml(argv[1], argv[3] if argv[2:3] == ["--wyoming"] else None)
    elif len(argv) == 3 and argv[0] == "timing":
        cmd_timing(argv[1], argv[2])
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
