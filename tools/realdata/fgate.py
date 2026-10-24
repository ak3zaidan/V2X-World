"""Stage 6 (falsification gate). Runs before any table is produced; a failure stops the run.

G1 jump     A 2 m cross-track step injected into a locked Wyoming trace must be reported as a
            RandomPosition (v1.0) trigger at exactly the injected pair, and nowhere else new.
G2 slip     A synthetic epoch slip (one BSM's position taken from the next fix epoch) must
            fire both adjacent pairs, carry the 'double' then 'zero' signature, and be counted
            by the timing-model slip estimator.
G3 placebo  Shuffling the secMark-phase class labels across the fit IDs must erase the class
            effect (free minus locked rate at the headline cell): the shuffled effects have
            |median| <= 20% of the real effect and the real effect exceeds their 97.5th centile.
G4 broken   The gate is re-run with a deliberately broken RandomPosition check (gpsDrift
            1e9 m, it can never fire) and with real labels replaced by shuffled ones; both
            must make the gate FAIL. If either passes, the gate cannot fail and the run stops.
"""
import math
import random

import bootci
import mbrchecks as mc
import pairing
import pipeline

JUMP_M = 2.0
PLACEBO_PERMS = 200
PLACEBO_SEED = "tools/realdata/gate/placebo"


def _shift(b, de, dn):
    lat = b.lat + math.degrees(dn / pairing.R_EARTH)
    lon = b.lon + math.degrees(de / (pairing.R_EARTH * math.cos(math.radians(b.lat))))
    return b._replace(lat=lat, lon=lon)


def rp_triggers(bs, gps_drift):
    """Indices i of pairs (i, i+1) that fire RandomPosition v1.0 under secMark, 0.5 s."""
    out = {}
    for i, p in enumerate(pairing.pairs(bs)):
        dt = p.dt[0]
        if p.v1 is None or p.h1 is None or not mc.judged(dt, 0.5):
            continue
        fired, r, _ = mc.run(p, dt, gps_drift)
        if fired[0]:
            out[i] = mc.slip_signature(p, dt)
    return out


def pick_index(bs, vmin=15.0):
    """A BSM k in the middle of the trace whose two adjacent pairs are single-interval and fast."""
    ps = pairing.pairs(bs)
    good = [k for k in range(1, len(ps)) if all(
        ps[i].dt[0] is not None and 0.05 < ps[i].dt[0] < 0.15 and ps[i].v1 is not None and ps[i].v1 >= vmin
        for i in (k - 1, k))]
    if not good:
        return None
    return good[len(good) // 2]


def g1_jump(bs, gps_drift):
    k = pick_index(bs)
    if k is None:
        return False, "no usable pair in the trace"
    base = rp_triggers(bs, gps_drift)
    h = math.radians(bs[k].heading)
    de, dn = JUMP_M * math.cos(h), -JUMP_M * math.sin(h)  # perpendicular to heading
    mod = bs[:k] + [_shift(b, de, dn) for b in bs[k:]]
    new = set(rp_triggers(mod, gps_drift)) - set(base)
    ok = new == {k - 1}
    return ok, f"injected step at BSM {k}: new triggers at pairs {sorted(new)} (want [{k - 1}])"


def g2_slip(bs, gps_drift):
    k = pick_index(bs)
    if k is None:
        return False, "no usable pair in the trace"
    b = bs[k]
    h = math.radians(b.heading)
    step = b.speed * mc.FIX_INTERVAL_S
    mod = list(bs)
    mod[k] = _shift(b, step * math.sin(h), step * math.cos(h))
    base = rp_triggers(bs, gps_drift)
    trig = rp_triggers(mod, gps_drift)
    new = {i: s for i, s in trig.items() if i not in base}
    est0 = pipeline.analyse_id("orig", bs)
    est1 = pipeline.analyse_id("slip", mod)
    counted = est1.slip_sizes[1] - est0.slip_sizes[1]
    ok = new.get(k - 1) == "double" and new.get(k) == "zero" and counted >= 1
    return ok, f"slip at BSM {k}: new triggers {dict(sorted(new.items()))}; estimator +{counted} slips of +1 epoch"


def class_effect(units):
    """units: (label, triggers, minutes). Returns rate(free) - rate(locked)."""
    f = [(t, m) for c, t, m in units if c == "free"]
    lk = [(t, m) for c, t, m in units if c == "locked"]
    return bootci.rate(f) - bootci.rate(lk)


def g3_placebo(units):
    real = class_effect(units)
    labels = [u[0] for u in units]
    r = random.Random(PLACEBO_SEED)
    effs = []
    for _ in range(PLACEBO_PERMS):
        r.shuffle(labels)
        effs.append(class_effect([(c, u[1], u[2]) for c, u in zip(labels, units)]))
    med = bootci.quantile(effs, 0.5)
    q975 = bootci.quantile(effs, 0.975)
    ok = real > 0 and abs(med) <= 0.2 * real and real > q975
    return ok, (f"real effect {real:.2f}/ID-min; shuffled median {med:.2f}, 97.5th centile {q975:.2f} "
                f"({PLACEBO_PERMS} shuffles)")


def run(trace, units, gps_drift=mc.GPS_DRIFT_M):
    res = {"G1 jump": g1_jump(trace, gps_drift), "G2 slip": g2_slip(trace, gps_drift),
           "G3 placebo": g3_placebo(units)}
    return all(v[0] for v in res.values()), res


def full(trace, units):
    """The gate and its self-test (G4). Returns (passed, rows) with rows (name, ok, detail)."""
    ok, res = run(trace, units)
    rows = [(k, v[0], v[1]) for k, v in res.items()]
    broken_ok, bres = run(trace, units, gps_drift=1e9)
    rows.append(("G4a broken check must fail the gate", not broken_ok,
                 "gpsDrift 1e9 m: gate " + ("PASSED (cannot fail!)" if broken_ok else "failed as it must")
                 + "; " + "; ".join(f"{k} {'ok' if v[0] else 'red'}" for k, v in bres.items())))
    r = random.Random(PLACEBO_SEED + "/g4")
    lab = [u[0] for u in units]
    r.shuffle(lab)
    shuffled = [(c, u[1], u[2]) for c, u in zip(lab, units)]
    sok, sdet = g3_placebo(shuffled)
    rows.append(("G4b shuffled labels must fail G3", not sok, sdet))
    return all(row[1] for row in rows), rows
