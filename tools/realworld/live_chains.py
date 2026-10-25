#!/usr/bin/env python3
"""Live cross-domain causal chains: measure every link of a chain from the run's own files
(docs/validation/live-chains-2026-10-25.md).

    python -I tools/realworld/live_chains.py extract <run-dir>...   # -> <run-dir>/realism.json
    python -I tools/realworld/live_chains.py derive <runs-dir>      # -> the X-* interaction runs
    python -I tools/realworld/live_chains.py faults <table> <runs-dir>

`extract` is `matrix.py extract` (run report, metrics.json pooled as `m.*`, run cost) plus
the per-link measures of the chains, `lc.*`, from the run report and the resolved scenario:

- `lc.bsms` = frames - fragment frames + fragmented SDUs (one SDU per BSM);
- `lc.bsms_per_veh_s` = lc.bsms / (vehicles created x duration): the BSM rate J2945/1 sets;
- `lc.psdu_per_bsm`, `lc.airtime_us_per_bsm`: what one BSM costs the channel;
- `lc.frag_share` = fragmented SDUs / BSMs;
- `lc.attempts_per_frame` = reception attempts / frames: the receivers a frame reaches
  (falls when J2945/1 power control lowers the radiated power);
- `lc.verifies_per_node_s` = receiver-validation checks / (nodes x duration);
- `lc.events_per_attempt` = DES events / reception attempts (the run's cost per reception).

`derive` writes interaction runs `X-<name>` whose realism.json holds differences of
differences (a 2 x 2 or a chain that only shows as a contrast), from DERIVED below.

`faults` proves each row of the table can go red: a direction, ratio or delta row is
re-judged with the variant replaced by its base (the knob did nothing); a value or band
row with its value moved outside the expectation; an identical row with a different run's
digest. A row that still passes under its fault is printed as CANNOT-FAIL (exit 1).
It shares no code with the engine.
"""

import copy
import importlib.util
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def _mod(name):
    spec = importlib.util.spec_from_file_location(name, os.path.join(HERE, name + '.py'))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def _duration(run_dir):
    for line in open(os.path.join(run_dir, 'scenario.resolved.yaml')):
        s = line.strip()
        if s.startswith('duration_s:'):
            return float(s.split(':', 1)[1])
    return float('nan')


def extract(run_dir):
    mx = _mod('matrix')
    d = mx.extract(run_dir)
    rep = json.load(open(os.path.join(run_dir, 'run-report.json')))['report']
    dur = _duration(run_dir)
    frames = rep.get('frames_transmitted') or 0
    frag_frames = rep.get('fragments_sent') or 0
    frag_sdus = rep.get('sdus_fragmented') or 0
    bsms = frames - frag_frames + frag_sdus
    nodes = rep.get('nodes_created') or 0
    att = rep.get('reception_attempts') or 0
    air = rep.get('air_bytes') or {}
    rv = rep.get('receiver_validation') or {}
    d['lc.duration_s'] = dur
    d['lc.bsms'] = bsms
    d['lc.bsms_per_veh_s'] = bsms / (nodes * dur) if nodes and dur else 0.0
    d['lc.psdu_per_bsm'] = (air.get('psdu') or 0) / bsms if bsms else 0.0
    d['lc.airtime_us_per_bsm'] = (air.get('airtime_us') or 0) / bsms if bsms else 0.0
    d['lc.frag_share'] = frag_sdus / bsms if bsms else 0.0
    d['lc.attempts_per_frame'] = att / frames if frames else 0.0
    d['lc.verifies_per_node_s'] = (rv.get('checked') or 0) / (nodes * dur) if nodes and dur else 0.0
    d['lc.events_per_attempt'] = d.get('cost.events', 0) / att if att else 0.0
    with open(os.path.join(run_dir, 'realism.json'), 'w') as f:
        json.dump(d, f, indent=1, sort_keys=True, default=str)
    return d


# name -> (measure, [(sign, run), ...]): value = sum(sign * run[measure]).
DERIVED = {
    # PQ x density: PDR lost to Falcon at 100 vehicles minus that lost at 40 (> 0: amplifies)
    'X-pq-density-pdr': ('m.pdr', [(+1, 'r100'), (-1, 'r100-falcon'), (-1, 'r40'), (+1, 'r40-falcon')]),
    'X-pq-density-coll': ('m.collision_rate', [(+1, 'r100-falcon'), (-1, 'r100'), (-1, 'r40-falcon'), (+1, 'r40')]),
    # DCC x density: CBR growth 40 -> 160 with J2945/1 minus without (< 0: DCC flattens it)
    'X-dcc-density-cbr': ('m.channel_load.mean', [(+1, 'r160'), (-1, 'r40'), (-1, 'r160-nodcc'), (+1, 'r40-nodcc')]),
    # DCC x OBU: verify wait added by switching DCC off at 160 on the slow OBU minus on the fast
    'X-dcc-obu-wait': ('m.verify_wait.mean', [(+1, 'r160-c2sw-nodcc'), (-1, 'r160-c2sw'), (-1, 'r160-nodcc'), (+1, 'r160')]),
}


def derive(runs_dir):
    for name, (measure, terms) in DERIVED.items():
        tot, ok = 0.0, True
        for sign, run in terms:
            p = os.path.join(runs_dir, run, 'realism.json')
            if not os.path.exists(p):
                ok = False
                break
            v = json.load(open(p)).get(measure)
            if not isinstance(v, (int, float)):
                ok = False
                break
            tot += sign * v
        if not ok:
            print(f'{name}: missing input')
            continue
        os.makedirs(os.path.join(runs_dir, name), exist_ok=True)
        with open(os.path.join(runs_dir, name, 'realism.json'), 'w') as f:
            json.dump({'x.value': tot, 'x.measure': measure}, f)
        print(f'{name}: {measure} contrast = {tot:.6g}')


def faults(table, runs_dir):
    import yaml
    mx = _mod('matrix')
    rr = _mod('runtime_realism')
    doc = yaml.safe_load(open(table))
    cache = {}

    def load(run):
        if run not in cache:
            p = os.path.join(runs_dir, run, 'realism.json')
            cache[run] = rr.derive(json.load(open(p))) if os.path.exists(p) else None
        return cache[run]

    bad = 0
    red = 0
    for row in doc['rows']:
        def get(run, metric, row=row):
            d = load(run)
            return 'NO-RUN' if d is None else d.get(metric, 'ABSENT')

        ok, _ = mx.judge(row, get)
        if ok is None:
            print(f"SKIP    {row['id']} (no run)")
            continue
        op = row['expect']
        f = copy.deepcopy(row)
        if op in ('up', 'down', 'ratio', 'delta', 'changes'):
            f['variant'] = f['base']          # the knob did nothing
            fget = get
        elif op == 'same':
            # the knob moved the metric by 50 %
            def fget(run, metric, row=row):
                v = get(run, metric)
                return v * 1.5 + (1.0 if v == 0 else 0.0) if run == row['variant'] else v
        elif op == 'identical':
            other = next(r for r in doc['runs'] if r not in (row['base'], row['variant'])
                         and load(r) is not None)
            f['variant'] = other
            fget = get
        elif op in ('band', 'eq', 'le', 'ge'):
            lo, hi = (row['band'] if op == 'band' else (row['value'], row['value']))
            span = abs(hi - lo) or abs(hi) or 1.0
            moved = {'band': hi + span, 'eq': hi + span, 'le': hi + span, 'ge': lo - span}[op]

            def fget(run, metric, moved=moved, row=row):
                return moved if run == row['variant'] else get(run, metric)
        elif op in ('true', 'false'):
            def fget(run, metric, row=row):
                v = get(run, metric)
                return (not v) if run == row['variant'] else v
        else:
            print(f"NOFAULT {row['id']} ({op})")
            bad += 1
            continue
        fok, shown = mx.judge(f, fget)
        if fok:
            print(f"CANNOT-FAIL {row['id']} ({op}) {shown}")
            bad += 1
        else:
            red += 1
    print(f'faults: {red} rows go red under their fault, {bad} cannot fail')
    return 1 if bad else 0


if __name__ == '__main__':
    cmd = sys.argv[1]
    if cmd == 'extract':
        for r in sys.argv[2:]:
            d = extract(r)
            print(f'{r}: {len(d)} measurements')
    elif cmd == 'derive':
        derive(sys.argv[2])
    elif cmd == 'faults':
        sys.exit(faults(sys.argv[2], sys.argv[3]))
