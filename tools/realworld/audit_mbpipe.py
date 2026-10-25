#!/usr/bin/env python3
"""Misbehaviour-pipeline audit: every hop from the on-device detector to the receiver that
drops a revoked sender, checked from a run's raw records only
(docs/design/scms-real/testing/audit-mbpipe.md).

    python3 -I tools/realworld/audit_mbpipe.py check <table.yaml> <runs-dir> [--fill]
    python3 -I tools/realworld/audit_mbpipe.py selftest <table.yaml> <runs-dir>
    python3 -I tools/realworld/audit_mbpipe.py hops <run-dir>

It reads `<runs-dir>/<run>/recording.mcap` (pki.report_fate, pki.flow, pki.case,
proto.revocation, ma.report, ma.received, ma.decision, node.rx, node.tx, node.security,
node.credentials, gt.subject, gt.attack.action, gt.device_exposure), `run-report.json`,
`metrics.json` and `scenario.resolved.yaml`. It shares no code with the engine and does
not read the engine's summaries except to compare them with what the records say.

`check` judges every row (`eq`, `le`, `ge`, `band` on the row's measure) and with `--fill`
writes `measured` and, where the row has no human verdict, `verdict`. `selftest` injects
each row's `fault` (an in-memory mutation of the records or of the engine value) and
requires that row to go red. `hops` prints each hop's count, loss and latency.
Needs the `mcap` and `zstandard` packages.
"""

import collections
import datetime as dt
import hashlib
import json
import os
import re
import sys

NS = 1_000_000_000
US_SUBTYPES = {'ConstantPosition', 'RandomPosition', 'MaxSpeed', 'MaxAcceleration',
               'BrakeAcceleration'}
TERMINAL = {'uploaded', 'evicted_quota', 'decayed_out', 'expired', 'held_at_end',
            'left_with_device'}
SHUFFLE_S = 86_400          # [CAMP122] SCMS-765: 10,000 reports or one day
SHUFFLE_N = 10_000
REVIEW_S = 28_800           # MaMode::ManualReview default, 8 h (proto G8)
POLL_DEADLINE_S = 3_660     # hourly conditional poll (proto I6) plus a minute
WEEK_S = 604_800            # nextCrl - issueDate (proto H8)
MIN_REPORTERS = 3           # threat/ma/corroborated defaults
MIN_SPAN_S = 10.0           # (min_events - 1) * event_window_s = 2 * 5 s
STAGE_ORDER = ['detect', 'report_sent', 'report_received', 'decision', 'resolved',
               'issued', 'published', 'downloaded', 'processed', 'enforced']
CASE_ORDER = ['reports-in', 'corroborated', 'decision', 'plv-lookup', 'la-seeds',
              'crl-published', 'enforced']
# US federal holidays (5 U.S.C. 6103, observed dates), 2026-2027.
HOLIDAYS = {dt.date(*d) for d in [
    (2026, 1, 1), (2026, 1, 19), (2026, 2, 16), (2026, 5, 25), (2026, 6, 19), (2026, 7, 3),
    (2026, 9, 7), (2026, 10, 12), (2026, 11, 11), (2026, 11, 26), (2026, 12, 25),
    (2027, 1, 1), (2027, 1, 18), (2027, 2, 15), (2027, 5, 31), (2027, 6, 18), (2027, 7, 5),
    (2027, 9, 6), (2027, 10, 11), (2027, 11, 11), (2027, 11, 25), (2027, 12, 24)]}


# ------------------------------------------------------------------ loading
def _t0(run_dir):
    for line in open(os.path.join(run_dir, 'scenario.resolved.yaml')):
        m = re.match(r"\s+t0:\s*'?([0-9T:\-]+)Z?'?", line)
        if m:
            return dt.datetime.fromisoformat(m.group(1)).replace(tzinfo=dt.timezone.utc)
    raise SystemExit('no time.t0 in scenario.resolved.yaml')


def _metric_samples(path, names):
    out = collections.defaultdict(list)
    blk = None
    for line in open(path):
        if line.startswith('    {'):
            blk = [line]
        elif blk is not None:
            blk.append(line)
            if line.startswith('    }'):
                s = ''.join(blk).rstrip().rstrip(',')
                blk = None
                m = re.search(r'"metric": "([^"]+)"', s)
                if m and m.group(1) in names:
                    out[m.group(1)].append(json.loads(s))
    return out


def load(run_dir):
    from mcap.reader import make_reader
    D = {'run': run_dir, 't0': _t0(run_dir)}
    rep = json.load(open(os.path.join(run_dir, 'run-report.json')))
    D['report'] = rep
    D['p2'] = dict(rep['report'].get('phase2') or {})
    D['end'] = int(round(float(re.search(r'duration_s:\s*([0-9.]+)', open(
        os.path.join(run_dir, 'scenario.resolved.yaml')).read()).group(1)) * NS))
    fates, flow, case, rev, mad, mar, marec, sec = [], [], [], [], [], [], [], []
    cred = {}
    subj_actor, onset, expo = {}, {}, {}
    txp = {}                       # (node, msg) -> pseudonym, BSMs
    report_sent = {}               # report id -> first relayed frame on the air (node.tx)
    tx_by_node = collections.defaultdict(list)   # node -> [(t, pseudonym)] BSMs
    tx_types = collections.Counter()
    tx_nodes = set()
    path = os.path.join(run_dir, 'recording.mcap')
    with open(path, 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            t = ch.topic[7:] if ch.topic.startswith('record/') else ch.topic
            if t == 'node.rx' or t == 'phy.rx' or t == 'node.verify' or t == 'metric.sample':
                continue
            if t == 'node.telemetry':
                tx_nodes.add(json.loads(m.data)['node'])
                continue
            if t == 'node.tx':
                q = json.loads(m.data)
                tx_nodes.add(q['node'])
                tx_types[(q['msg_type'], q.get('channel'))] += 1
                if q['msg_type'] == 'bsm':
                    txp[(q['node'], q['msg'])] = q.get('pseudonym')
                    tx_by_node[q['node']].append((q['t'], q.get('pseudonym')))
                else:
                    txp[(q['node'], q['msg'])] = q.get('pseudonym')
                    rid = q.get('report_id')
                    if rid:
                        report_sent[rid] = min(report_sent.get(rid, q['t']), q['t'])
            elif t == 'pki.report_fate':
                fates.append(json.loads(m.data))
            elif t == 'pki.flow':
                q = json.loads(m.data)
                if q['flow'] in ('report', 'linkage-resolution', 'crl-issuance'):
                    for p in q.get('pdus') or []:
                        p.pop('hex', None)
                        p.pop('opened', None)
                    flow.append(q)
            elif t == 'pki.case':
                case.append(json.loads(m.data))
            elif t == 'proto.revocation':
                rev.append(json.loads(m.data))
            elif t == 'ma.decision':
                mad.append(json.loads(m.data))
            elif t == 'ma.report':
                mar.append(json.loads(m.data))
            elif t == 'ma.received':
                marec.append(json.loads(m.data))
            elif t == 'node.security':
                q = json.loads(m.data)
                sec.append({'t': q['t'], 'node': q['node'], 'link': q.get('link'),
                            'self_revoked': q.get('self_revoked'),
                            'crls': [dict(c) for c in q.get('crls') or []]})
            elif t == 'node.credentials':
                q = json.loads(m.data)
                c = cred.setdefault(q['node'], {'device': q.get('device'),
                                                'provider': q.get('provider'), 'cells': {}})
                for cell in q.get('cells') or []:
                    if cell.get('id8'):
                        c['cells'][cell['id8']] = cell['i']
            elif t == 'gt.subject':
                q = json.loads(m.data)
                subj_actor.setdefault(q['subject'], q['actor'])
            elif t == 'gt.attack.action':
                q = json.loads(m.data)
                a = q.get('actor')
                # The onset is the first action that changed bytes on the air (08 §2.4:
                # an attack that never reached the air has not begun).
                if a is not None and q.get('changed_bytes_on_air'):
                    onset[a] = min(onset.get(a, q['t']), q['t'])
            elif t == 'gt.device_exposure':
                q = json.loads(m.data)
                expo[q['node']] = q
    D.update(fates=fates, flow=flow, case=case, rev=rev, mad=mad, mar=mar, marec=marec,
             sec=sec, cred=cred, subj_actor=subj_actor, onset=onset, expo=expo, txp=txp,
             tx_by_node=dict(tx_by_node), tx_types=tx_types, report_sent=report_sent)
    D['rsus'] = sorted(n for n in tx_nodes if n not in cred and n not in expo)
    _static(D)
    _rx_pass(D, path)
    D['metrics'] = _metric_samples(os.path.join(run_dir, 'metrics.json'), {
        'revocation_latency_stage', 'crl_entries', 'crl_bytes', 'det_tp', 'det_fp', 'det_fn',
        'det_tn', 'false_accusations', 'time_to_detect', 'time_to_decision'})
    return D


def _static(D):
    """Joins that the mutations never touch: who is who."""
    p2n = {}
    for (n, _), p in D['txp'].items():
        if p:
            p2n.setdefault(p, n)
    for n, c in D['cred'].items():
        for id8 in c['cells']:
            p2n.setdefault(id8, n)
    D['p2n'] = p2n
    D['dev2node'] = {c['device']: n for n, c in D['cred'].items()}
    links = collections.defaultdict(collections.Counter)
    for r in D['sec']:
        if r['link']:
            links[r['node']][r['link']] += 1
    D['link'] = {n: c.most_common(1)[0][0] for n, c in links.items()}
    D['attacker'] = {n for n, e in D['expo'].items() if e.get('attacker')}
    # Revoked devices: every subject a case was published for.
    dec_ids = {r['id'] for r in D['rev'] if r['stage'] == 'decision'}
    D['revoked_nodes'] = {D['p2n'][s] for s in dec_ids if s in D['p2n']}


def _rx_pass(D, path):
    from mcap.reader import make_reader
    rsus = set(D['rsus'])
    need = collections.defaultdict(set)          # reporter -> subjects it reported
    for f in D['fates']:
        if f['fate'] == 'stored' and f.get('reporter') is not None:
            need[f['reporter']].add(f['subject'])
    relay_rep = {n for n, l in D['link'].items() if l != 'cellular'}
    revoked = D['revoked_nodes']
    deliv = collections.defaultdict(list)        # (rx, pseudonym) -> [t]
    relay_rx = collections.defaultdict(list)     # reporter -> [(t, rsu, msg_type)]
    crl_rx = collections.defaultdict(list)       # vehicle -> [(t, rsu, msg_type)]
    enf_rx = []
    census = collections.Counter()
    n_rev = n_rev_rsu = 0
    with open(path, 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            if ch.topic != 'record/node.rx':
                continue
            q = json.loads(m.data)
            rx, tx, mt, oc = q['rx'], q['tx'], q['msg_type'], q['outcome']
            census[(mt, oc, q.get('cause'))] += 1
            if q.get('cause') == 'revoked':
                n_rev += 1
                n_rev_rsu += rx in rsus
            if oc == 'delivered' and mt == 'bsm' and rx in need:
                p = D['txp'].get((tx, q['msg']))
                if p in need[rx]:
                    deliv[(rx, p)].append(q['t'])
            if mt != 'bsm' and oc == 'delivered':
                if rx in rsus and tx in relay_rep:
                    relay_rx[tx].append((q['t'], rx, mt))
                if tx in rsus and rx not in rsus:
                    crl_rx[rx].append((q['t'], tx, mt))
            if tx in revoked and mt == 'bsm':
                enf_rx.append({'t': q['t'], 'rx': rx, 'tx': tx, 'msg': q['msg'],
                               'outcome': oc, 'cause': q.get('cause'),
                               'verification': q.get('verification')})
    D.update(deliv=dict(deliv), relay_rx=dict(relay_rx), crl_rx=dict(crl_rx), enf_rx=enf_rx,
             rx_census=census, rx_revoked=n_rev, rx_revoked_rsu=n_rev_rsu)


# ------------------------------------------------------------------ helpers
def p95(xs):
    if not xs:
        return 0.0
    xs = sorted(xs)
    k = (len(xs) - 1) * 0.95
    lo = int(k)
    hi = min(lo + 1, len(xs) - 1)
    return xs[lo] + (xs[hi] - xs[lo]) * (k - lo)


def by_report(fates):
    g = collections.defaultdict(list)
    for f in fates:
        g[f['report_id']].append(f)
    return g


def report_instances(flow):
    g = collections.defaultdict(list)
    for i, q in enumerate(flow):
        if q['flow'] == 'report':
            g[q['instance']].append((i, q))
    return g


def uploads_to_ra(flow):
    """instance -> the upload msg that reached an RA (report from a device or a unit)."""
    out = {}
    for q in flow:
        if q['flow'] == 'report' and q['kind'] == 'msg' and q['name'] == 'report' \
                and '/ra/' in (q.get('to') or ''):
            out.setdefault(q['instance'], q)
    return out


def forwards(flow):
    return [(i, q) for i, q in enumerate(flow) if q['flow'] == 'report' and q['kind'] == 'msg'
            and q['name'] == 'report-forward']


def batches(flow):
    """RA shuffle batches: forwards grouped by RA and forward instant."""
    up = uploads_to_ra(flow)
    g = collections.defaultdict(list)
    for i, q in forwards(flow):
        g[(q['from'], q['t'] // NS)].append((i, q))
    out = []
    for (ra, sec_), members in sorted(g.items()):
        arr = [up[q['instance']]['t'] for _, q in members if q['instance'] in up]
        out.append({'ra': ra, 'close': min(q['t'] for _, q in members), 'members': members,
                    'arrivals': arr})
    return out


def kendall_tau(a, b):
    n = len(a)
    if n < 2:
        return 0.0
    conc = disc = 0
    for i in range(n):
        for j in range(i + 1, n):
            s = (a[i] - a[j]) * (b[i] - b[j])
            conc += s > 0
            disc += s < 0
    return (conc - disc) / max(1, conc + disc)


def cases(D):
    g = collections.defaultdict(lambda: collections.defaultdict(list))
    meta = {}
    for q in D['case']:
        g[q['case']][q['stage']].append(q)
        meta.setdefault(q['case'], {'provider': q['provider'], 'instance': q.get('instance')})
    return g, meta


def first_t(stage_rows):
    return min(r['t'] for r in stage_rows)


def to_local(D, t_ns):
    from zoneinfo import ZoneInfo
    return (D['t0'] + dt.timedelta(seconds=t_ns / NS)).astimezone(ZoneInfo('America/New_York'))


def next_slot(D, t_ns):
    """The first 10:00 ET business-day instant at or after t (sim ns)."""
    from zoneinfo import ZoneInfo
    tz = ZoneInfo('America/New_York')
    loc = to_local(D, t_ns)
    day = loc.date()
    for k in range(15):
        d = day + dt.timedelta(days=k)
        if d.weekday() >= 5 or d in HOLIDAYS:
            continue
        cand = dt.datetime(d.year, d.month, d.day, 10, 0, 0, tzinfo=tz)
        if cand >= loc:
            return int(round((cand.astimezone(dt.timezone.utc) - D['t0']).total_seconds() * NS))
    return None


def time32(D, t_ns):
    """IEEE 1609.2 Time32 (TAI seconds since 2004-01-01) of a sim instant: UTC plus the 5
    leap seconds inserted since 2004 (2005, 2008, 2012, 2015, 2016)."""
    epoch = dt.datetime(2004, 1, 1, tzinfo=dt.timezone.utc)
    return (D['t0'] - epoch).total_seconds() + t_ns / NS + 5


def publications(D):
    """Change-driven publications: crl-issuance `published` stages of instances with an
    append, with the list each carries (its CRACA and issue date, from what nodes hold)."""
    appended = {q['instance'] for q in D['flow'] if q['flow'] == 'crl-issuance'
                and q['kind'] == 'msg' and q['name'] == 'crl-append'}
    held = set()
    for r in D['sec']:
        for c in r['crls']:
            held.add((c['craca'], c['issue_date']))
    out = []
    seen = set()
    for q in D['flow']:
        if q['flow'] == 'crl-issuance' and q['kind'] == 'stage' and q['name'] == 'published' \
                and q['instance'] in appended:
            key = (q['provider'], q['t'] // NS)
            if key in seen:
                continue
            seen.add(key)
            t32 = time32(D, q['t'])
            cand = sorted(held, key=lambda ci: abs(ci[1] - t32))
            lst = cand[0] if cand and abs(cand[0][1] - t32) <= 120 else None
            ids = {r['id'] for r in D['rev'] if r['stage'] == 'published'
                   and abs(r['t'] - q['t']) <= 0.1 * NS}
            out.append({'provider': q['provider'], 't': q['t'], 'list': lst, 'ids': ids})
    return out


def install_times(D):
    """(receiver, revoked node) -> first enforced instant."""
    T = {}
    for r in D['rev']:
        if r['stage'] == 'enforced' and r.get('node') is not None:
            d = D['p2n'].get(r['id'])
            if d is None:
                continue
            k = (r['node'], d)
            T[k] = min(T.get(k, r['t']), r['t'])
    for k, t in joiner_installs(D).items():
        T[k] = min(T.get(k, t), t)
    return T


def revoked_pseudonyms(D):
    """Revoked node -> its pseudonyms of i >= i_rev (all of them when the period is unknown)."""
    g, meta = cases(D)
    irev = {}
    for cid, st in g.items():
        for row in st.get('la-seeds', []):
            for r in D['rev']:
                if r['stage'] == 'decision' and abs(r['t'] - first_t(st['decision'])) <= 0.01 * NS:
                    n = D['p2n'].get(r['id'])
                    if n is not None:
                        irev[n] = min(irev.get(n, 1 << 30), row['detail'].get('i_rev', 0))
    out = {}
    for n in D['revoked_nodes']:
        cells = D['cred'].get(n, {}).get('cells', {})
        ir = irev.get(n, 0)
        ps = {p for p, i in cells.items() if i >= ir}
        ps |= {p for _, p in D['tx_by_node'].get(n, []) if p and p not in cells}
        out[n] = ps
    return out


# ------------------------------------------------------------------ measures
def m_det_unheard(D):
    bad = 0
    n = 0
    for f in D['fates']:
        if f['fate'] != 'stored':
            continue
        n += 1
        ts = D['deliv'].get((f['reporter'], f['subject']), [])
        if not any(f['t'] - NS <= t <= f['t'] for t in ts):
            bad += 1
    return bad, f'{bad} of {n} stored reports name a pseudonym the reporter had not heard in the second before'


def m_det_subtypes(D):
    bad = n = 0
    names = collections.Counter()
    for q in D['case']:
        if q['stage'] != 'reports-in':
            continue
        for ev in (q.get('detail') or {}).get('evidence') or []:
            n += 1
            st = [s.get('name') for s in ev.get('subtypes') or []]
            names.update(st)
            if not st or any(s not in US_SUBTYPES for s in st):
                bad += 1
    return bad, f'{bad} of {n} case evidence items; subtypes {dict(names)}'


def m_det_rate(D):
    # The limit binds the judged messages: a report id is r-<reporter>-<subject>-<the
    # judged message's reception instant> (corrected 2026-10-24: the stored instants add
    # each verification's own delay, 0.999 s apart for judged messages 1.0008 s apart).
    g = collections.defaultdict(list)
    for f in D['fates']:
        if f['fate'] == 'stored':
            g[(f['reporter'], f['subject'])].append(int(f['report_id'].rsplit('-', 1)[1]))
    gaps = [b - a for ts in g.values() for a, b in zip(sorted(ts), sorted(ts)[1:])]
    v = min(gaps) / NS if gaps else 1e9
    return round(v, 3), f'{len(gaps)} repeat pairs; smallest gap {v:.3f} s'


def m_up_posts(D):
    bad = n = 0
    for q in D['flow']:
        if q['flow'] == 'report' and q['kind'] == 'msg' and q['name'] == 'report':
            n += 1
            k = sum(1 for p in q.get('pdus') or [] if p['type'] == 'SecuredMisbehaviorReport')
            bad += k != 1
    return bad, f'{bad} of {n} report posts do not carry exactly one SecuredMisbehaviorReport'


def m_store_unbalanced(D):
    bad = 0
    g = by_report(D['fates'])
    terms = collections.Counter()
    for rid, fs in g.items():
        st = sum(f['fate'] == 'stored' for f in fs)
        te = [f['fate'] for f in fs if f['fate'] in TERMINAL]
        terms.update(te)
        bad += st != 1 or len(te) != 1
    return bad, f'{bad} of {len(g)} reports unbalanced; terminal fates {dict(terms)}'


def m_store_weight(D):
    w = [f.get('weight') for f in D['fates'] if f['fate'] == 'stored']
    bad = sum(1 for x in w if x != 1.0)
    return bad, f'{bad} of {len(w)} stored at a weight other than 1.0'


def m_store_order(D):
    # The order a device *sends* its reports in: a relayed report is sent when its first
    # frame is on the air (node.tx report_id, since 2026-10-25) and confirmed later, so
    # its confirmation order is not its send order (282 apparent violations on AUD); an
    # upload over the modem is sent at its `uploaded` fate. At each send, no report still
    # pending (stored, unsent, not ended) may be newer than the one sent.
    sent_at = D.get('report_sent', {})
    life = collections.defaultdict(dict)   # reporter -> rid -> [stored, sent, ended]
    for f in D['fates']:
        r, rid = f.get('reporter'), f['report_id']
        if f['fate'] == 'stored':
            life[r][rid] = [f['t'], None, None]
        elif f['fate'] in TERMINAL and rid in life[r] and life[r][rid][2] is None:
            e = life[r][rid]
            e[2] = f['t']
            if f['fate'] == 'uploaded':
                e[1] = min(sent_at.get(rid, f['t']), f['t'])
    bad = exercised = relayed = 0
    for reps in life.values():
        for rid, (st, s, _end) in reps.items():
            if s is None:
                continue
            relayed += rid in sent_at
            pending = [st2 for r2, (st2, s2, e2) in reps.items() if r2 != rid and st2 <= s
                       and (s2 is None or s2 > s) and (e2 is None or e2 > s)]
            if pending:
                exercised += 1
                if max(pending) > st:
                    bad += 1
    return bad, (f'{bad} of {exercised} sends made with other reports pending went out of order '
                 f'({relayed} relayed, sent at their first frame on the air)')


def m_store_engine(D):
    c = collections.Counter(f['fate'] for f in D['fates'])
    p = D['p2']
    pairs = [('reports_evicted', c['evicted_quota'] + c['decayed_out']),
             ('reports_expired', c['expired']), ('reports_unsent', c['held_at_end'])]
    mis = sum(abs(p.get(k, 0) - v) for k, v in pairs)
    return mis, '; '.join(f'{k} engine {p.get(k)} records {v}' for k, v in pairs)


def _post_link(D, q):
    n = D['dev2node'].get(q.get('from'))
    return n, D['link'].get(n)


def m_up_link(D):
    bad = n = 0
    kinds = collections.Counter()
    for q in D['flow']:
        if q['flow'] == 'report' and q['kind'] == 'msg' and q['name'] == 'report' \
                and (q.get('from') or '').startswith('ee/'):
            node, l = _post_link(D, q)
            n += 1
            kinds[(l, q.get('transport'))] += 1
            if (l == 'cellular') != (q.get('transport') == 'cellular-uu'):
                bad += 1
    return bad, f'{bad} of {n} device posts on the wrong link; {dict(kinds)}'


def m_up_relay_unheard(D):
    bad = n = 0
    for f in D['fates']:
        if f['fate'] != 'uploaded' or D['link'].get(f['reporter']) == 'cellular':
            continue
        n += 1
        ts = D['relay_rx'].get(f['reporter'], [])
        # A unit's node.rx row is stamped at delivery, 0.5-2 ms after the arrival that
        # confirms the upload (corrected 2026-10-24: all 92 first misses had it then).
        if not any(f['t'] - 5 * NS <= t <= f['t'] + NS // 100 for t, _, _ in ts):
            bad += 1
    return bad, f'{bad} of {n} relay uploads with no unit reception of the reporter in the 5 s before'


def m_up_engine(D):
    c = collections.Counter()
    for f in D['fates']:
        if f['fate'] == 'uploaded':
            c['cell' if D['link'].get(f['reporter']) == 'cellular' else 'relay'] += 1
    # Corrected 2026-10-24: a `detect` stage is logged only for a report that is posted
    # (2,473 = the posts), so it cannot count reports_sent; the real reports posted
    # (reports_mbr_real) are what the posts count.
    posts = sum(1 for q in D['flow'] if q['flow'] == 'report' and q['kind'] == 'msg'
                and q['name'] == 'report' and (q.get('from') or '').startswith('ee/'))
    p = D['p2']
    pairs = [('reports_uploaded_cellular', c['cell']), ('reports_uploaded_relay', c['relay']),
             ('reports_mbr_real', posts), ('reports_from_rsus', 0)]
    mis = sum(abs(p.get(k, 0) - v) for k, v in pairs)
    return mis, '; '.join(f'{k} engine {p.get(k)} records {v}' for k, v in pairs)


def m_up_cell_latency(D):
    st = {f['report_id']: f['t'] for f in D['fates'] if f['fate'] == 'stored'}
    lat = [(f['t'] - st[f['report_id']]) / NS for f in D['fates'] if f['fate'] == 'uploaded'
           and f['report_id'] in st and D['link'].get(f['reporter']) == 'cellular']
    v = p95(lat)
    return round(v, 4), f'{len(lat)} cellular uploads; p95 {v:.4f} s, max {max(lat) if lat else 0:.4f} s'


def m_ra_once(D):
    up = uploads_to_ra(D['flow'])
    nf = collections.Counter(q['instance'] for _, q in forwards(D['flow']))
    dup = sum(1 for k, v in nf.items() if v > 1)
    lost = sum(1 for k, q in up.items() if nf[k] == 0
               and q['t'] + (SHUFFLE_S + 60) * NS < D['end'])
    held = sum(1 for k, q in up.items() if nf[k] == 0) - lost
    return dup + lost, f'{len(up)} at an RA; {dup} forwarded twice, {lost} never forwarded, {held} still in the shuffle at the end'


def m_ra_window(D):
    err = 0.0
    det = []
    for b in batches(D['flow']):
        if not b['arrivals'] or len(b['members']) >= SHUFFLE_N:
            continue
        e = abs((b['close'] - min(b['arrivals'])) / NS - SHUFFLE_S)
        err = max(err, e)
        det.append(f"{b['ra']}@{b['close'] / NS:.1f}s n={len(b['members'])} open+{(b['close'] - min(b['arrivals'])) / NS:.3f}s")
    return round(err, 3), f'{len(det)} batches: ' + '; '.join(det[:6])


def m_ra_tau(D):
    up = uploads_to_ra(D['flow'])
    worst = -1.0
    n = 0
    for b in batches(D['flow']):
        ms = [(i, q) for i, q in b['members'] if q['instance'] in up]
        if len(ms) < 10:
            continue
        n += 1
        arr = [up[q['instance']]['t'] for _, q in ms]
        order = [i for i, _ in ms]
        worst = max(worst, kendall_tau(arr, order))
    return round(worst, 3), f'{n} batches of 10 or more; largest arrival/forward tau {worst:.3f}'


def m_ra_strip(D):
    up = uploads_to_ra(D['flow'])
    bad = n = skipped = 0
    for _, q in forwards(D['flow']):
        u = up.get(q['instance'])
        lens = [p['len'] for p in (u or {}).get('pdus') or [] if p['type'] == 'SecuredMisbehaviorReport']
        if not lens:
            skipped += 1
            continue
        n += 1
        bad += q.get('bytes') != lens[0]
    return bad, f'{bad} of {n} forwards are not the bare report ({skipped} without an upload PDU)'


def m_ma_review(D):
    g, _ = cases(D)
    err = 0.0
    det = []
    for cid, st in g.items():
        if 'decision' not in st or 'corroborated' not in st:
            continue
        d = (first_t(st['decision']) - first_t(st['corroborated'])) / NS
        mode = st['decision'][0].get('detail', {}).get('mode')
        e = abs(d - REVIEW_S) if mode == 'manual-review' else 1e9
        err = max(err, e)
        det.append(f'{cid} {d:.3f}s {mode}')
    return round(err, 3), f'{len(det)} cases: ' + '; '.join(det[:6])


def m_ma_stage_order(D):
    g, _ = cases(D)
    bad = 0
    for cid, st in g.items():
        ts = [(k, first_t(st[s])) for k, s in enumerate(CASE_ORDER) if s in st]
        bad += any(a[1] > b[1] for a, b in zip(ts, ts[1:]))
    return bad, f'{bad} of {len(g)} cases with stages out of order'


def _support(D, subj, t):
    rows = [r for r in D['marec'] if r['subject_cert'] == subj and r['t'] <= t]
    ok = [r for r in rows if not r.get('set_aside')]
    return rows, ok


def m_ma_rule(D):
    bad = 0
    det = []
    for d in D['mad']:
        if d.get('decision') != 'revoke':
            continue
        rows, ok = _support(D, d['subject'], d['t'])
        reps = {r['reporter_cert'] for r in ok}
        span = (max(r['evidence_end'] for r in ok) - min(r['evidence_end'] for r in ok)) / NS if ok else 0
        good = len(reps) >= MIN_REPORTERS and span >= MIN_SPAN_S
        bad += not good
        det.append(f"{d['subject'][:8]} reporters={len(reps)} span={span:.1f}s")
    return bad, f'{bad} of {len(det)} revoke decisions without the rule''s support: ' + '; '.join(det[:6])


def m_ma_set_aside(D):
    bad = n = 0
    for d in D['mad']:
        if d.get('decision') != 'revoke':
            continue
        n += 1
        rows, ok = _support(D, d['subject'], d['t'])
        bad += not ok
    aside = collections.Counter(r.get('set_aside') for r in D['marec'])
    return bad, f'{bad} of {n} decisions rest only on set-aside reports; ma.received by set_aside {dict(aside)}'


def m_ma_honest(D):
    bad = n = 0
    for d in D['mad']:
        if d.get('decision') != 'revoke':
            continue
        n += 1
        node = D['p2n'].get(d['subject'])
        bad += node not in D['attacker']
    return bad, f'{bad} of {n} revoke decisions about a device that is not an attacker ({len(D["attacker"])} attackers)'


def m_ma_home(D):
    g, meta = cases(D)
    bad = n = 0
    for cid, st in g.items():
        if 'decision' not in st:
            continue
        n += 1
        td = first_t(st['decision'])
        ids = [r['id'] for r in D['rev'] if r['stage'] == 'decision' and abs(r['t'] - td) <= 0.01 * NS]
        provs = {D['cred'].get(D['p2n'].get(i), {}).get('provider') for i in ids}
        bad += meta[cid]['provider'] not in provs
    return bad, f'{bad} of {n} decided cases not at the accused certificate''s provider'


def m_la_paths(D):
    g, meta = cases(D)
    msgs = collections.defaultdict(list)
    for q in D['flow']:
        if q['kind'] == 'msg' and q['flow'] in ('linkage-resolution', 'crl-issuance'):
            msgs[q['instance']].append(q)
    appends = [q for q in D['flow'] if q['flow'] == 'crl-issuance' and q['kind'] == 'msg'
               and q['name'] == 'crl-append']
    bad = n = 0
    why = collections.Counter()
    for cid, st in g.items():
        if 'decision' not in st:
            continue
        n += 1
        prov = meta[cid]['provider']
        ms = msgs.get(meta[cid]['instance'], [])
        c = collections.Counter((q['name'], q['from'].split('/')[1], q['to'].split('/')[1]) for q in ms)
        want = {('pca-lookup-request', 'ma', 'aca'): 1, ('pca-lookup-response', 'aca', 'ma'): 1,
                ('blocklist-request', 'ma', 'ra'): 1, ('blocklist-response', 'ra', 'ma'): 1,
                ('blocklist-notice', 'ra', 'eca'): 1, ('seed-request', 'ma', 'la1'): 1,
                ('seed-request', 'ma', 'la2'): 1, ('seed-response', 'la1', 'ma'): 1,
                ('seed-response', 'la2', 'ma'): 1}
        ok = all(c.get(k, 0) == v for k, v in want.items())
        if not ok:
            why['messages'] += 1
        if any(not (q['from'].startswith(prov + '/') and q['to'].startswith(prov + '/')) for q in ms):
            ok = False
            why['cross-provider'] += 1
        if 'la-seeds' in st:
            tl = first_t(st['la-seeds'])
            if not any(q['from'] == f'{prov}/ma/0' and '/crlg/' in q['to'] and tl <= q['t'] <= tl + NS
                       for q in appends):
                ok = False
                why['no-append'] += 1
        bad += not ok
    return bad, f'{bad} of {n} decided cases off the specified path {dict(why)}'


def m_la_forward(D):
    g, _ = cases(D)
    bad = n = 0
    for cid, st in g.items():
        if 'la-seeds' not in st:
            continue
        n += 1
        ir = st['la-seeds'][0]['detail'].get('i_rev', 0)
        iss = [s.get('i') for r in st.get('reports-in', []) for s in (r['detail'].get('subjects') or [])]
        bad += ir == 0 or (iss and ir < min(iss))
    return bad, f'{bad} of {n} seed releases backward of the reported certificate'


def m_crl_slot(D):
    g, _ = cases(D)
    bad = n = 0
    det = []
    for cid, st in g.items():
        if 'crl-published' not in st or 'decision' not in st:
            continue
        n += 1
        tp, td = first_t(st['crl-published']), first_t(st['decision'])
        slot = next_slot(D, td)
        loc = to_local(D, tp)
        ok = slot is not None and abs(tp - slot) <= 2 * NS
        bad += not ok
        det.append(f'{cid} decided {to_local(D, td):%a %H:%M} published {loc:%a %Y-%m-%d %H:%M:%S %Z}')
    return bad, f'{bad} of {n}: ' + '; '.join(det[:4])


def m_crl_next(D):
    bad = set()
    seen = set()
    for r in D['sec']:
        for c in r['crls']:
            k = (c['craca'], c['issue_date'])
            seen.add(k)
            if c['next_crl'] - c['issue_date'] != WEEK_S or c['series'] != 1:
                bad.add(k)
    return len(bad), f'{len(bad)} of {len(seen)} held lists with nextCrl - issueDate != 7 d or series != 1'


def m_crl_entries(D):
    g, meta = cases(D)
    dec = collections.defaultdict(list)
    for cid, st in g.items():
        if 'decision' in st:
            dec[meta[cid]['provider']].append(first_t(st['decision']))
    mis = n = 0
    for r in D['rev']:
        if r['stage'] != 'published' or r.get('entries') is None:
            continue
        prov = D['cred'].get(D['p2n'].get(r['id']), {}).get('provider')
        exp = sum(1 for t in dec.get(prov, []) if t <= r['t'])
        n += 1
        mis += abs(r['entries'] - exp)
    issued = collections.Counter(q['provider'] for q in D['flow'] if q['flow'] == 'crl-issuance'
                                 and q['kind'] == 'stage' and q['name'] == 'issued')
    eng = D['p2'].get('revocations_by_provider') or {}
    for p in set(issued) | set(eng):
        mis += abs(eng.get(p, 0) - issued.get(p, 0))
    return mis, f'{n} published records; revocations_by_provider engine {dict(eng)} records {dict(issued)}'


def m_crl_rollback(D):
    last = {}
    bad = 0
    for r in sorted(D['sec'], key=lambda r: r['t']):
        for c in r['crls']:
            k = (r['node'], c['craca'], c['series'])
            v = (c['counter'], c['issue_date'])
            if k in last and v < last[k]:
                bad += 1
            last[k] = v
    return bad, f'{bad} rollbacks over {len(last)} (node, CRACA, series)'


def m_dist_late(D):
    late = n = 0
    worst = 0.0
    for P in publications(D):
        for r in D['rev']:
            if r['stage'] != 'enforced' or r['id'] not in P['ids'] or r.get('node') is None:
                continue
            node = r['node']
            if D['link'].get(node) != 'cellular':
                continue
            e = D['expo'].get(node)
            if e and e['joined'] > P['t']:
                continue
            n += 1
            d = (r['t'] - P['t']) / NS
            worst = max(worst, d)
            late += d > POLL_DEADLINE_S
    return late, f'{late} of {n} cellular installs later than {POLL_DEADLINE_S} s; latest {worst:.1f} s'


def _first_rows_after(D, t):
    out = {}
    for r in D['sec']:
        if r['t'] >= t and r['node'] not in out:
            out[r['node']] = r
    return out


def m_dist_unaccounted(D):
    bad = n = 0
    det = []
    for P in publications(D):
        if P['list'] is None:
            bad += 1
            det.append(f"no held list matches the {P['provider']} publication")
            continue
        craca, issue = P['list']
        # Corrected 2026-10-24: a relay-only vehicle has no fetch of its own, so it is
        # owed the list only once it received a unit's CRL frame after the publication
        # (the 7 first misses were relay-only vehicles frozen in the fast-forward, whose
        # first row is the one the fast-forward ends on, with no frame heard since).
        for node, r in _first_rows_after(D, P['t'] + POLL_DEADLINE_S * NS).items():
            if node in D['rsus']:
                continue
            if D['link'].get(node) != 'cellular':
                heard = [t for t, _, _ in D['crl_rx'].get(node, []) if P['t'] < t]
                if not heard:
                    continue
                later = [x for x in D['sec'] if x['node'] == node and x['t'] > min(heard)]
                if not later:
                    continue
                r = min(later, key=lambda x: x['t'])
            n += 1
            h = [c['issue_date'] for c in r['crls'] if c['craca'] == craca]
            if not h or max(h) < issue:
                bad += 1
                det.append(f"node {node} ({D['link'].get(node)}) at {r['t'] / NS:.0f}s")
    return bad, f'{bad} of {n} vehicles without the list: ' + '; '.join(det[:6])


def m_dist_relay_unheard(D):
    bad = n = 0
    rows = collections.defaultdict(list)
    for r in D['sec']:
        rows[r['node']].append(r)
    for node, rs in rows.items():
        if D['link'].get(node) == 'cellular' or node in D['rsus']:
            continue
        rs.sort(key=lambda r: r['t'])
        for a, b in zip(rs, rs[1:]):
            old = {(c['craca'], c['issue_date']) for c in a['crls']}
            new = {(c['craca'], c['issue_date']) for c in b['crls']} - old
            if not new:
                continue
            n += 1
            heard = any(a['t'] < t <= b['t'] for t, _, _ in D['crl_rx'].get(node, []))
            bad += not heard
    return bad, f'{bad} of {n} new lists at relay-only vehicles with no unit frame received since the row before'


def m_dist_rsu(D):
    bad = n = 0
    enf = {(r['node'], r['id']) for r in D['rev'] if r['stage'] == 'enforced'}
    for P in publications(D):
        rows = _first_rows_after(D, P['t'] + POLL_DEADLINE_S * NS)
        for u in D['rsus']:
            n += 1
            r = rows.get(u)
            held = r is not None and P['list'] is not None and any(
                c['craca'] == P['list'][0] and c['issue_date'] >= P['list'][1] for c in r['crls'])
            if not held and not any((u, i) in enf for i in P['ids']):
                bad += 1
    return bad, f'{bad} of {n} (unit, publication) pairs where the unit never holds the list'


def joiner_installs(D):
    """(node, revoked node) -> first row: a vehicle that joined after a publication holds
    the list from its first node.security row (R-JOINCRL), with no proto.revocation row."""
    out = {}
    pubs = [P for P in publications(D) if P['list']]
    first = {}
    for r in sorted(D['sec'], key=lambda r: r['t']):
        first.setdefault(r['node'], r)
    for node, r in first.items():
        for P in pubs:
            if r['t'] <= P['t']:
                continue
            if any(c['craca'] == P['list'][0] and c['issue_date'] >= P['list'][1] for c in r['crls']):
                # It fetched on its RA connection when it joined, before its first row.
                joined = (D['expo'].get(node) or {}).get('joined', r['t'])
                for i in P['ids']:
                    d = D['p2n'].get(i)
                    if d is not None:
                        out.setdefault((node, d), min(joined, r['t']))
    return out


def m_dist_engine(D):
    # Corrected 2026-10-24: crls_installed also counts the joiners' installs at join
    # (crls_installed_at_join), which have no proto.revocation row; they are counted from
    # each joiner's first node.security row.
    n = sum(1 for r in D['rev'] if r['stage'] == 'enforced' and r.get('node') is not None)
    j = len(joiner_installs(D))
    e = D['p2'].get('crls_installed', 0)
    ej = D['p2'].get('crls_installed_at_join', 0)
    return abs(e - n - j) + abs(ej - j), f'crls_installed engine {e} records {n} + {j} at join; crls_installed_at_join engine {ej}'


def m_enf_after(D):
    T = install_times(D)
    rp = revoked_pseudonyms(D)
    bad = n = 0
    for q in D['enf_rx']:
        k = (q['rx'], q['tx'])
        if k not in T or q['t'] <= T[k] + 0.01 * NS:
            continue
        if D['txp'].get((q['tx'], q['msg'])) not in rp.get(q['tx'], set()):
            continue
        if q['outcome'] in ('lost',) and q['cause'] not in ('revoked',):
            continue
        n += 1
        bad += q['outcome'] == 'delivered'
    return bad, f'{bad} of {n} receptions of a revoked sender after the receiver installed the list were accepted'


def m_enf_before(D):
    T = install_times(D)
    bad = 0
    n = 0
    rsu = 0
    for q in D['enf_rx']:
        if q['cause'] != 'revoked':
            continue
        if q['rx'] in D['rsus']:
            # No record shows a unit's install (AU.DIST.rsu, open): not judged here.
            rsu += 1
            continue
        n += 1
        k = (q['rx'], q['tx'])
        bad += k not in T or q['t'] < T[k]
    other = D['rx_revoked'] - n - rsu
    return bad + max(0, other), f'{bad} of {n} vehicle refusals before the receiver held the list; {other} refusals of senders never revoked; {rsu} unit refusals not judged'


def m_enf_self(D):
    own = {}
    for r in D['rev']:
        if r['stage'] == 'enforced' and r.get('node') is not None and D['p2n'].get(r['id']) == r['node']:
            own[r['node']] = min(own.get(r['node'], r['t']), r['t'])
    for r in sorted(D['sec'], key=lambda r: r['t']):
        if r['self_revoked'] and r['node'] not in own:
            own[r['node']] = r['t']
    # Corrected 2026-10-24: the rule binds a conforming device; an armed attacker runs its
    # own software and keeps sending (run.rs, own-revocation check), and the fleet is
    # protected by every receiver's CRL check (AU.ENF.drop). Judged on honest devices.
    bad = 0
    att = 0
    for n, T in own.items():
        k = sum(1 for t, _ in D['tx_by_node'].get(n, []) if t > T + 0.2 * NS)
        if n in D['attacker']:
            att += k
        else:
            bad += k
    return bad, f'{bad} BSMs sent by honest self-revoked devices after they found themselves on the list; {att} by {sum(1 for n in own if n in D["attacker"])} revoked attackers (refused by receivers)'


def m_enf_engine(D):
    p = D['p2']
    pairs = [('revoked_receptions', D['rx_revoked']), ('rsu_revoked_receptions', D['rx_revoked_rsu'])]
    return sum(abs(p.get(k, 0) - v) for k, v in pairs), '; '.join(
        f'{k} engine {p.get(k)} records {v}' for k, v in pairs)


def m_met_stages(D):
    first = collections.defaultdict(dict)
    for r in D['rev']:
        first[r['id']].setdefault(r['stage'], r['t'])
    want = collections.defaultdict(list)
    for i, st in first.items():
        em = [(STAGE_ORDER.index(s), s, t) for s, t in st.items() if s in STAGE_ORDER]
        em.sort()
        for (_, a, ta), (_, b, tb) in zip(em, em[1:]):
            if tb >= ta:
                want[f'{a}->{b}'].append((tb - ta) / NS)
    got = collections.defaultdict(lambda: [0, 0.0])
    for s in D['metrics'].get('revocation_latency_stage', []):
        v = s['value']
        if v.get('n'):
            g = got[s['dims'].get('stage')]
            g[0] += v['n']
            g[1] += v['mean'] * v['n']
    mis = 0
    det = []
    for k in sorted(set(want) | set(got)):
        n, sm = len(want.get(k, [])), sum(want.get(k, []))
        gn, gs = got[k] if k in got else (0, 0.0)
        if n != gn or abs(sm - gs) > 0.001 * max(n, 1) + 1e-6:
            mis += 1
            det.append(f'{k}: records n={n} sum={sm:.3f}, metric n={gn} sum={gs:.3f}')
    return mis, f'{len(want)} transitions; ' + ('; '.join(det[:6]) or 'all equal')


def m_met_crl(D):
    rec_e = rec_b = None
    for r in D['rev']:
        if r.get('entries') is not None:
            rec_e = r['entries']
        if r.get('size_bytes') is not None:
            rec_b = r['size_bytes']
    def last(name):
        s = D['metrics'].get(name, [])
        return s[-1]['value'].get('count') if s else None
    me, mb = last('crl_entries'), last('crl_bytes')
    mis = (me != rec_e) + (mb != rec_b)
    return mis, f'crl_entries metric {me} records {rec_e}; crl_bytes metric {mb} records {rec_b}'


def m_met_detection(D):
    subj = D['subj_actor']
    att = set(D['onset'])
    reported = {r['subject'] for r in D['mar']}
    revoked_subj = {d['subject'] for d in D['mad'] if d.get('decision') == 'revoke'}
    cells = {}
    for lvl in ('report', 'vehicle'):
        tp = fp = fn = tn = 0
        if lvl == 'report':
            pop = [(s, a) for s, a in subj.items()]
            pred = lambda s: s in reported
        else:
            acts = collections.defaultdict(set)
            for s, a in subj.items():
                acts[a].add(s)
            pop = [(ss, a) for a, ss in acts.items()]
            pred = lambda ss: bool(ss & revoked_subj)
        for s, a in pop:
            p, tr = pred(s), a in att
            tp += p and tr
            fp += p and not tr
            fn += (not p) and tr
            tn += (not p) and not tr
        cells[lvl] = {'tp': tp, 'fp': fp, 'fn': fn, 'tn': tn}
    mis = 0
    det = []
    for name, cell in (('det_tp', 'tp'), ('det_fp', 'fp'), ('det_fn', 'fn'), ('det_tn', 'tn')):
        for lvl in ('report', 'vehicle'):
            s = [x for x in D['metrics'].get(name, []) if x['dims'].get('level') == lvl]
            got = s[-1]['value'].get('count') if s else None
            if got != cells[lvl][cell]:
                mis += 1
                det.append(f'{name}/{lvl} metric {got} records {cells[lvl][cell]}')
    for lvl in ('report', 'vehicle'):
        s = [x for x in D['metrics'].get('false_accusations', []) if x['dims'].get('level') == lvl]
        got = s[-1]['value'].get('count') if s else None
        if got != cells[lvl]['fp']:
            mis += 1
            det.append(f'false_accusations/{lvl} metric {got} records {cells[lvl]["fp"]}')
    # Latencies: onset -> first report about the attacker at the MA / its revoke decision.
    acts = collections.defaultdict(set)
    for s, a in subj.items():
        acts[a].add(s)
    for name, rows in (('time_to_detect', D['mar']), ('time_to_decision',
                       [d for d in D['mad'] if d.get('decision') == 'revoke'])):
        lat = []
        for a, t0 in D['onset'].items():
            ts = [r['t'] for r in rows if r['subject'] in acts.get(a, ()) and r['t'] >= t0]
            if ts:
                lat.append((min(ts) - t0) / NS)
        # Corrected 2026-10-24: the latencies are cumulative over the run (each window
        # repeats the run so far), so the last sample is the run's.
        s = [x for x in D['metrics'].get(name, []) if x['value'].get('n')]
        gn = s[-1]['value']['n'] if s else 0
        gs = s[-1]['value']['n'] * s[-1]['value']['mean'] if s else 0.0
        if gn != len(lat) or abs(gs - sum(lat)) > 0.001 * max(1, len(lat)):
            mis += 1
            det.append(f'{name} metric n={gn} sum={gs:.3f} records n={len(lat)} sum={sum(lat):.3f}')
    return mis, f'cells {cells}; ' + ('; '.join(det[:8]) or 'all equal')


def digest(run_dir):
    from mcap.reader import make_reader
    h = hashlib.sha256()
    with open(os.path.join(run_dir, 'recording.mcap'), 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            h.update(ch.topic.encode())
            h.update(m.log_time.to_bytes(8, 'little'))
            h.update(m.data)
    def scrub(x):
        if isinstance(x, dict):
            return {k: scrub(v) for k, v in x.items()
                    if not re.search(r'wall|elapsed|rss|peak_mem|cpu_s|host|build_utc', k)}
        if isinstance(x, list):
            return [scrub(v) for v in x]
        return x
    h.update(json.dumps(scrub(json.load(open(os.path.join(run_dir, 'run-report.json')))),
                        sort_keys=True).encode())
    with open(os.path.join(run_dir, 'metrics.json'), 'rb') as f:
        for blk in iter(lambda: f.read(1 << 20), b''):
            h.update(blk)
    return h.hexdigest()


MEASURES = {
    'x.det.unheard_reports': m_det_unheard, 'x.det.non_us_subtypes': m_det_subtypes,
    'x.det.min_pair_gap_s': m_det_rate, 'x.up.posts_not_one_report': m_up_posts,
    'x.store.unbalanced': m_store_unbalanced, 'x.store.initial_weight_not_1': m_store_weight,
    'x.store.order_violations': m_store_order, 'x.store.engine_mismatch': m_store_engine,
    'x.up.wrong_link': m_up_link, 'x.up.relay_unheard': m_up_relay_unheard,
    'x.up.engine_mismatch': m_up_engine, 'x.up.cell_store_to_upload_p95_s': m_up_cell_latency,
    'x.ra.lost_or_duplicated': m_ra_once, 'x.ra.batch_close_error_s': m_ra_window,
    'x.ra.max_order_tau': m_ra_tau, 'x.ra.forward_not_inner': m_ra_strip,
    'x.ma.review_error_s': m_ma_review, 'x.ma.stage_order_violations': m_ma_stage_order,
    'x.ma.unsupported_decisions': m_ma_rule, 'x.ma.decided_on_set_aside': m_ma_set_aside,
    'x.ma.honest_decisions': m_ma_honest, 'x.ma.foreign_cases': m_ma_home,
    'x.la.bad_paths': m_la_paths, 'x.la.backward_seeds': m_la_forward,
    'x.crl.slot_violations': m_crl_slot, 'x.crl.bad_next_or_series': m_crl_next,
    'x.crl.entries_mismatch': m_crl_entries, 'x.crl.rollbacks': m_crl_rollback,
    'x.dist.late_installs': m_dist_late, 'x.dist.unaccounted': m_dist_unaccounted,
    'x.dist.relay_install_unheard': m_dist_relay_unheard, 'x.dist.rsus_without_list': m_dist_rsu,
    'x.dist.engine_mismatch': m_dist_engine, 'x.enf.accepted_after_install': m_enf_after,
    'x.enf.refused_before_install': m_enf_before, 'x.enf.tx_after_self_revoked': m_enf_self,
    'x.enf.engine_mismatch': m_enf_engine, 'x.met.stage_mismatch': m_met_stages,
    'x.met.crl_mismatch': m_met_crl, 'x.met.detection_mismatch': m_met_detection,
}


# ------------------------------------------------------------------ faults
def _cp(D, key):
    D = dict(D)
    D[key] = list(D[key]) if isinstance(D[key], list) else dict(D[key])
    return D


def _first(xs, pred):
    for i, x in enumerate(xs):
        if pred(x):
            return i
    return None


def _set(D, key, pred, fn):
    D = _cp(D, key)
    i = _first(D[key], pred)
    if i is None:
        return D
    x = json.loads(json.dumps(D[key][i]))
    fn(x)
    D[key][i] = x
    return D


def f_engine(k):
    def f(D):
        D = dict(D)
        D['p2'] = dict(D['p2'])
        D['p2'][k] = D['p2'].get(k, 0) + 1
        return D
    return f


def f_report_twice(D):
    D = _cp(D, 'fates')
    i = _first(D['fates'], lambda f: f['fate'] == 'stored')
    base, at = D['fates'][i]['report_id'].rsplit('-', 1)
    x = dict(D['fates'][i], t=D['fates'][i]['t'] + NS // 10, report_id=f'{base}-{int(at) + NS // 10}')
    D['fates'].insert(i + 1, x)
    return D


def f_post_two(D):
    def g(q):
        q['pdus'] = (q.get('pdus') or []) * 2 or [{'type': 'SecuredMisbehaviorReport', 'len': 1}] * 2
    return _set(D, 'flow', lambda q: q['flow'] == 'report' and q['kind'] == 'msg' and q['name'] == 'report', g)


def f_drop_terminal(D):
    D = _cp(D, 'fates')
    i = _first(D['fates'], lambda f: f['fate'] in TERMINAL)
    del D['fates'][i]
    return D


def f_upload_oldest(D):
    D = _cp(D, 'fates')
    base = dict(D['fates'][0])
    for rid, t, fate in (('fx-a', 10, 'stored'), ('fx-b', 20, 'stored'), ('fx-a', 30, 'uploaded'),
                         ('fx-b', 40, 'uploaded')):
        D['fates'].append(dict(base, reporter=999_999, report_id=rid, t=t * NS, fate=fate, weight=1.0))
    return D


def f_wrong_link(D):
    def g(q):
        n = D['dev2node'].get(q['from'])
        q['transport'] = 'rsu-relay' if D['link'].get(n) == 'cellular' else 'cellular-uu'
    return _set(D, 'flow', lambda q: q['flow'] == 'report' and q['kind'] == 'msg'
                and q['name'] == 'report' and (q.get('from') or '').startswith('ee/'), g)


def f_relay_rx_dropped(D):
    D = dict(D)
    D['relay_rx'] = {}
    return D


def f_upload_late(D):
    D = _cp(D, 'fates')
    D['fates'] = [dict(f, t=f['t'] + 5 * NS) if f['fate'] == 'uploaded' else f for f in D['fates']]
    return D


def f_forward_dup(D):
    D = _cp(D, 'flow')
    i = _first(D['flow'], lambda q: q['flow'] == 'report' and q['kind'] == 'msg' and q['name'] == 'report-forward')
    D['flow'].insert(i + 1, dict(D['flow'][i]))
    return D


def f_forward_early(D):
    D = _cp(D, 'flow')
    b = batches(D['flow'])[0]
    for i, q in b['members']:
        D['flow'][i] = dict(q, t=q['t'] - 100 * NS)
    return D


def f_forward_in_order(D):
    D = _cp(D, 'flow')
    up = uploads_to_ra(D['flow'])
    b = max(batches(D['flow']), key=lambda b: len(b['members']))
    idx = [i for i, _ in b['members']]
    qs = sorted((q for _, q in b['members']), key=lambda q: up.get(q['instance'], {'t': 0})['t'])
    for i, q in zip(idx, qs):
        D['flow'][i] = q
    return D


def f_forward_wrapper(D):
    up = uploads_to_ra(D['flow'])
    def g(q):
        q['bytes'] = up[q['instance']]['bytes']
    return _set(D, 'flow', lambda q: q['flow'] == 'report' and q['kind'] == 'msg'
                and q['name'] == 'report-forward' and q['instance'] in up, g)


def f_case_stage(stage, dt_s):
    def f(D):
        return _set(D, 'case', lambda q: q['stage'] == stage,
                    lambda q: q.__setitem__('t', q['t'] + int(dt_s * NS)))
    return f


def f_case_swap(D):
    g, _ = cases(D)
    cid = next(c for c, st in g.items() if 'decision' in st)
    td = first_t(g[cid]['decision'])
    return _set(D, 'case', lambda q: q['case'] == cid and q['stage'] == 'reports-in',
                lambda q: q.__setitem__('t', td + NS))


def _first_decided(D):
    return next(d['subject'] for d in D['mad'] if d.get('decision') == 'revoke')


def f_decision_unsupported(D):
    s = _first_decided(D)
    D = _cp(D, 'marec')
    keep = _first(D['marec'], lambda r: r['subject_cert'] == s)
    D['marec'] = [r for i, r in enumerate(D['marec']) if r['subject_cert'] != s or i == keep]
    return D


def f_set_aside(D):
    s = _first_decided(D)
    D = _cp(D, 'marec')
    D['marec'] = [dict(r, set_aside='evidence-unverifiable') if r['subject_cert'] == s else r
                  for r in D['marec']]
    return D


def f_decision_honest(D):
    honest = next(p for p, n in D['p2n'].items() if n not in D['attacker'] and n in D['cred'])
    return _set(D, 'mad', lambda d: d.get('decision') == 'revoke', lambda d: d.__setitem__('subject', honest))


def f_case_foreign(D):
    g, meta = cases(D)
    cid = next(c for c, st in g.items() if 'decision' in st)
    other = {'provider-a': 'provider-b', 'provider-b': 'provider-a'}[meta[cid]['provider']]
    D = _cp(D, 'case')
    D['case'] = [dict(q, provider=other) if q['case'] == cid else q for q in D['case']]
    return D


def f_seed_one_la(D):
    D = _cp(D, 'flow')
    i = _first(D['flow'], lambda q: q['kind'] == 'msg' and q['name'] == 'seed-request' and '/la2/' in q['to'])
    del D['flow'][i]
    return D


def f_irev_zero(D):
    return _set(D, 'case', lambda q: q['stage'] == 'la-seeds',
                lambda q: q['detail'].__setitem__('i_rev', 0))


def f_held(fn, pred=lambda r: bool(r['crls'])):
    def f(D):
        return _set(D, 'sec', pred, fn)
    return f


def f_held_rollback(D):
    D = _cp(D, 'sec')
    i = max((k for k, r in enumerate(D['sec']) if r['crls']), key=lambda k: D['sec'][k]['t'])
    r = json.loads(json.dumps(D['sec'][i]))
    r['t'] += NS
    for c in r['crls']:
        c['counter'] -= 1
        c['issue_date'] -= WEEK_S
    D['sec'].append(r)
    return D


def f_published_entries(D):
    return _set(D, 'rev', lambda r: r['stage'] == 'published' and r.get('entries') is not None,
                lambda r: r.__setitem__('entries', r['entries'] + 1))


def f_install_late(D):
    return _set(D, 'rev', lambda r: r['stage'] == 'enforced' and r.get('node') is not None
                and D['link'].get(r['node']) == 'cellular',
                lambda r: r.__setitem__('t', r['t'] + 4000 * NS))


def f_held_stale(D):
    P = publications(D)[0]
    craca, issue = P['list']
    rows = _first_rows_after(D, P['t'] + POLL_DEADLINE_S * NS)
    node = next(n for n in rows if n not in D['rsus'] and D['link'].get(n) == 'cellular')
    t = rows[node]['t']
    def g(r):
        for c in r['crls']:
            if c['craca'] == craca:
                c['issue_date'] = issue - 1
    return _set(D, 'sec', lambda r: r['node'] == node and r['t'] == t, g)


def f_relay_crl_dropped(D):
    D = dict(D)
    D['crl_rx'] = {}
    return D


def f_rsu_dropped(D):
    D = dict(D)
    rs = set(D['rsus'])
    D['sec'] = [r for r in D['sec'] if r['node'] not in rs]
    D['rev'] = [r for r in D['rev'] if r.get('node') not in rs]
    return D


def f_rx_accepted(D):
    T = install_times(D)
    return _set(D, 'enf_rx', lambda q: q['cause'] == 'revoked' and (q['rx'], q['tx']) in T,
                lambda q: q.update(outcome='delivered', cause=None))


def f_rx_early(D):
    D = _cp(D, 'enf_rx')
    T = install_times(D)
    q = next((q for q in D['enf_rx'] if (q['rx'], q['tx']) in T), None)
    if q is None:
        q = {'t': 0, 'rx': -1, 'tx': next(iter(D['revoked_nodes'])), 'msg': -1}
        D['enf_rx'].append(dict(q, outcome='lost', cause='revoked', verification=None))
    else:
        D['enf_rx'].append(dict(q, t=T[(q['rx'], q['tx'])] - 10 * NS, outcome='lost', cause='revoked'))
    D['rx_revoked'] = D['rx_revoked'] + 1
    return D


def f_tx_after(D):
    # An honest device marked self-revoked half-way through its BSMs.
    n = next(k for k, v in D['tx_by_node'].items() if k not in D['attacker'] and k in D['cred'] and len(v) > 10)
    tx = D['tx_by_node'][n]
    D = _cp(D, 'sec')
    D['sec'].insert(0, {'t': tx[len(tx) // 2][0], 'node': n, 'link': None, 'self_revoked': True, 'crls': []})
    return D


def f_metric(name, field='count'):
    def f(D):
        D = dict(D)
        D['metrics'] = dict(D['metrics'])
        s = list(D['metrics'].get(name, []))
        k = len(s) - 1 if field == 'count' else _first(s, lambda x: x['value'].get('n'))
        x = json.loads(json.dumps(s[k]))
        x['value'][field] = x['value'].get(field, 0) + 1
        s[k] = x
        D['metrics'][name] = s
        return D
    return f


def f_det(D):
    D = dict(D)
    D['metrics'] = dict(D['metrics'])
    s = list(D['metrics']['det_tp'])
    k = max(i for i, x in enumerate(s) if x['dims'].get('level') == 'report')
    x = json.loads(json.dumps(s[k]))
    x['value']['count'] += 1
    s[k] = x
    D['metrics']['det_tp'] = s
    return D


def f_subject_unheard(D):
    return _set(D, 'fates', lambda f: f['fate'] == 'stored', lambda f: f.__setitem__('subject', '00' * 8))


def f_subtype(D):
    def g(q):
        q['detail']['evidence'][0]['subtypes'][0]['name'] = 'Distance'
    return _set(D, 'case', lambda q: q['stage'] == 'reports-in' and q['detail'].get('evidence'), g)


FAULTS = {
    'report-subject-unheard': f_subject_unheard, 'case-subtype-obs': f_subtype,
    'report-twice': f_report_twice, 'post-two-pdus': f_post_two,
    'fate-drop-terminal': f_drop_terminal,
    'fate-weight': lambda D: _set(D, 'fates', lambda f: f['fate'] == 'stored', lambda f: f.__setitem__('weight', 0.9)),
    'upload-oldest-first': f_upload_oldest, 'engine-evicted': f_engine('reports_evicted'),
    'upload-wrong-link': f_wrong_link, 'relay-rx-dropped': f_relay_rx_dropped,
    'engine-uploaded': f_engine('reports_uploaded_cellular'), 'upload-late': f_upload_late,
    'forward-duplicate': f_forward_dup, 'forward-early': f_forward_early,
    'forward-in-order': f_forward_in_order, 'forward-wrapper': f_forward_wrapper,
    'case-review-short': f_case_stage('decision', -3600), 'case-stage-swap': f_case_swap,
    'decision-unsupported': f_decision_unsupported, 'received-set-aside': f_set_aside,
    'decision-honest': f_decision_honest, 'case-foreign': f_case_foreign,
    'seed-one-la': f_seed_one_la, 'seed-irev-zero': f_irev_zero,
    'publish-saturday': f_case_stage('crl-published', 86_400),
    'held-next-6d': f_held(lambda r: r['crls'][0].__setitem__('next_crl', r['crls'][0]['next_crl'] - 86_400)),
    'published-entries': f_published_entries, 'held-rollback': f_held_rollback,
    'install-late': f_install_late, 'held-stale': f_held_stale,
    'relay-crl-rx-dropped': f_relay_crl_dropped, 'rsu-list-dropped': f_rsu_dropped,
    'engine-installed': f_engine('crls_installed'), 'rx-revoked-accepted': f_rx_accepted,
    'rx-revoked-early': f_rx_early, 'tx-after-revoked': f_tx_after,
    'engine-revoked': f_engine('revoked_receptions'),
    'metric-stage': f_metric('revocation_latency_stage', 'n'),
    'metric-crl': f_metric('crl_entries'), 'metric-det': f_det,
}


# ------------------------------------------------------------------ judging
def judge(row, v):
    e = row['expect']
    if e == 'eq':
        return v == row['value']
    if e == 'le':
        return v <= row['value']
    if e == 'ge':
        return v >= row['value']
    if e == 'band':
        return row['band'][0] <= v <= row['band'][1]
    raise SystemExit(f'unknown expect {e}')


def evaluate(row, runs_dir, cache, fault=None):
    if row['measure'] == 'x.det.digest_differs':
        a = cache.setdefault(('digest', 'AUD'), digest(os.path.join(runs_dir, 'AUD')))
        b = cache.setdefault(('digest', row['run']), digest(os.path.join(runs_dir, row['run'])))
        if fault:
            b = b[:-1] + ('0' if b[-1] != '0' else '1')
        return int(a != b), f'AUD {a[:16]} {row["run"]} {b[:16]}'
    D = cache.get(row['run'])
    if D is None:
        D = cache[row['run']] = load(os.path.join(runs_dir, row['run']))
    if fault:
        D = FAULTS[fault](D)
    return MEASURES[row['measure']](D)


def _fill(path, results):
    text = open(path).read()
    out = []
    cur = None
    for line in text.split('\n'):
        m = re.match(r'- id: (\S+)', line)
        if m:
            cur = m.group(1)
        if cur in results and line.startswith('  measured:'):
            v, ok = results[cur]
            line = f'  measured: {json.dumps(v)}'
            out.append(line)
            continue
        if cur in results and line.startswith('  verdict:'):
            if line.split(':', 1)[1].strip() in ('pass', 'FAIL', 'pending'):
                line = f"  verdict: {'pass' if results[cur][1] else 'FAIL'}"
            results[cur] = (results[cur][0], None)   # a human verdict is kept as it is
        out.append(line)
    text = '\n'.join(out)
    # Rows that had no verdict line get one after `measured`.
    for rid, (v, ok) in results.items():
        if ok is None:
            continue
        text = re.sub(rf'(- id: {re.escape(rid)}\n(?:  .*\n)*?  measured: .*)\n',
                      lambda m: m.group(1) + f"\n  verdict: {'pass' if ok else 'FAIL'}\n", text, count=1)
    open(path, 'w').write(text)


def main(argv):
    import yaml
    if argv[1] == 'hops':
        return hops(load(argv[2]))
    table = yaml.safe_load(open(argv[2]))
    runs_dir = argv[3]
    cache = {}
    if argv[1] == 'check':
        results = {}
        fails = 0
        for row in table['rows']:
            v, det = evaluate(row, runs_dir, cache)
            ok = judge(row, v)
            fails += not ok
            human = row.get('verdict') in ('fixed', 'wrong-expectation', 'open')
            print(f"{'PASS' if ok else 'FAIL'} {row['id']} {row['measure']} = {v} ({det})"
                  + (f" [{row['verdict']}]" if human else ''))
            results[row['id']] = (v, ok)
        print(f'{len(table["rows"]) - fails} pass, {fails} fail')
        if '--fill' in argv:
            _fill(argv[2], results)
        return 1 if fails else 0
    if argv[1] == 'selftest':
        bad = 0
        for row in table['rows']:
            if not row.get('fault'):
                print(f'NO FAULT {row["id"]}')
                bad += 1
                continue
            try:
                v, det = evaluate(row, runs_dir, cache, fault=row['fault'])
                red = not judge(row, v)
            except Exception as e:      # a fault that cannot even be placed is not a proof
                red, v, det = False, None, f'fault not placed: {e!r}'
            bad += not red
            print(f"{'red' if red else 'NOT RED'} {row['id']} under {row['fault']}: {v} ({det[:160]})")
        print(f'{len(table["rows"]) - bad} of {len(table["rows"])} rows go red under their fault')
        return 1 if bad else 0
    raise SystemExit(__doc__)


def hops(D):
    """Count, loss and latency of each hop, from the records."""
    fl = D['flow']
    st = {f['report_id']: f for f in D['fates'] if f['fate'] == 'stored'}
    up = {f['report_id']: f for f in D['fates'] if f['fate'] == 'uploaded'}
    dl = {f['report_id']: f for f in D['fates'] if f['fate'] == 'delivered'}
    ra = uploads_to_ra(fl)
    fw = {q['instance']: q for _, q in forwards(fl)}
    rcv = {}
    for q in fl:
        if q['flow'] == 'report' and q['kind'] == 'stage' and q['name'] == 'report_received':
            rcv.setdefault(q['instance'], q['t'])
    def lat(xs):
        xs = sorted(xs)
        if not xs:
            return 'n=0'
        return f'n={len(xs)} median={xs[len(xs) // 2]:.3f}s p95={p95(xs):.3f}s max={xs[-1]:.3f}s'
    print('fates', dict(collections.Counter(f['fate'] for f in D['fates'])))
    for kind in ('cellular', 'rsu-relay'):
        print(f'stored->uploaded [{kind}]', lat([(up[k]['t'] - s['t']) / NS for k, s in st.items()
              if k in up and D['link'].get(s['reporter']) == kind]))
    print('uploaded->delivered (MA)', lat([(dl[k]['t'] - up[k]['t']) / NS for k in dl if k in up]))
    print('RA upload->forward', lat([(fw[k]['t'] - q['t']) / NS for k, q in ra.items() if k in fw]))
    print('forward->MA received', lat([(rcv[k] - fw[k]['t']) / NS for k in fw if k in rcv]))
    g, _ = cases(D)
    for a, b in zip(CASE_ORDER, CASE_ORDER[1:]):
        print(f'case {a}->{b}', lat([(first_t(s[b]) - first_t(s[a])) / NS for s in g.values() if a in s and b in s]))
    for P in publications(D):
        ins = sorted((r['t'] - P['t']) / NS for r in D['rev'] if r['stage'] == 'enforced' and r['id'] in P['ids'])
        print(f"publication {P['provider']} at {P['t'] / NS:.0f}s list {P['list']} ids {len(P['ids'])}: installs", lat(ins))
    print('rx census (non-bsm or refused)', {k: v for k, v in D['rx_census'].items() if k[0] != 'bsm' or k[2] in ('revoked',)})
    print('tx types', dict(D['tx_types']))
    print('links', dict(collections.Counter(D['link'].values())), 'rsus', D['rsus'],
          'attackers', sorted(D['attacker']), 'revoked', sorted(D['revoked_nodes']))
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
