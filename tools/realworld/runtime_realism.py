#!/usr/bin/env python3
"""SCMS runtime realism: what an operator would see, measured from a run's own records.

    python -I tools/realworld/runtime_realism.py extract <run-dir> [--json OUT]
    python -I tools/realworld/runtime_realism.py check <expectations.yaml> <runs-dir>

`extract` reads `<run-dir>/run-report.json` and the PKI view records of
`<run-dir>/recording.mcap` (`pki.flow`, `pki.knowledge`, `pki.case`, `pki.state`; the run
needs `security.scms.pki_view` on and `--record-level events` or more) and prints one flat
JSON object of named measurements: message counts per entity pair, PDU sizes, per-device
request counts, flow latencies, case stage times, the separation-of-duties and
sealed-recipient checks, the recorded component list, run cost.

`check` evaluates every row of an expectation table (crates/v2xw-engine/tests/expectations/
scms-realism.yaml's companion, tools/realworld/runtime_expectations.yaml) against the
extracted runs in `<runs-dir>/<run>/realism.json` and prints one line per row and a verdict
line; exit 1 when a row FAILs. A row is

    {id, run, metric, expect: <op>, value | band | vs, source}

with `op` one of `eq`, `le`, `ge`, `band` (value in [lo, hi]), `ratio` (run/vs in band),
`delta` (run - vs in band), `true`, `false`, `empty`.

Reads with the `mcap` and `zstandard` packages (the project venv). Written by the realism
agent; it shares no code with the engine.
"""

import collections
import json
import os
import statistics
import sys

NS = 1e9


def ent(x):
    """`ee/123` -> `ee`; `provider-a/ra/0` -> `provider-a/ra`."""
    if not x:
        return None
    if x.startswith('ee/'):
        return 'ee'
    return x.rsplit('/', 1)[0]


def role(x):
    e = ent(x)
    if e is None or e == 'ee':
        return e
    return e.split('/', 1)[1] if '/' in e else e


def keyset(v, pre=''):
    out = set()
    if isinstance(v, dict):
        for k, x in v.items():
            out.add(k)
            out |= keyset(x)
    elif isinstance(v, list):
        for x in v:
            out |= keyset(x)
    return out


# What each role's own records may hold (01 §2.2, 02 §9.5; BRECHT §X): a device identity
# (an enrolment certificate, a canonical id, the ground-truth device) and a linkage
# identifier (a linkage value, a pre-linkage value, a linkage-chain id) never together,
# and the ground-truth `device` never at all.
IDENTITY = {'enrolment', 'canonical_id', 'device', 'reporter'}
LINKAGE = {'lv', 'subject_lv', 'lci', 'plv', 'plv1', 'plv2', 'seed'}


def extract(run_dir):
    from mcap.reader import make_reader
    rep = json.load(open(os.path.join(run_dir, 'run-report.json')))
    r = rep['report']
    p2 = r.get('phase2', {})
    out = {}
    # ---- run cost (the time(1) lines of <run-dir>.log when present)
    log = run_dir.rstrip('/') + '.log'
    if os.path.exists(log):
        for line in open(log):
            w = line.split()
            if len(w) >= 6 and w[1] == 'real':
                out['cost.wall_s'] = float(w[0])
                out['cost.cpu_s'] = float(w[2]) + float(w[4])
            if 'peak memory footprint' in line:
                out['cost.peak_mb'] = int(w[0]) / 1e6
    out['cost.events'] = sum(r.get('events_by_class', {}).values())
    out['cost.backend_ticks'] = r.get('fast_forward', {}).get('backend_ticks', 0)
    out['cost.skipped_s'] = r.get('fast_forward', {}).get('skipped_ns', 0) / NS
    for k, v in p2.items():
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            out['p2.' + k] = v
    for k, v in (p2.get('access') or {}).items():
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            out['access.' + k] = v
    for name, t in p2.get('revocation_stages') or []:
        out['stage.' + name + '_s'] = t / NS
    for k, v in (r.get('receiver_validation') or {}).items():
        if isinstance(v, (int, float)):
            out['rv.' + k] = v
    out['rv.refused_total'] = sum((r.get('receiver_validation') or {}).get('refused', {}).values())
    air = r.get('air_bytes') or {}
    total_air = air.get('airtime_us', 0)
    out['air.airtime_us'] = total_air
    for t, a in (r.get('air_by_type') or {}).items():
        out[f'air.{t}.frames'] = a['frames']
        out[f'air.{t}.psdu_mean'] = a['psdu_mean']
        out[f'air.{t}.share'] = a['airtime_us'] / total_air if total_air else 0.0
    vbp = p2.get('verified_by_provider') or {}
    out['verified.from_b'] = sum(v for k, v in vbp.items() if k.endswith('<-provider-b'))
    out['verified.from_a'] = sum(v for k, v in vbp.items() if k.endswith('<-provider-a'))
    out['p2.revocations'] = sum((p2.get('revocations_by_provider') or {}).values())
    vs = p2.get('verification_states') or {}
    out['p2.unverified_states'] = sum(v for k, v in vs.items() if k != 'Verified')

    # ---- the PKI view records
    edges = collections.Counter()
    edge_bytes = collections.Counter()
    pdu_len = collections.defaultdict(list)
    per_dev = collections.defaultdict(collections.Counter)
    inst_t = collections.defaultdict(list)
    topup = collections.defaultdict(dict)
    sealed_bad = 0
    sealed_n = 0
    components = set()
    case_t = collections.defaultdict(dict)
    sep = collections.defaultdict(set)
    holds_device = set()
    shared_from = collections.Counter()
    # What each node's receiver holds (node.security `crls`, OF-3): its last row, and every
    # (craca, issue date) any node held, with the first time it was seen held.
    held_last = {}
    first_held = {}
    next_minus_issue = set()
    preload_crl = collections.Counter()
    path = os.path.join(run_dir, 'recording.mcap')
    first_state = True
    with open(path, 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            t = ch.topic
            if t == 'record/pki.flow':
                q = json.loads(m.data)
                fr, to = q.get('from'), q.get('to')
                if q['kind'] in ('msg', 'pdu') and to:
                    edges[(ent(fr), ent(to))] += 1
                    edge_bytes[(ent(fr), ent(to))] += q.get('bytes') or 0
                if q['kind'] == 'msg' and q['name'] == 'report-shared':
                    shared_from[role(fr)] += 1
                for d in q.get('pdus') or []:
                    if d['from'] == q.get('from') and d['to'] == q.get('to'):
                        pdu_len[d['type']].append(d['len'])
                        if d['from'].startswith('ee/'):
                            per_dev[d['type']][d['from']] += 1
                    st = d.get('sealed_to')
                    op = (d.get('opened') or {}).get('by')
                    if st and op:
                        sealed_n += 1
                        if st != op and not (st == 'ee' and op.startswith('ee/')):
                            sealed_bad += 1
                if q['flow'] == 'preload' and q.get('name') == 'SecuredCrl' and (to or '').startswith('ee/'):
                    preload_crl[to] += 1
                inst_t[(q['flow'], q['instance'])].append(q['t'])
                if q['flow'] == 'topup' and q['kind'] == 'stage':
                    topup[q['instance']].setdefault(q['name'], q['t'])
            elif t == 'record/node.security':
                q = json.loads(m.data)
                crls = q.get('crls') or []
                held_last[q['node']] = (q['t'], {c['craca']: c['issue_date'] for c in crls})
                for c in crls:
                    first_held.setdefault((c['craca'], c['issue_date']), q['t'])
                    next_minus_issue.add(c['next_crl'] - c['issue_date'])
            elif t == 'record/pki.case':
                q = json.loads(m.data)
                case_t[q['case']].setdefault(q['stage'], q['t'])
            elif t == 'record/pki.knowledge':
                q = json.loads(m.data)
                ks = keyset(q.get('holds'))
                sep[q['role']] |= ks
                if 'device' in ks:
                    holds_device.add(q['entity'])
            elif t == 'record/pki.state' and first_state:
                first_state = False
                q = json.loads(m.data)
                for pr in q.get('providers', []):
                    for c in pr.get('components', []):
                        components.add(c.get('role'))
    # Edges by role pair (providers merged), and the ones the US design never has.
    roles = collections.Counter()
    for (a, b), n in edges.items():
        ra = a if a in (None, 'ee') else a.split('/', 1)[1]
        rb = b if b in (None, 'ee') else b.split('/', 1)[1]
        cross = a not in (None, 'ee') and b not in (None, 'ee') and a.split('/')[0] != b.split('/')[0]
        roles[(ra, rb, cross)] += n
    for (ra, rb, cross), n in roles.items():
        out[f'edge.{ra}->{rb}{".x" if cross else ""}'] = n
    out['topology.components'] = sorted(c for c in components if c)
    out['topology.ee_peers'] = sorted({rb for (ra, rb, _), n in roles.items() if ra == 'ee'})
    out['topology.ee_senders'] = sorted({ra for (ra, rb, _), n in roles.items() if rb == 'ee'})
    out['topology.shared_report_senders'] = sorted(shared_from)
    for k, v in pdu_len.items():
        out[f'pdu.{k}.n'] = len(v)
        out[f'pdu.{k}.min'] = min(v)
        out[f'pdu.{k}.max'] = max(v)
    for k, c in per_dev.items():
        vals = list(c.values())
        out[f'perdev.{k}.devices'] = len(vals)
        out[f'perdev.{k}.max'] = max(vals)
        out[f'perdev.{k}.median'] = statistics.median(vals)
    # Flow durations (first to last record of an instance), median per flow, seconds.
    durs = collections.defaultdict(list)
    for (fl, _), ts in inst_t.items():
        durs[fl].append((max(ts) - min(ts)) / NS)
    for fl, v in durs.items():
        out[f'flow.{fl}.n'] = len(v)
        out[f'flow.{fl}.median_s'] = statistics.median(v)
        out[f'flow.{fl}.max_s'] = max(v)
    tl = [(s['installed'] - s['requested']) / NS for s in topup.values() if 'installed' in s and 'requested' in s]
    if tl:
        out['topup.latency_median_s'] = statistics.median(tl)
        out['topup.latency_max_s'] = max(tl)
        out['topup.requested_min_s'] = min(s['requested'] for s in topup.values() if 'requested' in s) / NS
    # Case stages: medians of each stage's time after reports-in.
    stg = collections.defaultdict(list)
    for c, s in case_t.items():
        base = s.get('reports-in')
        for k, v in s.items():
            if base is not None:
                stg[k].append((v - base) / NS)
        for k, v in s.items():
            stg['abs.' + k].append(v / NS)
    # The operator's review: corroboration to decision, per case (R-CORR).
    for s in case_t.values():
        if 'corroborated' in s and 'decision' in s:
            stg['review'].append((s['decision'] - s['corroborated']) / NS)
    # How long a case waited from its reports to its opening (R-SERIAL: the engine opens
    # one case per provider at a time, so under the 8 h review a case queues 8 h per case
    # ahead of it).
    waits = [(s['corroborated'] - s['reports-in']) / NS for s in case_t.values()
             if 'corroborated' in s and 'reports-in' in s]
    if waits:
        out['case.open_wait_max_s'] = max(waits)
    for k, v in stg.items():
        out[f'case.{k}_s'] = statistics.median(v)
    out['case.n'] = len(case_t)
    # Separation of duties from the records.
    bad = []
    allow = {'ra': IDENTITY - {'device'}, 'eca': IDENTITY - {'device'}, 'aca': LINKAGE, 'la1': LINKAGE - {'lv', 'subject_lv'},
             'la2': LINKAGE - {'lv', 'subject_lv'}, 'ma': {'subject_lv'}}
    for rl, ks in sep.items():
        has_id = ks & IDENTITY
        has_ln = ks & LINKAGE
        if has_id and has_ln:
            bad.append(f'{rl}: identity {sorted(has_id)} with linkage {sorted(has_ln)}')
        extra = (has_id | has_ln) - allow.get(rl, set())
        if extra:
            bad.append(f'{rl}: holds {sorted(extra)}')
    out['separation.violations'] = bad
    out['separation.roles'] = sorted(sep)
    out['separation.holds_device'] = sorted(holds_device)
    out['sealed.checked'] = sealed_n
    out['sealed.mismatch'] = sealed_bad
    # CRL holdings (06 §3 item 8; OF-3). Every CRACA's list, each node's last row; a node is
    # stale when another node already held a newer list of that CRACA more than an hour (the
    # conditional poll period) before the stale node's last row.
    cracas = {k[0] for k in first_held}
    out['crl.cracas'] = len(cracas)
    out['crl.held_cracas_min'] = min((len(h) for _, h in held_last.values()), default=0)
    stale = 0
    for tn, h in held_last.values():
        for ca, iss in h.items():
            if any(c == ca and i > iss and t0 + 3600 * NS < tn for (c, i), t0 in first_held.items()):
                stale += 1
                break
    out['crl.nodes'] = len(held_last)
    out['crl.nodes_stale'] = stale
    out['crl.next_minus_issue_s'] = sorted(next_minus_issue)
    out['crl.preload_devices'] = len(preload_crl)
    out['crl.preload_per_device_min'] = min(preload_crl.values(), default=0)
    out['crl.preload_per_device_max'] = max(preload_crl.values(), default=0)
    out['crl.holders_minus_preload'] = len(held_last) - len(preload_crl)
    # Determinism: the run's metrics and records, byte for byte (06 §3 item 16).
    import hashlib
    for name in ('metrics.json', 'recording.mcap'):
        fp = os.path.join(run_dir, name)
        if os.path.exists(fp):
            out['digest.' + name] = hashlib.sha256(open(fp, 'rb').read()).hexdigest()
    return out


def check(table, runs_dir):
    import yaml
    rows = yaml.safe_load(open(table))
    cache = {}

    def get(run, metric):
        if run not in cache:
            p = os.path.join(runs_dir, run, 'realism.json')
            cache[run] = json.load(open(p)) if os.path.exists(p) else None
        d = cache[run]
        if d is None:
            return 'NO-RUN'
        # An edge with no recorded message is zero messages, not a missing measurement.
        return d.get(metric, 0 if metric.startswith('edge.') else 'ABSENT')

    fails = 0
    opens = 0
    for row in rows:
        v = get(row['run'], row['metric'])
        op = row['expect']
        ok = None
        shown = v
        if v in ('NO-RUN', 'ABSENT'):
            ok = None
        elif op == 'eq':
            ok = v == row['value']
        elif op == 'le':
            ok = v <= row['value']
        elif op == 'ge':
            ok = v >= row['value']
        elif op == 'band':
            lo, hi = row['band']
            ok = lo <= v <= hi
        elif op in ('ratio', 'delta'):
            b = get(row['vs'], row['metric'])
            if b in ('NO-RUN', 'ABSENT'):
                ok = None
            else:
                x = (v / b if b else float('inf') if v else 1.0) if op == 'ratio' else v - b
                lo, hi = row['band']
                ok = lo <= x <= hi
                shown = f'{v} vs {b} -> {x:.4g}'
        elif op == 'same':
            b = get(row['vs'], row['metric'])
            ok = None if b in ('NO-RUN', 'ABSENT') else v == b
            shown = f'{str(v)[:16]} vs {str(b)[:16]}'
        elif op == 'true':
            ok = bool(v)
        elif op == 'false':
            ok = not v
        elif op == 'empty':
            ok = len(v) == 0
        elif op == 'subset':
            ok = set(v) <= set(row['value'])
        status = row.get('status')
        verdict = 'PASS' if ok else ('MISSING' if ok is None else ('OPEN' if status == 'open' else 'FAIL'))
        if verdict == 'FAIL':
            fails += 1
        if verdict == 'OPEN':
            opens += 1
        print(f"{verdict:7} {row['id']:28} {row['run']}:{row['metric']} = {shown}")
    print(f"realism: {'FAIL' if fails else 'PASS'} ({len(rows)} rows, {fails} failed, {opens} open)")
    return 1 if fails else 0


if __name__ == '__main__':
    if sys.argv[1] == 'extract':
        d = extract(sys.argv[2])
        if '--json' in sys.argv:
            with open(sys.argv[sys.argv.index('--json') + 1], 'w') as f:
                json.dump(d, f, indent=1, sort_keys=True)
        else:
            for k in sorted(d):
                print(k, d[k])
    elif sys.argv[1] == 'check':
        sys.exit(check(sys.argv[2], sys.argv[3]))
