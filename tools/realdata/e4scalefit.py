"""P1-E4 body refit (2026-10-28): a per-device reported-speed scale error fitted on Tampa, the
chain-device core walk refitted without it, and the engine's own GNSS nuisance refitted on the
speed-matched engine runs; the E4 KS predicted against Wyoming (held out).

  python3 -I tools/realdata/e4scalefit.py fit <tampa.csv> <stepfit.json> [--boot N] > scalefit.json
  python3 -I tools/realdata/e4scalefit.py nuisance <wyoming.csv> <stepfit.json>
  python3 -I tools/realdata/e4scalefit.py predict <wyoming.csv> <stepfit.json> <scalefit.json>
  python3 -I tools/realdata/e4scalefit.py yaml <scalefit.json> <chain-device.yaml> > out.yaml

<stepfit.json> is `e4stepfit.py fit <tampa.csv> --boot 40` (candidate E). `fit` reads Tampa
(nm7w-nvbm) only. `nuisance` and `predict` read Wyoming (7qu7-j8rx) only for its speeds (a
covariate: the E4 run is speed-matched to it) and, in `predict`, as the KS reference.

Why. Candidate E ran at KS 0.099 (E + speed change 0.0915) on the speed-matched E4 highway
against the emulator's 0.082, with the body too narrow (22.5 % of pairs under 0.68 cm against
Wyoming's 12.7 %; p1-stamp-e.yaml P1E.CEM.ks). Two causes, both speed effects:
  1. The emulator's engine nuisance, 4.5 mm per axis, was fitted on runs that started from rest
     and turned corners (v p10 13.9 m/s). At cruise the engine's GNSS layer adds less: its
     smoothed output moves with the receiver's own velocity, so only beta x (raw fix - estimate)
     reaches an increment (gauss_markov.rs, "Receiver output"). `nuisance` refits it on the
     speed-matched runs SBM, CEM and CESM.
  2. Real devices report a speed that differs from their own displacement by a per-device scale
     factor: along residual / (v dt) is steady within an ID and spread across IDs (Tampa), so
     the along residual grows with speed and is persistent (lag-1 +0.67 at v >= 15 m/s on chain
     IDs, against -0.01 at 10-15 m/s). At Tampa's 12 m/s it is a 2-3 mm term inside the core
     walk; at Wyoming's 34.4 m/s it is 6-7 mm. The engine's reported speed has only white
     output noise, so the body is missing it at speed.
"""
import json
import math
import random
import statistics
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import calib  # noqa: E402
import e4stepfit as ef  # noqa: E402
import normalise  # noqa: E402

EPS_MIN_PAIRS = 15
EPS_CLIP_M = 0.05  # pairs over 5 cm along are steps, not the scale term
# The speed-matched engine runs of fix round 5, stamp track (crates/v2xw-engine/tests/
# expectations/p1-stamp-e.yaml runs SBM, CEM, CESM; p1-e4-u.yaml, v p10 / p50 / p90 29.6 / 34.4 /
# 35.2 m/s): x.e4.ks and the residual p10 / p25 / p50 / p75 / p90 in cm from tools/realworld/
# paper1/summ.py --e4 on each run (the run summaries of that track).
ENGINE_RUNS_MATCHED = {
    "SBM": ("instant", 0.144946, (0.4626, 0.9263, 1.5715, 2.7550, 6.8826)),
    "CEM": ("E", 0.099235, (0.3776, 0.7523, 1.4648, 3.8942, 9.5283)),
    "CESM": ("E+speed", 0.091500, (0.3879, 0.7708, 1.4803, 3.9144, 9.4869)),
}
NUIS_GRID_MM = tuple(0.5 * i for i in range(0, 11))  # 0 .. 5 mm
QS = (0.1, 0.25, 0.5, 0.75, 0.9)


# ---------------------------------------------------------------------------- the scale term

def eps_pairs(seq):
    """(eps, v) per exactly-100 ms same-stamp pair at v >= 10 m/s: the along residual with the
    pair's own acceleration removed, over v dt. eps > 0: the device moved further than its
    speed says (it reports v (1 - eps))."""
    out = []
    for x in seq:
        if x is None or x["v"] < ef.VMIN or x["nom"] != 100 or x["o1"] != x["o2"]:
            continue
        al = x["al"] - 0.5 * (x["v2"] - x["v"]) * x["dt"]
        if abs(al) < EPS_CLIP_M:
            out.append((al / (x["v"] * x["dt"]), x["v"]))
    return out


def per_id_eps(seqs):
    """[(id, mean eps, its standard error, pairs, speed p50)] for IDs with enough pairs."""
    out = []
    for tid, _, seq in seqs:
        e = eps_pairs(seq)
        if len(e) >= EPS_MIN_PAIRS:
            xs = [a for a, _ in e]
            out.append((tid, statistics.mean(xs), statistics.stdev(xs) / math.sqrt(len(xs)), len(xs),
                        statistics.median(v for _, v in e)))
    return out


def device_spread(per):
    """Method of moments: mean, and the device sd = sqrt(between-ID variance - mean se^2)."""
    ms = [p[1] for p in per]
    mu = statistics.mean(ms)
    between = statistics.variance(ms) if len(ms) > 1 else 0.0
    within = statistics.mean(p[2] ** 2 for p in per)
    return mu, math.sqrt(max(0.0, between - within)), math.sqrt(between), math.sqrt(within)


def scale_or_bias(seqs, spread_mps=8.0):
    """Residual sums of squares of y = along / dt against y = s v (scale, one parameter per ID)
    and y = b (bias, one per ID), over IDs whose pairs span at least spread_mps."""
    rss = {"scale": 0.0, "bias": 0.0}
    n_id = n = 0
    for _, _, seq in seqs:
        pts = [(v, a * v) for a, v in eps_pairs(seq)]  # y = eps v = along / dt
        if len(pts) < EPS_MIN_PAIRS or max(p[0] for p in pts) - min(p[0] for p in pts) < spread_mps:
            continue
        n_id += 1
        n += len(pts)
        s = sum(v * y for v, y in pts) / sum(v * v for v, _ in pts)
        b = statistics.mean(y for _, y in pts)
        rss["scale"] += sum((y - s * v) ** 2 for v, y in pts)
        rss["bias"] += sum((y - b) ** 2 for _, y in pts)
    lr = n * math.log(rss["bias"] / rss["scale"]) if n and rss["scale"] > 0 else None
    return {"ids": n_id, "pairs": n, "rss": rss, "loglik_scale_minus_bias": None if lr is None else lr / 2}


def shrunk(per, mu, sd):
    """Each ID's eps shrunk toward the pooled mean (empirical Bayes), by ID."""
    return {tid: mu + (sd * sd / (sd * sd + se * se)) * (m - mu) if sd > 0 else mu for tid, m, se, _, _ in per}


def corrected(seqs, eps_by_id, mu):
    """The sequences with each ID's scale term taken out of the along residual."""
    out = []
    for tid, pm, seq in seqs:
        e = eps_by_id.get(tid, mu)
        out.append((tid, pm, [None if x is None else dict(x, al=x["al"] - e * x["v"] * x["dt"]) for x in seq]))
    return out


def cmd_fit(path, stepfit_path, boot=200, joint_iters=600):
    with open(stepfit_path) as f:
        base = json.load(f)
    seqs = ef.sequences(normalise.stream_ids(path, "tampa"))
    chain, fixed = ef.split(seqs)
    doc = {"source": "nm7w-nvbm (Tampa), locked IDs; eps = along residual / (v dt), exactly-100 ms "
                     "same-stamp pairs at v >= 10 m/s under 5 cm, acceleration removed",
           "reported_speed": "v_reported = v (1 + s), s = -eps"}
    for name, grp in (("chain", chain), ("fixed", fixed), ("locked", chain + fixed)):
        per = per_id_eps(grp)
        mu, sd, between, within = device_spread(per)
        doc[name] = {"ids": len(per), "eps_mean": mu, "eps_device_sd": sd, "eps_between_sd": between,
                     "eps_within_se_rms": within,
                     "per_id": [{"eps": m, "se": se, "pairs": n, "v_p50": v} for _, m, se, n, v in per]}
    doc["scale_or_bias"] = scale_or_bias(chain + fixed)
    # bootstrap over locked IDs (the device sample)
    per = per_id_eps(chain + fixed)
    rng = random.Random(7)
    reps = []
    for _ in range(boot):
        s = [per[rng.randrange(len(per))] for _ in per]
        reps.append(device_spread(s)[:2])
    doc["locked"]["boot"] = {"n": boot, "eps_mean": ef.interval([r[0] for r in reps]),
                             "eps_device_sd": ef.interval([r[1] for r in reps])}
    # lag-1 of consecutive along residuals: persistent (scale) or white
    doc["lag1_along"] = {}
    for lo, hi in ((10.0, 15.0), (15.0, 99.0)):
        xs = []
        for _, _, seq in chain:
            for a, b in zip(seq, seq[1:]):
                if a is None or b is None or not (lo <= a["v"] < hi):
                    continue
                if a["nom"] != 100 or b["nom"] != 100 or a["o1"] != a["o2"] or b["o1"] != b["o2"]:
                    continue
                xa = a["al"] - 0.5 * (a["v2"] - a["v"]) * a["dt"]
                xb = b["al"] - 0.5 * (b["v2"] - b["v"]) * b["dt"]
                if abs(xa) < EPS_CLIP_M and abs(xb) < EPS_CLIP_M:
                    xs.append((xa, xb))
        doc["lag1_along"]["%g-%g" % (lo, hi)] = {"pairs": len(xs), "r": _corr(xs)}
    # the chain-device core walk refitted with each chain ID's (shrunk) scale term taken out,
    # warm-started from candidate E's joint fit
    mu_l, sd_l = doc["locked"]["eps_mean"], doc["locked"]["eps_device_sd"]
    eps_id = shrunk(per_id_eps(chain), mu_l, sd_l)
    jobs = ef.joint_pairs(corrected(chain, eps_id, mu_l))
    j = ef.fit_joint(jobs, 2, False, u0=base["joint"]["u"], iters=joint_iters)
    j0 = base["joint"]
    doc["joint_refit"] = j
    doc["joint_base"] = {"core": j0["core"], "loglik": j0["loglik"], "prob_on_transition": j0["prob_on_transition"],
                         "prob_stay_zero": j0["prob_stay_zero"]}
    return doc


def _corr(xs):
    if len(xs) < 3:
        return None
    ma = statistics.mean(a for a, _ in xs)
    mb = statistics.mean(b for _, b in xs)
    cov = sum((a - ma) * (b - mb) for a, b in xs)
    va = sum((a - ma) ** 2 for a, _ in xs)
    vb = sum((b - mb) ** 2 for _, b in xs)
    return cov / math.sqrt(va * vb) if va > 0 and vb > 0 else None


# ---------------------------------------------------------------------------- emulation

def e_kwargs(base, joint=None, speed=False):
    j = joint or base["joint"]
    lw, l0 = j["size"], j["size_stay_zero"]
    kw = {"chain": (base["chain"]["stay_minus1"], base["chain"]["stay_zero"]), "walk": dict(j["core"]),
          "step": (j["prob_on_transition"], lw["along_p50_m"], lw["along_p90_m"], lw["cross_p50_m"], lw["cross_p90_m"]),
          "step0": (j["prob_stay_zero"], l0["along_p50_m"], l0["along_p90_m"], l0["cross_p50_m"], l0["cross_p90_m"])}
    if speed:
        sp = base["speed"]
        kw.update(k_v=sp["k_mps_per_m"], tau_v=sp["tau_v_s"], follow=sp["follow"]["follow_fraction"])
    return kw


def run_kwargs(base, which):
    return {"instant": {"stamp": "instant"}, "E": e_kwargs(base), "E+speed": e_kwargs(base, speed=True)}[which]


def cmd_nuisance(wy_path, stepfit_path, n=20000, seeds=(1, 2)):
    """The engine's GNSS nuisance at cruise: the grid value whose emulated residual quantiles
    best match the speed-matched engine runs (least squared log ratio, p10-p90, all runs)."""
    with open(stepfit_path) as f:
        base = json.load(f)
    _, speeds = ef.wyoming_reference(wy_path, None)
    table = []
    for mm in NUIS_GRID_MM:
        err, row = 0.0, {}
        for name, (which, eks, eq) in ENGINE_RUNS_MATCHED.items():
            res = []
            for sd in seeds:
                res += ef.emulate(speeds, n, seed=sd, extra_m=mm * 1e-3, **run_kwargs(base, which))[0]
            q = ef.quant(res, QS)
            err += sum(math.log(100 * a / b) ** 2 for a, b in zip(q, eq))
            row[name] = [round(100 * x, 4) for x in q]
        table.append((mm, err, row))
    best = min(table, key=lambda t: t[1])
    return {"grid_mm": [t[0] for t in table], "sq_log_error": [round(t[1], 5) for t in table],
            "best_mm": best[0], "best_quantiles_cm": best[2],
            "engine_quantiles_cm": {k: v[2] for k, v in ENGINE_RUNS_MATCHED.items()}}


def candidates(base, sf):
    lk = sf["locked"]
    s = (-lk["eps_mean"], lk["eps_device_sd"])  # reported-speed scale s = -eps
    s0 = (0.0, lk["eps_device_sd"])
    jr = sf["joint_refit"]
    return [
        ("E (shipped as profile tampa_chain)", e_kwargs(base)),
        ("E + speed change", e_kwargs(base, speed=True)),
        ("E + scale (Tampa mean and sd), core as shipped", dict(e_kwargs(base), scale=s)),
        ("E' = E on the refitted core + scale", dict(e_kwargs(base, jr), scale=s)),
        ("E' + scale, zero mean", dict(e_kwargs(base, jr), scale=s0)),
        ("E' + scale + speed change", dict(e_kwargs(base, jr, speed=True), scale=s)),
        ("E' without the scale", e_kwargs(base, jr)),
    ]


def cmd_predict(wy_path, stepfit_path, scalefit_path, extras_mm, n=20000, seeds=(1, 2, 3)):
    with open(stepfit_path) as f:
        base = json.load(f)
    with open(scalefit_path) as f:
        sf = json.load(f)
    real, speeds = ef.wyoming_reference(wy_path, None)
    out = {"wyoming_pairs": len(real), "wyoming_under_0p68cm": sum(r < 0.0068 for r in real) / len(real),
           "rows": []}
    for mm in extras_mm:
        for name, kw in candidates(base, sf):
            ks_s, at_s, under = [], [], []
            for sd in seeds:
                res, lab = ef.emulate(speeds, n, seed=sd, extra_m=mm * 1e-3, **kw)
                k, at = ef.ks_at(res, real)
                ks_s.append(k)
                at_s.append(at)
                under.append(sum(r < 0.0068 for r in res) / len(res))
            res, lab = ef.emulate(speeds, n, seed=seeds[0], extra_m=mm * 1e-3, **kw)
            out["rows"].append({"nuisance_mm": mm, "candidate": name, "ks": statistics.mean(ks_s),
                                "ks_range": [min(ks_s), max(ks_s)], "at_cm": 100 * sorted(at_s)[len(at_s) // 2],
                                "under_0p68cm": statistics.mean(under),
                                "q_cm": [round(100 * x, 3) for x in ef.quant(res, QS + (0.99,))],
                                "over5_on_0m1": ef.over5_on_transition(res, lab)})
    # the device-sample spread of the scale term: E' + scale over the bootstrap of eps
    lk = sf["locked"]
    rng = random.Random(5)
    ks_b = []
    per = lk["per_id"]
    jr = sf["joint_refit"]
    for i in range(30):
        smp = [per[rng.randrange(len(per))] for _ in per]
        mu, sd, _, _ = device_spread([(None, p["eps"], p["se"], p["pairs"], p["v_p50"]) for p in smp])
        res, _ = ef.emulate(speeds, n // 2, seed=200 + i, extra_m=extras_mm[0] * 1e-3,
                            scale=(-mu, sd), **e_kwargs(base, jr))
        ks_b.append(ef.ks_at(res, real)[0])
    out["boot_ks"] = {"nuisance_mm": extras_mm[0], "n": len(ks_b), "interval": ef.interval(ks_b),
                      "share_le_0p092": sum(k <= 0.092 for k in ks_b) / len(ks_b)}
    return out


# ---------------------------------------------------------------------------- the calibration

def cmd_yaml(scalefit_path, chain_device_path):
    with open(scalefit_path) as f:
        sf = json.load(f)
    lk, core = sf["locked"], sf["joint_refit"]["core"]
    text = open(chain_device_path).read()
    old = [ln for ln in text.splitlines() if ln.strip().startswith("position_noise:")]
    if len(old) != 2:  # the locked class's and the free class's
        raise SystemExit("expected two position_noise lines in " + chain_device_path)
    # the walk floor of calib.position_noise (1e-4 m, the shipped v3 walk's smallest sd)
    fl = [max(calib.PN_FLOOR_M, x) for x in core["along_m"]], [max(calib.PN_FLOOR_M, x) for x in core["cross_m"]]
    new_pn = "    position_noise: {dist: normal_mixture, tau_s: %.1f, p: %s, along_m: %s, cross_m: %s}" % (
        core["tau_s"], ef._lst(core["p"]), ef._lst(fl[0]), ef._lst(fl[1]))
    on_old = [ln for ln in text.splitlines() if ln.strip().startswith("output_noise:")][0]
    s_mean, s_sd = -lk["eps_mean"], lk["eps_device_sd"]
    b = lk["boot"]
    on_new = on_old.rstrip("}") + ", speed_scale: {mean: %s, sd: %s}}" % (ef._f(s_mean), ef._f(s_sd))
    head = (
        "# CANDIDATE (2026-10-28, P1-E4 body refit): bsm-timing-cvpilot-chain-device.yaml with TWO changes\n"
        "# to the locked class, both fitted on Tampa (nm7w-nvbm) only, Wyoming held out\n"
        "# (research/synthesis/p1-realdata-baseline.md section 7.6):\n"
        "# 1. output_noise.speed_scale: a per-device reported-speed scale error s, drawn once per device,\n"
        "#    the transmitted speed v (1 + s). Tampa %d locked IDs: along residual / (v dt) has mean %.4f %%\n"
        "#    and a device sd of %.4f %% (95 %% bootstrap over IDs %.4f-%.4f %%; between-ID sd %.4f %%, within-ID se\n"
        "#    %.4f %%). NOT READ BY THE ENGINE YET: crates/v2xw-node/src/bsm_timing.rs (OutputNoise, report_inner).\n"
        "# 2. position_noise: the joint stamp-step model's core walk refitted on the chain IDs with each ID's\n"
        "#    scale term (shrunk to the pooled mean) taken out of the along residual.\n"
        "# GENERATED by `python3 -I tools/realdata/e4scalefit.py yaml scalefit.json "
        "data/calibration/bsm-timing-cvpilot-chain-device.yaml` - do not edit by hand.\n"
        % (lk["ids"], 100 * lk["eps_mean"], 100 * lk["eps_device_sd"], 100 * b["eps_device_sd"][0],
           100 * b["eps_device_sd"][2], 100 * lk["eps_between_sd"], 100 * lk["eps_within_se_rms"]))
    out = text.replace(old[0], "    # was: " + old[0].strip() + "\n" + new_pn, 1)
    out = out.replace(on_old, "    # was: " + on_old.strip() + "\n" + on_new, 1)
    return head + out


def main(argv):
    if argv[:1] == ["fit"]:
        boot = int(argv[argv.index("--boot") + 1]) if "--boot" in argv else 200
        json.dump(cmd_fit(argv[1], argv[2], boot), sys.stdout, indent=1, sort_keys=True)
    elif argv[:1] == ["nuisance"]:
        json.dump(cmd_nuisance(argv[1], argv[2]), sys.stdout, indent=1, sort_keys=True)
    elif argv[:1] == ["predict"]:
        mm = [float(v) for v in argv[argv.index("--nuisance-mm") + 1].split(",")] if "--nuisance-mm" in argv \
            else [ef.EMU_EXTRA_M * 1e3]
        json.dump(cmd_predict(argv[1], argv[2], argv[3], mm), sys.stdout, indent=1, sort_keys=True)
    elif argv[:1] == ["yaml"]:
        sys.stdout.write(cmd_yaml(argv[1], argv[2]))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
