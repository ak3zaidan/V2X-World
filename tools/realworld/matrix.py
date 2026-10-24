#!/usr/bin/env python3
"""SCMS configuration matrix: same-seed base/variant runs checked against expectations
written before the runs (docs/design/scms-real/testing/matrix.md).

    python -I tools/realworld/matrix.py extract <run-dir>            # -> <run-dir>/realism.json
    python -I tools/realworld/matrix.py check <matrix.yaml> <runs-dir> [--fill]

`extract` is `runtime_realism.py extract` (the run report, the PKI view records and the
run cost) plus the run's pooled metric samples from `metrics.json`, as `m.<metric>` for an
undimensioned metric and `m.<metric>.<dim value>` for one dimension (stage, level,
msg_type, cause, primitive, class, channel):
- a ratio pools as sum(numerator) / sum(denominator);
- a distribution as its n-weighted mean (`.mean`), the largest window p95 (`.p95`) and n
  (`.n`);
- a count as the sum, a scalar as the mean of the window points.

`check` reads the table (schema v2xw/scms-run-expectations/1, the effects table's form:
`runs` and `rows`) and judges each row on `<runs-dir>/<run>/realism.json`. A row is
`{id, base, variant, area, measure, expect, value|band, source, ...}`:
- `eq`, `le`, `ge`, `band` judge the variant's value (`value`, or `band: [lo, hi]`);
- `ratio`, `delta` judge variant/base or variant - base in `band`;
- `up`, `down` need variant > base (< base) by more than `min_change` (relative, default
  0.0); `same` needs |variant/base - 1| <= `tol` (default 0.0: identical);
- `true`, `false`, `empty`, `subset` judge the variant's value.
`--fill` writes each row's `measured` (base -> variant, or the value) and the checker's
verdict into the table where the row has no human verdict (`fixed`, `wrong-expectation`
or `open` are kept). It prints one line per row and a totals line; exit 1 on a FAIL.

It shares no code with the engine: it reads the run's files only.
"""

import importlib.util
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DIMS = ('stage', 'level', 'msg_type', 'cause', 'primitive', 'class', 'channel', 'app')


def _realism():
    spec = importlib.util.spec_from_file_location('runtime_realism', os.path.join(HERE, 'runtime_realism.py'))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def pool_metrics(path):
    out = {}
    if not os.path.exists(path):
        return out
    acc = {}
    for s in json.load(open(path))['samples']:
        dims = s['dims']
        if len(dims) > 1 or (dims and next(iter(dims)) not in DIMS):
            continue
        key = 'm.' + s['metric'] + ('.' + str(next(iter(dims.values()))) if dims else '')
        v = s['value']
        a = acc.setdefault(key, {'kind': v.get('kind'), 'num': 0.0, 'den': 0.0, 'wsum': 0.0, 'n': 0,
                                 'p95': -math.inf, 'cnt': 0, 'pts': []})
        k = v.get('kind')
        if k == 'ratio':
            # a ratio of sums, or a proportion (successes of trials)
            a['num'] += v.get('numerator') or v.get('successes') or 0.0
            a['den'] += v.get('denominator') or v.get('trials') or 0.0
        elif k == 'distribution':
            n = v.get('n') or 0
            if n and v.get('mean') is not None:
                a['wsum'] += v['mean'] * n
                a['n'] += n
                if v.get('p95') is not None:
                    a['p95'] = max(a['p95'], v['p95'])
        elif k == 'count':
            a['cnt'] += v.get('count') or 0
        elif k == 'scalar' and v.get('point') is not None:
            a['pts'].append(v['point'])
    for key, a in acc.items():
        k = a['kind']
        if k == 'ratio' and a['den']:
            out[key] = a['num'] / a['den']
        elif k == 'distribution' and a['n']:
            out[key + '.mean'] = a['wsum'] / a['n']
            out[key + '.p95'] = a['p95']
            out[key + '.n'] = a['n']
        elif k == 'count':
            out[key] = a['cnt']
        elif k == 'scalar' and a['pts']:
            out[key] = sum(a['pts']) / len(a['pts'])
    return out


def extract(run_dir):
    d = _realism().extract(run_dir)
    d.update(pool_metrics(os.path.join(run_dir, 'metrics.json')))
    rep = json.load(open(os.path.join(run_dir, 'run-report.json')))['report']
    for k in ('frames_transmitted', 'frames_received', 'receptions_ok', 'reception_attempts',
              'nodes_created', 'vru_devices_created', 'records', 'mac_drops', 'sdus_fragmented',
              'fragments_sent', 'reassembly_lost', 'p2pcd_responses_on_air', 'credentials_refused'):
        if isinstance(rep.get(k), (int, float)):
            d['r.' + k] = rep[k]
    for k, v in (rep.get('rx_losses') or {}).items():
        d['r.rx_loss.' + k] = v
    for k, v in ((rep.get('receiver_validation') or {}).get('refused') or {}).items():
        d['rv.refused.' + k] = v
    air = rep.get('air_bytes') or {}
    for k, v in air.items():
        if isinstance(v, (int, float)):
            d['air.' + k] = v
    return d


def judge(row, get):
    op = row['expect']
    v = get(row['variant'], row['measure'])
    if v in ('NO-RUN', 'ABSENT'):
        return None, f'{v}'
    if op in ('ratio', 'delta', 'up', 'down', 'same', 'changes'):
        b = get(row['base'], row['measure'])
        if b in ('NO-RUN', 'ABSENT'):
            return None, f'base {b}'
        shown = f'{fmt(b)} -> {fmt(v)}'
        if op in ('ratio', 'delta'):
            x = (v / b if b else (math.inf if v else 1.0)) if op == 'ratio' else v - b
            lo, hi = row['band']
            return lo <= x <= hi, shown + f' ({op} {x:.4g})'
        r = (v / b - 1.0) if b else (math.inf if v > 0 else (-math.inf if v < 0 else 0.0))
        mc = row.get('min_change', 0.0)
        if op == 'up':
            return r > mc, shown
        if op == 'down':
            return r < -mc, shown
        if op == 'changes':
            return abs(r) > mc, shown
        return abs(r) <= row.get('tol', 0.0), shown
    shown = fmt(v)
    if op == 'eq':
        return v == row['value'], shown
    if op == 'le':
        return v <= row['value'], shown
    if op == 'ge':
        return v >= row['value'], shown
    if op == 'band':
        lo, hi = row['band']
        return lo <= v <= hi, shown
    if op == 'true':
        return bool(v), shown
    if op == 'false':
        return not v, shown
    if op == 'empty':
        return len(v) == 0, shown
    if op == 'subset':
        return set(v) <= set(row['value']), shown
    raise SystemExit(f"row {row['id']}: unknown expect {op}")


def fmt(v):
    if isinstance(v, float):
        return f'{v:.6g}'
    if isinstance(v, list):
        return ','.join(map(str, v)) or '[]'
    return str(v)


def check(table, runs_dir, fill):
    import yaml
    doc = yaml.safe_load(open(table))
    cache = {}

    def get(run, metric):
        if run not in cache:
            p = os.path.join(runs_dir, run, 'realism.json')
            cache[run] = _realism().derive(json.load(open(p))) if os.path.exists(p) else None
        d = cache[run]
        if d is None:
            return 'NO-RUN'
        return d.get(metric, 0 if metric.startswith('edge.') else 'ABSENT')

    tally = {'PASS': 0, 'FAIL': 0, 'MISSING': 0, 'OPEN': 0}
    for row in doc['rows']:
        for f in ('base', 'variant'):
            if row[f] not in doc['runs']:
                raise SystemExit(f"row {row['id']}: run {row[f]} not defined")
        ok, shown = judge(row, get)
        human = row.get('verdict') in ('fixed', 'wrong-expectation', 'open')
        if ok is None:
            st = 'MISSING'
        elif ok:
            st = 'PASS'
        else:
            st = 'OPEN' if human else 'FAIL'
        tally[st] += 1
        print(f"{st:7} {row['id']:34} {row['measure']} = {shown}"[:220])
        if fill and ok is not None:
            row['measured'] = shown
            if not human:
                row['verdict'] = 'pass' if ok else 'FAIL'
    print('matrix: ' + ', '.join(f'{k} {v}' for k, v in tally.items()) + f" ({len(doc['rows'])} rows)")
    if fill:
        head = []
        for line in open(table):
            if line.startswith('#'):
                head.append(line)
            else:
                break
        with open(table, 'w') as f:
            f.writelines(head)
            yaml.safe_dump(doc, f, sort_keys=False, width=100, allow_unicode=True)
    return 1 if tally['FAIL'] else 0


if __name__ == '__main__':
    if sys.argv[1] == 'extract':
        d = extract(sys.argv[2])
        out = sys.argv[3] if len(sys.argv) > 3 else os.path.join(sys.argv[2], 'realism.json')
        with open(out, 'w') as f:
            json.dump(d, f, indent=1, sort_keys=True, default=str)
        print(f'{sys.argv[2]}: {len(d)} measurements')
    elif sys.argv[1] == 'check':
        sys.exit(check(sys.argv[2], sys.argv[3], '--fill' in sys.argv))
