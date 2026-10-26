#!/usr/bin/env python3
"""SCMS audit "prov": pseudonym provisioning checked from a run's raw records.

    python3 -I tools/pdu_check/audit_prov.py RUN [--expect FILE] [--mutate ROW|all] [--twin RUN2]

Reads RUN/recording.mcap (pki.flow messages, stages and SPDUs, node.credentials certificate
bytes, node.security, backend.state, pki.state) and RUN/run-report.json, decodes
certificates with the oracle's own COER codec (tools/oracle), and judges each row of
crates/v2xw-engine/tests/expectations/scms-audit-prov.yaml. Nothing is taken from the
engine's Rust; the rules cite the standard or the deployment (see the rows' `spec`).

`--mutate ROW` injects one fault into the loaded records (or the engine value) for that row
and requires the row to go red; `--mutate all` does it for every row and prints the silent
ones. `--twin RUN2` compares the provisioning records of a same-seed run byte for byte.
"""

import collections
import copy
import datetime as dt
import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'oracle'))
from scms_oracle import coer, time1609  # noqa: E402

NS = 1_000_000_000
WEEK = 604800
I_EPOCH = dt.datetime(2015, 1, 6, 9, 0, tzinfo=dt.timezone.utc)  # [SMP] Table 2
CERTS_PER_PERIOD = 20
POOL_PERIODS = 2
MIN_WAIT_S = 300
MAX_GEN_DELAY_S = WEEK
CAMP_NEW_PER_YEAR = 17_000_000


def i_of_time32(v):
    return int((time1609.from_time32(v) - I_EPOCH).total_seconds() // WEEK)


def period_start_s(R, i):
    """Run seconds of i-period i's start."""
    return (I_EPOCH + dt.timedelta(seconds=i * WEEK) - R.t0).total_seconds()


class Rec:
    pass


def load(run):
    from mcap.reader import make_reader
    R = Rec()
    R.run = run
    for line in open(os.path.join(run, 'scenario.resolved.yaml')):
        m = re.match(r"\s*t0:\s*['\"]?([0-9T:\-]+)Z?['\"]?", line)
        if m:
            R.t0 = dt.datetime.fromisoformat(m.group(1)).replace(tzinfo=dt.timezone.utc)
            break
    R.msgs, R.stages, R.lone = [], [], []
    R.creds = collections.defaultdict(list)   # device -> [record]
    R.node_dev = {}
    R.sec = collections.defaultdict(list)     # node -> [(t_s, row)]
    R.certs = {}                              # id8 -> hex (every store)
    R.backend = None
    R.pki_state = None
    R.digest = hashlib.sha256()
    with open(os.path.join(run, 'recording.mcap'), 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            t = ch.topic
            if t == 'record/pki.flow':
                R.digest.update(m.data)
                q = json.loads(m.data)
                {'msg': R.msgs, 'stage': R.stages, 'pdu': R.lone}[q['kind']].append(q)
            elif t == 'record/node.credentials':
                R.digest.update(m.data)
                q = json.loads(m.data)
                R.creds[q['device']].append(q)
                R.node_dev[q['node']] = q['device']
                R.certs.update(q.get('certificates') or {})
            elif t == 'record/node.security':
                q = json.loads(m.data)
                R.sec[q['node']].append((q['t'] / NS, q))
            elif t == 'record/backend.state':
                R.backend = json.loads(m.data)
            elif t == 'record/pki.state':
                q = json.loads(m.data)
                R.certs.update(q.get('certificates') or {})
                R.pki_state = q
    R.digest = R.digest.hexdigest()
    R.report = json.load(open(os.path.join(run, 'run-report.json')))['report']['phase2']
    return R


def role(eid):
    """'provider-a/ra/0' -> 'ra'; 'ee/1000003' -> 'ee'."""
    p = (eid or '').split('/')
    return p[1] if len(p) >= 3 else p[0]


def prov(eid):
    return (eid or '').split('/')[0]


# --------------------------------------------------------------------------------------------
# derived views, all from the records

def derive(R):
    D = Rec()
    pre = [p for q in R.lone if q['flow'] == 'preload' for p in q['pdus']]
    D.pre = pre
    inrun = [p for q in R.msgs for p in q.get('pdus') or []]
    D.inrun = inrun
    # RSUs: the devices with an application-certificate flow, in the run or preloaded with
    # a plain (non-butterfly) request. Pre-run: an RSU's requests are those of a device that
    # never receives pseudonym PLVs. Identify RSUs from the in-run app-cert flow.
    D.rsus = {q['from'] for q in R.msgs if q['flow'] == 'rsu-application-cert' and q['name'] == 'app-cert-request'}
    D.vehicles = set(R.creds) - D.rsus
    D.by_inst = collections.defaultdict(list)
    for q in R.msgs + R.stages:
        D.by_inst[q['instance']].append(q)
    return D


def cert_rows(R, D):
    """Decoded pseudonym certificates per device: device -> {i: set(id8)}, plus decode list."""
    per = collections.defaultdict(lambda: collections.defaultdict(set))
    dec = {}
    for dev, recs in R.creds.items():
        for q in recs:
            for k, h in (q.get('certificates') or {}).items():
                if k not in dec:
                    dec[k] = coer.decode('Certificate', bytes.fromhex(h))
                c = dec[k]
                if c['toBeSigned'].get('crlSeries') == 1:
                    i = i_of_time32(c['toBeSigned']['validityPeriod']['start'])
                    per[dev][i].add(k)
    return per, dec


def first_cells(R, dev):
    q = R.creds[dev][0]
    return q


# --------------------------------------------------------------------------------------------
# the rows: each returns (ok, measured)

def r_request_once(R, D):
    n = collections.Counter(p['from'] for p in D.pre if p['type'] == 'EeRaCertRequestSpdu')
    n.update(p['from'] for p in D.inrun if p['type'] == 'EeRaCertRequestSpdu' and p['from'] in D.vehicles)
    bad = {v: n.get(v, 0) for v in D.vehicles if n.get(v, 0) != 1}
    return not bad, f'{len(D.vehicles)} vehicles, counts {sorted(collections.Counter(n[v] for v in D.vehicles).items())}, off {len(bad)}'


def r_ack_per_request(R, D):
    req = collections.Counter(p['from'] for p in D.pre if p['type'] == 'EeRaCertRequestSpdu')
    ack = collections.Counter(p['to'] for p in D.pre if p['type'] == 'RaEeCertAckSpdu')
    for p in D.inrun:
        if p['type'] == 'EeRaCertRequestSpdu':
            req[p['from']] += 1
        if p['type'] == 'RaEeCertAckSpdu':
            ack[p['to']] += 1
    diff = sum(abs(req[k] - ack[k]) for k in set(req) | set(ack))
    return diff == 0, f'requests {sum(req.values())}, acks {sum(ack.values())}, per-device diff {diff}'


def r_plv_per_cert(R, D):
    per = collections.Counter()
    sealed_bad = 0
    for p in D.pre:
        if p['type'] == 'EncryptedIndividualPLV':
            per[(p['from'], prov(p['from']))] += 1
            if role(p.get('sealed_to')) != 'aca':
                sealed_bad += 1
    veh_by_prov = collections.Counter(R.creds[v][0]['provider'] for v in D.vehicles)
    bad = []
    for (la, pv), n in per.items():
        if n != 40 * veh_by_prov[pv]:
            bad.append((la, n, veh_by_prov[pv]))
    las = {la for la, _ in per}
    ok = not bad and sealed_bad == 0 and len(las) == 2 * len(veh_by_prov)
    return ok, f'PLVs per LA {dict((k[0], v) for k, v in per.items())}, vehicles {dict(veh_by_prov)}, ratio-off {bad}, not sealed to ACA {sealed_bad}'


def r_aca_per_cert(R, D):
    c = collections.Counter(p['type'] for p in D.pre + D.inrun)
    a, b, e = c['RaAcaCertRequestSpdu'], c['AcaRaCertResponseSpdu'], c['AcaEeCertResponsePrivateSpdu']
    # per vehicle (pre-run): private responses addressed to each vehicle
    pv = collections.Counter(p['to'] for p in D.pre if p['type'] == 'AcaEeCertResponsePrivateSpdu')
    off = [v for v in D.vehicles if pv.get(v, 0) != 40]
    # The weekly job makes the week of every account, also of devices that have not asked
    # (flow `pregeneration`): those answers stay in the RA's batch files until the device
    # downloads, so they have no AcaEe-private delivery in the run.
    # Each delivered private response is matched to the AcaRa answer that carried it (a
    # 40-hex-digit chunk of its bytes inside the answer's bytes).
    W = 40
    chunk = {}
    for p in D.pre + D.inrun:
        if p['type'] == 'AcaEeCertResponsePrivateSpdu' and p.get('hex') and len(p['hex']) >= 200:
            chunk[p['hex'][100:100 + W]] = p
    flows = {}
    for q in R.msgs:
        for p in q.get('pdus') or []:
            flows[id(p)] = q['flow']
    matched, held, lost = set(), 0, 0
    for p in D.pre + D.inrun:
        if p['type'] != 'AcaRaCertResponseSpdu' or not p.get('hex'):
            continue
        hx = p['hex']
        hit = next((chunk[hx[k:k + W]] for k in range(0, len(hx) - W + 1, 2) if hx[k:k + W] in chunk), None)
        if hit is not None:
            matched.add(id(hit))
        elif flows.get(id(p)) == 'pregeneration':
            held += 1
        else:
            lost += 1
    orphans = e - len(matched)
    ok = a == b and orphans == 0 and lost == 0 and not off
    return ok, (f'RaAca {a}, AcaRa {b}, AcaEePrivate {e} = delivered {len(matched)}; undelivered AcaRa: '
                f'{held} held at the RA for devices that have not asked, {lost} lost; private with no AcaRa {orphans}; '
                f'vehicles with != 40 pre-run {len(off)}')


def r_encrypted_to_device(R, D):
    priv = [p for p in D.pre + D.inrun if p['type'] == 'AcaEeCertResponsePrivateSpdu']
    not_ee = sum(1 for p in priv if p.get('sealed_to') != 'ee')
    opened_ra = sum(1 for p in priv if p.get('opened') and role(p['to']) == 'ra')
    return not_ee == 0 and opened_ra == 0 and priv, f'{len(priv)} private responses, not sealed to ee {not_ee}, opened at an RA {opened_ra}'


def r_aca_learns_no_device(R, D):
    names = {}
    for dev, recs in R.creds.items():
        q = recs[0]
        ids = []
        if q.get('enrolment'):
            ids.append(q['enrolment'])
        idx = int(dev.split('/')[1])
        ids.append(idx.to_bytes(4, 'big').hex())
        names[dev] = ids
    # the request hash8 of each vehicle's butterfly request is the SHA-256 low 8 bytes of
    # the EeRaCertRequestSpdu as sent (DL-ACK, [ASN-2.1])
    for p in D.pre:
        if p['type'] == 'EeRaCertRequestSpdu' and p.get('hex'):
            h = hashlib.sha256(bytes.fromhex(p['hex'])).digest()[-8:].hex()
            names.setdefault(p['from'], []).append(h)
    leaks = 0
    checked = 0
    for p in D.pre + D.inrun:
        if p['type'] == 'RaAcaCertRequestSpdu' and p.get('hex'):
            checked += 1
            hx = p['hex']
            for dev, ids in names.items():
                for x in ids:
                    if len(x) >= 16 and x in hx:
                        leaks += 1
    return leaks == 0 and checked > 0, f'{checked} RA-to-ACA requests searched for {sum(len(v) for v in names.values())} device identifiers: {leaks} found'


def ra_aca_requests(R):
    """Every in-run RA-to-ACA certificate request as the ACA receives it, in record order:
    (batch key, message index, pdu, instance). A batch is what one RA released at one
    instant (the shuffle releases a batch at once); the instance is ground truth (one
    account's job: one device's week), never an input to an attack."""
    out = []
    for k, q in enumerate(R.msgs):
        if role(q.get('from')) == 'ra' and role(q.get('to')) in ('aca', 'pca'):
            for p in q.get('pdus') or []:
                if p['type'] == 'RaAcaCertRequestSpdu':
                    out.append(((q['from'], q['t']), k, p, q['instance']))
    return out


def aca_batches(R):
    b = collections.defaultdict(list)
    for key, k, p, inst in ra_aca_requests(R):
        b[key].append((k, p, inst))
    return b


def r_aca_cannot_group(R, D):
    """Each message carries one request, and every ACA batch mixes devices: the largest
    share of one device in a batch is the population baseline 1/k (k devices whose week
    is in the batch, each with the same number of requests)."""
    per_msg = collections.Counter(k for _, k, _, _ in ra_aca_requests(R))
    most_per_msg = max(per_msg.values(), default=0)
    B = aca_batches(R)
    worst_share, worst_excess, min_dev, n_req = 0.0, 0.0, None, 0
    for key, xs in B.items():
        c = collections.Counter(inst for _, _, inst in xs)
        share = max(c.values()) / len(xs)
        worst_share = max(worst_share, share)
        worst_excess = max(worst_excess, share - 1 / len(c))
        min_dev = len(c) if min_dev is None else min(min_dev, len(c))
        n_req += len(xs)
    ok = B and most_per_msg == 1 and min_dev >= 2 and worst_excess <= 1e-9
    return ok, (f'{len(B)} ACA batches, {n_req} requests; requests per message max {most_per_msg}; '
                f'devices per batch min {min_dev}; largest share of one device {worst_share:.4f} '
                f'(over the 1/k baseline by {worst_excess:.4f})')


def _ngrams(hx, n=8):
    b = bytes.fromhex(hx)
    return {b[i:i + n] for i in range(0, max(0, len(b) - n + 1))}


def grouping_attacks(R):
    """What the ACA sees of each batch (arrival order, message length, SPDU bytes) and three
    ways to link two requests as one device's: adjacent in arrival order; equal length; a
    shared 8-byte string that is not common to most of the batch (any field, any offset:
    a per-device constant anywhere in the SPDU would show). Scored against the batch's
    population baseline, the chance that two random requests of the batch are one
    device's. Returns {attack: (links, same-device links, expected at baseline, z)}."""
    res = {}
    acc = {a: [0, 0, 0.0, 0.0] for a in ('order', 'length', 'bytes')}
    for key, xs in aca_batches(R).items():
        xs = sorted(xs, key=lambda x: x[0])
        n = len(xs)
        if n < 2:
            continue
        c = collections.Counter(inst for _, _, inst in xs)
        p0 = sum(v * (v - 1) for v in c.values()) / (n * (n - 1))
        links = {'order': [(a, a + 1) for a in range(n - 1)], 'length': [], 'bytes': set()}
        by_len = collections.defaultdict(list)
        for a, (_, p, _) in enumerate(xs):
            by_len[p['len']].append(a)
        if len(by_len) > 1:
            for v in by_len.values():
                if len(v) <= n // 2:
                    links['length'] += [(v[a], v[b]) for a in range(len(v)) for b in range(a + 1, len(v))]
        holders = collections.defaultdict(set)
        for a, (_, p, _) in enumerate(xs):
            for g in _ngrams(p.get('hex') or ''):
                holders[g].add(a)
        for g, hs in holders.items():
            if 1 < len(hs) <= n // 2:
                hs = sorted(hs)
                links['bytes'].update((hs[a], hs[b]) for a in range(len(hs)) for b in range(a + 1, len(hs)))
        for att, L in links.items():
            same = sum(1 for a, b in L if xs[a][2] == xs[b][2])
            acc[att][0] += len(L)
            acc[att][1] += same
            acc[att][2] += p0 * len(L)
            acc[att][3] += p0 * (1 - p0) * len(L)
    for att, (L, S, E, V) in acc.items():
        z = (S - E) / V ** 0.5 if V > 0 else (0.0 if S <= E else float('inf'))
        res[att] = (L, S, E, z)
    return res


def r_aca_grouping_attack(R, D):
    """The ACA tries to link certificates of one device from what it sees; every attack must
    do no better than the population baseline (z <= 3 over all batches)."""
    res = grouping_attacks(R)
    if not ra_aca_requests(R):
        return False, 'no RA-to-ACA request in the run'
    ok = all(z <= 3 for _, _, _, z in res.values())
    return ok, '; '.join(f'{a}: {L} links, {S} one device\'s vs {E:.1f} at baseline (z {z:.2f})'
                         for a, (L, S, E, z) in res.items())


def r_lop_not_deployed(R, D):
    hops = sum(1 for q in R.msgs if role(q.get('from')) == 'lop' or role(q.get('to')) == 'lop')
    view = [e for e in (R.backend or {}).get('entities', []) if e['id'].endswith('lop')]
    shown = sum(e['traffic']['received'] + e['traffic']['sent'] for e in view)
    return hops == 0, f'pki.flow LOP hops {hops}; backend.state still shows {len(view)} LOP boxes with {shown} messages'


def r_cert_implicit(R, D, per_dec=None):
    per, dec = per_dec or cert_rows(R, D)
    ps = [c for c in dec.values() if c['toBeSigned'].get('crlSeries') == 1]
    expl = sum(1 for c in ps if c['type'] != 'implicit' or c['toBeSigned']['verifyKeyIndicator'][0] != 'reconstructionValue')
    return expl == 0 and ps, f'{len(ps)} pseudonym certificates, explicit {expl}'


def r_cert_validity(R, D, per_dec=None):
    per, dec = per_dec or cert_rows(R, D)
    bs = bd = n = 0
    for c in dec.values():
        tb = c['toBeSigned']
        if tb.get('crlSeries') != 1:
            continue
        n += 1
        st = tb['validityPeriod']['start']
        i = i_of_time32(st)
        exp = time1609.time32(I_EPOCH + dt.timedelta(seconds=i * WEEK))
        if st != exp:
            bs += 1
        du = tb['validityPeriod']['duration']
        mins = {'minutes': 1, 'hours': 60, 'seconds': 1 / 60}.get(du[0], 0) * du[1]
        if mins != 169 * 60:
            bd += 1
    return bs == 0 and bd == 0 and n, f'{n} certificates, start off the i-period grid {bs}, duration not 169 h {bd}'


def r_cert_linkage(R, D, per_dec=None):
    per, dec = per_dec or cert_rows(R, D)
    bad = n = 0
    for c in dec.values():
        tb = c['toBeSigned']
        if tb.get('crlSeries') != 1:
            continue
        n += 1
        idk = tb['id']
        if idk[0] != 'linkageData':
            bad += 1
            continue
        ld = idk[1]
        if ld['iCert'] != i_of_time32(tb['validityPeriod']['start']) or len(ld['linkage-value']) != 9:
            bad += 1
    return bad == 0 and n, f'{n} certificates, linkage data off {bad}'


def r_pool_20(R, D, per_dec=None):
    per, dec = per_dec or cert_rows(R, D)
    dist = collections.Counter(len(s) for dev in D.vehicles for s in per[dev].values())
    return set(dist) == {CERTS_PER_PERIOD}, f'certificates per (vehicle, i-period): {dict(dist)}'


def r_pool_at_join(R, D, per_dec=None):
    dist = collections.Counter()
    for dev in D.vehicles:
        q = R.creds[dev][0]
        dist[len({i_of_time32(coer.decode('Certificate', bytes.fromhex(h))['toBeSigned']['validityPeriod']['start'])
                  for h in (q.get('certificates') or {}).values()
                  if coer.decode('Certificate', bytes.fromhex(h))['toBeSigned'].get('crlSeries') == 1})] += 1
    return set(dist) == {POOL_PERIODS}, f'i-periods held at the first node.credentials row: {dict(dist)}'


def topups(R, D):
    """instance -> {stage: t, msgs}, for flow topup."""
    out = {}
    for inst, xs in D.by_inst.items():
        if not xs or xs[0]['flow'] != 'topup':
            continue
        st = {}
        for x in xs:
            if x['kind'] == 'stage':
                st.setdefault(x['name'], x['t'] / NS)
        dev = next((x['from'] for x in xs if x['kind'] == 'stage' and x['name'] == 'requested'), None)
        out[inst] = (dev, st, [x for x in xs if x['kind'] == 'msg'])
    return out


def r_topup_at_boundary(R, D):
    b = period_start_s(R, 615)
    T = topups(R, D)
    started = {dev: st['requested'] for dev, st, _ in T.values() if 'requested' in st}
    need = set()
    for node, rows in R.sec.items():
        dev = R.node_dev.get(node)
        if dev not in D.vehicles:
            continue
        before = [r for t, r in rows if b - 10 <= t < b]
        after = [r for t, r in rows if b < t <= b + 10]
        if before and after and before[-1].get('pool_last_period') == 615:
            need.add(dev)
    missing = need - set(started)
    lag = max((started[d] - b for d in need & set(started)), default=None)
    ok = need and not missing and lag is not None and 0 <= lag <= 0.1
    return ok, f'on the road across the boundary with pool to 615: {len(need)}; top-ups started {len(started)}; missing {len(missing)}; max start - boundary {lag:.4f} s' if lag is not None else f'need {len(need)}, none started'


def r_topup_no_new_request(R, D):
    T = topups(R, D)
    new = sum(1 for p in D.inrun if p['type'] == 'EeRaCertRequestSpdu' and p['from'] in D.vehicles)
    per = collections.Counter(sum(1 for m in ms for p in m.get('pdus') or [] if p['type'] == 'EeRaDownloadRequestSpdu') for _, _, ms in T.values())
    return new == 0 and set(per) == {2}, f'vehicle butterfly requests in the run {new}; download requests per top-up {dict(per)}'


def r_topup_20(R, D, per_dec=None):
    per, dec = per_dec or cert_rows(R, D)
    T = topups(R, D)
    got = collections.Counter()
    periods = collections.Counter()
    for dev, st, ms in T.values():
        n = sum(1 for m in ms if m['name'] == 'batch-download' for p in m.get('pdus') or [] if p['type'] == 'AcaEeCertResponsePrivateSpdu')
        got[n] += 1
        if 'installed' in st:
            periods[len(per[dev].get(616, ()))] += 1
    return set(got) == {20} and set(periods) == {20}, f'certificates per top-up download {dict(got)}; i=616 certificates held after install {dict(periods)}'


def r_pregen_delay(R, D):
    b = period_start_s(R, 615)
    T = topups(R, D)
    d = [st['expanded'] - b for _, st, _ in T.values() if 'expanded' in st]
    return d and max(d) <= MAX_GEN_DELAY_S and min(d) >= 0, f'{len(d)} weekly jobs; expanded - boundary {min(d):.3f} .. {max(d):.3f} s' if d else 'no weekly job'


def r_download_after_ready(R, D):
    T = topups(R, D)
    early = late = n = 0
    for dev, st, ms in T.values():
        r = st.get('batch_ready')
        for m in ms:
            if m['name'] == 'batch-download-request':
                n += 1
                t = m['t'] / NS
                if r is None or t < r:
                    early += 1
                elif t > r + MIN_WAIT_S:
                    late += 1
    return n and early == 0 and late == 0, f'{n} batch requests; before the batch was ready {early}; more than 300 s after {late}'


def r_topup_conservation(R, D):
    T = topups(R, D)
    req = sum(1 for _, st, _ in T.values() if 'requested' in st)
    inst = sum(1 for _, st, _ in T.values() if 'installed' in st)
    ref = sum(1 for _, st, ms in T.values() if 'installed' not in st and any('refused' in m['name'] for m in ms))
    fly = sum(1 for _, st, ms in T.values() if 'installed' not in st and not any('refused' in m['name'] for m in ms))
    return req == inst + ref + fly and req > 0 and inst <= req, f'requested {req} = installed {inst} + refused {ref} + in flight {fly}'


def appcert_flows(R, D):
    out = {}
    for inst, xs in D.by_inst.items():
        if xs and xs[0]['flow'] == 'rsu-application-cert':
            out[inst] = xs
    return out


def r_rsu_renewal(R, D):
    F = appcert_flows(R, D)
    per = collections.Counter()
    la = shuf = unsealed = 0
    b = period_start_s(R, 615)
    late = 0
    for xs in F.values():
        for x in xs:
            if x['kind'] == 'msg' and x['name'] == 'app-cert-request':
                per[x['from']] += 1
                if abs(x['t'] / NS - b) > 1:
                    late += 1
                for p in x.get('pdus') or []:
                    if p['type'] == 'EeRaCertRequestSpdu' and role(p.get('sealed_to')) != 'ra':
                        unsealed += 1
            if x['kind'] == 'msg' and (role(x.get('to')).startswith('la') or role(x.get('from')).startswith('la')):
                la += 1
            if x['kind'] == 'stage' and x['name'] in ('shuffled', 'expanded', 'pre_linkage_ready'):
                shuf += 1
    ok = per and set(per.values()) == {1} and la == 0 and shuf == 0 and unsealed == 0 and late == 0
    return ok, f'requests per RSU {dict(per)}; LA messages {la}; shuffle/expansion stages {shuf}; not sealed to the RA {unsealed}; off the boundary {late}'


def r_rsu_cert(R, D):
    bad = n = 0
    for k, h in R.certs.items():
        c = coer.decode('Certificate', bytes.fromhex(h))
        tb = c['toBeSigned']
        if tb.get('crlSeries') != 3 or c['type'] != 'implicit' and tb['id'][0] == 'none' and False:
            continue
        if tb.get('crlSeries') != 3:
            continue
        du = tb['validityPeriod']['duration']
        mins = {'minutes': 1, 'hours': 60}.get(du[0], 0) * du[1]
        if mins == 721 * 60:   # an OBU identification certificate, not an RSU's
            continue
        n += 1
        if c['type'] != 'implicit' or mins != 169 * 60 or tb['id'][0] == 'linkageData':
            bad += 1
    return n > 0 and bad == 0, f'{n} series-3 application certificates in the stores; off {bad}'


def r_m_topups(R, D, per_dec=None):
    T = topups(R, D)
    started = sum(1 for _, st, _ in T.values() if 'requested' in st)
    done = sum(1 for _, st, _ in T.values() if 'installed' in st)
    certs = sum(1 for _, st, ms in T.values() if 'installed' in st for m in ms if m['name'] == 'batch-download'
                for p in m.get('pdus') or [] if p['type'] == 'AcaEeCertResponsePrivateSpdu')
    rp = R.report
    diff = abs(rp['topups_started'] - started) + abs(rp['topups_completed'] - done) + abs(rp['certs_topped_up'] - certs)
    return diff == 0, f'engine {rp["topups_started"]}/{rp["topups_completed"]}/{rp["certs_topped_up"]}, records {started}/{done}/{certs}'


def r_m_topup_latency(R, D):
    T = topups(R, D)
    # "request to completion" (backend.rs ExchangeLatency): the request is the device's
    # decision at the trigger, the i-period boundary (topup_below_periods = 1), not the
    # `requested` stamp, which comes after the device has signed its request.
    b = period_start_s(R, 615)
    lat = [st['installed'] - b for _, st, _ in T.values() if 'installed' in st and 'requested' in st]
    e = R.report['access']['exchange_latency']['topup']
    ds = abs(e['sum_ns'] / NS - sum(lat))
    dm = abs(e['max_ns'] / NS - max(lat))
    ok = e['count'] == len(lat) and ds < 1e-3 and dm < 1e-3
    return ok, f'engine n {e["count"]} sum {e["sum_ns"] / NS:.3f} s max {e["max_ns"] / NS:.3f} s; records n {len(lat)} sum {sum(lat):.3f} s max {max(lat):.3f} s'


def r_m_rsu(R, D):
    F = appcert_flows(R, D)
    inst = sum(1 for xs in F.values() if any(x['kind'] == 'stage' and x['name'] == 'installed' for x in xs))
    pre = sum(1 for p in D.pre if p['type'] == 'EeRaCertRequestSpdu' and p['from'] in D.rsus)
    rp = R.report
    diff = abs(rp['rsu_app_certs_renewed'] - inst) + abs(rp['rsu_app_certs_preloaded'] - pre)
    return diff == 0, f'engine renewed {rp["rsu_app_certs_renewed"]} preloaded {rp["rsu_app_certs_preloaded"]}; records {inst} / {pre}'


def r_m_backend(R, D):
    ents = {e['id']: e for e in R.backend['entities']}
    # pki.state names components provider/role/n; backend.state names the home provider's
    # bare (`ra`) and the others' `provider-b/ra`.
    home = next(p['label'] for p in R.pki_state['providers'] if p.get('home'))
    rows = []
    for e_id, e in ents.items():
        r = e_id.split('/')[-1]
        if r not in ('la1', 'la2', 'pca', 'ra'):
            continue
        pv = e_id.split('/')[0] if '/' in e_id else home
        rr = 'aca' if r == 'pca' else r
        full = f'{pv}/{rr}/0'
        rx = sum(1 for q in R.msgs if q.get('to') == full)
        tx = sum(1 for q in R.msgs if q.get('from') == full)
        rows.append((e_id, e['traffic']['received'], rx, e['traffic']['sent'], tx))
    bad = [r for r in rows if r[1] != r[2] or r[3] != r[4]]
    return rows and not bad, f'{len(rows)} entities; mismatched (id, view rx, records rx, view tx, records tx): {bad}'


def r_m_issued(R, D):
    allp = D.pre + D.inrun
    rows = []
    for pv in R.pki_state['providers']:
        for c in pv['components']:
            r, cid, k = c['role'], c['id'], c.get('counters') or {}
            if r in ('la1', 'la2') and 'plv_issued' in k:
                rec = sum(1 for p in allp if p['type'] == 'EncryptedIndividualPLV' and p['from'] == cid)
                rows.append((cid, 'plv_issued', k['plv_issued'], rec))
            if r in ('aca', 'pca') and 'certs_issued' in k:
                rec = sum(1 for p in allp if p['type'] == 'AcaRaCertResponseSpdu' and p['from'] == cid)
                rows.append((cid, 'certs_issued', k['certs_issued'], rec))
            if r == 'ra' and 'requests_accepted' in k:
                rec = sum(1 for p in allp if p['type'] == 'RaEeCertAckSpdu' and p['from'] == cid and p['to'] in D.vehicles)
                rows.append((cid, 'requests_accepted', k['requests_accepted'], rec))
    bad = [r for r in rows if r[2] != r[3]]
    return rows and not bad, f'{len(rows)} counters; off (entity, counter, view, records): {bad}'


def r_m_pool_fields(R, D):
    bad = n = 0
    for node, rows in R.sec.items():
        dev = R.node_dev.get(node)
        if dev not in D.vehicles:
            continue
        cr = R.creds[dev]
        for t, r in rows:
            prior = [q for q in cr if q['t'] / NS <= t]
            if not prior:
                continue
            q = prior[-1]
            # pool_stored is "certificates stored at all" (sec_records.rs), expired included
            stored = sum(1 for c in q['cells'] if c.get('id8'))
            n += 1
            if r.get('pool_stored') != stored:
                bad += 1
    return n > 0 and bad == 0, f'{n} node.security rows; pool_stored off node.credentials {bad}'


def r_cell_expiry(R, D):
    """A node.credentials cell says `expired` only once its certificate's own validity
    (start + hours(169)) has ended, and `valid`/`active` only inside it."""
    early = late = n = 0
    vu = {}
    for k, h in R.certs.items():
        tb = coer.decode('Certificate', bytes.fromhex(h))['toBeSigned']
        if tb.get('crlSeries') != 1:
            continue
        st = (time1609.from_time32(tb['validityPeriod']['start']) - R.t0).total_seconds()
        vu[k] = (st, st + 169 * 3600)
    for dev in D.vehicles:
        for q in R.creds[dev]:
            t = q['t'] / NS
            for c in q['cells']:
                w = vu.get(c.get('id8'))
                if not w:
                    continue
                n += 1
                if c['state'] == 'expired' and t < w[1]:
                    early += 1
                if c['state'] in ('valid', 'active') and not (w[0] <= t < w[1]):
                    late += 1
    return n > 0 and early == 0 and late == 0, f'{n} cells; expired while the certificate is still valid {early}; valid/active outside its validity {late}'


def r_view_pdu_carrier(R, D):
    short = []
    for q in R.msgs:
        s = sum(p['len'] for p in q.get('pdus') or [])
        if s and (q.get('bytes') or 0) < s:
            short.append(q['name'])
    return not short, f'messages shorter than the SPDUs they carry: {len(short)} {dict(collections.Counter(short))}'


def r_camp_sizing(R, D):
    T = topups(R, D)
    done = [x for x in T.values() if 'installed' in x[1]]
    k = len(done) or 1
    info = sum(1 for _, _, ms in done for m in ms if m['name'] == 'info-request') / k
    files = sum(1 for _, _, ms in done for m in ms if m['name'] == 'batch-download') / k
    # ACA and LA work per account-week: one weekly job per account (the device that asked
    # first carries its week on its top-up flow, every other account's runs on a
    # `pregeneration` flow), so counted over the jobs, not over the top-ups.
    jobs = [xs for xs in D.by_inst.values() if xs and xs[0]['flow'] in ('topup', 'pregeneration')
            and any(x['kind'] == 'stage' and x['name'] == 'expanded' for x in xs)]
    kj = len(jobs) or 1
    aca = sum(1 for xs in jobs for m in xs if m['kind'] == 'msg' for p in m.get('pdus') or [] if p['type'] == 'RaAcaCertRequestSpdu') / kj
    plv = sum(1 for xs in jobs for m in xs if m['kind'] == 'msg' and m['name'] == 'pre-linkage-response' for p in m.get('pdus') or [] if p['type'] == 'EncryptedIndividualPLV') / kj / 2
    rate = 0.5 * CAMP_NEW_PER_YEAR * info / WEEK
    ok = info == 1 and files == 1 and aca == 20 and plv == 20 and 13.5 <= rate <= 14.5
    return ok, f'per device-week: .info {info}, files {files} ({k} top-ups); ACA {aca}, PLV per LA {plv} ({len(jobs)} weekly jobs); .info year-1 rate {rate:.2f} req/s (CAMP 14)'


def r_determinism(R, D, twin=None):
    if twin is None:
        return False, 'no twin run'
    a, b = R.digest, twin.digest
    return a == b, f'pki.flow + node.credentials sha256 {a[:16]} vs {b[:16]}'


ROWS = {
    'PV.request-once': r_request_once,
    'PV.ack-per-request': r_ack_per_request,
    'PV.plv-per-cert': r_plv_per_cert,
    'PV.aca-per-cert': r_aca_per_cert,
    'PV.encrypted-to-device': r_encrypted_to_device,
    'PV.aca-learns-no-device': r_aca_learns_no_device,
    'PV.aca-cannot-group': r_aca_cannot_group,
    'PV.aca-grouping-attack': r_aca_grouping_attack,
    'PV.lop-not-deployed': r_lop_not_deployed,
    'PV.cert-implicit': r_cert_implicit,
    'PV.cert-validity': r_cert_validity,
    'PV.cert-linkage': r_cert_linkage,
    'PV.pool-20-per-week': r_pool_20,
    'PV.pool-at-join': r_pool_at_join,
    'PV.topup-at-boundary': r_topup_at_boundary,
    'PV.topup-no-new-request': r_topup_no_new_request,
    'PV.topup-20-certs': r_topup_20,
    'PV.pregen-delay': r_pregen_delay,
    'PV.download-after-ready': r_download_after_ready,
    'PV.topup-conservation': r_topup_conservation,
    'PV.rsu-renewal': r_rsu_renewal,
    'PV.rsu-cert': r_rsu_cert,
    'PV.m.topups': r_m_topups,
    'PV.m.topup-latency': r_m_topup_latency,
    'PV.m.rsu-app-certs': r_m_rsu,
    'PV.m.backend-traffic': r_m_backend,
    'PV.m.issued-counters': r_m_issued,
    'PV.m.pool-fields': r_m_pool_fields,
    'PV.cell-expiry': r_cell_expiry,
    'PV.view-pdu-carrier': r_view_pdu_carrier,
    'PV.camp-sizing': r_camp_sizing,
    'PV.determinism': r_determinism,
}


# --------------------------------------------------------------------------------------------
# faults: one per row, on a deep copy of the records

def _first(xs, pred):
    return next(x for x in xs if pred(x))


def _mut_cert(R, fn):
    """Re-encode one pseudonym certificate with `fn` applied to its decoded value."""
    for dev in sorted(R.creds):
        for q in R.creds[dev]:
            for k, h in (q.get('certificates') or {}).items():
                c = coer.decode('Certificate', bytes.fromhex(h))
                if c['toBeSigned'].get('crlSeries') == 1:
                    fn(c)
                    q['certificates'][k] = coer.encode('Certificate', c).hex()
                    return


def _bump(d, k, by=1):
    d[k] += by


FAULTS = {
    'PV.request-once': lambda R, D: D.pre.append(dict(_first(D.pre, lambda p: p['type'] == 'EeRaCertRequestSpdu' and p['from'] in D.vehicles))),
    'PV.ack-per-request': lambda R, D: D.pre.remove(_first(D.pre, lambda p: p['type'] == 'RaEeCertAckSpdu')),
    'PV.plv-per-cert': lambda R, D: _first(D.pre, lambda p: p['type'] == 'EncryptedIndividualPLV').__setitem__('sealed_to', 'provider-a/ra/0'),
    'PV.aca-per-cert': lambda R, D: D.pre.remove(_first(D.pre, lambda p: p['type'] == 'AcaRaCertResponseSpdu')),
    'PV.encrypted-to-device': lambda R, D: _first(D.pre, lambda p: p['type'] == 'AcaEeCertResponsePrivateSpdu').__setitem__('sealed_to', None),
    'PV.aca-learns-no-device': lambda R, D: (lambda p, e: p.__setitem__('hex', p['hex'] + e))(
        _first(D.pre, lambda p: p['type'] == 'RaAcaCertRequestSpdu' and p.get('hex')), R.creds[sorted(R.creds)[0]][0]['enrolment']),
    'PV.aca-cannot-group': lambda R, D: _per_device_instants(R),
    'PV.aca-grouping-attack': lambda R, D: _arrival_by_device(R),
    'PV.lop-not-deployed': lambda R, D: R.msgs.append(dict(R.msgs[0], to='provider-a/lop/0')),
    'PV.cert-implicit': lambda R, D: _mut_cert(R, lambda c: c.__setitem__('type', 'explicit')),
    'PV.cert-validity': lambda R, D: _mut_cert(R, lambda c: c['toBeSigned']['validityPeriod'].__setitem__('duration', ('hours', 168))),
    'PV.cert-linkage': lambda R, D: _mut_cert(R, lambda c: c['toBeSigned']['id'][1].__setitem__('iCert', c['toBeSigned']['id'][1]['iCert'] + 1)),
    'PV.pool-20-per-week': lambda R, D: (lambda q: q['certificates'].pop(sorted(q['certificates'])[0]))(R.creds[sorted(D.vehicles)[0]][0]),
    'PV.pool-at-join': lambda R, D: (lambda q: [q['certificates'].pop(k) for k in sorted(q['certificates'])
                                                 if i_of_time32(coer.decode('Certificate', bytes.fromhex(q['certificates'][k]))['toBeSigned']['validityPeriod']['start']) == 615])(R.creds[sorted(D.vehicles)[0]][0]),
    'PV.topup-at-boundary': lambda R, D: R.stages.remove(_first(R.stages, lambda s: s['flow'] == 'topup' and s['name'] == 'requested')),
    'PV.topup-no-new-request': lambda R, D: _first(R.msgs, lambda q: q['flow'] == 'topup' and q['name'] == 'info-request').__setitem__(
        'pdus', [dict(_first(D.pre, lambda p: p['type'] == 'EeRaCertRequestSpdu' and p['from'] in D.vehicles))]),
    'PV.topup-20-certs': lambda R, D: _first(R.msgs, lambda q: q['name'] == 'batch-download').__setitem__('pdus', _first(R.msgs, lambda q: q['name'] == 'batch-download')['pdus'][:19]),
    'PV.pregen-delay': lambda R, D: _first(R.stages, lambda s: s['flow'] == 'topup' and s['name'] == 'expanded').__setitem__('t', 600 * NS + (WEEK + 1) * NS),
    'PV.download-after-ready': lambda R, D: _first(R.msgs, lambda q: q['name'] == 'batch-download-request' and q['flow'] == 'topup').__setitem__('t', 1 * NS),
    'PV.topup-conservation': lambda R, D: R.stages.append(dict(_first(R.stages, lambda s: s['flow'] == 'topup' and s['name'] == 'installed'), instance='x/topup/dup', t=1)),
    'PV.rsu-renewal': lambda R, D: R.msgs.append(dict(_first(R.msgs, lambda q: q['name'] == 'app-cert-request'))),
    'PV.rsu-cert': None,              # red on the real records (no series-3 certificate recorded)
    'PV.m.topups': lambda R, D: _bump(R.report, 'certs_topped_up'),
    'PV.m.topup-latency': lambda R, D: _bump(R.report['access']['exchange_latency']['topup'], 'sum_ns', NS),
    'PV.m.rsu-app-certs': lambda R, D: _bump(R.report, 'rsu_app_certs_renewed'),
    'PV.m.backend-traffic': lambda R, D: _bump(next(e for e in R.backend['entities'] if e['id'] == 'la1')['traffic'], 'received'),
    'PV.m.issued-counters': lambda R, D: _bump(next(c for pv in R.pki_state['providers'] for c in pv['components'] if c['role'] == 'la1')['counters'], 'plv_issued'),
    'PV.m.pool-fields': lambda R, D: next(r for rows in R.sec.values() for t, r in rows if R.node_dev.get(r['node']) in D.vehicles).__setitem__('pool_stored', 99),
    'PV.cell-expiry': lambda R, D: _first(R.creds[sorted(D.vehicles)[0]][0]['cells'], lambda c: c['state'] == 'valid').__setitem__('state', 'expired'),
    'PV.view-pdu-carrier': lambda R, D: _first(R.msgs, lambda q: q.get('pdus') and q['name'] == 'batch-download').__setitem__('bytes', 10),
    'PV.camp-sizing': lambda R, D: _first(R.msgs, lambda q: q['flow'] == 'topup' and q['name'] == 'info-request').__setitem__('name', 'x'),
    'PV.determinism': lambda R, D: setattr(R, 'digest', hashlib.sha256(R.digest.encode() + b'x').hexdigest()),
}


def _per_device_instants(R):
    """The pre-fix shape: each device's week leaves the RA on its own, at its own instant,
    in one message (the batch then holds one device)."""
    groups = collections.defaultdict(list)
    keep = []
    for q in R.msgs:
        if role(q.get('from')) == 'ra' and role(q.get('to')) in ('aca', 'pca') and any(
                p['type'] == 'RaAcaCertRequestSpdu' for p in q.get('pdus') or []):
            groups[q['instance']].append(q)
        else:
            keep.append(q)
    for d, (inst, qs) in enumerate(sorted(groups.items())):
        pdus = [p for q in qs for p in q['pdus']]
        keep.append(dict(qs[0], pdus=pdus, bytes=sum(p['len'] for p in pdus), t=qs[0]['t'] + d + 1))
    R.msgs[:] = keep


def _arrival_by_device(R):
    """An unshuffled release: each batch's requests arrive grouped by device."""
    idx = [k for k, q in enumerate(R.msgs) if role(q.get('from')) == 'ra' and role(q.get('to')) in ('aca', 'pca')
           and any(p['type'] == 'RaAcaCertRequestSpdu' for p in q.get('pdus') or [])]
    qs = sorted((R.msgs[k] for k in idx), key=lambda q: (q['from'], q['t'], q['instance']))
    for k, q in zip(idx, qs):
        R.msgs[k] = q


# rows red on the real records: a repair of the records that must turn them green, so a
# red row is shown to be a finding and not a rule that cannot pass
GREEN = {}


def judge(R, D, row, twin=None):
    f = ROWS[row]
    if row == 'PV.determinism':
        return f(R, D, twin)
    return f(R, D)


def main():
    a = sys.argv[1:]
    run = a[0]
    opt = dict(zip(a[1::2], a[2::2]))
    R = load(run)
    D = derive(R)
    twin = load(opt['--twin']) if '--twin' in opt else None
    results = {}
    for row in ROWS:
        try:
            ok, meas = judge(R, D, row, twin)
        except Exception as e:  # a row that cannot be judged is red
            ok, meas = False, f'error {type(e).__name__}: {e}'
        results[row] = (bool(ok), meas)
        print(('PASS ' if ok else 'FAIL ') + row + ': ' + meas)
    print(f'VERDICT {sum(v[0] for v in results.values())}/{len(results)} rows pass')
    mut = opt.get('--mutate')
    if mut:
        silent = []
        for row in (ROWS if mut == 'all' else [mut]):
            fault = FAULTS.get(row)
            if fault is None:
                print(f'MUTATE {row}: n/a (red on the real records)')
                continue
            R2 = copy.deepcopy(R)
            D2 = derive(R2)
            fault(R2, D2)
            # D2.pre holds R2's own PDU dicts; a fault may add or drop entries of that list
            D3 = derive(R2)
            D3.pre = D2.pre
            try:
                ok, meas = judge(R2, D3, row, twin)
            except Exception as e:
                ok, meas = False, f'error {e}'
            if ok:
                silent.append(row)
            print(f'MUTATE {row}: {"SILENT" if ok else "red"} ({meas[:120]})')
        print(f'MUTATE silent: {silent}')
        for row, fix in GREEN.items():
            R2 = copy.deepcopy(R)
            fix(R2, None)
            D3 = derive(R2)
            ok, meas = judge(R2, D3, row, twin)
            print(f'GREEN {row}: {"green" if ok else "still red"} ({meas[:120]})')
    if '--json' in opt:
        json.dump({k: {'pass': v[0], 'measured': v[1]} for k, v in results.items()}, open(opt['--json'], 'w'), indent=1)


if __name__ == '__main__':
    main()
