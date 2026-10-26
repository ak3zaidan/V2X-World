#!/usr/bin/env python3
"""Judge only the still-pending rows of a live expectations table on a new runs folder.

    python3 -I tools/realworld/live_scms_fill.py <table> <runs-dir> <stamp> [--write]

`matrix.py check --fill` re-judges every row on one runs folder. A table measured over
several sessions and binaries keeps its judged rows on the runs that judged them, so this
fills a row only when its `verdict` is `pending` and both its runs are in `<runs-dir>`. It
uses matrix.py's own `judge` and derived measures, writes `measured`, `verdict` (`pass`,
or `FAIL` for a person to settle) and `measured_on: <stamp>`, and prints one line per row
it judged. Judged rows, and rows whose runs are not in the folder, are left as they are.
"""
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def _matrix():
    spec = importlib.util.spec_from_file_location('matrix', os.path.join(HERE, 'matrix.py'))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def main(table, runs_dir, stamp, write):
    import yaml
    m = _matrix()
    doc = yaml.safe_load(open(table))
    cache = {}

    def get(run, metric):
        if run not in cache:
            p = os.path.join(runs_dir, run, 'realism.json')
            cache[run] = m._realism().derive(json.load(open(p))) if os.path.exists(p) else None
        d = cache[run]
        if d is None:
            return 'NO-RUN'
        return d.get(metric, 0 if metric.startswith('edge.') else 'ABSENT')

    n = {'pass': 0, 'FAIL': 0, 'missing': 0}
    for row in doc['rows']:
        if row.get('verdict') != 'pending':
            continue
        if get(row['base'], row['measure']) == 'NO-RUN' or get(row['variant'], row['measure']) == 'NO-RUN':
            continue
        ok, shown = m.judge(row, get)
        if ok is None:
            n['missing'] += 1
            print(f"MISSING {row['id']:34} {row['measure']} = {shown}"[:220])
            continue
        v = 'pass' if ok else 'FAIL'
        n[v] += 1
        print(f"{v:7} {row['id']:34} {row['measure']} = {shown}"[:220])
        row['measured'] = shown
        row['verdict'] = v
        row['measured_on'] = stamp
    print('fill: ' + ', '.join(f'{k} {v}' for k, v in n.items()))
    if write:
        head = []
        for line in open(table):
            if line.startswith('#'):
                head.append(line)
            else:
                break
        with open(table, 'w') as f:
            f.writelines(head)
            yaml.safe_dump(doc, f, sort_keys=False, width=100, allow_unicode=True)


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2], sys.argv[3], '--write' in sys.argv)
