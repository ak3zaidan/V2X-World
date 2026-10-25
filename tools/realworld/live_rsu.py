#!/usr/bin/env python3
"""Live RSU track (docs/validation/live-rsu-2026-10-25.md): an independent reader of a run's
raw records for the roadside-unit and application rows of
crates/v2xw-engine/tests/expectations/live-rsu.yaml.

    live_rsu.py measure <run-dir>              matrix.py extract + the g.*, s.* and c.* measures
                                               -> <run-dir>/realism.json
    live_rsu.py faults <runs-dir> <table.yaml> re-judges every row with its injected fault
                                               and prints whether it goes red

It reads `recording.mcap` (node.tx, phy.prr, app.advice, msg.latency, proto.revocation), `metrics.json`,
`run-report.json` and the run lock's cost line (`<run-dir>.cost.json`), and never imports
engine code.

Faults (a row's `fault`, default `knob-ignored`):
- knob-ignored: the variant is measured on the base's records, as if the setting had been
  read and dropped (the commonest live defect: a configuration that moves nothing);
- spat-half: every second SPaT frame of each unit is dropped from the records (a 5 Hz unit);
- outage-ignored: the outage unit's frames after the outage are kept (measured on the base);
- record-flip: one record of the variant changes (the digest must differ);
- delay-zero: every relayed-report and CRL hop of the variant is given zero backhaul delay.
"""

import collections
import copy
import importlib.util
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
NS = 1_000_000_000
PRR_BIN_M = 20.0          # phy.prr bins (engine records.rs: 20 m)
OUTAGE_NODE = 2           # glosa-out.yaml: the unit at site 10, junction 10
OUTAGE_JUNCTION = 10
OUTAGE_T_NS = 60 * NS
SPAT_FRESH_NS = 1 * NS    # a SPaT older than 1 s is not used (apps.rs signal_situation)
SCMS_OUTAGE_NODE = 5      # scms-out.yaml: unit 5 off from 30 s
SCMS_OUTAGE_T_NS = 30 * NS


def _mod(name):
    spec = importlib.util.spec_from_file_location(name, os.path.join(HERE, name + '.py'))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def records(path, topics):
    from mcap.reader import make_reader
    with open(path, 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            t = ch.topic.split('/', 1)[-1]
            if t in topics:
                yield t, json.loads(m.data)


def pct(xs, q):
    if not xs:
        return 0.0
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(math.ceil(q * len(xs))) - 1)]


def raw(run_dir):
    """The records the measures read, as plain lists (so a fault can mutate them)."""
    r = {'tx': [], 'prr': [], 'advice': [], 'flow': [], 'lat': [], 'rev': []}
    for t, d in records(os.path.join(run_dir, 'recording.mcap'),
                        {'node.tx', 'phy.prr', 'app.advice', 'msg.latency', 'proto.revocation'}):
        if t == 'node.tx':
            r['tx'].append({k: d.get(k) for k in ('t', 'node', 'msg', 'msg_type', 'bytes_on_wire',
                                                  't_generated', 'power_dbm')})
        elif t == 'phy.prr':
            if d.get('msg_type') in ('spat', 'map', 'crl'):
                r['prr'].append(d)
        elif t == 'app.advice':
            r['advice'].append(d)
        elif t == 'msg.latency':
            if d.get('flow') == 'mbr':
                r['lat'].append(d)
        else:
            r['rev'].append({k: d.get(k) for k in ('t', 'stage', 'node', 'id')})
    rep = json.load(open(os.path.join(run_dir, 'run-report.json')))['report']
    cost = {}
    cp = run_dir.rstrip('/') + '.cost.json'
    if os.path.exists(cp):
        cost = json.load(open(cp))
    r['report'] = rep
    r['cost'] = cost
    r['metrics'] = os.path.join(run_dir, 'metrics.json')
    return r


def measures(r):
    d = {}
    rep = r['report']
    # ---- cost
    d['c.wall_s'] = r['cost'].get('wall_s', 0)
    d['c.maxrss_mb'] = r['cost'].get('maxrss_mb', 0)
    d['c.events'] = sum((rep.get('events_by_class') or {}).values())
    d['c.frames_tx'] = len(r['tx'])
    # ---- what the units put on the air
    by = collections.defaultdict(list)
    for x in r['tx']:
        by[(x['node'], x['msg_type'])].append(x['t'])
    units = sorted({n for (n, k) in by if k in ('spat', 'map')})
    d['g.units_on_air'] = len(units)
    for k in ('spat', 'map', 'crl', 'bsm'):
        d[f'g.{k}_frames'] = sum(len(v) for (n, kk), v in by.items() if kk == k)
    # per-unit SPaT rate, Hz, over the unit's active span (first to last frame)
    rates = []
    for n in units:
        ts = sorted(by.get((n, 'spat'), []))
        if len(ts) > 10:
            rates.append((len(ts) - 1) / ((ts[-1] - ts[0]) / NS))
    d['g.spat_hz_min'] = min(rates) if rates else 0.0
    d['g.spat_hz_max'] = max(rates) if rates else 0.0
    mts = sorted(t for (n, k), v in by.items() if k == 'map' for t in v)
    d['g.map_hz_per_unit'] = (len(mts) / max(1, len(units))) / max(1e-9, (max(mts) - min(mts)) / NS) if len(mts) > 2 else 0.0
    d['g.outage_unit_spat_after'] = sum(1 for t in by.get((OUTAGE_NODE, 'spat'), []) if t > OUTAGE_T_NS)
    d['g.spat_tx_power_dbm'] = max((x['power_dbm'] or 0) for x in r['tx'] if x['msg_type'] == 'spat') if d['g.spat_frames'] else 0.0
    d['g.spat_bytes_mean'] = (sum(x['bytes_on_wire'] for x in r['tx'] if x['msg_type'] == 'spat') / d['g.spat_frames']) if d['g.spat_frames'] else 0.0
    # ---- SPaT coverage: the truth census of every receiver in each 20 m ring
    att = collections.Counter()
    ok = collections.Counter()
    for p in r['prr']:
        if p['msg_type'] != 'spat':
            continue
        for b, a, s in p['bins']:
            att[b] += a
            ok[b] += s
    def prr(lo, hi):
        bs = [b for b in att if lo <= b * PRR_BIN_M < hi]
        A = sum(att[b] for b in bs)
        return sum(ok[b] for b in bs) / A if A else 0.0
    for lo, hi in ((0, 100), (100, 200), (200, 300), (300, 400), (400, 600)):
        d[f'g.spat_prr_{lo}_{hi}'] = prr(lo, hi)
    def reach(th):
        # the outer edge of the last 20 m ring, walking out, before the PRR drops under th
        edge = 0.0
        for b in sorted(att):
            if att[b] < 20:
                continue
            if ok[b] / att[b] < th:
                break
            edge = (b + 1) * PRR_BIN_M
        return edge
    d['g.spat_range90_m'] = reach(0.9)
    d['g.spat_range50_m'] = reach(0.5)
    d['g.spat_heard'] = sum(ok.values())
    # ---- GLOSA's advice
    g = [a for a in r['advice'] if a.get('app') == 'glosa']
    d['g.glosa_advice'] = len(g)
    d['g.glosa_advised_vehicles'] = len({a['node'] for a in g})
    first = {}
    for a in sorted(g, key=lambda a: a['t']):
        first.setdefault((a['node'], a['intersection']), a['distance_m'])
    d['g.glosa_first_dist_p50'] = pct(list(first.values()), 0.5)
    d['g.glosa_first_dist_p90'] = pct(list(first.values()), 0.9)
    d['g.glosa_junctions'] = len({a['intersection'] for a in g})
    d['g.advice_outage_junction_after'] = sum(
        1 for a in g if a['intersection'] == OUTAGE_JUNCTION and a['t'] > OUTAGE_T_NS + SPAT_FRESH_NS)
    d['g.advice_with_target'] = sum(1 for a in g if a.get('advised_mps') is not None and a.get('state') != 'stop-and-remain')
    # ---- traffic and energy, from the engine's own per-window samples (passenger class)
    acc = collections.defaultdict(lambda: [0.0, 0.0, 0])
    for s in json.load(open(r['metrics']))['samples'] if os.path.exists(r['metrics']) else []:
        if s['dims'].get('class') != 'passenger':
            continue
        v = s['value']
        if v.get('kind') == 'ratio' and s['metric'] in ('stops_per_km', 'fuel_per_100km', 'co2_per_km', 'nox_per_km'):
            a = acc[s['metric']]
            a[0] += v.get('numerator') or 0.0
            a[1] += v.get('denominator') or 0.0
            a[2] += v.get('n') or 0
    for k, (num, den, n) in acc.items():
        d['g.' + k] = num / den if den else 0.0
    if acc.get('stops_per_km') and acc['stops_per_km'][2]:
        # vehicle-steps of 100 ms in the energy windows -> the mean speed of the class
        num, den, n = acc['stops_per_km']
        d['g.mean_speed_mps'] = den * 1000.0 / (n * 0.1)
    # ---- the SCMS path through the units (scms-* runs)
    p2 = rep.get('phase2') or {}
    acc2 = p2.get('access') or {}
    car = acc2.get('carrier') or {}
    for k in ('backhaul_messages', 'backhaul_line_bytes', 'backhaul_retx', 'fallbacks'):
        d['s.' + k] = car.get(k, 0)
    for k in ('reports_uploaded_relay', 'rsu_crls_installed', 'crl_broadcasts', 'rsus',
              'crl_versions_published', 'reports_from_rsus', 'relay_delivered'):
        d['s.' + k] = p2.get(k, 0)
    d['s.relay_only_vehicles'] = acc2.get('relay_only_vehicles', 0)
    d['s.retx_share'] = d['s.backhaul_retx'] / d['s.backhaul_messages'] if d['s.backhaul_messages'] else 0.0
    d['s.backhaul_bytes'] = acc2.get('backhaul_bytes', 0)
    flows = scms_flows(r)
    d.update(flows)
    return d


def scms_flows(r):
    """Reports relayed through the units (msg.latency spans) and the CRL path through them
    (proto.revocation `published` and `downloaded`, the units' own CRL frames in node.tx)."""
    d = {}
    zero = r.get('_zero_delay')
    bh = []
    e2e = []
    for x in r['lat']:
        if x.get('flow') != 'mbr':
            continue
        spans = x.get('spans') or []
        b = [s for s in spans if s['stage'] == 'backhaul']
        if not b:
            continue
        dt = sum(s['end'] - s['start'] for s in b)
        if zero:
            dt = 0
        bh.append(dt / 1e6)
        e2e.append((max(s['end'] for s in spans) - x['t_origin']) / 1e6 - (0 if not zero else sum(
            s['end'] - s['start'] for s in b) / 1e6))
    d['s.relayed_reports'] = len(bh)
    d['s.backhaul_ms_mean'] = sum(bh) / len(bh) if bh else 0.0
    d['s.backhaul_ms_p50'] = pct(bh, 0.5)
    d['s.backhaul_ms_max'] = max(bh) if bh else 0.0
    d['s.report_e2e_ms_mean'] = sum(e2e) / len(e2e) if e2e else 0.0
    pub = sorted(x['t'] for x in r['rev'] if x['stage'] == 'published')
    d['s.crl_published'] = len(pub)
    crl_tx = sorted((x['t'], x['node']) for x in r['tx'] if x['msg_type'] == 'crl')
    d['s.crl_frames'] = len(crl_tx)
    d['s.crl_units'] = len({n for _, n in crl_tx})
    d['s.crl_frames_from_outage_unit'] = sum(1 for t, n in crl_tx if n == SCMS_OUTAGE_NODE and t > SCMS_OUTAGE_T_NS)
    # each publication's first unit frame after it, and every unit's first frame after it
    lags, unit_lags = [], []
    for i, p in enumerate(pub):
        nxt = pub[i + 1] if i + 1 < len(pub) else math.inf
        after = [(t, n) for t, n in crl_tx if p <= t < nxt]
        if not after:
            continue
        lags.append((after[0][0] - p) / NS)
        first = {}
        for t, n in after:
            first.setdefault(n, t)
        unit_lags += [(t - p) / NS for t in first.values()]
    if zero:
        lags = [0.0 for _ in lags]
        unit_lags = [0.0 for _ in unit_lags]
    d['s.crl_first_frame_lag_s_min'] = min(lags) if lags else -1.0
    d['s.crl_first_frame_lag_s_max'] = max(lags) if lags else -1.0
    d['s.crl_first_frame_lag_s_mean'] = sum(lags) / len(lags) if lags else -1.0
    d['s.crl_unit_lag_s_min'] = min(unit_lags) if unit_lags else -1.0
    d['s.crl_unit_lag_s_mean'] = sum(unit_lags) / len(unit_lags) if unit_lags else -1.0
    # vehicles: a relay-only vehicle's `downloaded` after the latest publication
    vl = []
    for x in r['rev']:
        if x['stage'] != 'downloaded' or x.get('node') is None:
            continue
        before = [p for p in pub if p <= x['t']]
        if before:
            vl.append((x['t'] - before[-1]) / NS)
    d['s.veh_crl_downloads'] = len(vl)
    d['s.veh_crl_lag_s_p50'] = pct(vl, 0.5)
    d['s.veh_crl_lag_s_mean'] = sum(vl) / len(vl) if vl else -1.0
    return d


def measure(run_dir, fault=None, base_dir=None):
    if fault and fault.startswith('use:'):
        # the variant measured on another run's records (e.g. the drivers had followed)
        return measures(raw(os.path.join(os.path.dirname(run_dir.rstrip('/')), fault[4:])))
    r = raw(run_dir)
    if fault == 'knob-ignored' or fault == 'outage-ignored':
        r = raw(base_dir)
    elif fault and fault.startswith('half:'):
        kind = fault[5:]
        seen = collections.Counter()
        keep = []
        for x in r['tx']:
            if x['msg_type'] == kind:
                seen[x['node']] += 1
                if seen[x['node']] % 2 == 0:
                    continue
            keep.append(x)
        r['tx'] = keep
    elif fault == 'advice-far':
        for a in r['advice']:
            a['distance_m'] = a['distance_m'] + 50.0
    elif fault == 'delay-zero':
        r['_zero_delay'] = True
    d = measures(r)
    return d


def write(run_dir):
    mx = _mod('matrix')
    d = mx.extract(run_dir)
    d.update(measure(run_dir))
    with open(os.path.join(run_dir, 'realism.json'), 'w') as f:
        json.dump(d, f, indent=1, sort_keys=True, default=str)
    print(f'{run_dir}: {len(d)} measurements')


def faults(runs_dir, table):
    """Each row re-judged with its fault injected into the variant: it must go red."""
    import yaml
    mx = _mod('matrix')
    doc = yaml.safe_load(open(table))
    cache = {}

    def real(run):
        if run not in cache:
            p = os.path.join(runs_dir, run, 'realism.json')
            cache[run] = json.load(open(p)) if os.path.exists(p) else None
        return cache[run]

    red = green = skip = 0
    for row in doc['rows']:
        f = row.get('fault', 'knob-ignored')
        b, v = row['base'], row['variant']
        if real(v) is None or real(b) is None:
            skip += 1
            print(f"SKIP    {row['id']:34} (no run)")
            continue
        if f in ('knob-ignored', 'outage-ignored'):
            if b == v:
                skip += 1
                print(f"SKIP    {row['id']:34} knob-ignored on a base-only row: name a fault")
                continue
            fv = copy.deepcopy(real(b))
        elif f == 'record-flip':
            fv = copy.deepcopy(real(v))
            fv['digest.record'] = (fv.get('digest.record') or '') + 'x'
        else:
            fv = copy.deepcopy(real(v))
            fv.update(measure(os.path.join(runs_dir, v), f, os.path.join(runs_dir, b)))
        def get(run, metric, fv=fv, v=v):
            src = fv if run == v else real(run)
            return src.get(metric, 'ABSENT')
        ok, shown = mx.judge(row, get)
        if ok is False:
            red += 1
            st = 'RED'
        else:
            green += 1
            st = 'GREEN!'
        print(f"{st:7} {row['id']:34} [{f}] {row['measure']} = {shown}"[:200])
    print(f'faults: {red} red, {green} stayed green, {skip} skipped')
    return 1 if green else 0


if __name__ == '__main__':
    if sys.argv[1] == 'measure':
        write(sys.argv[2])
    elif sys.argv[1] == 'faults':
        sys.exit(faults(sys.argv[2], sys.argv[3]))
