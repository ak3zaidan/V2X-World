#!/usr/bin/env python3
"""Open-loop multi-day surrogate of the honest misbehaviour-report flow (paper plan §1.4
item G, experiment P1-E10; shared with Paper 3). ANALYTICAL: it is fed by rates measured
from engine runs and simulates nothing; nothing it prints is an engine result.

    python3 -I tools/surrogate/surrogate.py summarise <run-dir> <summary.json>
    python3 -I tools/surrogate/surrogate.py fit <summary.json> <rates.json>
    python3 -I tools/surrogate/surrogate.py observe <summary.json> <realism.json>
    python3 -I tools/surrogate/surrogate.py predict <rates.json> <exposure-summary.json> <realism.json> [--fault F]
    python3 -I tools/surrogate/surrogate.py project <rates.json> <profile.json> <out.json>
    python3 -I tools/surrogate/surrogate.py compare <rates.json> <target-summary.json> <runs-dir>
    python3 -I tools/surrogate/surrogate.py fill <table.yaml> <runs-dir>
    python3 -I tools/surrogate/surrogate.py selftest <table.yaml> <runs-dir>

The model, stage by stage (each stage's numbers come from the calibration run only):

1. Verdicts. A Poisson stream per (device class, corridor, detector) at rate lambda per
   second of the subject's exposure (its BSMs on the air x 0.1 s). Class is the secMark
   phase class of P1-E2 (tools/realdata/devclass.py); corridor is an avenue band
   (records.corridor_of). A cell the calibration never saw falls back to its class's
   pooled rate. Per device the rate carries a gamma multiplier whose shape is fitted to
   the calibration's per-subject counts (method of moments), so per-device totals have the
   engine's dispersion rather than Poisson's.
2. Reports. Each verdict becomes a stored report with the class's measured probability;
   a stored report is uploaded (by the day's end), lost (quota, decay, left with its
   device) or held, with the class's measured probabilities and relay share.
3. The RA. Each upload waits in the RA's report shuffle, released one window
   (`report_shuffle_window_s`, CAMP-EE SCMS-765: one day) after the batch's first arrival,
   or at `report_shuffle_threshold` reports; the MA receives the measured share.
4. The MA (projection only). Honest devices whose LA-linked MA receipts in a rolling
   7-day window reach k would be blocked under the k-threshold rule (P1-E7's family).

Open loop: no revocation feeds back into traffic, and the rates are held constant
across days (they were measured at one time of day, at one density).
"""
import hashlib
import json
import math
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
VERSION = '1.0.0'
DAY = 86400.0
CLASSES = ('locked', 'free', 'mixed', 'unclassified')
FAULTS = ('rate_x1.5', 'no_shuffle', 'pool_class', 'pool_corridor', 'shift_corridor',
          'drop_window1', 'no_conversion', 'flip_relay', 'cases_1pct', 'flat_corridor',
          'threshold_x2')


def _cell(key):
    w, c, k = key.split('|')
    return int(w), c, k


# ---------------------------------------------------------------- engine-observed measures

def observe(S):
    """The day-1 measures of an engine run, from its summary (the target of P1-E10)."""
    V = [v for v in S['verdicts'] if v['t'] < DAY]
    R = S['reports']
    up = [r for r in R if r.get('uploaded', DAY) < DAY]
    lost = [r for r in R if min(r.get('evicted_quota', DAY), r.get('decayed_out', DAY),
                                r.get('left_with_device', DAY)) < DAY]
    batch = [m for m in S['ma'] if m[0] < DAY + 7200.0]
    out = {
        'x.e10.triggers': len(V),
        'x.e10.verdicts': S.get('verdicts_fired', len(V)) if S.get('trigger') == 'stored' else len(V),
        'x.e10.stored': sum(1 for r in R if r.get('stored', DAY) < DAY),
        'x.e10.uploaded': len(up),
        'x.e10.lost': len(lost),
        'x.e10.relay_share': (sum(1 for r in up if r['relay']) / len(up)) if up else 0.0,
        'x.e10.triggers.free_share': (sum(1 for v in V if v['cls'] == 'free') / len(V)) if V else 0.0,
        'x.e10.ma_batch': len(batch),
        'x.e10.ma_first_t': S['ma'][0][0] if S['ma'] else -1.0,
        'x.e10.cases': S['cases'],
        'corridor_profile': _profile([v['corr'] for v in V]),
    }
    for w in range(len(S['windows'])):
        if S['windows'][w][0] < DAY:
            out[f'x.e10.triggers.w{w}'] = sum(1 for v in V if v['w'] == w)
    return out


def _profile(keys):
    n = len(keys)
    c = {}
    for k in keys:
        c[str(k)] = c.get(str(k), 0) + 1
    return {k: v / n for k, v in sorted(c.items())} if n else {}


def tvd(p, q):
    keys = set(p) | set(q)
    return 0.5 * sum(abs(p.get(k, 0.0) - q.get(k, 0.0)) for k in keys)


# ---------------------------------------------------------------- calibration

def fit(S):
    """Rates from one calibration run's summary."""
    E = {}                                   # (cls, corr) -> exposure s
    for key, e in S['exposure_s'].items():
        _, c, k = _cell(key)
        E[(c, k)] = E.get((c, k), 0.0) + e
    Etot = sum(E.values())
    Ecls = {}
    for (c, k), e in E.items():
        Ecls[c] = Ecls.get(c, 0.0) + e
    N, Ncls, unloc = {}, {}, {}
    for v in S['verdicts']:
        if v['corr'] is None or (v['cls'], v['corr']) not in E:
            unloc[v['det']] = unloc.get(v['det'], 0) + 1
            continue
        N.setdefault((v['cls'], v['corr']), {}).setdefault(v['det'], 0)
        N[(v['cls'], v['corr'])][v['det']] += 1
        Ncls.setdefault(v['cls'], {}).setdefault(v['det'], 0)
        Ncls[v['cls']][v['det']] += 1
    lam = {f'{c}|{k}': {d: n / E[(c, k)] for d, n in sorted(dd.items())}
           for (c, k), dd in sorted(N.items())}
    lam_cls = {c: {d: n / Ecls[c] for d, n in sorted(dd.items())} for c, dd in sorted(Ncls.items())}
    vcls = {}
    for v in S['verdicts']:
        vcls[v['cls']] = vcls.get(v['cls'], 0) + 1
    conv = {}
    for c in sorted(set(r['cls'] for r in S['reports']) | set(vcls)):
        Rc = [r for r in S['reports'] if r['cls'] == c and 'stored' in r]
        up = [r for r in Rc if r.get('uploaded', DAY) < DAY]
        lost = [r for r in Rc if min(r.get('evicted_quota', DAY), r.get('decayed_out', DAY),
                                     r.get('left_with_device', DAY)) < DAY]
        conv[c] = {
            'stored_per_verdict': len(Rc) / vcls[c] if vcls.get(c) else 0.0,
            'uploaded': len(up) / len(Rc) if Rc else 0.0,
            'lost': len(lost) / len(Rc) if Rc else 0.0,
            'relay_share': sum(1 for r in up if r['relay']) / len(up) if up else 0.0,
            'upload_delay_s': sorted(round(r['uploaded'] - r['stored'], 3) for r in up)[:2000],
        }
    ups = sorted(r['uploaded'] for r in S['reports'] if 'uploaded' in r)
    w0 = S['windows'][0][0]
    first_up = (ups[0] - w0) if ups else 0.0
    batch = [m for m in S['ma'] if m[0] < DAY + 7200.0]
    n_up = sum(1 for r in S['reports'] if r.get('uploaded', DAY) < DAY)
    n_ra = max(1, len({m[1] for m in S['ma']}))
    sh = S['shuffle']
    # the shuffle model run on the calibration's own upload instants: what it releases
    # by the horizon, against what the MAs received (the delivery share)
    released, first_rel = shuffle_release(ups, sh['window_s'], sh['threshold'], n_ra, DAY + 7200.0)
    ma = {
        'received_per_upload': len(batch) / n_up if n_up else 0.0,
        'delivered_per_released': len(batch) / released if released else 0.0,
        'latency_s': (S['ma'][0][0] - first_rel) if (S['ma'] and first_rel is not None) else 0.0,
        'ras': n_ra,
        'first_after_first_upload_s': (S['ma'][0][0] - ups[0]) if (S['ma'] and ups) else S['shuffle']['window_s'],
        'cases_per_received': S['cases'] / len(S['ma']) if S['ma'] else 0.0,
        'providers': sorted({m[1] for m in S['ma']}),
    }
    return {
        'schema': 'v2xw.surrogate.rates.v1', 'version': VERSION, 'label': 'analytical',
        'from_run': S['run'], 'corridors': S['corridors'], 'windows': S['windows'],
        'exposure_s': S['exposure_s'],
        'lambda_per_s': lam, 'lambda_class_per_s': lam_cls,
        'lambda_unlocated_per_s': {d: n / Etot for d, n in sorted(unloc.items())} if Etot else {},
        'dispersion_shape': _dispersion(S.get('subjects', [])),
        'conversion': conv, 'first_upload_after_window_s': first_up, 'ma': ma,
        'shuffle': S['shuffle'], 'trigger': S.get('trigger', 'det.observation'),
        'verdicts_per_trigger': (S['verdicts_fired'] / len(S['verdicts']))
            if S.get('trigger') == 'stored' and S['verdicts'] else 1.0,
        'not_filed_per_trigger': S['not_filed'] / len(S['verdicts']) if S['verdicts'] else 0.0,
    }


def shuffle_release(ups, window_s, threshold, n_ra, horizon):
    """The RA report shuffle (CAMP-EE SCMS-765, `release_report_shuffle`): the first
    report arms a timer of `window_s`; the batch goes to the MA when the timer fires or the
    moment it holds `threshold` reports. Uploads are split evenly over `n_ra` RAs
    (ASSUMPTION: devices are assigned to providers evenly). Returns (reports released by
    `horizon`, the first release instant or None)."""
    released, first = 0, None
    for i in range(n_ra):
        mine = ups[i::n_ra]
        armed, nb = None, 0
        for t in mine:
            if armed is not None and t >= armed + window_s:
                if armed + window_s < horizon:
                    released += nb
                    first = armed + window_s if first is None else min(first, armed + window_s)
                armed, nb = None, 0
            if armed is None:
                armed = t
            nb += 1
            if nb >= threshold:
                if t < horizon:
                    released += nb
                    first = t if first is None else min(first, t)
                armed, nb = None, 0
        if armed is not None and armed + window_s < horizon:
            released += nb
            first = armed + window_s if first is None else min(first, armed + window_s)
    return released, first


def _upload_times(n_by_window, windows, first_up):
    """Expected upload instants: each window's uploads spread evenly over the window (the
    first window from the calibration's first-upload offset on)."""
    out = []
    for w, n in sorted(n_by_window.items()):
        a, b = windows[w]
        if w == 0:
            a = min(b, a + first_up)
        k = int(round(n))
        out.extend(a + (b - a) * (j + 0.5) / k for j in range(k)) if k else None
    return sorted(out)


def _dispersion(subjects):
    """Gamma shape per class of the per-device rate multiplier (method of moments on the
    per-subject counts: Var n_i = m e_i + (m e_i)^2 / shape)."""
    out = {}
    for c in CLASSES:
        rows = [(e, n) for cc, e, n in subjects if cc == c and e > 0]
        if not rows:
            continue
        m = sum(n for _, n in rows) / sum(e for e, _ in rows)
        num = sum((m * e) ** 2 for e, _ in rows)
        den = sum((n - m * e) ** 2 - m * e for e, n in rows)
        out[c] = min(1e6, max(0.05, num / den)) if den > 0 else 1e6
    return out


# ---------------------------------------------------------------- prediction (day 1)

def _apply_faults(rates, fault):
    r = json.loads(json.dumps(rates))
    if fault in (None, ''):
        return r
    if fault not in FAULTS:
        raise SystemExit(f'unknown fault {fault}; one of {FAULTS}')
    if fault == 'rate_x1.5':
        for tab in (r['lambda_per_s'], r['lambda_class_per_s']):
            for dd in tab.values():
                for d in dd:
                    dd[d] *= 1.5
    elif fault == 'no_shuffle':
        r['ma']['first_after_first_upload_s'] -= r['shuffle']['window_s']
    elif fault == 'pool_class':
        # one rate per corridor for every class: the class dimension thrown away
        E, N = {}, {}
        for key, e in r['exposure_s'].items():
            _, c, k = _cell(key)
            E[k] = E.get(k, 0.0) + e
        for ck, dd in r['lambda_per_s'].items():
            c, k = ck.split('|')
            ec = sum(e for key, e in r['exposure_s'].items() if _cell(key)[1:] == (c, k))
            for d, l in dd.items():
                N.setdefault(k, {}).setdefault(d, 0.0)
                N[k][d] += l * ec
        pooled = {k: {d: n / E[k] for d, n in dd.items()} for k, dd in N.items()}
        r['lambda_per_s'] = {f'{c}|{k}': dict(pooled.get(k, {}))
                             for c in CLASSES for k in E}
    elif fault == 'pool_corridor':
        r['lambda_per_s'] = {}
    elif fault == 'shift_corridor':
        # every corridor's rates moved one avenue band east: the corridor labels scrambled
        def shift(k):
            return f'av{int(k[2:]) + 1:+d}' if k.startswith('av') else k
        r['lambda_per_s'] = {f"{ck.split('|')[0]}|{shift(ck.split('|')[1])}": dd
                             for ck, dd in r['lambda_per_s'].items()}
        r['lambda_class_per_s'] = {c: {d: 0.0 for d in dd}
                                   for c, dd in r['lambda_class_per_s'].items()}
    elif fault == 'flip_relay':
        for cv in r['conversion'].values():
            cv['relay_share'] = 1.0 - cv['relay_share']
    elif fault == 'threshold_x2':
        r['shuffle']['threshold'] *= 2
    elif fault == 'flat_corridor':
        pass                                     # applied to the exposure in predict
    elif fault == 'cases_1pct':
        r['ma']['cases_per_received'] = 0.01
    elif fault == 'no_conversion':
        # every verdict a report and every report uploaded: the store stage thrown away
        for cv in r['conversion'].values():
            cv['stored_per_verdict'] = 1.0
            cv['uploaded'] = 1.0
    r['fault'] = fault
    return r


def predict(rates, exposure_s, windows, fault=None):
    """Expected day-1 measures of a run with this exposure (seconds per window|class|corridor)."""
    r = _apply_faults(rates, fault)
    lam, lam_c = r['lambda_per_s'], r['lambda_class_per_s']
    if fault == 'flat_corridor':
        # the target's exposure spread evenly over its corridors: traffic's layout ignored
        ks = sorted({_cell(k)[2] for k in exposure_s})
        tot = {}
        for key, e in exposure_s.items():
            w, c, _ = _cell(key)
            tot[(w, c)] = tot.get((w, c), 0.0) + e
        exposure_s = {f'{w}|{c}|{k}': e / len(ks) for (w, c), e in tot.items() for k in ks}
    V = {}                                   # (w, cls, corr) -> expected verdicts
    Etot = 0.0
    for key, e in exposure_s.items():
        w, c, k = _cell(key)
        if windows[w][0] >= DAY or (fault == 'drop_window1' and w == 1):
            continue
        Etot += e
        dd = lam.get(f'{c}|{k}', lam_c.get(c, {}))
        V[(w, c, k)] = V.get((w, c, k), 0.0) + e * sum(dd.values())
    unloc = Etot * sum(r['lambda_unlocated_per_s'].values())
    nV = sum(V.values()) + unloc
    by_cls = {}
    for (w, c, k), n in V.items():
        by_cls[c] = by_cls.get(c, 0.0) + n
    by_cls['unclassified'] = by_cls.get('unclassified', 0.0) + unloc
    stored = up = lost = relay = 0.0
    for c, n in by_cls.items():
        cv = r['conversion'].get(c)
        if not cv:
            continue
        s = n * cv['stored_per_verdict']
        stored += s
        up += s * cv['uploaded']
        lost += s * cv['lost']
        relay += s * cv['uploaded'] * cv['relay_share']
    first_up = windows[0][0] + r['first_upload_after_window_s']
    up_w = {}
    for (w, c, k), n in V.items():
        cv = r['conversion'].get(c)
        if cv:
            up_w[w] = up_w.get(w, 0.0) + n * cv['stored_per_verdict'] * cv['uploaded']
    m = r['ma']
    W, thr = r['shuffle']['window_s'], r['shuffle']['threshold']
    if fault == 'no_shuffle':
        W, thr = 0.0, 1.0
    rel, first_rel = shuffle_release(_upload_times(up_w, windows, r['first_upload_after_window_s']),
                                     W, thr, m.get('ras', 1), DAY + 7200.0)
    out = {
        'x.e10.triggers': nV,
        'x.e10.verdicts': nV * r.get('verdicts_per_trigger', 1.0),
        'x.e10.stored': stored,
        'x.e10.uploaded': up,
        'x.e10.lost': lost,
        'x.e10.relay_share': relay / up if up else 0.0,
        'x.e10.triggers.free_share': by_cls.get('free', 0.0) / nV if nV else 0.0,
        'x.e10.ma_batch': rel * m.get('delivered_per_released', 1.0),
        'x.e10.ma_first_t': first_rel + m.get('latency_s', 0.0) if first_rel is not None else -1.0,
        'x.e10.cases': round(m['cases_per_received'] * rel * m.get('delivered_per_released', 1.0)),
        'surrogate': {'label': 'analytical', 'version': VERSION, 'fault': fault,
                      'from_run': r['from_run']},
    }
    corr = {}
    for (w, c, k), n in V.items():
        corr[k] = corr.get(k, 0.0) + n
        out[f'x.e10.triggers.w{w}'] = out.get(f'x.e10.triggers.w{w}', 0.0) + n
    if unloc:
        corr['None'] = unloc
    out['corridor_profile'] = {k: v / nV for k, v in sorted(corr.items())} if nV else {}
    for w in range(len(windows)):
        if windows[w][0] < DAY:
            out.setdefault(f'x.e10.triggers.w{w}', 0.0)
    out['x.e10.digest'] = hashlib.sha256(json.dumps(out, sort_keys=True).encode()).hexdigest()
    return out


def finish(observed, predicted):
    """The comparison measure that needs both sides: the corridor profile's distance."""
    observed['x.e10.corridor_tvd'] = tvd(observed['corridor_profile'], predicted['corridor_profile'])
    predicted['x.e10.corridor_tvd'] = 0.0
    return observed, predicted


# ---------------------------------------------------------------- multi-day projection

def _gamma(rng, shape):
    return 1.0 if shape >= 1e5 else rng.gammavariate(shape, 1.0 / shape)


def project(rates, profile):
    """Monte Carlo over a fleet for `days` days; seeded, so byte-identical on a rerun.

    profile: devices, days, seed, drive_s_per_day, class_share {cls: share}, drive_hour_local
    (the hour the day's driving starts), t0_hour_local (the local hour of t = 0),
    k_rules [k...], window_days (the MA's rolling window).

    A device is followed as the *subject* of reports. Store effects (the SM-MBR quota,
    decay, a device leaving with its store) act on the reporters' stores, so they enter
    only through the calibration's measured `lost` and `uploaded` shares; a report neither
    uploaded nor lost on its day gets the same chance again the next day."""
    P = dict(profile)
    rng = random.Random(P.get('seed', 1))
    days, ndev = int(P['days']), int(P['devices'])
    W = rates['shuffle']['window_s']
    thr = rates['shuffle']['threshold']
    lam, lam_c = rates['lambda_per_s'], rates['lambda_class_per_s']
    # corridor mix of exposure per class, from the calibration
    mix = {}
    for key, e in rates['exposure_s'].items():
        _, c, k = _cell(key)
        mix.setdefault(c, {}).setdefault(k, 0.0)
        mix[c][k] += e
    unloc = sum(rates['lambda_unlocated_per_s'].values())
    shares = P.get('class_share') or {c: sum(m.values()) for c, m in mix.items()}
    tot = sum(shares.values())
    classes = sorted(shares)
    cum, acc = [], 0.0
    for c in classes:
        acc += shares[c] / tot
        cum.append(acc)
    drive = float(P.get('drive_s_per_day', 3600.0))
    start_h = float(P.get('drive_hour_local', 8.0)) - float(P.get('t0_hour_local', 17.0))
    per_day = [dict(day=d + 1, verdicts=0, stored=0, uploaded=0, lost=0, ma=0)
               for d in range(days + 2)]
    uploads = []                              # (t, device, reports): one chunk per device-day
    for dev in range(ndev):
        u = rng.random()
        c = classes[next(i for i, x in enumerate(cum) if u <= x + 1e-12)]
        cv = rates['conversion'].get(c, {'stored_per_verdict': 0.0, 'uploaded': 0.0, 'lost': 0.0})
        m = mix.get(c, {})
        msum = sum(m.values())
        rate = sum(sum(lam.get(f'{c}|{k}', lam_c.get(c, {})).values()) * e / msum
                   for k, e in m.items()) if msum else 0.0
        rate = (rate + unloc) * _gamma(rng, rates['dispersion_shape'].get(c, 1e6))
        held = 0
        for d in range(days):
            t_start = d * DAY + (start_h % 24.0) * 3600.0
            n = _poisson(rng, rate * drive)
            per_day[d]['verdicts'] += n
            s = _binom(rng, n, min(1.0, cv['stored_per_verdict']))
            per_day[d]['stored'] += s
            gone = _binom(rng, s, cv.get('lost', 0.0))
            per_day[d]['lost'] += gone
            # the day's new reports and the held ones get the day's upload chance
            pool = held + s - gone
            k_up = _binom(rng, pool, min(1.0, cv['uploaded'] / max(1e-12, 1.0 - cv.get('lost', 0.0))))
            held = pool - k_up
            per_day[d]['uploaded'] += k_up
            if k_up:
                uploads.append((t_start + rng.random() * drive, dev, k_up))
    # the RA's shuffle (count-based), then the MA's receipts per device
    uploads.sort()
    p_ma = rates['ma'].get('delivered_per_released', rates['ma']['received_per_upload'])
    ma_t = {}
    batch, armed, nb = [], None, 0

    def release(at):
        for dv, k in batch:
            got = _binom(rng, k, min(1.0, p_ma))
            if got:
                ma_t.setdefault(dv, []).append((at, got))
    for t, dev, k in uploads:
        if armed is not None and t >= armed + W:
            release(armed + W)
            batch, armed, nb = [], None, 0
        if armed is None:
            armed = t
        batch.append((dev, k))
        nb += k
        if nb >= thr:
            release(t)
            batch, armed, nb = [], None, 0
    if batch:
        release(armed + W)
    for ts in ma_t.values():
        for t, k in ts:
            d = int(t // DAY)
            if d < len(per_day):
                per_day[d]['ma'] += k
    win = float(P.get('window_days', 7)) * DAY
    rules = {}
    dev_h = ndev * drive * days / 3600.0
    for k in P.get('k_rules', [1, 2, 5, 10, 20, 50]):
        blocked = 0
        for ts in ma_t.values():
            ts = sorted(ts)
            j, run = 0, 0
            for i in range(len(ts)):
                run += ts[i][1]
                while ts[i][0] - ts[j][0] >= win:
                    run -= ts[j][1]
                    j += 1
                if run >= k:
                    blocked += 1
                    break
        rules[str(k)] = {'honest_blocked': blocked,
                         'per_1e6_device_hours': blocked / dev_h * 1e6 if dev_h else 0.0}
    return {'schema': 'v2xw.surrogate.projection.v1', 'label': 'analytical',
            'version': VERSION, 'rates_from': rates['from_run'], 'profile': P,
            'per_day': per_day, 'k_rules': rules}


def _poisson(rng, mu):
    if mu <= 0:
        return 0
    if mu > 50:
        return max(0, int(round(rng.gauss(mu, math.sqrt(mu)))))
    L, k, p = math.exp(-mu), 0, 1.0
    while True:
        p *= rng.random()
        if p <= L:
            return k
        k += 1


def _binom(rng, n, p):
    if n <= 0 or p <= 0:
        return 0
    if p >= 1:
        return n
    if n > 200:
        return min(n, max(0, int(round(rng.gauss(n * p, math.sqrt(n * p * (1 - p)))))))
    return sum(1 for _ in range(n) if rng.random() < p)


# ---------------------------------------------------------------- the table and its selftest

def _matrix():
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        'matrix', os.path.join(HERE, '..', 'realworld', 'matrix.py'))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def e10_rows(table):
    import yaml
    doc = yaml.safe_load(open(table))
    return doc, [r for r in doc['rows'] if r['id'].startswith(('P1.E10.', 'P1.E10U.'))]


def judge_rows(rows, measures):
    mx = _matrix()
    out = {}
    for row in rows:
        ok, shown = mx.judge(row, lambda run, metric: measures[run].get(metric, 'ABSENT'))
        out[row['id']] = (ok, shown)
    return out


def selftest(table, runs_dir):
    """Each fault must turn at least one of its named rows red (rows' `fault` field)."""
    doc, rows = e10_rows(table)
    load = lambda p: json.load(open(p))
    rates = load(os.path.join(runs_dir, 'E10CAL', 'rates.json'))
    target = load(os.path.join(runs_dir, 'E10A', 'summary.json'))
    obs = observe(target)
    bad, ever_red = 0, set()
    for fault in FAULTS:
        named = [r for r in rows if fault in (r.get('fault') or '').split()]
        if not named:
            print(f'NO-ROW   {fault}: no row names this fault')
            bad += 1
            continue
        o, p = finish(dict(obs), predict(rates, target['exposure_s'], target['windows'], fault))
        pu = predict(rates, rates['exposure_s'], target['windows'], fault)
        clean = predict(rates, target['exposure_s'], target['windows'])
        res = judge_rows(named, {'E10A': o, 'E10S': p, 'E10SU': pu, 'E10S2': clean})
        red = [i for i, (ok, _) in res.items() if ok is False]
        ever_red.update(red)
        st = 'RED' if red else 'SILENT'
        bad += 0 if red else 1
        print(f'{st:8} {fault}: {len(red)}/{len(named)} named rows red ({", ".join(red)[:150]})')
    never = [r['id'] for r in rows if r['id'] not in ever_red]
    print(f'selftest: {len(FAULTS) - bad}/{len(FAULTS)} faults caught; rows never red: {never or "none"}')
    return 1 if (bad or never) else 0


def fill(table, runs_dir):
    """Judges the P1.E10 rows on `<runs-dir>/<run>/realism.json` and writes each row's
    `measured` and `verdict` in place, leaving every other row and comment untouched
    (matrix.py --fill re-dumps the whole file)."""
    doc, rows = e10_rows(table)
    meas = {}
    for run in ('E10A', 'E10S', 'E10S2', 'E10SU'):
        p = os.path.join(runs_dir, run, 'realism.json')
        meas[run] = json.load(open(p)) if os.path.exists(p) else {}
    res = judge_rows(rows, meas)
    lines = open(table).read().split('\n')
    out, cur = [], None
    for ln in lines:
        if ln.startswith('- id: '):
            cur = ln[6:].strip()
        if cur in res and ln.startswith('  measured:'):
            ok, shown = res[cur]
            out.append(f'  measured: {json.dumps(shown)}')
            out.append(f"  verdict: {'pass' if ok else ('MISSING' if ok is None else 'FAIL')}")
            continue
        if cur in res and ln.startswith('  verdict:'):
            continue
        out.append(ln)
    open(table, 'w').write('\n'.join(out))
    tally = {}
    for i, (ok, shown) in res.items():
        st = 'PASS' if ok else ('MISSING' if ok is None else 'FAIL')
        tally[st] = tally.get(st, 0) + 1
        print(f'{st:7} {i:24} {shown}'[:160])
    print('e10:', tally)


def _f(v):
    return f'{v:.4g}' if isinstance(v, float) else str(v)[:12]


def _dump(obj, path):
    with open(path, 'w') as f:
        json.dump(obj, f, indent=1, sort_keys=True)
        f.write('\n')


def main(argv):
    cmd = argv[1] if len(argv) > 1 else ''
    load = lambda p: json.load(open(p))
    if cmd == 'summarise':
        import records
        _dump(records.summarise(argv[2]), argv[3])
    elif cmd == 'fit':
        _dump(fit(load(argv[2])), argv[3])
    elif cmd == 'observe':
        _dump(observe(load(argv[2])), argv[3])
    elif cmd == 'predict':
        fault = argv[argv.index('--fault') + 1] if '--fault' in argv else None
        tgt = load(argv[3])
        _dump(predict(load(argv[2]), tgt['exposure_s'], tgt['windows'], fault), argv[4])
    elif cmd == 'project':
        _dump(project(load(argv[2]), load(argv[3])), argv[4])
    elif cmd == 'selftest':
        return selftest(argv[2], argv[3])
    elif cmd == 'fill':
        fill(argv[2], argv[3])
    elif cmd == 'compare':
        # compare <rates.json> <target-summary.json> <runs-dir>: writes the realism.json of
        # E10A (engine), E10S and E10S2 (prediction on the target's exposure, twice) and
        # E10SU (prediction on the calibration's own exposure: fully open loop)
        rates, tgt, rd = load(argv[2]), load(argv[3]), argv[4]
        o, p = finish(observe(tgt), predict(rates, tgt['exposure_s'], tgt['windows']))
        pu = finish(observe(tgt), predict(rates, rates['exposure_s'], tgt['windows']))[1]
        p2 = predict(rates, tgt['exposure_s'], tgt['windows'])
        p2['x.e10.corridor_tvd'] = 0.0
        for run, d in (('E10A', o), ('E10S', p), ('E10S2', p2), ('E10SU', pu)):
            os.makedirs(os.path.join(rd, run), exist_ok=True)
            _dump(d, os.path.join(rd, run, 'realism.json'))
        for k in sorted(o):
            if k.startswith('x.'):
                print(f'{k:28} engine {_f(o[k]):>12}  surrogate {_f(p[k]):>12}  open-loop {_f(pu[k]):>12}')
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == '__main__':
    sys.path.insert(0, HERE)
    sys.exit(main(sys.argv))
