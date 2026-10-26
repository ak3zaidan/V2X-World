#!/usr/bin/env python3
"""Live SCMS knob tests: the extra measures of crates/v2xw-engine/tests/expectations/live-scms.yaml.

    python3 -I tools/realworld/live_scms.py extend <run-dir>   # adds x.* to <run-dir>/realism.json
    python3 -I tools/realworld/live_scms.py faults <table> <runs-dir>

`extend` reads only the run's own files (run-report.json, realism.json from matrix.py
extract) and adds scale-free forms the rows need:
- x.share_b, x.verified_share_b, x.revocations_b: provider-b's share of the devices, of
  the verified receptions (by signer provider) and its revocations;
- x.revocation_latency_s: the run report's mean revocation latency in seconds;
- x.airtime_per_frame_us: airtime over frames on the air;
- x.crl_polls: CRL downloads plus not-modified polls (every poll a vehicle made);
- xm.<from>-><to>, xb.<from>-><to>: backend link messages (pki.flow `msg`) and their bytes
  per entity pair, each entity with its provider (`provider-a/ra`, `ee`);
- x.unknown_signer_loss: m.loss_rate.unknown-signer, 0 when no frame was lost to it.

`faults` proves every row can go red: for each row it injects the fault the row exists
to catch into the variant's measures, in memory, and re-judges with matrix.py's judge:
- "knob ignored": the variant's measures replaced by the base's (rows on two runs whose
  expectation needs a change);
- "value off": the row's measure moved past what the row allows (every other row).
It prints one line per row that stays green under its fault and exits 1 if any does.
"""
import copy
import importlib.util
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def _matrix():
    spec = importlib.util.spec_from_file_location('matrix', os.path.join(HERE, 'matrix.py'))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def extend(run_dir, report_dir=None):
    rp = os.path.join(report_dir or run_dir, 'run-report.json')
    r = json.load(open(rp))['report']
    p = r.get('phase2') or {}
    path = os.path.join(run_dir, 'realism.json')
    d = json.load(open(path))
    dev = p.get('devices_by_provider') or {}
    n = sum(dev.values())
    d['x.share_b'] = dev.get('provider-b', 0) / n if n else 0.0
    vbp = p.get('verified_by_provider') or {}
    a = sum(v for k, v in vbp.items() if k.endswith('<-provider-a'))
    b = sum(v for k, v in vbp.items() if k.endswith('<-provider-b'))
    d['x.verified_share_b'] = b / (a + b) if a + b else 0.0
    d['x.revocations_b'] = (p.get('revocations_by_provider') or {}).get('provider-b', 0)
    d['x.revocation_latency_s'] = (p.get('revocation_latency_ns') or 0) / 1e9
    air = r.get('air_bytes') or {}
    fr = air.get('frames') or 0
    d['x.airtime_per_frame_us'] = air.get('airtime_us', 0) / fr if fr else 0.0
    d['x.crl_polls'] = (p.get('crl_downloads') or 0) + (p.get('crl_polls_not_modified') or 0)
    # The loss ratio by cause is only sampled for a cause that occurred: absent is none lost.
    d['x.unknown_signer_loss'] = d.get('m.loss_rate.unknown-signer', 0.0)
    # Backend load per entity: link messages (pki.flow kind `msg`, not the SPDUs inside
    # them) and their bytes, per sending and receiving entity with its provider.
    rec = os.path.join(run_dir, 'recording.mcap')
    if os.path.exists(rec):
        from mcap.reader import make_reader
        ent = _matrix()._realism().ent
        n_msg, n_bytes = {}, {}
        with open(rec, 'rb') as f:
            for _, _ch, m in make_reader(f).iter_messages(topics=['record/pki.flow']):
                q = json.loads(m.data)
                if q.get('kind') == 'msg' and q.get('to'):
                    k = f"{ent(q.get('from'))}->{ent(q['to'])}"
                    n_msg[k] = n_msg.get(k, 0) + 1
                    n_bytes[k] = n_bytes.get(k, 0) + (q.get('bytes') or 0)
        for k, v in n_msg.items():
            d['xm.' + k] = v
            d['xb.' + k] = n_bytes[k]
    with open(path, 'w') as f:
        json.dump(d, f, indent=1, sort_keys=True, default=str)


def _off(row, v, b):
    """A value the row refuses."""
    op = row['expect']
    if op in ('eq',):
        return (v + 1) if isinstance(v, (int, float)) else str(v) + 'x'
    if op == 'le':
        return row['value'] + abs(row['value']) * 0.5 + 1
    if op == 'ge':
        return row['value'] - abs(row['value']) * 0.5 - 1
    if op == 'band':
        lo, hi = row['band']
        return hi + (hi - lo) + 1
    if op == 'ratio':
        lo, hi = row['band']
        return (b or 1) * (hi * 2 + 1) if math.isfinite(hi) else (b or 1) * lo * 0.5 - 1
    if op == 'delta':
        lo, hi = row['band']
        return b + hi + (hi - lo) + 1
    if op == 'same':
        return (b or 1) * (1 + row.get('tol', 0.0) * 2 + 0.1)
    if op == 'identical':
        return str(b) + 'x'
    if op == 'up':
        return b
    if op == 'down':
        return b
    return None


def faults(table, runs_dir):
    import yaml
    m = _matrix()
    doc = yaml.safe_load(open(table))
    data = {}
    for run in doc['runs']:
        p = os.path.join(runs_dir, run, 'realism.json')
        data[run] = m._realism().derive(json.load(open(p))) if os.path.exists(p) else None
    green = red = missing = no_run = 0
    for row in doc['rows']:
        base, var = row['base'], row['variant']
        if data.get(var) is None or data.get(base) is None:
            no_run += 1
            continue
        d = copy.deepcopy(data)
        two = base != var and row['expect'] not in ('same', 'identical')
        if two and row['expect'] not in ('eq', 'le', 'ge', 'band'):
            d[var] = copy.deepcopy(d[base])
            how = 'knob ignored'
        else:
            v = d[var].get(row['measure'])
            b = d[base].get(row['measure'])
            if v is None:
                print(f"ABSENT  {row['id']}")
                continue
            d[var] = dict(d[var])
            d[var][row['measure']] = _off(row, v, b)
            how = 'value off'

        def get(run, metric):
            x = d.get(run)
            return 'NO-RUN' if x is None else x.get(metric, 'ABSENT')
        ok, shown = m.judge(row, get)
        if ok:
            # knob ignored left it green (the variant equals the base by design): move the value
            d2 = copy.deepcopy(data)
            d2[var] = dict(d2[var])
            d2[var][row['measure']] = _off(row, d2[var].get(row['measure']), d2[base].get(row['measure']))

            def get2(run, metric):
                return d2[run].get(metric, 'ABSENT')
            ok, shown = m.judge(row, get2)
            how += ' + value off'
        if ok:
            green += 1
            print(f"GREEN   {row['id']:30} under {how}: {shown}")
        elif ok is None:
            missing += 1
        else:
            red += 1
    print(f'faults: {len(doc["rows"])} rows: {red} red under their fault, {green} stay green, '
          f'{missing} not judged (a measure absent), {no_run} without their runs')
    return 1 if green else 0


if __name__ == '__main__':
    if sys.argv[1] == 'extend':
        extend(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)
    elif sys.argv[1] == 'faults':
        sys.exit(faults(sys.argv[2], sys.argv[3]))
