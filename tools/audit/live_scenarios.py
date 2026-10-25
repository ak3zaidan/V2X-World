#!/usr/bin/env python3
"""Live scenario audit: every shipped scenario boots, runs a short window and stays sane.

    python3 -I tools/audit/live_scenarios.py check    <table.yaml> <runs-dir> [--only ID,...]
    python3 -I tools/audit/live_scenarios.py selftest <table.yaml> <runs-dir> [--only ID,...]

The table is crates/v2xw-engine/tests/expectations/live-scenarios.yaml. `<runs-dir>/<run>/`
holds what a run left: run-report.json, metrics.json or metrics.json.gz, run.log (stderr with
the time(1) -l lines and a final `rc=N` line). A row's `runs` is a list of run names or the
word `all` (every run directory). Each row's check returns a list of failures for a run.

`check` prints one line per row (pass, or the first failures) and exits 1 when a row's
outcome is not its `expect`. `selftest` applies each row's injected fault (`fault`, the
mutation named in the row, applied in memory to one run the row covers) and requires the
check to go red: no check here is one that cannot fail.

Independent of the engine: reads the engine's outputs and the scenario files only.
"""

import copy
import gzip
import json
import math
import os
import sys

import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# ------------------------------------------------------------------------------ loading


def _merge(a, b):
    if isinstance(a, dict) and isinstance(b, dict):
        o = dict(a)
        for k, v in b.items():
            o[k] = _merge(a[k], v) if k in a else v
        return o
    return b


def load_scenario(path, depth=0):
    d = yaml.safe_load(open(path)) or {}
    base = (d.get('meta') or {}).get('base')
    if base and depth < 8:
        d = _merge(load_scenario(os.path.normpath(os.path.join(os.path.dirname(path), base)), depth + 1), d)
    return d


class Run:
    def __init__(self, d):
        self.dir = d
        self.name = os.path.basename(d.rstrip('/'))
        p = os.path.join(d, 'run-report.json')
        self.doc = json.load(open(p)) if os.path.exists(p) else None
        self.report = (self.doc or {}).get('report', {})
        m = os.path.join(d, 'metrics.json')
        self.samples = []
        try:
            if os.path.exists(m + '.gz'):
                self.samples = json.load(gzip.open(m + '.gz'))['samples']
            elif os.path.exists(m):
                self.samples = json.load(open(m))['samples']
        except ValueError:
            pass  # a run that never started leaves an empty file
        lp = os.path.join(d, 'run.log')
        self.log = open(lp, errors='replace').read() if os.path.exists(lp) else ''
        self.rc = None
        self.window = (self.doc or {}).get('duration_s')
        for line in self.log.splitlines():
            if line.startswith('rc='):
                self.rc = int(line[3:])
            if line.startswith('window=') and self.window is None:
                self.window = float(line[7:])
        meta = os.path.join(d, 'scenario.txt')
        self.scenario_path = open(meta).read().strip() if os.path.exists(meta) else None
        self.scenario = load_scenario(os.path.join(ROOT, self.scenario_path)) if self.scenario_path else {}

    def channels(self):
        return (self.doc or {}).get('channels') or {}

    def chn(self, name):
        c = self.channels().get(name)
        return (c or {}).get('records', 0) if isinstance(c, dict) else (c or 0)


def finite(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x)


NONNEG_UNITS = {'count', 'veh/h', 'veh/km', 'B/s', 'bit/s', '1/s', 'ms/s', 'B/(veh·h)', '1/(veh·h)', 'ms', 's',
                'm', 'B/B', 'ratio', '1'}
# mac_access_delay is not here: 802.11 sends at once on a medium idle for AIFS, so 0 is right.
LATENCY = {'e2e_latency', 'aoi', 'aoi_peak', 'verify_cost'}
# Ratio-of-sums metrics whose numerator is a part of the denominator: a proportion.
PART_OF_WHOLE = {'security_overhead', 'cert_bytes_share', 'full_cert_share', 'cbr', 'channel_load',
                 'channel_occupancy', 'collision_rate', 'loss_rate', 'pdr', 'pdr_all_pairs', 'per',
                 'half_duplex_rate', 'delivery_ratio', 'backend_link_up', 'unverified_ratio'}


def numeric_fields(v):
    if isinstance(v, dict):
        for k, x in v.items():
            if isinstance(x, (int, float)) and not isinstance(x, bool):
                yield k, x


# ------------------------------------------------------------------------------ checks


def c_boots(run, p):
    out = []
    if run.rc != 0:
        out.append('exit %s' % run.rc)
    if run.doc is None:
        out.append('no run-report.json')
    return out


def c_no_panic(run, p):
    return ['panic: ' + l[:160] for l in run.log.splitlines() if 'panicked' in l][:2]


def c_records(run, p):
    out = []
    v = (run.doc or {}).get('verified') or {}
    if not v.get('integrity_verified'):
        out.append('recording not verified: %s' % v)
    if not (run.doc or {}).get('recording_bytes'):
        out.append('empty recording')
    if not run.report.get('records'):
        out.append('report.records 0')
    if not run.samples:
        out.append('no metric samples')
    return out


def c_finite(run, p):
    out = []
    for s in run.samples:
        for k, x in numeric_fields(s['value']):
            if not math.isfinite(x):
                out.append('%s.%s = %s at t=%s' % (s['metric'], k, x, s['t']))
    return out[:5]


def c_sample_arithmetic(run, p):
    """Each sample is internally consistent: a proportion is successes/trials in [0, 1] inside
    its interval; a ratio of sums is numerator/denominator; a summary's quantiles are ordered
    with the mean inside [min, max]; a count is a non-negative integer; non-negative units hold."""
    out = []
    for s in run.samples:
        v, m, u = s['value'], s['metric'], s.get('unit')
        if not isinstance(v, dict):
            continue
        kind, st = v.get('kind'), v.get('state')
        tag = '%s%s t=%ss' % (m, s.get('dims') or '', s['t'] / 1e9)
        q = max(s.get('quantum') or 0, 1e-9)
        if kind == 'ratio' and st == 'proportion':
            S, T, P = v['successes'], v['trials'], v['point']
            if not (0 <= S <= T):
                out.append('%s successes %s trials %s' % (tag, S, T))
            elif T and abs(P - S / T) > q + 1e-9:
                out.append('%s point %s != %s/%s' % (tag, P, S, T))
            if not (-1e-9 <= v['ci_lo'] <= P + q and P - q <= v['ci_hi'] <= 1 + 1e-9):
                out.append('%s interval [%s, %s] around %s' % (tag, v['ci_lo'], v['ci_hi'], P))
        elif kind == 'ratio' and st == 'ratio-of-sums':
            N, D, P = v['numerator'], v['denominator'], v['point']
            if N < 0 or D < 0:
                out.append('%s negative sums %s/%s' % (tag, N, D))
            elif P and abs(N / P - D) > 0.0005 + 1e-6 * D and abs(P * D - N) > 0.0005 + 1e-6 * N and abs(P - N / D) > max(q, 1e-6 * abs(P)):
                # the two sums are written on the 1e-3 grid (Quantum::SUM, D9), so the
                # denominator a reader sees is the true one to within half a step
                out.append('%s point %s != %s/%s' % (tag, P, N, D))
            elif not P and D and N / D > q:
                out.append('%s point 0 with numerator %s' % (tag, N))
            if m in PART_OF_WHOLE and not (-1e-9 <= P <= 1 + q):
                out.append('%s proportion %s outside [0, 1]' % (tag, P))
        elif kind == 'ratio' and st == 'insufficient':
            if v.get('successes', 0) > v.get('trials', 0) or v.get('trials', 0) < 0:
                out.append('%s insufficient with successes %s trials %s' % (tag, v.get('successes'), v.get('trials')))
        elif kind == 'distribution' and st == 'summary':
            a = [v['min'], v['p50'], v['p95'], v['p99'], v['max']]
            if any(a[i] > a[i + 1] + q for i in range(4)):
                out.append('%s quantiles out of order %s' % (tag, a))
            if not (v['min'] - q <= v['mean'] <= v['max'] + q):
                out.append('%s mean %s outside [%s, %s]' % (tag, v['mean'], v['min'], v['max']))
            if v['n'] <= 0 or v.get('rejected', 0) < 0:
                out.append('%s n %s rejected %s' % (tag, v['n'], v.get('rejected')))
            if u in NONNEG_UNITS and v['min'] < -q:
                out.append('%s negative min %s (%s)' % (tag, v['min'], u))
        elif kind == 'scalar' and st == 'value':
            if u in NONNEG_UNITS and v['point'] < -q:
                out.append('%s negative %s (%s)' % (tag, v['point'], u))
        elif kind == 'count':
            c = v.get('count')
            if not isinstance(c, int) or c < 0:
                out.append('%s count %s' % (tag, c))
    return out[:6]


def c_latency_positive(run, p):
    out = []
    for s in run.samples:
        v = s['value']
        if s['metric'] in LATENCY and isinstance(v, dict) and v.get('state') == 'summary':
            if v['min'] < 0 or v['mean'] <= 0:
                out.append('%s%s t=%ss min %s mean %s' % (s['metric'], s.get('dims') or '', s['t'] / 1e9, v['min'], v['mean']))
    return out[:5]


def c_kinematics_bounds(run, p):
    """Ground-truth speeds within [0, vmax] and accelerations within the physical envelope:
    tyre-road friction caps braking near 1 g (μ ≈ 1.0 dry asphalt), and no road vehicle
    accelerates above about 5 m/s² (Wong, Theory of Ground Vehicles)."""
    vmax, amin, amax = p.get('vmax', 45.0), p.get('amin', -10.0), p.get('amax', 5.0)
    out = []
    for s in run.samples:
        v = s['value']
        if not (isinstance(v, dict) and v.get('state') == 'summary'):
            continue
        if s['metric'] in ('speed', 'mean_speed') and (v['min'] < -1e-6 or v['max'] > vmax):
            out.append('%s%s t=%ss [%s, %s]' % (s['metric'], s.get('dims') or '', s['t'] / 1e9, v['min'], v['max']))
        if s['metric'] == 'acceleration' and (v['min'] < amin or v['max'] > amax):
            out.append('acceleration%s t=%ss [%s, %s]' % (s.get('dims') or '', s['t'] / 1e9, v['min'], v['max']))
    return out[:5]


def _demand(run):
    veh = ((run.scenario.get('actors') or {}).get('vehicles') or {})
    dem = veh.get('demand') or {}
    rate = dem.get('rate_veh_per_h') or dem.get('rate_veh_per_h_per_lane_km') or 0
    cap = (dem.get('params') or {}).get('max_total_vehicles') or dem.get('max_total_vehicles')
    return rate, cap, veh


def c_demand(run, p):
    """Vehicles appear when there is demand, and never more than a stated cap."""
    rate, cap, veh = _demand(run)
    spawned = run.report.get('actors_spawned')
    n = spawned if isinstance(spawned, int) else sum((spawned or {}).values()) if isinstance(spawned, dict) else 0
    out = []
    if rate and run.doc is not None and n <= 0:
        out.append('demand %s but 0 actors spawned' % rate)
    if cap and n > cap + (p.get('cap_slack') or 0) + _peds_cap(run):
        out.append('spawned %s over max_total_vehicles %s' % (n, cap))
    return out


def _peds_cap(run):
    act = run.scenario.get('actors') or {}
    n = 0
    for k in ('pedestrians', 'cyclists', 'motorcycles', 'vru'):
        if act.get(k):
            n += 10**6
    return n


def c_radio_output(run, p):
    """A run with vehicles and a radio sends frames and decodes some of them."""
    if run.doc is None:
        return ['no run']
    tx, ok = run.report.get('frames_transmitted', 0), run.report.get('receptions_ok', 0)
    out = []
    if not tx:
        out.append('0 frames transmitted')
    elif not ok:
        out.append('%s frames transmitted, 0 receptions ok' % tx)
    if tx and not run.chn('node.tx'):
        out.append('frames transmitted but no node.tx records')
    return out


def c_scms_output(run, p):
    """A scenario with the SCMS on writes its backend and PKI records."""
    sec = run.scenario.get('security') or {}
    proto = (sec.get('protocol') or {}).get('id', '') if isinstance(sec.get('protocol'), dict) else str(sec.get('protocol') or '')
    if not sec.get('scms') and 'scms' not in proto and not p.get('force'):
        return []
    have = [c for c in ('pki.flow', 'backend.state', 'pki.state', 'node.credentials') if run.chn(c)]
    return [] if have else ['security.scms set but no pki.flow/backend.state/pki.state/node.credentials records']


def c_threat_output(run, p):
    """A scenario with attackers produces attack records and detector verdicts."""
    thr = run.scenario.get('threats') or run.scenario.get('attacks')
    if not thr:
        return []
    starts = []
    for a in (thr.get('attackers') or []) + (thr.get('jammers') or []):
        sch = a.get('schedule') or a.get('params') or {}
        starts.append(sch.get('from_s', 0.0) or 0.0)
    if not starts:
        return []  # an empty attacker list
    if run.window is not None and min(starts) >= run.window:
        return []  # every attack starts after the window ends
    n_att = (run.report.get('phase2') or {}).get('attackers')
    if n_att == 0 and not thr.get('jammers'):
        # A Bernoulli draw per vehicle can give no attacker in a short window: that is a pass
        # only when it is not improbable, (1 - f)^N >= 1e-3 for the vehicles that ran.
        f = sum(a.get('fraction') or 0 for a in thr.get('attackers') or [])
        n = run.report.get('actors_spawned') or 0
        return [] if (1 - min(f, 1)) ** n >= 1e-3 else ['0 attackers of %s vehicles at fraction %s' % (n, f)]
    have = [c for c in run.channels() if c.startswith(THREAT_PREFIXES)]
    return [] if have else ['threats configured but no threat/attack/detector records (channels: %s)' % sorted(run.channels())[:12]]


def c_ped_output(run, p):
    """A scenario with pedestrians spawns walkers (gt.subject) and measures them (ped_* metrics)."""
    act = run.scenario.get('actors') or {}
    if not act.get('pedestrians') and not (act.get('vru') or {}).get('pedestrians'):
        return []
    seen = {s['metric'] for s in run.samples if s['metric'].startswith('ped_')}
    return [] if seen else ['pedestrians configured but no ped_* metric sample']


def c_vru_devices(run, p):
    """VRU devices: device_fraction of the walkers and riders (configured population plus the
    ones that replaced a completed trip), within 4 binomial sigma."""
    vru = (run.scenario.get('actors') or {}).get('vru') or {}
    if not vru or run.doc is None:
        return []
    n = (vru.get('pedestrians') or 0) + (vru.get('cyclists') or 0)
    f = vru.get('device_fraction')
    if f is None or not n:
        return []
    done = (run.report.get('despawn_causes') or {}).get('TripComplete', 0)
    got = run.report.get('vru_devices_created') or 0
    lo = f * n - 4 * math.sqrt(n * f * (1 - f))
    hi = f * (n + done) + 4 * math.sqrt((n + done) * f * (1 - f))
    return [] if lo <= got <= hi else ['vru_devices_created %s outside [%.0f, %.0f] (N %s, f %s)' % (got, lo, hi, n, f)]


def _pool(run, metric, dims=None, t_from=0):
    """Pool every summary sample of `metric` (dims matching) from t_from: n-weighted mean and the
    window means' median."""
    xs = []
    for s in run.samples:
        if s['metric'] != metric or s['t'] < t_from * 1e9:
            continue
        if dims and any((s.get('dims') or {}).get(k) != v for k, v in dims.items()):
            continue
        v = s['value']
        if not isinstance(v, dict):
            continue
        if v.get('state') == 'summary':
            xs.append((v['mean'], v['n'], v))
        elif v.get('state') in ('proportion',):
            xs.append((v['point'], v['trials'], v))
        elif v.get('state') == 'ratio-of-sums':
            xs.append((v['point'], v['denominator'], v))
        elif v.get('state') == 'value':
            xs.append((v['point'], 1, v))
    return xs


def measure(run, p):
    xs = _pool(run, p['metric'], p.get('dims'), p.get('t_from', 0))
    if not xs:
        return None
    field = p.get('field', 'mean')
    if field == 'mean':
        w = sum(n for _, n, _ in xs)
        return sum(x * n for x, n, _ in xs) / w if w else None
    vals = sorted(v.get(field) for _, _, v in xs if v.get(field) is not None)
    if not vals:
        return None
    return vals[len(vals) // 2]


def c_band(run, p):
    """A pooled metric inside the published or physical band [lo, hi]."""
    x = measure(run, p)
    if x is None:
        return ['%s: no samples' % p['metric']]
    lo, hi = p.get('lo', -math.inf), p.get('hi', math.inf)
    return [] if lo <= x <= hi else ['%s %s = %.4g outside [%s, %s]' % (p['metric'], p.get('field', 'mean'), x, lo, hi)]


def c_pdr_distance(run, p):
    """PDR falls with distance: each distance bin's pooled PDR is at most the nearer bin's plus
    a tolerance, the nearest bin is above `near_min` and the farthest below `far_max`."""
    bins = {}
    for s in run.samples:
        if s['metric'] != p.get('metric', 'pdr'):
            continue
        d = (s.get('dims') or {}).get(p.get('dim', 'dist_bin'))
        v = s['value']
        if d is None or not isinstance(v, dict) or v.get('state') != 'proportion':
            continue
        key = float(str(d).split('-')[0].strip('[(<>= m+'))
        a = bins.setdefault(key, [0, 0])
        a[0] += v['successes']
        a[1] += v['trials']
    pts = [(k, a[0] / a[1]) for k, a in sorted(bins.items()) if a[1] >= p.get('min_trials', 30)]
    if len(pts) < 3:
        return ['fewer than 3 distance bins with data: %s' % pts]
    out = []
    tol = p.get('tol', 0.05)
    for (d0, a), (d1, b) in zip(pts, pts[1:]):
        if b > a + tol:
            out.append('PDR rises from %.3f at %s m to %.3f at %s m' % (a, d0, b, d1))
    if pts[0][1] < p.get('near_min', 0.9):
        out.append('nearest bin %s m PDR %.3f under %s' % (pts[0][0], pts[0][1], p.get('near_min', 0.9)))
    if pts[-1][1] > p.get('far_max', 1.0):
        out.append('farthest bin %s m PDR %.3f over %s' % (pts[-1][0], pts[-1][1], p.get('far_max')))
    return out


def c_fundamental(run, p):
    """Edie's definitions make q = k * v exact per window (flow veh/h/ln, density veh/km/ln,
    mean_speed m/s): q / (3.6 k v) = 1 within the three quanta; and the network-wide values stay
    under the physical bounds: q at most the HCM basic-segment capacity (2,400 pc/h/ln), k at most
    a jam density (about 150 veh/km/ln for 6.7 m spacing)."""
    by = {}
    for s in run.samples:
        if s['metric'] in ('flow', 'density', 'mean_speed') and not s.get('dims'):
            v = s['value']
            if isinstance(v, dict) and 'point' in v and v.get('state') in ('value', 'ratio-of-sums'):
                by.setdefault(s['t'], {})[s['metric']] = v['point']
    out, n = [], 0
    for t, d in sorted(by.items()):
        if len(d) < 3:
            continue
        q, k, v = d['flow'], d['density'], d['mean_speed']
        if q > p.get('q_max', 2400) or k > p.get('k_max', 150):
            out.append('t=%ss q %s k %s over the physical bound' % (t / 1e9, q, k))
        if k * v > 0.05 and q > 0.05:
            n += 1
            # each of the three is rounded (TRAFFIC quantum 1e-3; speed 1e-3): the tolerance
            # is the propagated half-step
            tol = 0.0005 / q + 0.0005 / k + 0.0005 / v + 1e-6
            if abs(q / (3.6 * k * v) - 1) > max(tol, p.get('rel', 0.002)):
                out.append('t=%ss q %s != 3.6 k v = %.4f' % (t / 1e9, q, 3.6 * k * v))
    if not n and p.get('need', False):
        out.append('no window with traffic')
    return out[:4]


def c_ratio_between(run, p, runs):
    """measure(this run) / measure(run `other`) inside [lo, hi]."""
    other = runs.get(p['other'])
    if other is None:
        return ['run %s missing' % p['other']]
    a, b = measure(run, p), measure(other, p)
    if a is None or b is None or not b:
        return ['%s: no samples (%s, %s)' % (p['metric'], a, b)]
    r = a / b
    return [] if p.get('lo', -math.inf) <= r <= p.get('hi', math.inf) else ['%s ratio %.4f (%.4g / %.4g) outside [%s, %s]' % (p['metric'], r, a, b, p.get('lo'), p.get('hi'))]


def c_same_digest(run, p, runs):
    other = runs.get(p.get('other') or p['other_of'].get(run.name))
    if other is None or run.doc is None or other.doc is None:
        return ['missing run']
    a, b = run.doc.get('record_digest'), other.doc.get('record_digest')
    return [] if a and a == b else ['record_digest %s != %s' % (a, b)]


CHECKS = {
    'boots': c_boots, 'no_panic': c_no_panic, 'records': c_records, 'finite': c_finite,
    'sample_arithmetic': c_sample_arithmetic, 'latency_positive': c_latency_positive,
    'kinematics_bounds': c_kinematics_bounds, 'demand': c_demand, 'radio_output': c_radio_output,
    'scms_output': c_scms_output, 'threat_output': c_threat_output, 'ped_output': c_ped_output,
    'band': c_band, 'pdr_distance': c_pdr_distance, 'same_digest': c_same_digest,
    'fundamental': c_fundamental, 'vru_devices': c_vru_devices, 'ratio_between': c_ratio_between,
}

PAIRED = {'same_digest', 'ratio_between'}
THREAT_PREFIXES = ('gt.attack', 'threat.', 'attack.', 'mbd.', 'det.', 'misbehavior', 'ma.', 'jam')

# ------------------------------------------------------------------------------ faults


def _first(run, pred):
    for s in run.samples:
        if pred(s):
            return s
    return None


def apply_fault(run, fault, row):
    r = copy.deepcopy(run)
    f = fault
    if f == 'exit-code':
        r.rc = 101
    elif f == 'panic-line':
        r.log += "\nthread 'main' panicked at crates/v2xw-engine/src/run.rs:1:1:\n"
    elif f == 'unverified':
        r.doc['verified']['integrity_verified'] = False
    elif f == 'nan':
        s = _first(r, lambda s: isinstance(s['value'], dict) and 'point' in s['value'])
        s['value']['point'] = float('nan')
    elif f == 'successes-over-trials':
        s = _first(r, lambda s: isinstance(s['value'], dict) and s['value'].get('state') == 'proportion')
        s['value']['successes'] = s['value']['trials'] + 1
    elif f == 'negative-latency':
        s = _first(r, lambda s: s['metric'] in LATENCY and s['value'].get('state') == 'summary')
        s['value']['min'] = -1.0
    elif f == 'speed-80':
        s = _first(r, lambda s: s['metric'] == 'speed' and s['value'].get('state') == 'summary')
        s['value']['max'] = 80.0
        s['value']['p99'] = min(s['value']['p99'], 80.0)
    elif f == 'no-spawn':
        r.report['actors_spawned'] = 0
    elif f == 'no-rx':
        r.report['receptions_ok'] = 0
    elif f == 'drop-scms-channels':
        for c in ('pki.flow', 'backend.state', 'pki.state', 'node.credentials'):
            r.doc['channels'].pop(c, None)
    elif f == 'drop-threat-channels':
        for c in list(r.doc['channels']):
            if c.startswith(THREAT_PREFIXES):
                r.doc['channels'].pop(c)
    elif f == 'drop-ped-metrics':
        r.samples = [s for s in r.samples if not s['metric'].startswith('ped_')]
    elif f.startswith('scale:'):
        k = float(f.split(':')[1])
        m = row['params']['metric']
        for s in r.samples:
            if s['metric'] == m and isinstance(s['value'], dict):
                for fld in ('mean', 'min', 'max', 'p50', 'p95', 'p99', 'point'):
                    if fld in s['value']:
                        s['value'][fld] *= k
    elif f == 'pdr-rise':
        # make the farthest bin perfect
        far = None
        for s in r.samples:
            d = (s.get('dims') or {}).get(row['params'].get('dim', 'dist_bin'))
            if s['metric'] == row['params'].get('metric', 'pdr') and d is not None:
                key = float(str(d).split('-')[0].strip('[(<>= m+'))
                far = key if far is None else max(far, key)
        for s in r.samples:
            d = (s.get('dims') or {}).get(row['params'].get('dim', 'dist_bin'))
            if d is not None and float(str(d).split('-')[0].strip('[(<>= m+')) == far and s['value'].get('state') == 'proportion':
                s['value']['successes'] = s['value']['trials']
    elif f == 'vru-devices-0':
        r.report['vru_devices_created'] = 0
    elif f == 'flow-x2':
        for s in r.samples:
            if s['metric'] == 'flow' and 'point' in s['value']:
                s['value']['point'] *= 2
    elif f == 'digest':
        r.doc['record_digest'] = 'f' * 64
    else:
        raise SystemExit('unknown fault %s' % f)
    return r


# ------------------------------------------------------------------------------ driver


def main():
    mode, table, rdir = sys.argv[1], sys.argv[2], sys.argv[3]
    only = None
    if '--only' in sys.argv:
        only = set(sys.argv[sys.argv.index('--only') + 1].split(','))
    doc = yaml.safe_load(open(table))
    names = sorted(n for n in os.listdir(rdir) if os.path.isdir(os.path.join(rdir, n)))
    cache = {}

    def get(n):
        if n not in cache:
            cache[n] = Run(os.path.join(rdir, n))
        return cache[n]

    bad = 0
    for row in doc['rows']:
        if only and row['id'] not in only:
            continue
        fn = CHECKS[row['check']]
        p = row.get('params') or {}
        exempt = row.get('exempt') or {}  # run -> the evidence that the row does not apply
        targets = [n for n in (names if row['runs'] == 'all' else [n for n in row['runs'] if n in names]) if n not in exempt]
        missing = [] if row['runs'] == 'all' else [n for n in row['runs'] if n not in names]
        if mode == 'check':
            fails = {}
            for n in targets:
                run = get(n)
                if run.doc is None and row['check'] not in ('boots', 'records'):
                    # a run that did not start fails SC.boots; the content rows judge only runs
                    # that ran, unless they name the run
                    if row['runs'] != 'all':
                        fails[n] = ['no run']
                    continue
                res = fn(run, p, {k: get(k) for k in names}) if row['check'] in PAIRED else fn(run, p)
                if res:
                    fails[n] = res
            outcome = 'pass' if not fails and not missing and targets else 'fail'
            want = row.get('expect', 'pass')
            mark = 'ok ' if outcome == want else 'BAD'
            if outcome != want:
                bad += 1
            print('%s %-28s %s %d runs%s' % (mark, row['id'], outcome, len(targets), (' missing %s' % missing) if missing else ''))
            for n, res in list(fails.items())[:p.get('show', 8)]:
                print('      %s: %s' % (n, '; '.join(res[:2])[:300]))
            if len(fails) > p.get('show', 8):
                print('      ... %d more runs' % (len(fails) - p.get('show', 8)))
        else:
            # Fault one run the row covers (its `fault_run`, else the first target where the
            # row passes and the fault applies) and require the check to go red there.
            cands = [row['fault_run']] if row.get('fault_run') else targets
            red = False
            for n in cands:
                run = get(n)
                if run.doc is None:
                    continue
                args = (p, {k: get(k) for k in names}) if row['check'] in PAIRED else (p,)
                if fn(run, *args):
                    continue
                try:
                    fr = apply_fault(run, row['fault'], row)
                except (TypeError, AttributeError, KeyError):
                    continue
                red = bool(fn(fr, *args))
                print('%s %-28s fault %-24s on %s -> %s' % ('ok ' if red else 'BAD', row['id'], row['fault'], n, 'red' if red else 'still green'))
                break
            else:
                print('BAD %-28s fault %-24s no run to fault' % (row['id'], row['fault']))
            if not red:
                bad += 1
    print('verdict: %s (%d bad)' % ('pass' if not bad else 'FAIL', bad))
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
