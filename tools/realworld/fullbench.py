#!/usr/bin/env python3
"""Summarise and judge a full-scale benchmark run (docs/design/scms-real/testing/full-bench.md).

    fullbench.py summarise <out-dir> <run.log> <summary.json>
    fullbench.py check <scms-full-bench.yaml> <run-id> <summary.json> [--inject]

`summarise` reads the run's progress.jsonl (wall per part, census), metrics.json (the live
windows around each fast-forward), run-report.json (Phase 2 and fast-forward counts) and the
`/usr/bin/time -l` tail of run.log (rc, peak footprint), and writes one flat JSON of keys.
`check` judges every `checks` entry of the rows whose variant is <run-id>. `--inject` proves
each check can fail: it moves the measured value just outside its expectation and requires
the check to go red (a check that cannot fail is reported and the exit code is 2).
"""
import json
import math
import os
import re
import sys

FF = [(240.0, 43170.0), (43230.0, 172800.0)]  # the benchmark's time.fast_forward windows


def _wall_at(lines, t):
    for ln in lines:
        if ln['t_s'] >= t - 1e-6:
            return ln['wall_s']
    return None


def summarise(out, log, dst):
    s = {}
    txt = open(log, errors='replace').read()
    m = re.findall(r'rc=(\d+)', txt)
    s['run.rc'] = int(m[-1]) if m else None
    m = re.search(r'(\d+)\s+peak memory footprint', txt)
    s['run.peak_mb'] = int(m.group(1)) / 1048576 if m else None
    m = re.search(r'(\d+)\s+maximum resident set size', txt)
    s['run.max_rss_mb'] = int(m.group(1)) / 1048576 if m else None
    m = re.search(r'([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys', txt)
    if m:
        s['run.real_s'], s['run.user_s'], s['run.sys_s'] = map(float, m.groups())
    prog = [json.loads(x) for x in open(os.path.join(out, 'progress.jsonl')) if x.strip()]
    prog.sort(key=lambda x: x['t_s'])
    s['progress.lines'] = len(prog)
    if prog:
        end = prog[-1]['wall_s']
        w240, w1, w2 = _wall_at(prog, FF[0][0]), _wall_at(prog, FF[0][1]), _wall_at(prog, FF[1][0])
        s['wall.first_window_s'] = w240
        if w1 is not None:
            s['wall.ff1_s'] = w1 - w240
        if w2 is not None:
            s['wall.rollover_s'] = w2 - w1
            s['wall.ff2_s'] = end - w2
        s['wall.engine_s'] = end
        s['progress.footprint_max_mb'] = max(x.get('footprint_mb', 0) for x in prog)
        s['progress.t_end'] = prog[-1]['t_s']
        first = [x['t_s'] for x in prog if x['census'].get('scms.ma_reports', 0) > 0]
        s['census.ma_first_receipt_s'] = first[0] if first else None
        s['census.ma_reports_end'] = prog[-1]['census'].get('scms.ma_reports')
        for x in prog:
            if abs(x['t_s'] - 240) < 1e-6 or abs(x['t_s'] - 43230) < 1e-6:
                s[f"census.nodes_{int(x['t_s'])}"] = x['census'].get('nodes')
                s[f"census.footprint_{int(x['t_s'])}"] = x.get('footprint_mb')
                s[f"census.kernel_steps_{int(x['t_s'])}"] = x['census'].get('scms.kernel_steps')
                s[f"census.pca_issued_{int(x['t_s'])}"] = x['census'].get('scms.pca_issued')
    mp = os.path.join(out, 'metrics.json')
    if os.path.exists(mp) and os.path.getsize(mp):
        _metrics(json.load(open(mp))['samples'], s)
    rp = os.path.join(out, 'run-report.json')
    if os.path.exists(rp):
        _report(json.load(open(rp)), s)
    json.dump(s, open(dst, 'w'), indent=1, sort_keys=True)
    return s


def _live(t):
    return not any(a < t <= b for a, b in FF) or t in (FF[0][0], FF[1][0])


def _metrics(samples, s):
    by = {}
    for x in samples:
        by.setdefault((x['metric'], tuple(sorted(x['dims'].items()))), []).append(x)
    ends = sorted({x['t'] / 1e9 for x in samples})
    s['win.ends'] = ends
    s['win.has_240'] = 240.0 in ends
    s['win.has_43230'] = 43230.0 in ends
    frozen = [t for t in ends if not _live(t)]
    s['win.frozen_ends'] = frozen

    def series(name, dims=()):
        return [(x['t'] / 1e9, x['value']) for x in by.get((name, dims), [])]

    def live_vals(name, dims=(), field='mean'):
        return [(t, v.get(field)) for t, v in series(name, dims) if _live(t) and v.get(field) is not None]

    # verify_rate is per primitive: sum the primitives' rates per window
    vr = {}
    for (name, dims), xs in by.items():
        if name == 'verify_rate':
            for x in xs:
                v = x['value']
                val = v.get('point', v.get('mean', v.get('value')))
                if isinstance(val, (int, float)):
                    vr[x['t'] / 1e9] = vr.get(x['t'] / 1e9, 0.0) + val
    s['win.verify_rate_240'] = vr.get(240.0)
    s['win.verify_rate_series'] = sorted(vr.items())
    cbr = [v for (name, dims), xs in by.items() if name == 'cbr' for x in xs
           if _live(x['t'] / 1e9) and x['t'] / 1e9 > 120 for v in [x['value']]]
    if cbr:
        s['live.cbr_mean'] = sum(v['mean'] * v['n'] for v in cbr if v.get('n')) / max(1, sum(v['n'] for v in cbr if v.get('n')))
        s['live.cbr_p95_max'] = max(v.get('p95') or 0 for v in cbr)
    for r in ('100', '300'):
        num = den = 0.0
        for (name, dims), xs in by.items():
            if name == 'pdr' and dims and dims[0][0] == 'radius' and str(dims[0][1]).rstrip('m') == r:
                for x in xs:
                    if _live(x['t'] / 1e9):
                        v = x['value']
                        num += v.get('successes') or v.get('numerator') or 0
                        den += v.get('trials') or v.get('denominator') or 0
        if den:
            s['live.pdr_' + r] = num / den
    e = [v for t, v in series('e2e_latency', (('msg_type', 'bsm'),)) if _live(t) and t > 120 and v.get('mean') is not None]
    if e:
        s['live.bsm_e2e_ms'] = sum(v['mean'] * v['n'] for v in e) / sum(v['n'] for v in e)
        s['live.bsm_e2e_p99_max'] = max(v.get('p99') or 0 for v in e)
    pool = {t: v for t, v in series('cert_pool_valid')}
    pv = pool.get(43210.0)
    if pv:
        s['pool.after_43210'] = pv.get('mean', pv.get('point'))
    for name, dims in (('unverified_ratio', ()), ('full_cert_share', ()), ('aoi', ()),
                       ('pseudonym_change_rate', ()), ('mean_speed', ())):
        vals = [v.get('point', v.get('mean')) for t, v in series(name, dims) if _live(t) and t > 120]
        vals = [x for x in vals if x is not None]
        if vals:
            s[f'live.{name}_min'], s[f'live.{name}_max'] = min(vals), max(vals)
    sp = [v.get('p50') for t, v in series('speed') if _live(t) and t > 120 and v.get('p50') is not None]
    if sp:
        s['live.speed_p50_series'] = sp
    for b in ('bytes_backend',):
        for (name, dims), xs in by.items():
            if name == b:
                for x in xs:
                    t = x['t'] / 1e9
                    if 43200 <= t <= 43230:
                        v = x['value']
                        s[f'{b}.{dims[0][1] if dims else "all"}.{int(t)}'] = v.get('count', v.get('point', v.get('mean')))


def _report(R, s):
    rep = R.get('report', R)
    ff = rep.get('fast_forward') or {}
    s['ff.backend_ticks'] = ff.get('backend_ticks')
    for k, v in (ff.get('stops_by_cause') or {}).items():
        s['ff.stops.' + k] = v
    s['ff.crl_poll_stops'] = (ff.get('stops_by_cause') or {}).get('crl-poll')
    p2 = rep.get('phase2') or {}
    for k, v in p2.items():
        if isinstance(v, (int, float)):
            s['p2.' + k] = v
    vb = p2.get('verdicts_by_detector') or {}
    for k, v in vb.items():
        s['p2.verdicts.' + k] = v
    s['p2.verdicts_total'] = sum(vb.values())
    s['p2.devices'] = sum((p2.get('devices_by_provider') or {}).values())
    for k in ('reports_not_filed_by',):
        for kk, v in (p2.get(k) or {}).items():
            s[f'p2.{k}.{kk}'] = v
    done = s.get('p2.topups_completed', 0)
    if done:
        s['topup.certs_per_topup'] = s.get('p2.certs_topped_up', 0) / done
    s['topup.left_area'] = s.get('p2.topups_left_area', 0)
    s['topup.incomplete'] = s.get('p2.topups_started', 0) - done - s['topup.left_area']
    up = s.get('p2.reports_uploaded_cellular', 0) + s.get('p2.reports_uploaded_relay', 0)
    if up:
        s['rep.uploaded'] = up
        s['rep.uploaded_minus_at_ma_share'] = (up - s.get('p2.reports_at_ma', 0)) / up
    for k in ('frames_transmitted', 'frames_received', 'nodes_created', 'records', 'mac_drops',
              'sdus_fragmented', 'fragments_sent', 'events'):
        if isinstance(rep.get(k), (int, float)):
            s['r.' + k] = rep[k]
    # the second fast-forward's cost per stop: its wall over its stops (crl-poll dominates)
    if s.get('wall.ff2_s') and ff.get('backend_ticks'):
        s['ff2.ms_per_stop'] = 1000 * s['wall.ff2_s'] / ff['backend_ticks']


def judge(op, want, v):
    if v is None:
        return None
    if op == 'eq':
        return v == want
    if op == 'le':
        return v <= want
    if op == 'ge':
        return v >= want
    if op == 'band':
        return want[0] <= v <= want[1]
    if op == 'true':
        return bool(v)
    raise SystemExit(f'unknown op {op}')


def fault(op, want, v):
    """A value just outside the expectation (what a defect would show)."""
    if op == 'eq':
        return want + 1 if isinstance(want, (int, float)) else not want
    if op == 'le':
        return want * 1.05 + 1
    if op == 'ge':
        return want * 0.95 - 1
    if op == 'band':
        return want[1] * 1.05 + 1
    if op == 'true':
        return False


def check(table, run, summ, inject):
    import yaml
    doc = yaml.safe_load(open(table))
    s = json.load(open(summ))
    bad = 0
    cannot_fail = 0
    for row in doc['rows']:
        if row.get('variant') != run or not row.get('checks'):
            continue
        res = []
        for key, op, want in row['checks']:
            v = s.get(key)
            ok = judge(op, want, v)
            vs = f'{v:.4g}' if isinstance(v, float) else str(v)
            res.append((key, ok, vs))
            if ok is False or ok is None:
                bad += 1
            if inject and judge(op, want, fault(op, want, v)) is not False:
                cannot_fail += 1
                print(f'  CANNOT FAIL {row["id"]} {key}')
        verdict = 'PASS' if all(r[1] for r in res) else 'FAIL'
        print(f'{verdict} {row["id"]}: ' + '; '.join(
            f'{k}={vs}' + ('' if ok else (' MISSING' if ok is None else ' RED')) for k, ok, vs in res))
    if inject:
        print(f'injection: {cannot_fail} checks could not fail')
        return 2 if cannot_fail else 0
    return 1 if bad else 0


if __name__ == '__main__':
    a = sys.argv[1:]
    if a and a[0] == 'summarise':
        s = summarise(a[1], a[2], a[3])
        print(json.dumps({k: v for k, v in s.items() if not isinstance(v, list)}, sort_keys=True)[:4000])
    elif a and a[0] == 'check':
        sys.exit(check(a[1], a[2], a[3], '--inject' in a))
    else:
        print(__doc__)
        sys.exit(64)
