#!/usr/bin/env python3
"""On-air security audit (db1-3): an independent reader of a run's raw records.

    onair.py measure <run-dir> [<out.json>]   the oa.* measures -> <run-dir>/realism.json
    onair.py faults <run-dir> <table.yaml>    re-measures under one injected fault per row
                                              and prints whether each row goes red

It reads only `recording.mcap` (node.tx, node.rx, node.verify, node.p2pcd,
node.security, node.credentials, sec.pseudonym, privacy.link), `metrics.json` and
`run-report.json`, and never imports engine code. The `oa.mi.*` and `oa.rep.*` measures
compare the engine's own metric samples and run report against the recount; every other
measure is a property of the records against the cited specification value
(crates/v2xw-engine/tests/expectations/scms-audit-db1-3.yaml).
"""

import collections
import hashlib
import json
import math
import os
import sys

NS = 1_000_000_000
WINDOW = 10 * NS
CERT_ATTACH_NS = 1_000 * 1_000_000  # the scenario's full_cert_every_ms
SIGSEQ_PREFIX = 2  # COER SequenceOfCertificate quantity: length determinant + 1-octet count
HASHEDID8 = 8


def records(path, topics, time_order=False):
    from mcap.reader import make_reader
    with open(path, 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages(log_time_order=time_order):
            t = ch.topic.split('/', 1)[-1]
            if t in topics:
                v = json.loads(m.data)
                v['_lt'] = m.log_time
                yield t, v


def kind_of(sent):
    if 'bsm' in sent:
        return 'veh'
    if 'psm' in sent:
        return 'vru'
    return 'rsu'


def pct(xs, q):
    if not xs:
        return None
    xs = sorted(xs)
    k = (len(xs) - 1) * q
    lo = math.floor(k)
    hi = math.ceil(k)
    return xs[lo] + (xs[hi] - xs[lo]) * (k - lo)


def win(t):
    """The metric window a time falls in, named by its end (samples are stamped there)."""
    return (t // WINDOW + 1) * WINDOW


def measure(run_dir, fault=None):
    rec = os.path.join(run_dir, 'recording.mcap')
    rep = json.load(open(os.path.join(run_dir, 'run-report.json')))['report']
    metrics = json.load(open(os.path.join(run_dir, 'metrics.json')))['samples']
    out = {}
    detail = {}

    # ------------------------------------------------------------ pass 1: small channels
    tx = {}  # (node, msg) -> (pseudonym, signer, msg_type)
    sent = collections.defaultdict(set)
    bsm = []
    p2pcd_frames = []
    p2pcd_rows = []
    provider = {}
    sec_rows = collections.defaultdict(list)
    sec_pseudo = []
    links = []
    alltx = []
    tel_w = collections.Counter()
    for t, v in records(rec, {'node.tx', 'node.p2pcd', 'node.credentials', 'node.security',
                              'sec.pseudonym', 'privacy.link', 'node.telemetry'}):
        if t == 'node.telemetry':
            tel_w[win(v['_lt'])] += v.get('p2pcd_requests') or 0
        elif t == 'node.tx':
            alltx.append((v['_lt'], v.get('signer'), v.get('envelope_bytes'), v.get('payload_bytes')))
            sent[v['node']].add(v['msg_type'])
            tx[(v['node'], v['msg'])] = (v.get('pseudonym'), v.get('signer'), v['msg_type'])
            if v['msg_type'] in ('bsm', 'psm'):
                bsm.append(v)
            elif v['msg_type'] == 'p2pcd':
                p2pcd_frames.append(v)
        elif t == 'node.p2pcd':
            p2pcd_rows.append(v)
        elif t == 'node.credentials':
            provider[v['node']] = v.get('provider')
        elif t == 'node.security':
            sec_rows[v['node']].append(v['t'])
        elif t == 'sec.pseudonym':
            sec_pseudo.append(v)
        elif t == 'privacy.link':
            links.append(v)
    kind = {n: kind_of(s) for n, s in sent.items()}
    if fault == 'cert_bytes':
        next(b for b in bsm if b['signer'] == 'certificate' and b['msg_type'] == 'bsm')['cert_bytes'] += 1
    if fault == 'envelope':
        next(b for b in bsm if b['signer'] == 'digest')['envelope_bytes'] += 1
    if fault == 'spdu':
        bsm[0]['bytes_on_wire'] += 1
    if fault == 'cadence':
        c = [b for b in bsm if b['signer'] == 'digest']
        c[len(c) // 2]['signer'] = 'certificate'
    if fault == 'temp_id':
        bsm[-1]['content']['temp_id'] = '00000000'
    if fault == 'first_digest':
        n0 = next(b['node'] for b in bsm if b['msg_type'] == 'bsm')
        min((b for b in bsm if b['node'] == n0), key=lambda b: b['t_generated'])['signer'] = 'digest'
    if fault == 'gap':
        for b in bsm:
            if b['msg'] % 2 == 0:
                b['signer'] = 'certificate'
    vnodes = sorted({b['node'] for b in bsm if b['msg_type'] == 'bsm'})
    if fault in ('change', 'age_early', 'age_late', 'reuse', 'msgcnt'):
        mine_b = collections.defaultdict(list)
        for b in bsm:
            mine_b[b['node']].append(b)
        for lst in mine_b.values():
            lst.sort(key=lambda b: b['t_generated'])
        if fault == 'change':
            lst = mine_b[vnodes[0]]
            for b in lst[len(lst) // 2:]:
                b['pseudonym'] = 'ff' * 8
                b['content']['temp_id'] = 'ff' * 4
        else:
            # a node with two changes or more; idx = its frames where the pseudonym changes
            for n in vnodes:
                lst = mine_b[n]
                idx = [i for i in range(1, len(lst)) if lst[i]['pseudonym'] != lst[i - 1]['pseudonym']]
                if len(idx) >= 2:
                    break
            i2 = idx[1]
            if fault == 'age_early':
                for b in lst[max(idx[0], i2 - 50):i2]:
                    b['pseudonym'] = lst[i2]['pseudonym']
                    b['content']['temp_id'] = lst[i2]['pseudonym'][:8]
            elif fault == 'age_late':
                for b in lst[i2:i2 + 50]:
                    b['pseudonym'] = lst[i2 - 1]['pseudonym']
                    b['content']['temp_id'] = lst[i2 - 1]['pseudonym'][:8]
            elif fault == 'reuse':
                b = dict(lst[-1])
                b['pseudonym'] = lst[0]['pseudonym']
                b['content'] = dict(b['content'], temp_id=lst[0]['pseudonym'][:8])
                b['t_generated'] += 1
                bsm.append(b)
            elif fault == 'msgcnt':
                for n in vnodes:
                    lst = mine_b[n]
                    for i in range(1, len(lst)):
                        if lst[i]['pseudonym'] != lst[i - 1]['pseudonym']:
                            lst[i]['content']['msg_count'] = ((lst[i - 1]['content'].get('msg_count') or 0) + 1) % 128
    if fault == 'nocert':
        tx = {k: (p, 'digest', m) for k, (p, s_, m) in tx.items()}

    # ------------------------------------------------------------ BSM signing on the air
    vbsm = [b for b in bsm if b['msg_type'] == 'bsm']
    out['oa.tx.bsm_frames'] = len(vbsm)
    out['oa.tx.cert_bytes_values'] = sorted({b['cert_bytes'] for b in vbsm
                                            if b['signer'] == 'certificate'})
    rest = collections.defaultdict(collections.Counter)
    spdu_bad = 0
    def length_determinant(n):
        # COER/X.696 §8.6: one octet below 128, else 1 + the octets of the length.
        return 1 if n < 128 else 1 + (n.bit_length() + 7) // 8

    for b in bsm:
        r = b['envelope_bytes'] - (b['cert_bytes'] + SIGSEQ_PREFIX if b['signer'] == 'certificate'
                                   else HASHEDID8) - length_determinant(b['payload_bytes'])
        rest[b['node']][r] += 1
        if b['spdu_bytes'] != b['payload_bytes'] + b['envelope_bytes']:
            spdu_bad += 1
        if b['bytes_on_wire'] != (b['spdu_bytes'] + b['net_header_bytes'] + b['link_bytes']
                                  + b['frag_header_bytes']):
            spdu_bad += 1
    out['oa.tx.envelope_accounting_violations'] = sum(sum(c.values()) - max(c.values())
                                                      for c in rest.values())
    out['oa.tx.spdu_accounting_violations'] = spdu_bad
    detail['envelope_rest'] = sorted({r for c in rest.values() for r in c})

    per = collections.defaultdict(list)  # node -> its BSM/PSM in generation order
    for b in bsm:
        per[b['node']].append(b)
    cad_bad = first_digest = tid_bad = 0
    gaps = []
    changes = []  # (node, t_first_new, t_first_old, last_cnt_old, first_cnt_new)
    reused = 0
    for n, lst in per.items():
        lst.sort(key=lambda b: (b['t_generated'], b['msg']))
        last_cert = {}
        first_t = {}
        order = []
        prev = None
        for b in lst:
            p = b['pseudonym']
            c = b.get('content') or {}
            if c.get('temp_id') is not None and c['temp_id'] != p[:8]:
                tid_bad += 1
            if p not in first_t:
                first_t[p] = b['t_generated']
                if b['signer'] != 'certificate':
                    first_digest += 1
                if order:
                    if p in order:
                        reused += 1
                    pc = prev.get('content') or {}
                    changes.append((n, b['t_generated'], first_t[order[-1]],
                                    pc.get('msg_count'), c.get('msg_count')))
                order.append(p)
            elif p != order[-1]:
                reused += 1  # back to an earlier pseudonym
            want = p not in last_cert or b['t_generated'] - last_cert[p] >= CERT_ATTACH_NS
            got = b['signer'] == 'certificate'
            if b['msg_type'] == 'bsm' and want != got:
                cad_bad += 1
            if got:
                if p in last_cert and b['msg_type'] == 'bsm':
                    gaps.append((b['t_generated'] - last_cert[p]) / 1e6)
                last_cert[p] = b['t_generated']
            prev = b
    out['oa.tx.cert_cadence_violations'] = cad_bad
    out['oa.tx.first_under_pseudonym_digest'] = first_digest
    out['oa.tx.cert_gap_p50_ms'] = pct(gaps, 0.5)
    out['oa.chg.temp_id_mismatch'] = tid_bad
    out['oa.chg.pseudonym_reused'] = reused
    vch = [c for c in changes if kind.get(c[0]) == 'veh']
    out['oa.chg.changes_tx'] = len(vch)
    # The age of a pseudonym: from the first frame under it to the first under the next;
    # a node's first pseudonym is left out (it was activated before its first frame).
    ages = []
    by_node = collections.defaultdict(list)
    for c in vch:
        by_node[c[0]].append(c)
    for n, cs in by_node.items():
        for a, b in zip(cs, cs[1:]):
            ages.append((b[1] - a[1]) / NS)
    out['oa.chg.age_min_s'] = min(ages) if ages else None
    out['oa.chg.age_max_s'] = max(ages) if ages else None
    out['oa.chg.ages_n'] = len(ages)
    cont = sum(1 for c in vch if c[3] is not None and c[4] == (c[3] + 1) % 128)
    out['oa.chg.msgcnt_continued_share'] = cont / len(vch) if vch else 0.0
    # sec.pseudonym rows against the changes on the air (vehicles that transmit again).
    sp_veh = [s for s in sec_pseudo if kind.get(s['node']) == 'veh']
    if fault == 'sec_pseudo' and sp_veh:
        sp_veh = sp_veh[1:]
    if fault == 'rate_hi':
        sp_veh = sp_veh + sp_veh
    # Every change on the air (a vehicle's pseudonym after its first) has its row, and the
    # new pseudonym of every row of a vehicle that transmits after it goes on the air. A
    # row whose old pseudonym never went on the air is a change made before the vehicle's
    # first frame (its pool was active before it joined the road): counted, not a fault.
    on_air = set()
    heard = collections.defaultdict(set)
    for n, lst in per.items():
        if kind.get(n) != 'veh':
            continue
        seq = []
        for b in lst:
            heard[n].add(b['pseudonym'])
            if not seq or seq[-1] != b['pseudonym']:
                seq.append(b['pseudonym'])
        on_air |= {(n, p) for p in seq[1:]}
    rows = {(s['node'], s.get('new_digest')) for s in sp_veh}
    missing = sum(1 for s in sp_veh if any(b['t_generated'] >= s['t'] for b in per.get(s['node'], []))
                  and s.get('new_digest') not in heard[s['node']])
    out['oa.chg.sec_vs_tx_diff'] = len(on_air - rows) + missing
    out['oa.chg.changes_before_first_frame'] = sum(1 for s in sp_veh
                                                   if s.get('old_digest') not in heard[s['node']])
    # Change rate: changes / vehicle-hours observed (node.security first..last row).
    veh_s = sum((max(ts) - min(ts)) / NS for n, ts in sec_rows.items() if kind.get(n) == 'veh')
    out['oa.chg.rate_per_veh_h'] = len(sp_veh) / (veh_s / 3600) if veh_s else 0.0

    # ------------------------------------------------------------ privacy and linkability
    owner = {}
    for (n, _), (p, _, _) in tx.items():
        if p:
            owner[p] = n
    if fault == 'link' and links:
        links.append(dict(links[0]))  # one decision the engine never made
    # A decision is across a change when the observer's previous decision about the
    # successor's vehicle (ground truth: who signed with it on the air) named another
    # pseudonym; a claim with no change behind it is spurious (the vehicle has just come
    # into hearing).
    last_heard = {}
    across = []
    for x in sorted(links, key=lambda x: x['t']):
        v = owner.get(x['successor'])
        across.append(v is not None and v in last_heard and last_heard[v] != x['successor'])
        if v is not None:
            last_heard[v] = x['successor']
    claims = [(x, a) for x, a in zip(sorted(links, key=lambda x: x['t']), across) if x['predecessor']]
    claimed = [x for x, _ in claims]
    correct = [x for x in claimed if owner.get(x['predecessor']) is not None
               and owner.get(x['predecessor']) == owner.get(x['successor'])]
    p2 = rep.get('phase2') or {}
    mine = {'privacy_link_decisions': len(links),
            'privacy_links_claimed': sum(1 for _, a in claims if a),
            'privacy_links_correct': len(correct),
            'privacy_links_spurious': sum(1 for _, a in claims if not a),
            'privacy_changes_heard': sum(across),
            'privacy_anonymity_set_sum': sum(x['anonymity_set_size'] for x in links),
            'pseudonym_changes': len(sec_pseudo)}
    detail['privacy'] = {k: (v, p2.get(k)) for k, v in mine.items()}
    out['oa.rep.privacy_diff'] = sum(abs(v - (p2.get(k) or 0)) for k, v in mine.items())
    out['oa.pv.links_claimed'] = len(claimed)
    out['oa.pv.links_correct'] = len(correct)

    # ------------------------------------------------------------ pass 2: rx and verify
    holds = set()  # (rx, pseudonym) whose certificate rx decoded and validated
    pairs_verified = set()
    no_cert_verified = cached_unverified = refused = stamp_bad = 0
    verify_tasks = collections.Counter()
    rx_tasks = []  # (rx, start, done, pseudonym, verdict, cause)
    deliv = collections.Counter()
    deliv_unv = collections.Counter()
    seen_cert = set()
    miss_w = collections.Counter()
    vrecs = []
    STAMPS = ('t_generated', 't_signed', 't_handoff', 't_tx_start', 't_tx_end', 't_arrival',
              't_rx_done', 't_verify_start', 't_verify_done', 't_delivered')
    benign_v = {'verified', 'unverified', 'unsecured', None}
    for t, v in records(rec, {'node.rx', 'node.verify'}, time_order=True):
        if t == 'node.verify':
            vrecs.append(v)
            continue
        if v['outcome'] == 'lost' and v['cause'] != 'unknown-signer' and v['t_verify_start'] is None:
            continue
        if fault == 'stamps' and stamp_bad == 0 and v.get('t_verify_done'):
            v['t_generated'] = v['t_verify_done'] + 1
        if fault == 'refused' and refused == 0 and v['verification'] == 'verified':
            v['verification'] = 'invalid'
        if fault == 'cached' and v['verification'] == 'verified' and (v['tx'], v['msg']) in tx \
                and tx[(v['tx'], v['msg'])][1] == 'digest' and v['msg'] % 50 == 0:
            v['verification'] = 'unverified'
        s = [v[k] for k in STAMPS if v.get(k) is not None]
        if any(a > b for a, b in zip(s, s[1:])):
            stamp_bad += 1
        p, signer, mt = tx.get((v['tx'], v['msg']), (None, None, None))
        ver = v['verification']
        if ver not in benign_v or (v['cause'] or '').startswith(('refused', 'invalid', 'revoked')):
            refused += 1
        if v['outcome'] == 'delivered':
            deliv[win(v['_lt'])] += 1
            if ver == 'unverified':
                deliv_unv[win(v['_lt'])] += 1
        key = (v['rx'], p)
        # The digest-miss count (the documented `p2pcd_requests` formula): a digest-signed
        # frame that reached the verifier stage from a signer whose certificate the receiver
        # had not been handed in an earlier frame.
        if p and (v['t_verify_start'] is not None or v['cause'] == 'unknown-signer'):
            if signer == 'certificate':
                seen_cert.add(key)
            elif signer == 'digest' and key not in seen_cert:
                miss_w[win(v['_lt'])] += 1
        if p and signer == 'digest':
            if ver == 'verified' and key not in holds:
                no_cert_verified += 1
            if key in holds and (ver == 'unverified' or v['cause'] == 'unknown-signer'):
                cached_unverified += 1
        if p and ver == 'verified':
            holds.add(key)
            first = key not in pairs_verified
            pairs_verified.add(key)
        else:
            first = None
        if v['t_verify_start'] is not None:
            rx_tasks.append((v['rx'], v['t_verify_start'], v['t_verify_done'], p, ver,
                             v['cause'], first))
    if fault == 'verify_join' and vrecs:
        vrecs.pop()
    out['oa.rx.verified_without_cert'] = no_cert_verified
    out['oa.rx.unverified_with_cert'] = cached_unverified
    out['oa.rx.refused'] = refused + sum((rep.get('receiver_validation') or {}).get('refused', {}).values())
    out['oa.rx.stamp_order_violations'] = stamp_bad
    rv = rep.get('receiver_validation') or {}
    ee = rv.get('ee_validations', 0)
    if fault == 'ee':
        ee += 1
    ee_pending = ee

    tasks = {}
    for v in vrecs:
        if v.get('t_start') is None:
            continue
        verify_tasks[(v['node'], v['t_start'], v['t_done'])] += 1
        tasks[(v['node'], v['t_start'], v['t_done'])] = v
    rxk = collections.Counter((r[0], r[1], r[2]) for r in rx_tasks)
    out['oa.rx.verify_join_unmatched'] = sum((rxk - verify_tasks).values()) + sum((verify_tasks - rxk).values())

    repeat = collections.defaultdict(collections.Counter)
    firsts = []
    unv = []
    # The kind's signature price: the commonest cost of a verified repeat (a cached signer).
    rep_all = collections.defaultdict(collections.Counter)
    for r in rx_tasks:
        tk = tasks.get((r[0], r[1], r[2]))
        if tk and r[4] == 'verified' and r[6] is False:
            rep_all[kind.get(r[0], 'rsu')][tk['cost_us']] += 1
    sig = {k: c.most_common(1)[0][0] for k, c in rep_all.items() if c}
    reval = 0
    for r in rx_tasks:
        tk = tasks.get((r[0], r[1], r[2]))
        if not tk:
            continue
        k = kind.get(r[0], 'rsu')
        if r[4] == 'verified':
            if r[6]:
                firsts.append((k, tk['cost_us']))
            elif sig.get(k) is not None and tk['cost_us'] > sig[k]:
                # A known pair validated again: its cache entry was invalidated (R17, a
                # fresh CRL) or evicted. Counted, and not a cached repeat.
                reval += 1
            else:
                repeat[k][tk['cost_us']] += 1
        elif r[4] == 'unverified' or r[5] == 'unknown-signer':
            unv.append((k, tk['cost_us']))
    if fault == 'repeat':
        repeat['veh'][401] += 1
        repeat['rsu'][501] += 1
        repeat['vru'][646] += 1
    for k in ('veh', 'rsu', 'vru'):
        out[f'oa.cost.repeat_values.{k}'] = sorted(repeat[k])
    out['oa.rx.revalidations'] = reval
    allowed = (rv.get('invalidated') or 0) + (rv.get('ee_evictions') or 0)
    out['oa.rx.revalidations_unexplained'] = max(0, reval - allowed)
    out['oa.rep.ee_validations_diff'] = ee_pending - len(pairs_verified) - reval
    detail['ee'] = (len(pairs_verified), reval, rv.get('ee_validations'), rv.get('ee_hits'),
                    rv.get('ee_evictions'), rv.get('invalidated'))
    if fault == 'first':
        firsts.append(('veh', 1))
    out['oa.cost.first_sight_not_dearer'] = sum(1 for k, c in firsts if sig.get(k) and c <= sig[k])
    if fault == 'unverifiable':
        unv.append(('veh', 400))
    out['oa.cost.unverifiable_charged_signature'] = sum(1 for k, c in unv
                                                        if sig.get(k) and c >= sig[k])
    out['oa.cost.unverifiable_tasks'] = len(unv)
    out['oa.cost.verify_tasks'] = len(tasks)
    out['oa.cost.unverifiable_us'] = sum(c for _, c in unv)
    out['oa.cost.all_us'] = sum(v['cost_us'] or 0 for v in tasks.values())

    # ------------------------------------------------------------ metric samples vs recount
    def samples(name, dims=None):
        return [s for s in metrics if s['metric'] == name and (dims is None or s['dims'] == dims)]

    fcs_s = collections.Counter()
    fcs_n = collections.Counter()
    env = collections.Counter()
    pay = collections.Counter()
    for lt, signer, e, pl in alltx:
        w = win(lt)
        if signer in ('certificate', 'digest'):
            fcs_n[w] += 1
            fcs_s[w] += signer == 'certificate'
        if e is not None and pl is not None:
            env[w] += e
            pay[w] += pl
    if fault == 'fcs':
        fcs_s[max(fcs_s)] += 1
    bad = []
    for s in samples('full_cert_share'):
        v = s['value']
        if v.get('trials') is not None and (v['successes'], v['trials']) != (fcs_s[s['t']], fcs_n[s['t']]):
            bad.append((s['t'] // NS, v['successes'], v['trials'], fcs_s[s['t']], fcs_n[s['t']]))
    out['oa.mi.full_cert_share_bad_windows'] = len(bad)
    detail['full_cert_share'] = bad[:3]
    if fault == 'env':
        env[max(env)] += 1
    bad = []
    for s in samples('envelope_overhead'):
        v = s['value']
        if v.get('numerator') is not None and (v['numerator'], v['denominator']) != (env[s['t']], pay[s['t']]):
            bad.append((s['t'] // NS, v['numerator'], v['denominator'], env[s['t']], pay[s['t']]))
    out['oa.mi.envelope_overhead_bad_windows'] = len(bad)
    detail['envelope_overhead'] = bad[:3]

    vc = collections.defaultdict(list)
    vw = collections.defaultdict(list)
    for v in tasks.values():
        if v['outcome'] not in ('valid', 'invalid'):
            continue
        vc[win(v['_lt'])].append(v['cost_us'] / 1000.0)
        vw[win(v['_lt'])].append((v['t_start'] - v['t_enqueue']) / 1e6)
    if fault == 'vcost':
        vc[max(vc)].append(99.0)
    bad = []
    for s in samples('verify_cost'):
        v = s['value']
        xs = vc.get(s['t'], [])
        if v.get('n') != len(xs) or (xs and (abs(v['min'] - min(xs)) > 1e-3 or abs(v['max'] - max(xs)) > 1e-3
                                              or abs(v['mean'] - sum(xs) / len(xs)) > 1e-3)):
            bad.append((s['t'] // NS, v.get('n'), v.get('mean'), len(xs), sum(xs) / len(xs) if xs else None))
    for s in samples('verify_rate'):
        if abs(s['value']['point'] - len(vc.get(s['t'], [])) / (WINDOW / NS)) > 1e-3:
            bad.append(('rate', s['t'] // NS, s['value']['point'], len(vc.get(s['t'], []))))
    for s in samples('verify_wait'):
        v = s['value']
        xs = vw.get(s['t'], [])
        if v.get('n') != len(xs) or (xs and abs(v['max'] - max(xs)) > 1e-3):
            bad.append(('wait', s['t'] // NS, v.get('n'), len(xs)))
    out['oa.mi.verify_cost_bad_windows'] = len(bad)
    detail['verify_cost'] = bad[:3]
    if fault == 'unv':
        deliv_unv[next(iter(deliv_unv))] += 1
    bad = []
    for s in samples('unverified_ratio'):
        v = s['value']
        if v.get('trials') is not None and (v['successes'], v['trials']) != (deliv_unv[s['t']], deliv[s['t']]):
            bad.append((s['t'] // NS, v['successes'], v['trials'], deliv_unv[s['t']], deliv[s['t']]))
    out['oa.mi.unverified_ratio_bad_windows'] = len(bad)
    detail['unverified_ratio'] = bad[:3]

    # pseudonym_change_rate: cumulative, every node with security rows (handsets included,
    # as the provider's `seen`), Σ changes / Σ (last row - first row) up to the sample.
    bad = []
    for s in samples('pseudonym_change_rate'):
        w1 = s['t']
        ch = sum(1 for x in sec_pseudo if x['_lt'] < w1) + (1 if fault == 'rate' else 0)
        spans = [(min(t for t in ts if t < w1), max(t for t in ts if t < w1))
                 for ts in sec_rows.values() if any(t < w1 for t in ts)]
        h = sum(b - a for a, b in spans) / NS / 3600
        if s['value'].get('point') is None:
            continue
        mine_v = ch / h if h else 0.0
        q = s.get('quantum') or 1e-4
        if abs(s['value']['point'] - mine_v) > q:
            bad.append((s['t'] // NS, s['value']['point'], round(mine_v, 4), ch))
    out['oa.mi.pseudonym_change_rate_bad_windows'] = len(bad)
    detail['pseudonym_change_rate'] = bad[:3]
    bad = []
    for s in samples('linkability'):
        v = s['value']
        succ = sum(1 for x in correct if x['t'] < s['t'])
        tri = sum(1 for x in sec_pseudo if x['t'] < s['t'])
        if fault == 'linkability':
            succ += 1
        if (v.get('successes'), v.get('trials')) != (succ, tri):
            bad.append((s['t'] // NS, v.get('successes'), v.get('trials'), succ, tri))
    out['oa.mi.linkability_bad_windows'] = len(bad)
    detail['linkability'] = bad[:3]

    # ------------------------------------------------------------ P2PCD
    sums = collections.Counter()
    learned_at = {}
    obs_t = collections.defaultdict(list)
    trig_w = collections.Counter()
    for r in p2pcd_rows:
        for k, x in r.items():
            if isinstance(x, int) and k not in ('t', 'node', '_lt'):
                sums[k] += x
        if fault == 'nolearn':
            r['learned'] = 0
        if r.get('learned') and r['node'] not in learned_at:
            learned_at[r['node']] = r['t']
        if r.get('requests_observed'):
            obs_t[r['node']].append(r['t'])
        trig_w[win(r['_lt'])] += r.get('triggers', 0)
    frames = len(p2pcd_frames) + (1 if fault == 'p2p_frames' else 0)
    onair = rep.get('p2pcd_responses_on_air', 0)
    out['oa.p2p.responses_diff'] = abs(frames - sums['responses_sent']) + abs(frames - onair)
    detail['p2pcd'] = (frames, sums['responses_sent'], onair, dict(sums))
    out['oa.p2p.learners'] = len(learned_at)
    lag = []
    for f in p2pcd_frames:
        before = [t for t in obs_t.get(f['node'], []) if t <= f['t_generated']]
        lag.append((f['t_generated'] - max(before)) / 1e6 if before else math.inf)
    if fault == 'backoff':
        lag.append(300.0)
    out['oa.p2p.response_after_request_max_ms'] = max(lag) if lag else None
    fb = fa = 0
    for r in rx_tasks:
        tx_node = None
        if r[4] != 'verified' or r[3] is None:
            continue
        tx_node = owner.get(r[3])
        a, b = provider.get(r[0]), provider.get(tx_node)
        if a and b and a != b:
            if r[0] not in learned_at or r[1] < learned_at[r[0]]:
                fb += 1
            else:
                fa += 1
    if fault == 'foreign':
        fb += 1
    out['oa.p2p.foreign_verified_before_learn'] = fb
    out['oa.p2p.foreign_verified_after_learn'] = fa
    bad = []
    trig_bad = 0
    for s in samples('p2pcd_requests'):
        mine_v = miss_w[s['t']] / (WINDOW / NS) + (1 if fault == 'trig' else 0)
        if abs(s['value']['point'] - mine_v) > 1e-3:
            bad.append((s['t'] // NS, s['value']['point'], mine_v, trig_w[s['t']] / (WINDOW / NS)))
        trig_bad += abs(s['value']['point'] - trig_w[s['t']] / (WINDOW / NS)) > 1e-3
    out['oa.mi.p2pcd_requests_vs_p2pcd_triggers_bad_windows'] = trig_bad
    # The metric against the per-node telemetry counter it sums (node.telemetry
    # p2pcd_requests, by record window), and that counter's run total against the
    # independent digest-miss recount.
    out['oa.mi.p2pcd_requests_vs_telemetry_bad_windows'] = sum(
        abs(s['value']['point'] - tel_w[s['t']] / (WINDOW / NS)) > 1e-3 for s in samples('p2pcd_requests'))
    out['oa.p2p.digest_miss_recount'] = sum(miss_w.values())
    out['oa.p2p.digest_miss_telemetry'] = sum(tel_w.values())
    out['oa.mi.p2pcd_requests_bad_windows'] = len(bad)
    detail['p2pcd_requests'] = bad[:3]

    # ------------------------------------------------------------ determinism
    if fault in (None, 'digest'):
        h = hashlib.sha256(b'x' if fault else b'')
        from mcap.reader import make_reader
        with open(rec, 'rb') as f:
            for _, ch, m in make_reader(f).iter_messages(log_time_order=False):
                h.update(ch.topic.encode() + b'\0' + m.log_time.to_bytes(8, 'little') + m.data)
        out['digest.records'] = h.hexdigest()
        out['digest.metrics.json'] = hashlib.sha256(
            (b'x' if fault else b'') + open(os.path.join(run_dir, 'metrics.json'), 'rb').read()).hexdigest()
    out['oa.detail'] = detail
    return out


# One fault per measure: the mutation that must turn its row red.
FAULTS = {
    'oa.tx.cert_bytes_values': 'cert_bytes', 'oa.tx.envelope_accounting_violations': 'envelope',
    'oa.tx.spdu_accounting_violations': 'spdu', 'oa.tx.cert_cadence_violations': 'cadence',
    'oa.chg.temp_id_mismatch': 'temp_id', 'oa.chg.sec_vs_tx_diff': 'sec_pseudo',
    'oa.rep.privacy_diff': 'link', 'oa.rx.verify_join_unmatched': 'verify_join',
    'oa.rep.ee_validations_diff': 'ee', 'oa.cost.repeat_values.veh': 'repeat',
    'oa.cost.repeat_values.rsu': 'repeat', 'oa.cost.repeat_values.vru': 'repeat',
    'oa.cost.first_sight_not_dearer': 'first', 'oa.cost.unverifiable_charged_signature': 'unverifiable',
    'oa.mi.full_cert_share_bad_windows': 'fcs', 'oa.mi.envelope_overhead_bad_windows': 'env',
    'oa.mi.verify_cost_bad_windows': 'vcost', 'oa.mi.unverified_ratio_bad_windows': 'unv',
    'oa.mi.pseudonym_change_rate_bad_windows': 'rate', 'oa.mi.linkability_bad_windows': 'linkability',
    'oa.p2p.responses_diff': 'p2p_frames', 'oa.p2p.response_after_request_max_ms': 'backoff',
    'oa.p2p.foreign_verified_before_learn': 'foreign', 'oa.mi.p2pcd_requests_bad_windows': 'trig',
    'oa.tx.first_under_pseudonym_digest': 'first_digest', 'oa.tx.cert_gap_p50_ms': 'gap',
    'oa.rx.verified_without_cert': 'nocert', 'oa.rx.unverified_with_cert': 'cached',
    'oa.rx.refused': 'refused', 'oa.rx.stamp_order_violations': 'stamps',
    'oa.chg.changes_tx': 'change', 'oa.chg.age_min_s': 'age_early', 'oa.chg.age_max_s': 'age_late',
    'oa.chg.msgcnt_continued_share': 'msgcnt', 'oa.chg.pseudonym_reused': 'reuse',
    'oa.chg.rate_per_veh_h': 'rate_hi', 'oa.p2p.learners': 'nolearn',
    'oa.p2p.foreign_verified_after_learn': 'nolearn', 'digest.records': 'digest',
    'digest.metrics.json': 'digest',
}


def faults(runs_dir, table):
    """Re-measure each row's run under the row's fault and judge it: it must go red."""
    import yaml
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
    import matrix
    doc = yaml.safe_load(open(table))
    cache = {}
    for row in doc['rows']:
        f = FAULTS.get(row['measure'])
        if not f:
            print(f"NOFAULT {row['id']:30} {row['measure']} (see the report's fault table)")
            continue
        run = row['variant']
        if not all(os.path.exists(os.path.join(runs_dir, r, 'realism.json')) for r in (run, row['base'])):
            print(f"NO-RUN  {row['id']}")
            continue
        key = (run, f)
        if key not in cache:
            cache[key] = measure(os.path.join(runs_dir, run), fault=f)
        d = cache[key]
        base = json.load(open(os.path.join(runs_dir, row['base'], 'realism.json')))
        ok, shown = matrix.judge(row, lambda r, m: (d if r == run else base).get(m, 'ABSENT'))
        print(f"{'RED' if ok is False else 'GREEN?'} {row['id']:30} {f:12} {shown}"[:200])


if __name__ == '__main__':
    if sys.argv[1] == 'measure':
        d = measure(sys.argv[2])
        dst = sys.argv[3] if len(sys.argv) > 3 else os.path.join(sys.argv[2], 'realism.json')
        with open(dst, 'w') as f:
            json.dump(d, f, indent=1, sort_keys=True, default=str)
        for k, v in sorted(d.items()):
            if k != 'oa.detail':
                print(k, json.dumps(v)[:120])
        print('detail', json.dumps(d['oa.detail'], default=str)[:1500])
    elif sys.argv[1] == 'faults':
        faults(sys.argv[2], sys.argv[3])
