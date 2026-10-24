"""Stage 7 (outputs), part 1: the fitted BSM timing model (v2xw.bsm_timing.v1) and the frozen
classifier. Derived statistics only; no raw row leaves the scratch directory."""
import json
import math
from collections import Counter

import bootci
import devclass
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
                                   "frozen": 0, "slip": 0, "sizes": Counter(), "hdr": Counter(),
                                   "excess": Counter()})
            a["ids"] += 1
            a["phase100"].update(r.phase100)
            a["wrapped"].update(r.wrapped)
            a["at_risk"] += r.at_risk
            a["frozen"] += r.ent_frozen
            a["slip"] += r.ent_slip
            a["sizes"].update(r.slip_sizes)
            a["hdr"].update(r.hdr_off_ms)
            a["excess"].update(r.period_excess_ms)
    return agg


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
         "schema: v2xw.bsm_timing.v1",
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
            L.append("    # normal fitted to the %.1f%% of locked BSMs within 5 ms of a 50 ms epoch" % (100 * core))
            L.append("    secmark_phase_ms: {dist: normal, mean: %s, sd: %s}" % (r6(mean), r6(sd)))
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
            L.append("    # draw the phase per device and let it drift; independent per-message draws double count")
            L.append("    # the slips below, which are the observed onsets of a position-epoch change")
        risk = a["at_risk"]
        L.append("    # onsets per single-interval moving pair (v >= 5 m/s) after a normal pair: %d at risk" % risk)
        L.append("    epoch_slip_prob: %s" % r6(a["slip"] / risk if risk else 0.0))
        dist, tot = _slip_dist(a["sizes"])
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
    return "\n".join(L) + "\n"
