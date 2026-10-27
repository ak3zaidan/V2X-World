#!/usr/bin/env python3
"""detect-fix3 live rows: the TS 103 759 suite in detection.local and the stated accuracy
(crates/v2xw-engine/tests/expectations/detect-fix3.yaml).

    python3 -I tools/realworld/detect_fix3.py extract <run-dir>        # -> <run-dir>/dfx.json
    python3 -I tools/realworld/detect_fix3.py check <table.yaml> <runs-dir> [--fill]
    python3 -I tools/realworld/detect_fix3.py selftest <table.yaml> <runs-dir>

`extract` reads `run-report.json` and the run slot's `cost.txt` (`wall_s=`, `maxrss_mb=`).
`check` judges every row (base and variant folders are `<runs-dir>/<run id>`); `--fill`
writes `measured` and `verdict` back for rows still pending. `selftest` applies each row's
`fault` and fails unless every row goes red under it.
"""
import json
import os
import re
import sys


def extract(run_dir):
    rep = json.load(open(os.path.join(run_dir, 'run-report.json')))
    r = rep['report']
    p = r.get('phase2') or {}
    d = {'digest': rep.get('stream_digest') or rep.get('record_digest') or rep.get('content_digest')}
    for k, v in r.items():
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            d['r.' + k] = v
    for k, v in p.items():
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            d['p2.' + k] = v
    for k, v in (p.get('verdicts_by_detector') or {}).items():
        d['p2.verdicts.' + k] = v
    d['air.psdu'] = (r.get('air_bytes') or {}).get('psdu')

    def rates(prefix, c):
        n = c.get('evaluated', 0)
        d[prefix + '.evaluated'] = n
        d[prefix + '.exceeded'] = c.get('exceeded', 0)
        d[prefix + '.fired'] = c.get('fired', 0)
        d[prefix + '.exceeded_rate'] = c.get('exceeded', 0) / n if n else None
        d[prefix + '.fired_rate'] = c.get('fired', 0) / n if n else None

    att = {'evaluated': 0, 'exceeded': 0, 'fired': 0}
    for cls, c in (p.get('ts_messages') or {}).items():
        rates('tsm.' + cls, c)
        if cls != 'honest':
            for k in att:
                att[k] += c.get(k, 0)
    if p.get('ts_messages'):
        rates('tsm.attacking', att)
    for check, by in (p.get('ts_checks') or {}).items():
        a = {'evaluated': 0, 'exceeded': 0, 'fired': 0}
        for cls, c in by.items():
            rates('tsc.%s.%s' % (check, cls), c)
            if cls != 'honest':
                for k in a:
                    a[k] += c.get(k, 0)
        rates('tsc.%s.attacking' % check, a)
    cost = os.path.join(run_dir, 'cost.txt')
    if os.path.exists(cost):
        for tok in open(cost).read().split():
            if '=' in tok:
                k, v = tok.split('=', 1)
                try:
                    d['cost.' + k] = float(v)
                except ValueError:
                    pass
    json.dump(d, open(os.path.join(run_dir, 'dfx.json'), 'w'), indent=0, sort_keys=True)
    return d


def load_yaml(path):
    import yaml
    return yaml.safe_load(open(path))


def measures(runs_dir, run):
    f = os.path.join(runs_dir, run, 'dfx.json')
    return json.load(open(f)) if os.path.exists(f) else None


def judge(row, base, var):
    """(measured, ok) or (None, None) when an input is missing."""
    m = row['measure']
    e = row['expect']
    v = var.get(m) if var else None
    if e in ('same', 'ratio_band', 'ge_base'):
        b = base.get(m) if base else None
        if v is None or b is None:
            return None, None
        if e == 'same':
            return '%s vs %s' % (v, b), v == b
        if e == 'ge_base':
            return '%s vs %s' % (v, b), v >= b
        lo, hi = row['value']
        ratio = v / b if b else float('inf')
        return '%.3f (%s / %s)' % (ratio, v, b), lo <= ratio <= hi
    if e == 'ge_measure':
        w = var.get(row['value']) if var else None
        if v is None or w is None:
            return None, None
        return '%.4g vs %.4g' % (v, w), v >= w and v > 0
    if v is None:
        return None, None
    x = row['value']
    if e == 'eq':
        return '%.4g' % v, v == x
    if e == 'le':
        return '%.4g' % v, v <= x
    if e == 'ge':
        return '%.4g' % v, v >= x
    if e == 'band':
        return '%.4g' % v, x[0] <= v <= x[1]
    raise ValueError(e)


FAULT = re.compile(r"^\s*'?([^=*+]+?)\s*(=|\+=|\*=)\s*([^\s']+)(?:\s+on\s+(\w+))?")


def apply_fault(row, base, var):
    mt = FAULT.match(row['fault'])
    if not mt:
        raise ValueError('fault %r' % row['fault'])
    key, op, val, on = mt.groups()
    key = key.strip()
    target = dict(var)
    tb = dict(base) if base is not None else None
    which = target
    if on and on == row['base'] and on != row['variant']:
        which = tb
    cur = which.get(key)
    if op == '=':
        try:
            which[key] = float(val)
        except ValueError:
            which[key] = val
    elif op == '+=':
        which[key] = (cur + 'x') if isinstance(cur, str) else (cur or 0) + float(val)
    else:
        which[key] = (cur or 0) * float(val)
    return tb, target


def main():
    cmd = sys.argv[1]
    if cmd == 'extract':
        d = extract(sys.argv[2])
        print(len(d), 'measures')
        return
    table = load_yaml(sys.argv[2])
    runs_dir = sys.argv[3]
    red = 0
    out = []
    for row in table['rows']:
        base = measures(runs_dir, row['base'])
        var = measures(runs_dir, row['variant'])
        got, ok = judge(row, base, var)
        if cmd == 'selftest':
            if got is None:
                print('SKIP', row['id'])
                continue
            fb, fv = apply_fault(row, base, var)
            _, fok = judge(row, fb, fv)
            status = 'red' if fok is False else 'STILL-GREEN'
            if fok is not False:
                red += 1
            print('%-26s fault -> %s' % (row['id'], status))
            continue
        verdict = 'missing' if got is None else ('pass' if ok else 'FAIL')
        out.append((row, got, verdict))
        print('%-26s %-9s %s' % (row['id'], verdict, got))
    if cmd == 'selftest':
        sys.exit(1 if red else 0)
    if '--fill' in sys.argv:
        text = open(sys.argv[2]).read()
        for row, got, verdict in out:
            if got is None or row.get('measured') != 'pending':
                continue
            pat = re.compile(r"(- id: %s\n(?:  .*\n)*?  measured: )pending(\n  verdict: )pending"
                             % re.escape(row['id']))
            text = pat.sub(lambda m: m.group(1) + "'%s'" % got + m.group(2)
                           + ('pass' if verdict == 'pass' else 'open'), text, count=1)
        open(sys.argv[2], 'w').write(text)


if __name__ == '__main__':
    main()
