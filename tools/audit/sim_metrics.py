#!/usr/bin/env python3
"""Whole-simulation SCMS metrics audit: every SCMS metric recomputed from the raw records.

    python3 -I tools/audit/sim_metrics.py check  <table.yaml> <runs-dir> [--only ID,...]
    python3 -I tools/audit/sim_metrics.py selftest <table.yaml> <runs-dir> [--only ID,...]

`check` evaluates every row of the expectation table (crates/v2xw-engine/tests/expectations/
scms-audit-sim-metrics.yaml) against `<runs-dir>/<run>/` (recording.mcap, metrics.json,
run-report.json, and `<run>.log` with the time(1) -l lines) and prints one line per row and
run, then a verdict line; exit 1 when a row's outcome is not its `expect`.

`selftest` runs every row's check again with its injected fault (the row's `fault`: a raw
record or an engine value mutated in memory) and requires each check to go red; it proves
that no check here is one that cannot fail.

Independent of the engine: it reads the records and the engine's outputs only, never engine
code, and recomputes each metric from the definitions the table cites. Reads MCAP with the
`mcap` package. Written by the sim-metrics audit.
"""

import collections
import copy
import hashlib
import json
import math
import os
import sys

import re

NS = 10**9
THIN = ('backend.state', 'pki.state', 'pki.knowledge')
T_FIELD = re.compile(rb'"t":(\d+)')
W = 10 * NS  # metrics.window_s

# ------------------------------------------------------------------------------ loading


class Run:
    def __init__(self, d):
        self.dir = d
        self.name = os.path.basename(d.rstrip('/'))
        self.report_doc = json.load(open(os.path.join(d, 'run-report.json')))
        self.report = self.report_doc['report']
        self.p2 = self.report.get('phase2', {})
        self.samples = json.load(open(os.path.join(d, 'metrics.json')))['samples']
        self._rec = None
        self.grid = sorted({s['t'] for s in self.samples if s['metric'] == 'cert_pool_valid'}
                           or {s['t'] for s in self.samples})

    def records(self, topics=None):
        """{channel: [(log_time, dict), ...]} in file order, cached."""
        if self._rec is None:
            from mcap.reader import make_reader
            out = collections.defaultdict(list)
            for path in self.mcaps():
                with open(path, 'rb') as f:
                    for _s, ch, msg in make_reader(f).iter_messages(log_time_order=False):
                        c = ch.topic.split('/', 1)[1] if ch.topic.startswith('record/') else ch.topic
                        if c in ('node.rx', 'phy.rx', 'phy.prr', 'gt.kinematics'):
                            continue
                        if c in THIN:
                            # Snapshots no check reads beyond their instant: keep `t` only.
                            m = T_FIELD.search(msg.data[:64])
                            out[c].append((msg.log_time, {'t': int(m.group(1))} if m else {}))
                            continue
                        out[c].append((msg.log_time, json.loads(msg.data)))
            self._rec = out
        return self._rec

    def mcaps(self):
        segs = sorted(f for f in os.listdir(self.dir) if f.startswith('recording') and f.endswith('.mcap'))
        return [os.path.join(self.dir, f) for f in segs]

    def ch(self, name):
        return self.records().get(name, [])

    def cost(self):
        log = self.dir.rstrip('/') + '.log'
        wall = peak = None
        if os.path.exists(log):
            for line in open(log):
                w = line.split()
                if len(w) >= 2 and w[1] == 'real':
                    wall = float(w[0])
                if 'peak memory footprint' in line:
                    peak = int(w[0]) / 1e6
        return wall, peak

    def end_ns(self):
        return int(round(self.report_doc['duration_s'] * NS))


def dkey(d):
    return json.dumps(d, sort_keys=True)


def sample_index(samples, metric):
    out = {}
    for s in samples:
        if s['metric'] == metric:
            out[(s['t'], dkey(s['dims']))] = s
    return out


_GRID = []


def set_grid(times):
    """The run's flush instants, sorted. A window is (previous flush, flush]: 10 s on the
    grid, but a fast-forward boundary flushes too (and restarts the grid there)."""
    _GRID[:] = sorted(times)


def wend(lt):
    """The flush instant of the window holding log time `lt`."""
    import bisect
    i = bisect.bisect_left(_GRID, lt)
    if i == 0 and lt <= _GRID[0] - W:
        return None  # warm-up: no provider window holds it
    return _GRID[i] if i < len(_GRID) else None


def wlen(t):
    """The window's length in seconds, ending at flush `t`."""
    import bisect
    i = bisect.bisect_left(_GRID, t)
    prev = _GRID[i - 1] if i > 0 else _GRID[0] - W
    return (t - prev) / NS


class Fail(Exception):
    pass


def need(cond, msg):
    if not cond:
        raise Fail(msg)


def close(a, b, q):
    """Equal on the metric's grid: within half a quantum plus float noise."""
    return abs(a - b) <= q / 2 + 1e-9 * max(1.0, abs(a), abs(b))


def windows_of(samples, metric):
    return sorted({s['t'] for s in samples if s['metric'] == metric})


# ------------------------------------------------------------------------------ checks
# Each check(run, fault) raises Fail with the first disagreement, or returns a short summary.
# With fault=True it first applies the row's injected fault to its own copy of the input.

SCMS_WINDOWED = ['verify_rate', 'verify_cost', 'verify_wait', 'verify_queue_depth', 'unverified_ratio',
                 'full_cert_share', 'p2pcd_requests', 'envelope_overhead', 'security_overhead',
                 'cert_bytes_share', 'cert_pool_valid', 'backend_link_up', 'bytes_backend',
                 'bytes_backhaul', 'bytes_uu_ul', 'bytes_uu_dl', 'linkability', 'pseudonym_change_rate',
                 'det_tp', 'det_fp', 'det_fn', 'det_tn', 'det_recall', 'det_precision', 'det_fpr',
                 'det_f1', 'det_accuracy', 'time_to_detect', 'time_to_decision', 'false_accusations',
                 'crl_entries', 'crl_bytes', 'e2e_latency', 'latency_stage']

UNITS = {'verify_rate': '1/s', 'p2pcd_requests': '1/s', 'bytes_backend': 'B/s', 'bytes_backhaul': 'B/s',
         'bytes_uu_ul': 'B/s', 'bytes_uu_dl': 'B/s', 'unverified_ratio': 'ratio', 'full_cert_share': 'ratio',
         'envelope_overhead': 'ratio', 'security_overhead': 'ratio', 'cert_bytes_share': 'ratio',
         'backend_link_up': 'ratio', 'linkability': 'ratio', 'cert_pool_valid': 'count',
         'pseudonym_change_rate': '1/(veh·h)', 'time_to_detect': 's', 'time_to_decision': 's',
         'crl_entries': 'count', 'crl_bytes': 'B', 'verify_cost': 'ms', 'verify_wait': 'ms',
         'det_tp': 'count', 'det_recall': 'ratio', 'false_accusations': 'count'}


def c_samples_recorded(run, fault):
    rec = [d for _lt, d in run.ch('metric.sample')]
    if fault:
        rec = rec[1:]
    js = collections.Counter((s['metric'], s['t'], dkey(s['dims'])) for s in run.samples)
    rc = collections.Counter((s['metric'], s['t'], dkey(s['dims'])) for s in rec)
    missing = js - rc
    extra = rc - js
    need(not missing, f'{sum(missing.values())} samples in metrics.json are not in the recording, '
         f'e.g. {sorted(missing)[:3]}; by metric {dict(collections.Counter(k[0] for k in missing))}')
    need(not extra, f'{sum(extra.values())} recorded samples are not in metrics.json')
    vj = {(s['metric'], s['t'], dkey(s['dims'])): s['value'] for s in run.samples}
    bad = []
    for s in rec:
        a, b = s['value'], vj[(s['metric'], s['t'], dkey(s['dims']))]
        for k in set(a) | set(b):
            x, y = a.get(k), b.get(k)
            if isinstance(x, float) or isinstance(y, float):
                if x is None or y is None or not close(x, y, 1e-6):
                    bad.append((s['metric'], s['t'], k, x, y))
            elif x != y:
                bad.append((s['metric'], s['t'], k, x, y))
    need(not bad, f'{len(bad)} recorded values differ from metrics.json, e.g. {bad[:2]}')
    return f'{len(rec)} samples equal'


def c_windows(run, fault):
    samples = copy.deepcopy(run.samples) if fault else run.samples
    if fault:
        for s in samples:
            if s['metric'] == 'full_cert_share':
                s['t'] += NS
                break
    warm = int(round(float(run.report.get('warmup_s', 0)) * NS)) if 'warmup_s' in run.report else None
    end = run.end_ns()
    import yaml
    scen = yaml.safe_load(open(os.path.join(run.dir, 'scenario.resolved.yaml')))
    ff = [(int(round(w['from_s'] * NS)), int(round(w['to_s'] * NS)))
          for w in (scen.get('time', {}).get('fast_forward') or [])]
    edges = {a for a, _b in ff} | {b for _a, b in ff}
    seen = collections.Counter()
    for s in samples:
        if s['metric'] not in SCMS_WINDOWED:
            continue
        seen[(s['metric'], s['t'], dkey(s['dims']))] += 1
        need(s['t'] % W == 0 or s['t'] in edges,
             f"{s['metric']} sampled at {s['t']} ns, neither on the 10 s grid nor at a fast-forward edge")
        need(not any(a < s['t'] < b for a, b in ff), f"{s['metric']} sampled at {s['t']} inside a fast-forward gap")
        need(0 < s['t'] <= end, f"{s['metric']} sampled at {s['t']} outside (0, end]")
    dup = [k for k, v in seen.items() if v > 1]
    need(not dup, f'{len(dup)} (metric, t, dims) flushed twice, e.g. {dup[:2]}')
    return f'{len(seen)} SCMS samples on the grid'


def c_units(run, fault):
    samples = copy.deepcopy(run.samples) if fault else run.samples
    if fault:
        for s in samples:
            if s['metric'] == 'verify_rate' or s['metric'] == 'full_cert_share':
                s['unit'] = 'x'
                break
    n = 0
    for s in samples:
        u = UNITS.get(s['metric'])
        if u is not None:
            need(s['unit'] == u, f"{s['metric']} unit {s['unit']!r}, the definition says {u!r}")
            n += 1
    return f'{n} units as defined'


def _tx(run, fault, mut):
    rows = run.ch('node.tx')
    if fault:
        rows = list(rows)
        for i, (lt, d) in enumerate(rows):
            if lt > run.grid[0] and d.get('signer') is not None and d.get('envelope_bytes'):
                d2 = dict(d)
                mut(d2)
                rows[i] = (lt, d2)
                break
    return rows


def _cmp_ratio_sums(run, metric, num, den, rows, bytype):
    idx = sample_index(run.samples, metric)
    acc = collections.defaultdict(lambda: [0, 0, 0])
    for lt, d in rows:
        if num(d) is None or den(d) is None:
            continue
        for key in ([None, d.get('msg_type')] if bytype else [None]):
            a = acc[(wend(lt), key)]
            a[0] += num(d)
            a[1] += den(d)
            a[2] += 1
    n = 0
    for (t, dk), s in idx.items():
        dims = json.loads(dk)
        key = dims.get('msg_type')
        if set(dims) - {'msg_type'}:
            continue
        a = acc.get((t, key), [0, 0, 0])
        v = s['value']
        if v['state'] == 'ratio-of-sums':
            need(int(round(v['numerator'])) == a[0] and int(round(v['denominator'])) == a[1] and v['n'] == a[2],
                 f'{metric} {dims} at {t / NS:.0f} s: engine {v["numerator"]}/{v["denominator"]} n={v["n"]}, records {a}')
            need(close(v['point'], a[0] / a[1], s['quantum']), f'{metric} point {v["point"]} vs {a[0] / a[1]}')
        else:
            need(a[1] == 0, f'{metric} {dims} at {t / NS:.0f} s insufficient but records give {a}')
        n += 1
    need(n > 0, f'no {metric} samples')
    return f'{n} samples equal'


def c_envelope_overhead(run, fault):
    rows = _tx(run, fault, lambda d: d.__setitem__('envelope_bytes', d['envelope_bytes'] + 1))
    return _cmp_ratio_sums(run, 'envelope_overhead', lambda d: d.get('envelope_bytes'),
                           lambda d: d.get('payload_bytes') if d.get('envelope_bytes') is not None else None, rows, False)


def c_security_overhead(run, fault):
    rows = _tx(run, fault, lambda d: d.__setitem__('envelope_bytes', d['envelope_bytes'] + 1))
    return _cmp_ratio_sums(run, 'security_overhead', lambda d: d.get('envelope_bytes'),
                           lambda d: d.get('bytes_on_wire') if d.get('envelope_bytes') is not None else None, rows, True)


def c_cert_bytes_share(run, fault):
    rows = _tx(run, fault, lambda d: d.__setitem__('cert_bytes', (d.get('cert_bytes') or 0) + 1))
    return _cmp_ratio_sums(run, 'cert_bytes_share', lambda d: d.get('cert_bytes'),
                           lambda d: d.get('envelope_bytes') if d.get('cert_bytes') is not None else None, rows, True)


def _cmp_prop(run, metric, rows_by_window, label):
    idx = sample_index(run.samples, metric)
    n = 0
    for (t, dk), s in idx.items():
        if json.loads(dk):
            continue
        succ, trials = rows_by_window.get(t, (0, 0))
        v = s['value']
        need(v.get('trials', 0) == trials and v.get('successes', 0) == succ,
             f'{metric} at {t / NS:.0f} s: engine {v.get("successes")}/{v.get("trials")}, records {succ}/{trials} ({label})')
        if v['state'] == 'proportion':
            need(close(v['point'], succ / trials, s['quantum']), f'{metric} point {v["point"]} vs {succ / trials}')
        n += 1
    need(n > 0, f'no {metric} samples')
    return f'{n} windows equal'


def c_full_cert_share(run, fault):
    def flip(d):
        d['signer'] = 'digest' if d['signer'] == 'certificate' else 'certificate'
    rows = _tx(run, fault, flip)
    acc = collections.defaultdict(lambda: [0, 0])
    for lt, d in rows:
        if d.get('signer') is None:
            continue
        a = acc[wend(lt)]
        a[0] += d['signer'] == 'certificate'
        a[1] += 1
    return _cmp_prop(run, 'full_cert_share', {k: tuple(v) for k, v in acc.items()}, 'node.tx signer')


def _sec_rows(run, fault, mut):
    rows = run.ch('node.security')
    if fault:
        rows = list(rows)
        for i, (lt, d) in enumerate(rows):
            if lt > 40 * NS:
                d2 = dict(d)
                mut(d2)
                rows[i] = (lt, d2)
                break
    return rows


def c_backend_link_up(run, fault):
    rows = _sec_rows(run, fault, lambda d: d.__setitem__('link_up', not d['link_up']))
    acc = collections.defaultdict(lambda: [0, 0])
    for lt, d in rows:
        a = acc[wend(lt)]
        a[0] += bool(d['link_up'])
        a[1] += 1
    return _cmp_prop(run, 'backend_link_up', {k: tuple(v) for k, v in acc.items()}, 'node.security link_up')


def c_cert_pool_valid(run, fault):
    rows = _sec_rows(run, fault, lambda d: d.__setitem__('pool_valid', 0 if d['pool_valid'] else 1))
    acc = collections.defaultdict(list)
    for lt, d in rows:
        acc[wend(lt)].append(d['pool_valid'])
    idx = sample_index(run.samples, 'cert_pool_valid')
    n = 0
    for (t, _dk), s in idx.items():
        xs = acc.get(t, [])
        v = s['value']
        need(v.get('n', 0) == len(xs), f'cert_pool_valid at {t / NS:.0f} s: n {v.get("n")} vs {len(xs)} rows')
        if v['state'] == 'summary':
            need(v['min'] == min(xs) and v['max'] == max(xs) and close(v['mean'], sum(xs) / len(xs), s['quantum']),
                 f'cert_pool_valid at {t / NS:.0f} s: {v["min"]}/{v["mean"]}/{v["max"]} vs {min(xs)}/{sum(xs) / len(xs)}/{max(xs)}')
        n += 1
    need(n > 0, 'no cert_pool_valid samples')
    return f'{n} windows equal'


def c_p2pcd_requests(run, fault):
    rows = run.ch('node.telemetry')
    acc = collections.Counter()
    seen = set()
    first = True
    for lt, d in rows:
        if d.get('p2pcd_requests') is None:
            continue
        x = d['p2pcd_requests']
        if fault and first and lt > 40 * NS:
            x += 1
            first = False
        acc[wend(lt)] += x
        seen.add(wend(lt))
    idx = sample_index(run.samples, 'p2pcd_requests')
    n = 0
    for (t, _dk), s in idx.items():
        v = s['value']
        need(v['n'] == acc[t], f'p2pcd_requests at {t / NS:.0f} s: n {v["n"]} vs telemetry sum {acc[t]}')
        need(close(v['point'], acc[t] / wlen(t), s['quantum']), f'p2pcd at {t / NS:.0f} s point {v["point"]} vs {acc[t] / wlen(t)}')
        n += 1
    windows = set(windows_of(run.samples, 'verify_queue_depth'))
    missing = sorted(t for t in seen & windows if (t, '{}') not in idx)
    need(not missing, f'p2pcd_requests missing in windows with telemetry: {missing[:3]}')
    need(n > 0, 'no p2pcd_requests samples')
    return f'{n} windows equal'


def c_bytes_buckets(run, fault):
    rows = run.ch('net.bytes')
    acc = collections.Counter()
    first = True
    for lt, d in rows:
        b = d['bytes_on_wire']
        if fault and first and lt > 40 * NS:
            b += 10
            first = False
        acc[(wend(lt), d['bucket'])] += b
    n = 0
    for metric in ('bytes_backend', 'bytes_backhaul', 'bytes_uu_ul', 'bytes_uu_dl'):
        for (t, dk), s in sample_index(run.samples, metric).items():
            b = json.loads(dk).get('bucket')
            v = s['value']
            got = acc[(t, b)]
            if v['state'] == 'value':
                need(close(v['point'], got / wlen(t), s['quantum']),
                     f'{metric}[{b}] at {t / NS:.0f} s: {v["point"]} B/s vs net.bytes {got / wlen(t)}')
            else:
                need(got == 0, f'{metric}[{b}] at {t / NS:.0f} s insufficient, records {got}')
            n += 1
    need(n > 0, 'no bytes_* samples')
    return f'{n} samples equal'


def _changes(run):
    """Pseudonym changes: one per sec.pseudonym record, else the node.security changes counter rise."""
    sp = run.ch('sec.pseudonym')
    if sp:
        return len(sp)
    last = {}
    for _lt, d in run.ch('node.security'):
        last[d['node']] = max(last.get(d['node'], 0), d.get('changes', 0))
    return sum(last.values())


def c_linkability(run, fault):
    ch = _changes(run) + (1 if fault else 0)
    idx = sample_index(run.samples, 'linkability')
    tmax = max(t for t, _ in idx)
    v = idx[(tmax, '{}')]['value']
    need(v.get('trials', 0) == ch, f'linkability trials {v.get("trials")} vs {ch} pseudonym changes in the records '
         f'(report pseudonym_changes {run.p2.get("pseudonym_changes")})')
    claims = [d for _lt, d in run.ch('privacy.link') if d.get('predecessor')]
    need(v.get('successes', 0) <= len(claims), f'linkability successes {v.get("successes")} > {len(claims)} claims')
    return f'trials {ch}, successes {v.get("successes", 0)} of {len(claims)} claims'


def c_det_matrix(run, fault):
    subj = {d['subject']: d['actor'] for _lt, d in run.ch('gt.subject')}
    attackers = {d['actor'] for _lt, d in run.ch('gt.attack.action') if d.get('changed_bytes_on_air')}
    at_ma = {d['subject'] for _lt, d in run.ch('ma.report')}
    if fault:
        honest = sorted(s for s, a in subj.items() if a not in attackers)
        at_ma = at_ma | {honest[0]}
    tp = sum(1 for s, a in subj.items() if a in attackers and s in at_ma)
    fn = sum(1 for s, a in subj.items() if a in attackers and s not in at_ma)
    fp = sum(1 for s, a in subj.items() if a not in attackers and s in at_ma)
    tn = sum(1 for s, a in subj.items() if a not in attackers and s not in at_ma)
    end = max(windows_of(run.samples, 'det_tp'))
    cells = {}
    for m in ('det_tp', 'det_fp', 'det_fn', 'det_tn'):
        for (t, dk), s in sample_index(run.samples, m).items():
            if t == end and json.loads(dk).get('level') == 'report':
                cells[m] = s['value']['count']
    got = (cells.get('det_tp'), cells.get('det_fp'), cells.get('det_fn'), cells.get('det_tn'))
    need(got == (tp, fp, fn, tn), f'report-level matrix engine {got} vs records {(tp, fp, fn, tn)} '
         f'({len(subj)} subjects, {len(attackers)} attackers, {len(at_ma)} subjects at the MA)')
    # summaries are functions of the cells
    for (t, dk), s in sample_index(run.samples, 'det_recall').items():
        if t == end and json.loads(dk).get('level') == 'report':
            v = s['value']
            need(v.get('trials') == tp + fn and v.get('successes') == tp, f'det_recall {v} vs tp {tp} fn {fn}')
    return f'tp {tp} fp {fp} fn {fn} tn {tn}'


def _verify_rows(run, fault):
    rows = run.ch('node.verify')
    need(rows, 'no node.verify records (needs --record-level receptions)')
    if fault:
        rows = list(rows)
        for i, (lt, d) in enumerate(rows):
            if lt > run.grid[0] and d.get('outcome') in ('valid', 'Valid'):
                del rows[i]
                break
    return rows


def _outcome(d):
    return str(d.get('outcome', '')).lower()


def c_verify_rate(run, fault):
    rows = _verify_rows(run, fault)
    acc = collections.Counter()
    for lt, d in rows:
        if _outcome(d) in ('valid', 'invalid'):
            acc[(wend(lt), d.get('primitive') or 'unspecified')] += 1
    n = 0
    for (t, dk), s in sample_index(run.samples, 'verify_rate').items():
        p = json.loads(dk).get('primitive')
        v = s['value']
        need(v['n'] == acc[(t, p)], f'verify_rate[{p}] at {t / NS:.0f} s: n {v["n"]} vs node.verify {acc[(t, p)]}')
        need(close(v['point'], acc[(t, p)] / wlen(t), s['quantum']), f'verify_rate point {v["point"]}')
        n += 1
    need(n > 0, 'no verify_rate samples')
    return f'{n} samples equal'


def c_unverified_ratio(run, fault):
    rows = run.ch('node.verify')
    need(rows, 'no node.verify records')
    acc = collections.defaultdict(lambda: [0, 0])
    first = True
    for lt, d in rows:
        o = _outcome(d)
        if fault and first and lt > 40 * NS and o == 'valid':
            o = 'skipped'
            first = False
        if o in ('valid', 'invalid', 'skipped'):
            a = acc[wend(lt)]
            a[0] += o == 'skipped'
            a[1] += 1
    return _cmp_prop(run, 'unverified_ratio', {k: tuple(v) for k, v in acc.items()}, 'node.verify outcomes')


def c_verify_cost_wait(run, fault):
    rows = _verify_rows(run, fault)
    cost = collections.defaultdict(list)
    wait = collections.defaultdict(list)
    for lt, d in rows:
        if _outcome(d) in ('valid', 'invalid'):
            if d.get('cost_us') is not None:
                cost[wend(lt)].append(d['cost_us'] / 1000.0)
            if d.get('t_start') is not None and d['t_start'] >= d['t_enqueue']:
                wait[wend(lt)].append((d['t_start'] - d['t_enqueue']) / 1e6)
    n = 0
    for metric, acc in (('verify_cost', cost), ('verify_wait', wait)):
        for (t, _dk), s in sample_index(run.samples, metric).items():
            xs = acc.get(t, [])
            v = s['value']
            need(v.get('n', 0) == len(xs), f'{metric} at {t / NS:.0f} s: n {v.get("n")} vs {len(xs)}')
            if v['state'] == 'summary':
                need(close(v['min'], min(xs), s['quantum']) and close(v['max'], max(xs), s['quantum']),
                     f'{metric} at {t / NS:.0f} s min/max {v["min"]}/{v["max"]} vs {min(xs)}/{max(xs)}')
            n += 1
    return f'{n} samples equal'


def c_crl_metrics(run, fault):
    rows = [d for _lt, d in run.ch('proto.revocation')]
    if fault and rows:
        # the last publication recorded one entry and 32 bytes larger than it was
        last = dict(rows[-1])
        last['entries'] = (last.get('entries') or 0) + 1
        last['size_bytes'] = (last.get('size_bytes') or 0) + 32
        rows = rows[:-1] + [last]
    last_e = last_b = None
    for d in rows:
        if d.get('entries') is not None:
            last_e = d['entries']
        if d.get('size_bytes') is not None:
            last_b = d['size_bytes']
    idx_e = sample_index(run.samples, 'crl_entries')
    idx_b = sample_index(run.samples, 'crl_bytes')
    if last_e is None:
        need(not idx_e and not idx_b, 'crl_entries sampled but no proto.revocation record carries entries')
        need(not rows or not fault, 'fault applied')
        stages = sample_index(run.samples, 'revocation_latency_stage')
        need(not stages, f'{len(stages)} revocation_latency_stage samples but no proto.revocation records')
        need(run.p2.get('crl_versions_published', 0) == 0 or not fault, 'fault')
        return (f'no proto.revocation records; crl_entries/crl_bytes never sampled although the run report '
                f'counts {run.p2.get("crl_versions_published")} CRL versions published and '
                f'{len(run.p2.get("revocations_by_provider") or {})} revoking providers')
    tmax = max(t for t, _ in idx_e)
    need(idx_e[(tmax, '{}')]['value']['count'] == last_e, f'crl_entries {idx_e[(tmax, "{}")]["value"]} vs {last_e}')
    need(idx_b[(tmax, '{}')]['value']['count'] == last_b, f'crl_bytes vs {last_b}')
    return f'crl_entries {last_e}, crl_bytes {last_b}'


def c_report_fate(run, fault):
    p = dict(run.p2)
    if fault:
        p['reports_sent'] += 1
    terms = ['reports_uploaded_cellular', 'reports_uploaded_relay', 'reports_unsent', 'reports_evicted',
             'reports_expired', 'reports_not_filed', 'reports_lost', 'reports_refused_mtu']
    s = sum(p.get(k, 0) for k in terms)
    need(p['reports_sent'] == s, f'reports_sent {p["reports_sent"]} != sum of fates {s} '
         f'({ {k: p.get(k, 0) for k in terms} })')
    return f'{s} reports, every fate counted'


def c_reports_received(run, fault):
    msgs = [d for _lt, d in run.ch('pki.flow') if d['kind'] == 'msg' and d.get('flow') == 'report' and d.get('name') == 'report']
    if fault:
        msgs = msgs[1:] if msgs else [{'kind': 'msg', 'flow': 'report', 'name': 'report'}]
    byp = sum((run.p2.get('reports_by_provider') or {}).values())
    up = run.p2.get('reports_uploaded_cellular', 0) + run.p2.get('reports_uploaded_relay', 0)
    rec = run.p2.get('reports_received', 0)
    need(rec == byp, f'reports_received {rec} != sum reports_by_provider {byp}')
    need(rec == len(msgs), f'reports_received {rec} != {len(msgs)} pki.flow report messages')
    need(rec <= up, f'reports_received {rec} > uploads {up}')
    return f'{rec} received = {len(msgs)} recorded uploads'


def c_report_truth(run, fault):
    a, h = run.p2.get('reports_about_attackers', 0), run.p2.get('reports_about_honest', 0) + (1 if fault else 0)
    need(a + h == run.p2['reports_sent'], f'about attackers {a} + honest {h} != reports_sent {run.p2["reports_sent"]}')
    return f'{a} + {h}'


def c_verify_states(run, fault):
    vs = dict(run.p2.get('verification_states') or {})
    if fault:
        vs['Verified'] = vs.get('Verified', 0) + 1
    s = sum(vs.values())
    need(s == run.p2['messages_checked'], f'sum verification_states {s} != messages_checked {run.p2["messages_checked"]}')
    return f'{s} states'


def c_privacy_links(run, fault):
    rows = run.ch('privacy.link')
    n = len(rows) - (1 if fault else 0)
    need(n == run.p2.get('privacy_link_decisions'), f'privacy.link records {n} != privacy_link_decisions {run.p2.get("privacy_link_decisions")}')
    claimed = sum(1 for _lt, d in rows if d.get('predecessor'))
    sp, co = run.p2.get('privacy_links_spurious', 0), run.p2.get('privacy_links_correct', 0)
    need(co <= claimed, f'correct links {co} > claims {claimed}')
    return f'{n} decisions, {claimed} claims, {sp} spurious, {co} correct'


def c_attack_onset(run, fault):
    """Corrected (see the table's note): the run report's `onset_ns` is the first claim the
    07-threats 4 label rule calls falsified (position > 1 m off, ...), which can follow the
    attacker's first on-air change; so it must not precede that change, and no report about
    the attacker may precede the change (nobody can detect a lie not yet told)."""
    msg_node = {d['msg']: d['node'] for _lt, d in run.ch('node.tx') if d.get('msg') is not None}
    first = {}
    for _lt, d in run.ch('gt.attack.action'):
        if d.get('changed_bytes_on_air') and d.get('msg') in msg_node:
            n = msg_node[d['msg']]
            first[n] = min(first.get(n, d['t']), d['t'])
    outs = list(run.p2.get('attacker_outcomes') or [])
    if fault and first:
        first[sorted(first)[0]] += 3600 * NS  # its lie told an hour later than recorded
    elif fault:
        outs.append({'node': -1, 'falsified_claims': 1})  # an attacker with no recorded change
    need(set(o['node'] for o in outs if o.get('falsified_claims')) <= set(first),
         f'attackers with falsified claims but no recorded on-air change: '
         f'{sorted(set(o["node"] for o in outs) - set(first))}')
    bad = []
    for o in outs:
        f = first.get(o['node'])
        if f is None:
            continue
        if o.get('onset_ns') is not None and o['onset_ns'] < f:
            bad.append((o['node'], 'label onset before the first change', o['onset_ns'] / NS, f / NS))
        if o.get('first_reported_ns') is not None and o['first_reported_ns'] < f:
            bad.append((o['node'], 'reported before the first change', o['first_reported_ns'] / NS, f / NS))
    need(not bad, f'{bad[:4]}')
    neg = [s for s in run.samples if s['metric'] == 'time_to_detect' and s['value'].get('state') == 'summary'
           and s['value']['min'] < 0]
    need(not neg, f'negative time_to_detect: {neg[:1]}')
    early = [(o['node'], round((o['onset_ns'] - first[o['node']]) / NS, 3)) for o in outs
             if o.get('onset_ns') is not None and o['node'] in first]
    return f'{len(outs)} attackers; label onset after first change by {early} s'


def c_preload(run, fault):
    reqs = [d for _lt, d in run.ch('pki.flow') if d['kind'] == 'pdu' and d.get('flow') == 'preload']
    ee = {d['from'] for d in reqs if d['name'] == 'EeEcaCertRequestSpdu'}
    if fault:
        ee = set(sorted(ee)[1:])
    dev = sum((run.p2.get('devices_by_provider') or {}).values())
    need(len(ee) == dev, f'{len(ee)} devices enrolled in the preload vs devices_by_provider {dev}')
    c = collections.Counter(d['name'] for d in reqs)
    need(c['RaAcaCertRequestSpdu'] == c['AcaRaCertResponseSpdu'], f'RA->ACA {c["RaAcaCertRequestSpdu"]} vs ACA->RA {c["AcaRaCertResponseSpdu"]}')
    return f'{dev} devices'


def c_time_order(run, fault):
    rec = run.records()
    end = run.end_ns()
    bad = []
    for ch, rows in rec.items():
        rows = list(rows)
        if fault and ch == 'node.tx' and len(rows) > 10:
            rows[5] = (rows[6][0] + 1, rows[5][1])  # a record logged after its successor
        prev = -1
        for lt, d in rows:
            if lt < prev:
                bad.append((ch, 'log time went back', prev, lt))
                break
            prev = lt
            if lt > end:
                bad.append((ch, 'after the end', lt))
                break
            t = d.get('t') if isinstance(d, dict) else None
            if isinstance(t, int) and t > lt:
                bad.append((ch, 't after log time', t, lt))
                break
    need(not bad, f'{bad[:4]}')
    return f'{sum(len(r) for r in rec.values())} records in order'


def c_cost(run, fault, bound):
    wall, peak = run.cost()
    need(wall is not None, 'no time(1) lines')
    b = dict(bound)
    if fault:
        b = {k: v / 1000 for k, v in b.items()}
    need(wall <= b['wall_s'] and peak <= b['peak_mb'], f'wall {wall:.0f} s / peak {peak:.0f} MB over {b}')
    ev = sum(run.report['events_by_class'].values())
    return f'wall {wall:.0f} s, peak {peak:.0f} MB, {ev} events'


def digest_of(run, fault):
    h = hashlib.sha256()
    for path in run.mcaps():
        from mcap.reader import make_reader
        with open(path, 'rb') as f:
            for i, (_s, ch, msg) in enumerate(make_reader(f).iter_messages(log_time_order=False)):
                data = msg.data
                if fault and i == 1000:
                    data = data + b' '
                h.update(ch.topic.encode())
                h.update(msg.log_time.to_bytes(8, 'little'))
                h.update(data)
    return h.hexdigest()[:16]


def same_digest(runs, fault):
    out = []
    ref = None
    for i, r in enumerate(runs):
        rd = r.report_doc.get('record_digest')
        md = hashlib.sha256(open(os.path.join(r.dir, 'metrics.json'), 'rb').read()).hexdigest()[:16]
        cd = digest_of(r, fault and i == len(runs) - 1)
        row = (rd, md, cd)
        out.append((r.name, row))
        if ref is None:
            ref = row
        need(row == ref, f'{r.name} differs from {runs[0].name}: {row} vs {ref}')
    return f'{len(runs)} runs identical: record {ref[0][:12]}, metrics {ref[1]}, records {ref[2]}'


CHECKS = {k[2:]: v for k, v in globals().items() if k.startswith('c_')}


# ------------------------------------------------------------------------------ server
def server_checks(run, fault, which, answers):
    import server_probe
    return server_probe.check(run, fault, which, json.load(open(answers)), Fail)


# ------------------------------------------------------------------------------ driver
def load_table(path):
    import yaml
    return yaml.safe_load(open(path))


def evaluate(table, runs_dir, only, fault):
    cache = {}

    def get(name):
        if name not in cache:
            cache[name] = Run(os.path.join(runs_dir, name))
        return cache[name]

    results = []
    order = []
    for row in table['rows']:
        for n in row['runs']:
            if n not in order:
                order.append(n)
    rows = []
    for name in order:
        for row in table['rows']:
            if row['runs'][-1 if row['check'] == 'same_digest' else 0] == name or (
                    row['check'] != 'same_digest' and name in row['runs']):
                rows.append((name, row))
    seen = set()
    for name, row in rows:
        for k in list(cache):
            if k != name and row['check'] != 'same_digest':
                del cache[k]
        if only and row['id'] not in only:
            continue
        key = (row['id'], name)
        if key in seen:
            continue
        seen.add(key)
        chk = row['check']
        targets = [row['runs']] if chk == 'same_digest' else [[name]]
        for names in targets:
            if any(not os.path.exists(os.path.join(runs_dir, n, 'run-report.json')) for n in names):
                results.append((row['id'], '+'.join(names), 'SKIP', 'run missing'))
                continue
            try:
                set_grid(get(names[0]).grid)
                if chk == 'same_digest':
                    msg = same_digest([get(n) for n in names], fault)
                elif chk == 'cost':
                    msg = c_cost(get(names[0]), fault, row['bound'])
                elif chk.startswith('server_'):
                    if not os.path.exists(os.path.join(runs_dir, names[0] + '.server.json')):
                        results.append((row['id'], '+'.join(names), 'SKIP', 'no server answers recorded'))
                        continue
                    msg = server_checks(get(names[0]), fault, chk, os.path.join(runs_dir, names[0] + '.server.json'))
                else:
                    msg = CHECKS[chk](get(names[0]), fault)
                results.append((row['id'], '+'.join(names), 'PASS', msg))
            except Fail as e:
                results.append((row['id'], '+'.join(names), 'RED', str(e)))
    return results


def main(argv):
    if len(argv) < 4 or argv[1] not in ('check', 'selftest'):
        print(__doc__)
        return 2
    table = load_table(argv[2])
    only = set(argv[argv.index('--only') + 1].split(',')) if '--only' in argv else None
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    expect = {r['id']: r.get('expect', 'pass') for r in table['rows']}
    known_red = {r['id'] for r in table['rows'] if r.get('verdict') == 'open'}
    fault = argv[1] == 'selftest'
    res = evaluate(table, argv[3], only, fault)
    bad = 0
    for rid, run, st, msg in res:
        if fault:
            ok = st in ('RED', 'SKIP')
        else:
            ok = st == 'SKIP' or (st == 'PASS') == (expect[rid] == 'pass') or (st == 'RED' and rid in known_red)
        bad += not ok
        print(f'{"ok " if ok else "BAD"} {rid:22s} {run:10s} {st:4s} {msg[:400]}')
    print(f'{"selftest" if fault else "check"}: {len(res) - bad} of {len(res)} as expected')
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
