#!/usr/bin/env python3
"""Live attack-and-detection tests: same-seed base/variant runs of the real engine, judged
against rows written before the runs (docs/validation/live-atk-2026-10-25.md).

    python3 -I tools/realworld/live_atk.py extract <run-dir> [<cost-file>]  # -> <run-dir>/atk.json
    python3 -I tools/realworld/live_atk.py check <table.yaml> <runs-dir> [--fill]
    python3 -I tools/realworld/live_atk.py selftest <table.yaml> <runs-dir>

`extract` reads only the run's files: `run-report.json` (the engine's phase-2 summary),
`recording.mcap` (gt.attack.action, gt.subject, ma.report, ma.decision, app.outcome,
proto.revocation) and the run-lock wrapper's cost line. Measures:

- `p2.<key>`: every scalar of the report's phase-2 section; `p2.verdicts.<detector>`;
  `p2.not_filed_unmapped` (reports a US device could not encode as an SM-MBR subtype).
- `ao.*`: from the per-attacker outcomes: `ao.n`, `ao.reported`, `ao.decided`,
  `ao.reported_share`, `ao.decided_share`, `ao.ttd_med_s` / `ao.ttd_max_s` (onset to the
  first report), `ao.ttr_med_s` / `ao.ttr_max_s` (onset to the MA's decision),
  `ao.decided_early_share` (decided among attackers whose onset left >= 60 s of run),
  `k.<Kind>.{n,reported,decided,ttd_med_s}` per attack type.
- `rec.*`: recomputed from the raw records, independently of the report: `rec.attackers`
  (actors with a gt.attack.action), `rec.revoked_att` / `rec.revoked_hon` (ma.decision
  revoke subjects joined to gt.subject), `rec.ma_reports`, `rec.ma_reports_hon`,
  `rec.ma_det.<detector>` (all), `rec.ma_det_hon.<detector>` (about honest subjects),
  `rec.kdet.<Kind>.<detector>` (reports about each attack type, by detector),
  `rec.warn_false_att` (app.outcome `false` whose subject is an attacker),
  `rec.warn_false_att_pre` (of those, before that attacker's revocation decision),
  `rec.warn_false_ghost` (false, subject no one signs under), `rec.warn_false`,
  `rec.warn_true`, `rec.enforced_n` (proto.revocation `enforced` stages).
- `cost.wall_s`, `cost.peak_mb`, `cost.events`, `cost.node_tasks`.

`check` judges each row (`expect`: eq, le, ge, band on the variant's value; up, down
(variant beyond base by more than `min_change`, relative, default 0), same (|v/b - 1| <=
`tol`), ratio/delta in `band`), and `--fill` writes `measured` and the verdict where the
row has no human verdict (fixed, wrong-expectation, open are kept).

`selftest` shows every row can go red: it applies the row's `fault` (a mutation of the raw
run report or records, before the measures are derived) to the row's variant run and
re-judges; a row that still passes under its fault is printed as CANNOT-FAIL (exit 1).
"""

import copy
import json
import os
import statistics
import sys

NS = 1e9

# The attack types each check is designed for (07-threats §3; detect.rs doc lines): a
# report naming the check about an attacker of any other type is a cross-fire.
DESIGNED = {
    'signatureVerification': {'InvalidSignature'},
    'certValidity': {'ExpiredCert', 'NotYetValid'},
    'sybilCoLocation': {'Sybil'},
    'staleOrReplay': {'DataReplay', 'DelayedMessages', 'OutOfOrder', 'ConstPos', 'EventualStop'},
}


def load_raw(run_dir):
    rep = json.load(open(os.path.join(run_dir, 'run-report.json')))
    raw = {'report': rep['report'], 'cost': {}, 'rec': None,
           'digest': rep.get('record_digest') or rep.get('content_digest')}
    sc = os.path.join(run_dir, 'scenario.resolved.yaml')
    if os.path.exists(sc):
        import yaml
        g = (((yaml.safe_load(open(sc)).get('actors') or {}).get('vehicles') or {}).get('gnss') or {})
        raw['gnss_tau'] = g.get('smoothing_tau_s')
    cost = os.path.join(run_dir, 'cost.txt')
    if os.path.exists(cost):
        for tok in open(cost).read().split():
            if '=' in tok:
                k, v = tok.split('=', 1)
                try:
                    raw['cost'][k] = float(v)
                except ValueError:
                    pass
    mc = os.path.join(run_dir, 'recording.mcap')
    if os.path.exists(mc):
        from mcap.reader import make_reader
        topics = ['record/gt.subject', 'record/gt.attack.action', 'record/ma.report',
                  'record/ma.decision', 'record/app.outcome', 'record/proto.revocation']
        rec = {t.split('/', 1)[1]: [] for t in topics}
        with open(mc, 'rb') as f:
            for _, ch, m in make_reader(f).iter_messages(topics=topics):
                rec[ch.topic.split('/', 1)[1]].append(json.loads(m.data))
        raw['rec'] = rec
    return raw


def med(xs):
    return statistics.median(xs) if xs else None


def derive(raw):
    d = {'digest': raw.get('digest'), 'cfg.gnss_tau': raw.get('gnss_tau')}
    rep = raw['report']
    p = rep.get('phase2') or {}
    for k, v in p.items():
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            d['p2.' + k] = v
    for k, v in (p.get('verdicts_by_detector') or {}).items():
        d['p2.verdicts.' + k] = v
    d['p2.not_filed_unmapped'] = sum(
        v for k, v in (p.get('reports_not_filed_by') or {}).items() if 'subtype' in k)
    end_ns = rep.get('end_ns') or 0
    ao_all = p.get('attacker_outcomes') or []
    # An attacker whose window never opened while it was on the road has no onset: it never
    # lied, so it is not scored (counted apart).
    ao = [a for a in ao_all if a.get('onset_ns') is not None]
    d['ao.no_onset'] = len(ao_all) - len(ao)
    d['ao.n'] = len(ao)
    rep_ = [a for a in ao if a.get('first_reported_ns') is not None]
    dec = [a for a in ao if a.get('decided_ns') is not None]
    d['ao.reported'] = len(rep_)
    d['ao.decided'] = len(dec)
    d['ao.reported_share'] = len(rep_) / len(ao) if ao else None
    d['ao.decided_share'] = len(dec) / len(ao) if ao else None
    early = [a for a in ao if end_ns - a['onset_ns'] >= 60 * NS]
    d['ao.early'] = len(early)
    d['ao.decided_early_share'] = (sum(1 for a in early if a.get('decided_ns') is not None)
                                   / len(early)) if early else None
    ttd = [(a['first_reported_ns'] - a['onset_ns']) / NS for a in rep_]
    ttr = [(a['decided_ns'] - a['onset_ns']) / NS for a in dec]
    d['ao.ttd_med_s'], d['ao.ttd_max_s'] = med(ttd), (max(ttd) if ttd else None)
    d['ao.ttr_med_s'], d['ao.ttr_max_s'] = med(ttr), (max(ttr) if ttr else None)
    d['ao.reports_med'] = med([a.get('reports') or 0 for a in ao])
    kinds = {}
    for a in ao:
        kinds.setdefault(a['attack'].rsplit('/', 1)[-1], []).append(a)
    for k, xs in kinds.items():
        d[f'k.{k}.n'] = len(xs)
        d[f'k.{k}.reported'] = sum(1 for a in xs if a.get('first_reported_ns') is not None)
        d[f'k.{k}.decided'] = sum(1 for a in xs if a.get('decided_ns') is not None)
        d[f'k.{k}.ttd_med_s'] = med([(a['first_reported_ns'] - a['onset_ns']) / NS
                                     for a in xs if a.get('first_reported_ns') is not None])
    ev = rep.get('events_by_class') or {}
    d['cost.events'] = sum(ev.values())
    d['cost.node_tasks'] = ev.get('NodeTask', 0)
    for k, v in raw['cost'].items():
        d['cost.' + k] = v
    apps = rep.get('apps') or {}
    for a, t in apps.items():
        d[f'apps.{a}.false'] = t.get('false_warnings', 0)
        d[f'apps.{a}.issued'] = t.get('issued', 0)
    rec = raw['rec']
    if rec is None:
        return d
    actor_of = {q['subject']: q['actor'] for q in rec['gt.subject']}
    att_kind = {}
    for q in rec['gt.attack.action']:
        att_kind.setdefault(q['actor'], q['attacker'].rsplit('/', 1)[-1])
    d['rec.attackers'] = len(att_kind)
    decided_at = {}
    revoked = set()
    for q in rec['ma.decision']:
        if q.get('decision') != 'revoke':
            continue
        a = actor_of.get(q['subject'])
        revoked.add(a)
        if a is not None:
            decided_at[a] = min(decided_at.get(a, q['t']), q['t'])
    d['rec.revoked_att'] = sum(1 for a in revoked if a in att_kind)
    d['rec.revoked_hon'] = sum(1 for a in revoked if a is not None and a not in att_kind)
    d['rec.revoked_unknown'] = sum(1 for a in revoked if a is None)
    d['rec.ma_reports'] = len(rec['ma.report'])
    hon = unknown = 0
    for det in DESIGNED:
        d['rec.kdet_other.' + det] = 0
    for q in rec['ma.report']:
        a = actor_of.get(q['subject'])
        det = q.get('detector') or 'none'
        d['rec.ma_det.' + det] = d.get('rec.ma_det.' + det, 0) + 1
        if a in att_kind:
            key = f'rec.kdet.{att_kind[a]}.{det}'
            d[key] = d.get(key, 0) + 1
            if det in DESIGNED and att_kind[a] not in DESIGNED[det]:
                d['rec.kdet_other.' + det] += 1
        elif a is None:
            unknown += 1
        else:
            hon += 1
            d['rec.ma_det_hon.' + det] = d.get('rec.ma_det_hon.' + det, 0) + 1
    d['rec.ma_reports_hon'] = hon
    d['rec.ma_reports_unknown'] = unknown
    fa = fa_pre = ghost = false = true = 0
    for o in rec['app.outcome']:
        if o.get('outcome') == 'true':
            true += 1
        if o.get('outcome') != 'false':
            continue
        false += 1
        s = o.get('subject')
        if s is None:
            ghost += 1
            continue
        actor = s - 1  # node 0 is the roadside unit; node n is actor n - 1
        if actor in att_kind:
            fa += 1
            if o['t'] < decided_at.get(actor, float('inf')):
                fa_pre += 1
    d.update({'rec.warn_false_att': fa, 'rec.warn_false_att_pre': fa_pre,
              'rec.warn_false_ghost': ghost, 'rec.warn_false': false, 'rec.warn_true': true,
              'rec.warn_pre_share': fa_pre / fa if fa else None})
    d['rec.enforced_n'] = sum(1 for q in rec['proto.revocation'] if q.get('stage') == 'enforced')
    return d


# ---- faults: mutations of the raw inputs, before the measures are derived -------------

def _p2(raw):
    return raw['report'].setdefault('phase2', {})


def fault(raw, name, row):
    r = copy.deepcopy(raw)
    p = _p2(r)
    rec = r['rec']
    m = row['measure']
    if name == 'honest-revoked':  # the MA revokes an honest device
        p['revoked_honest'] = p.get('revoked_honest', 0) + 1
        p['ma_revoke_decisions'] = p.get('ma_revoke_decisions', 0) + 1
        if rec is not None:
            hon = next((q['subject'] for q in rec['gt.subject']
                        if q['actor'] not in {a['actor'] for a in rec['gt.attack.action']}), None)
            rec['ma.decision'].append({'t': 0, 'subject': hon, 'decision': 'revoke'})
    elif name == 'attack-off':  # the attack is never put on the air
        for k in list(p):
            if isinstance(p[k], (int, float)) and ('attack' in k or 'falsified' in k):
                p[k] = 0
        p['attacker_outcomes'] = []
        p['verdicts_by_detector'] = {}
        if rec is not None:
            rec['gt.attack.action'] = []
            rec['ma.report'] = []
            rec['ma.decision'] = []
            rec['app.outcome'] = [o for o in rec['app.outcome'] if o.get('outcome') != 'false']
    elif name == 'detector-blind':  # receivers' detectors never fire
        p['verdicts_by_detector'] = {}
        for k in ('verdicts_fired', 'reports_sent', 'reports_about_attackers', 'attackers_reported',
                  'reports_about_honest', 'honest_reported', 'reports_at_ma', 'reports_received',
                  'ma_revoke_decisions', 'revoked_attackers', 'cases_opened'):
            p[k] = 0
        for a in p.get('attacker_outcomes') or []:
            a['first_reported_ns'] = None
            a['decided_ns'] = None
            a['reports'] = 0
        if rec is not None:
            rec['ma.report'] = []
            rec['ma.decision'] = []
    elif name == 'no-revocation':  # the MA decides nothing
        for k in ('ma_revoke_decisions', 'revoked_attackers', 'crls_issued'):
            p[k] = 0
        for a in p.get('attacker_outcomes') or []:
            a['decided_ns'] = None
        if rec is not None:
            rec['ma.decision'] = []
            rec['proto.revocation'] = [q for q in rec['proto.revocation'] if q.get('stage') == 'detect']
    elif name == 'slow':  # every stage 100 s later
        for a in p.get('attacker_outcomes') or []:
            for k in ('first_reported_ns', 'decided_ns'):
                if a.get(k) is not None:
                    a[k] += int(100 * NS)
        r['cost']['wall_s'] = r['cost'].get('wall_s', 0) * 10 + 1000
    elif name == 'cost-x10':  # the run costs ten times as much
        for k in r['cost']:
            r['cost'][k] = r['cost'][k] * 10 + 10
        ev = r['report'].get('events_by_class') or {}
        for k in ev:
            ev[k] = ev[k] * 10 + 10
        r['report']['events_by_class'] = ev
    elif name == 'swap-detector':  # reports name some other check
        vb = p.get('verdicts_by_detector') or {}
        p['verdicts_by_detector'] = {'kalmanConsistency': sum(vb.values())}
        if rec is not None:
            for q in rec['ma.report']:
                q['detector'] = 'kalmanConsistency'
    elif name == 'honest-reported':  # every honest vehicle reported, on every check
        p['reports_about_honest'] = p.get('reports_about_honest', 0) + 1000
        p['honest_reported'] = p.get('honest_reported', 0) + 40
        vb = p.setdefault('verdicts_by_detector', {})
        for k in list(vb) + ['positionJump', 'implausibleAcceleration', 'positionSpeedInconsistency',
                             'mapOffRoad', 'constantPositionFrozen', 'sybilCoLocation',
                             'headingInconsistency', 'staleOrReplay', 'beaconFrequency',
                             'implausibleSpeed', 'signatureVerification', 'certValidity']:
            vb[k] = vb.get(k, 0) + 1000
        if rec is not None:
            att = {a['actor'] for a in rec['gt.attack.action']}
            hon = next((q['subject'] for q in rec['gt.subject'] if q['actor'] not in att), None)
            for det in ('positionJump', 'constantPositionFrozen', 'implausibleAcceleration',
                        'positionSpeedInconsistency', 'sybilCoLocation', 'staleOrReplay'):
                rec['ma.report'] += [{'t': 0, 'reporter': 0, 'subject': hon, 'detector': det}] * 50
    elif name == 'all-caught':  # every attacker reported 1 s and revoked 2 s after onset
        for a in p.get('attacker_outcomes') or []:
            a['first_reported_ns'] = a['onset_ns'] + int(NS)
            a['decided_ns'] = a['onset_ns'] + int(2 * NS)
        p['revoked_attackers'] = p.get('attackers', 0)
        if rec is not None:
            att = {a['actor'] for a in rec['gt.attack.action']}
            for q in rec['gt.subject']:
                if q['actor'] in att:
                    rec['ma.decision'].append({'t': 0, 'subject': q['subject'], 'decision': 'revoke'})
    elif name == 'crossfire':  # every report about an attacker names the row's check
        det = m.rsplit('.', 1)[-1]
        if rec is not None:
            for q in rec['ma.report']:
                q['detector'] = det
    elif name == 'warn-late':  # attackers keep raising warnings long after their revocation
        if rec is not None and rec['gt.attack.action']:
            a = rec['gt.attack.action'][0]['actor']
            sub = next((q['subject'] for q in rec['gt.subject'] if q['actor'] == a), None)
            rec['ma.decision'].append({'t': 1, 'subject': sub, 'decision': 'revoke'})
            rec['app.outcome'] += [{'t': 10**15, 'node': 1, 'app': 'fcw', 'subject': a + 1,
                                    'outcome': 'false'}] * 5000
    elif name == 'warn-none':  # no warning ever comes from an attacker or a ghost
        if rec is not None:
            rec['app.outcome'] = [o for o in rec['app.outcome'] if o.get('outcome') != 'false']
        for t in (r['report'].get('apps') or {}).values():
            t['false_warnings'] = 0
    elif name == 'warn-flood':  # every warning is false and from an attacker
        if rec is not None:
            if rec['gt.attack.action']:
                a = rec['gt.attack.action'][0]['actor']
                rec['app.outcome'] += [{'t': 0, 'node': 1, 'app': 'fcw', 'subject': a + 1,
                                        'outcome': 'false'}] * 5000
            rec['app.outcome'] += [{'t': 0, 'node': 1, 'app': 'fcw', 'subject': None,
                                    'outcome': 'false'}] * 5000
        for t in (r['report'].get('apps') or {}).values():
            t['false_warnings'] = t.get('false_warnings', 0) + 5000
    elif name == 'nondeterministic':  # a second run that differs
        r['digest'] = str(r.get('digest')) + '-other'
    elif name == 'scalar':  # the measured value itself pushed out of the row's band
        r['_scalar'] = m
    else:
        raise SystemExit(f'unknown fault {name}')
    return r


# ---- judging ------------------------------------------------------------------------

def judge(row, b, v):
    e = row['expect']
    if v is None or (e in ('up', 'down', 'same', 'ratio', 'delta', 'identical') and b is None):
        return 'FAIL', 'missing'
    mc = float(row.get('min_change', 0.0))
    if e == 'eq':
        ok = v == row['value']
    elif e == 'le':
        ok = v <= row['value']
    elif e == 'ge':
        ok = v >= row['value']
    elif e == 'band':
        lo, hi = row['band']
        ok = lo <= v <= hi
    elif e == 'up':
        ok = v > b * (1 + mc) if b > 0 else v > b + mc
    elif e == 'down':
        ok = v < b * (1 - mc) if b > 0 else v < b - mc
    elif e == 'same':
        tol = float(row.get('tol', 0.0))
        ok = (v == b) if b == 0 else abs(v / b - 1) <= tol
    elif e == 'identical':
        ok = v == b
    elif e == 'ratio':
        lo, hi = row['band']
        ok = b != 0 and lo <= v / b <= hi
    elif e == 'delta':
        lo, hi = row['band']
        ok = lo <= v - b <= hi
    else:
        return 'FAIL', f'unknown expect {e}'
    return ('PASS' if ok else 'FAIL'), None


def push_out(row, v, b):
    """A value that violates the row (the `scalar` fault)."""
    e = row['expect']
    if e == 'eq':
        return row['value'] + 1
    if e == 'le':
        return row['value'] + 1
    if e == 'ge':
        return row['value'] - 1
    if e == 'band':
        return row['band'][1] + 1
    if e in ('up', 'down'):
        return b
    if e == 'same':
        return (b or 0) * 2 + 1
    if e == 'identical':
        return str(b) + '-mutated'
    if e == 'ratio':
        return (b or 1) * (row['band'][1] + 1)
    if e == 'delta':
        return (b or 0) + row['band'][1] + 1
    return None


def fmt(v):
    if isinstance(v, float):
        return f'{v:.4g}'
    return str(v)


def load_table(path):
    import yaml
    return yaml.safe_load(open(path))


def summaries(runs_dir, names):
    out = {}
    for n in names:
        p = os.path.join(runs_dir, n, 'atk.json')
        if os.path.exists(p):
            d = json.load(open(p))
            if not d.get('placeholder'):
                out[n] = d
    return out


def check(table, runs_dir, fill):
    doc = load_table(table)
    rows = doc['rows']
    s = summaries(runs_dir, doc['runs'].keys())
    fails = 0
    results = {}
    for row in rows:
        bn, vn = row.get('base'), row['variant']
        if vn not in s or (row['expect'] in ('up', 'down', 'same', 'ratio', 'delta', 'identical') and bn not in s):
            print(f"{row['id']:<34} PENDING")
            continue
        v = s[vn]['measures'].get(row['measure'])
        b = s[bn]['measures'].get(row['measure']) if bn in s else None
        if v is None and row.get('absent_is_zero'):
            v = 0
        if b is None and bn in s and row.get('absent_is_zero'):
            b = 0
        verdict, why = judge(row, b, v)
        m = f'{fmt(b)} -> {fmt(v)}' if row['expect'] in ('up', 'down', 'same', 'ratio', 'delta', 'identical') else fmt(v)
        results[row['id']] = (m, verdict)
        human = row.get('verdict') in ('fixed', 'wrong-expectation', 'open')
        if verdict == 'FAIL' and not human:
            fails += 1
        print(f"{row['id']:<34} {verdict:<5} {m}{'  (' + row['verdict'] + ')' if human else ''}{'  ' + why if why else ''}")
    print(f'{len(results)} judged of {len(rows)}; {fails} FAIL without a human verdict')
    if fill:
        fill_table(table, results)
    return 1 if fails else 0


def fill_table(path, results):
    """Rewrite `measured:` and `verdict:` lines of judged rows in place (keeps comments)."""
    lines = open(path).read().split('\n')
    out, cur = [], None
    for ln in lines:
        st = ln.strip()
        if st.startswith('- id:'):
            cur = st.split(':', 1)[1].strip()
        if cur in results and st.startswith('measured:'):
            ln = ln[:len(ln) - len(ln.lstrip())] + f"measured: '{results[cur][0]}'"
        if cur in results and st.startswith('verdict:') and st.split(':', 1)[1].strip() in ('pending', 'pass', 'FAIL'):
            v = 'pass' if results[cur][1] == 'PASS' else 'FAIL'
            ln = ln[:len(ln) - len(ln.lstrip())] + f'verdict: {v}'
        out.append(ln)
    open(path, 'w').write('\n'.join(out))


def selftest(table, runs_dir):
    doc = load_table(table)
    raws = {}
    bad = 0
    n = 0
    for row in doc['rows']:
        vn, bn = row['variant'], row.get('base')
        if not os.path.exists(os.path.join(runs_dir, vn, 'raw.json')):
            continue
        if vn not in raws:
            raws[vn] = json.load(open(os.path.join(runs_dir, vn, 'raw.json')))
        if bn and bn not in raws and os.path.exists(os.path.join(runs_dir, bn, 'raw.json')):
            raws[bn] = json.load(open(os.path.join(runs_dir, bn, 'raw.json')))
        b = derive(raws[bn]).get(row['measure']) if bn in raws else None
        if b is None and row.get('absent_is_zero'):
            b = 0
        f = row.get('fault', 'scalar')
        mutated = fault(raws[vn], f, row)
        v = derive(mutated).get(row['measure'])
        if v is None and row.get('absent_is_zero'):
            v = 0
        if '_scalar' in mutated:
            v = push_out(row, v, b)
        verdict, _ = judge(row, b, v)
        n += 1
        if verdict != 'FAIL':
            bad += 1
            print(f"{row['id']:<34} CANNOT-FAIL under {f}: {fmt(b)} -> {fmt(v)}")
    print(f'selftest: {n} rows faulted, {bad} stayed green')
    return 1 if bad else 0


def main():
    cmd = sys.argv[1]
    if cmd == 'extract':
        run = sys.argv[2]
        if len(sys.argv) > 3:
            import shutil
            shutil.copy(sys.argv[3], os.path.join(run, 'cost.txt'))
        raw = load_raw(run)
        json.dump(raw, open(os.path.join(run, 'raw.json'), 'w'))
        d = derive(raw)
        json.dump({'measures': d}, open(os.path.join(run, 'atk.json'), 'w'), indent=0, sort_keys=True)
        print(f'{run}: {len(d)} measures')
    elif cmd == 'check':
        sys.exit(check(sys.argv[2], sys.argv[3], '--fill' in sys.argv))
    elif cmd == 'selftest':
        sys.exit(selftest(sys.argv[2], sys.argv[3]))
    elif cmd == 'show':
        d = json.load(open(os.path.join(sys.argv[2], 'atk.json')))['measures']
        pre = sys.argv[3:] or ['']
        for k in sorted(d):
            if any(k.startswith(p) for p in pre):
                print(k, fmt(d[k]))


if __name__ == '__main__':
    main()
