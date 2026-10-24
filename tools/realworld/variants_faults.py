#!/usr/bin/env python3
"""Fault proof for an SCMS run-expectation table (rerun-2026-10-24-b.md §1).

    python3 tools/realworld/variants_faults.py <table.yaml> <runs-dir>

For every row that `matrix.py check` judges PASS on the real runs, inject one fault into a
copy of the variant's measurement (in memory; nothing is written) and judge the row again:
- band, ratio, delta: the value is moved just outside the band (above it, or below it if
  the band's top is unbounded);
- eq, le, ge: the value is moved one unit past the bound (or 10 % for a float);
- up, down: the variant takes the base's value (no change);
- same, identical: the variant's value is changed (x1.5, or a different digest);
- empty, true, false: a non-empty list, or the opposite truth value.
A row that still passes under its fault cannot fail and is printed SILENT. Exit 1 on any.
"""
import copy
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('matrix', os.path.join(HERE, 'matrix.py'))
M = importlib.util.module_from_spec(spec)
spec.loader.exec_module(M)


def fault(row, v, b):
    op = row['expect']
    if op == 'band':
        lo, hi = row['band']
        return hi * 1.1 + 1
    if op in ('ratio', 'delta'):
        lo, hi = row['band']
        x = hi * 1.1 + (0.1 if op == 'ratio' else 1)
        return b * x if op == 'ratio' else b + x
    if op == 'eq':
        return row['value'] + 1
    if op == 'le':
        return row['value'] * 1.1 + 1
    if op == 'ge':
        return row['value'] - 1
    if op in ('up', 'down'):
        return b
    if op == 'same':
        return v * 1.5 + 1
    if op == 'identical':
        return str(v) + '-fault'
    if op == 'empty':
        return ['fault']
    if op in ('true', 'false'):
        return op == 'false'
    raise SystemExit(f"row {row['id']}: no fault for {op}")


def main(table, runs):
    import yaml
    doc = yaml.safe_load(open(table))
    cache = {}

    def load(run):
        if run not in cache:
            p = os.path.join(runs, run, 'realism.json')
            cache[run] = json.load(open(p)) if os.path.exists(p) else None
        return cache[run]

    def getter(over):
        def get(run, metric):
            if (run, metric) in over:
                return over[(run, metric)]
            d = load(run)
            if d is None:
                return 'NO-RUN'
            return d.get(metric, 0 if metric.startswith('edge.') else 'ABSENT')
        return get

    red = silent = skipped = 0
    for row in doc['rows']:
        ok, _ = M.judge(row, getter({}))
        if not ok:
            skipped += 1
            continue
        g = getter({})
        v = g(row['variant'], row['measure'])
        b = g(row['base'], row['measure'])
        over = {(row['variant'], row['measure']): fault(row, v, b)}
        if row['base'] == row['variant'] and row['expect'] in ('up', 'down', 'same', 'identical', 'ratio', 'delta'):
            raise SystemExit(f"row {row['id']}: a relative row needs two runs")
        ok2, shown = M.judge(row, getter(over))
        if ok2:
            silent += 1
            print(f"SILENT {row['id']}: {shown}")
        else:
            red += 1
            print(f"red    {row['id']:30} {shown}"[:160])
    print(f'faults: {red} red, {silent} silent, {skipped} rows not passing (not injected)')
    return 1 if silent else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1], sys.argv[2]))
