#!/usr/bin/env python3
"""SCMS audit, governance and trust anchors (docs/design/scms-real/testing/audit-gov.md).

Reads one run's raw records (recording.mcap, run-report.json, scenario.resolved.yaml) and
judges the rows of crates/v2xw-engine/tests/expectations/scms-audit-gov.yaml. Nothing here
reads engine code or engine-computed summaries except the metric being recomputed: the
certificates are decoded with the independent oracle's COER codec
(tools/oracle/scms_oracle), the published certificates come from
third_party/scms-artefacts (SCMS Manager's files), and Time32, the i-period and the
Eastern business days are computed here from their definitions.

    audit_gov.py RUN [--base OTHER_RUN]      judge every row (DET.* needs --base)
    audit_gov.py RUN --mutate ROW            inject ROW's fault; ROW must go red
    audit_gov.py RUN --selftest [--base B]   every row's fault in turn (each must go red)

Prints one line per row: id, PASS/FAIL, measured.
"""
import copy
import datetime as dt
import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, '..', '..'))
sys.path.insert(0, os.path.join(REPO, 'tools', 'oracle'))
from scms_oracle import certs, chain, coer, ec, prims  # noqa: E402

def _artefacts():
    """third_party/scms-artefacts/files (git-ignored): this checkout's, else the main checkout's."""
    if os.environ.get('SCMS_ARTEFACTS'):
        return os.environ['SCMS_ARTEFACTS']
    here = os.path.join(REPO, 'third_party', 'scms-artefacts', 'files')
    if os.path.isdir(here):
        return here
    m = re.match(r'(.*)/\.claude/worktrees/[^/]+$', REPO)
    return os.path.join(m.group(1) if m else REPO, 'third_party', 'scms-artefacts', 'files')


ARTEFACTS = _artefacts()


def validate_chain(ch, anchors):
    """[subject, issuer, ..., root]: issuer HashedId8, explicit signatures, validity nesting,
    region containment, the root self-signed and on the CTL. Chain-length permissions are not
    judged: the oracle's group rule (chain.check_pair) rejects SCMS Manager's own published
    production chain (audit-gov.md §3), so it cannot decide this."""
    errs = []
    for n, c in enumerate(ch):
        if n + 1 < len(ch):
            iss = ch[n + 1]
            if c['issuer'][0] == 'self' or c['issuer'][1] != certs.hashed_id8(iss):
                errs.append(f'cert {n}: issuer HashedId8 is not cert {n + 1}')
                continue
            if c['type'] == 'explicit' and not certs.verify_explicit(c, iss):
                errs.append(f'cert {n}: signature does not verify')
            s0, s1 = chain.validity(c['toBeSigned'])
            i0, i1 = chain.validity(iss['toBeSigned'])
            if s0 < i0 or s1 > i1:
                errs.append(f'cert {n}: validity outside its issuer')
            sr, ir = c['toBeSigned'].get('region'), iss['toBeSigned'].get('region')
            if sr and ir and sr[0] == ir[0] == 'identifiedRegion' and \
                    not {x[1] for x in sr[1]} <= {x[1] for x in ir[1]}:
                errs.append(f'cert {n}: region outside its issuer')
        else:
            if c['issuer'][0] != 'self':
                errs.append('chain does not end at a self-signed root')
            elif not certs.verify_explicit(c, None):
                errs.append('root self-signature does not verify')
            elif prims.sha256(certs.encode_canonical(c)) not in anchors:
                errs.append('root not on the CTL')
    return errs

# --- definitions, computed here ----------------------------------------------------------
TIME32_EPOCH = 1072915200          # 2004-01-01T00:00:00Z
LEAPS_UNIX = [1136073600, 1230768000, 1341100800, 1435708800, 1483228800]  # after 2005-12-31 ... 2016-12-31
IPERIOD_EPOCH = 1420534800         # 2015-01-06 09:00:00 UTC = 04:00 EST ([SMP] Table 2)
WEEK = 604800
# OPM 2026 federal holidays (observed dates), 5 U.S.C. 6103.
HOLIDAYS = {dt.date(2026, m, d) for m, d in [(1, 1), (1, 19), (2, 16), (5, 25), (6, 19), (7, 3), (9, 7),
                                              (10, 12), (11, 11), (11, 26), (12, 25)]}


def time32(unix):
    return int(unix) - TIME32_EPOCH + sum(1 for l in LEAPS_UNIX if unix >= l)


def eastern(utc):
    """US Eastern wall time of a UTC datetime (DST: second Sunday of March to first Sunday of November, 2 am)."""
    y = utc.year

    def nth_sunday(month, n):
        d = dt.datetime(y, month, 1)
        d += dt.timedelta(days=(6 - d.weekday()) % 7)
        return d + dt.timedelta(weeks=n - 1)
    start = nth_sunday(3, 2) + dt.timedelta(hours=7)   # 2 am EST = 07:00 UTC
    end = nth_sunday(11, 1) + dt.timedelta(hours=6)    # 2 am EDT = 06:00 UTC
    off = -4 if start <= utc.replace(tzinfo=None) < end else -5
    return utc.replace(tzinfo=None) + dt.timedelta(hours=off), off


def add_business_days(utc, n):
    """`n` business days after `utc`, on the Eastern calendar, same Eastern wall time."""
    local, off = eastern(utc)
    d = local
    while n > 0:
        d += dt.timedelta(days=1)
        if d.weekday() < 5 and d.date() not in HOLIDAYS:
            n -= 1
    _, off2 = eastern(d - dt.timedelta(hours=off))
    return (d - dt.timedelta(hours=off2)).replace(tzinfo=dt.timezone.utc)


# --- loading -------------------------------------------------------------------------------
def _t0(run):
    for line in open(os.path.join(run, 'scenario.resolved.yaml')):
        m = re.match(r"\s*t0:\s*['\"]?([0-9T:\-]+)Z?['\"]?", line)
        if m:
            return dt.datetime.fromisoformat(m.group(1)).replace(tzinfo=dt.timezone.utc)
    raise SystemExit('no time.t0')


def load(run):
    from mcap.reader import make_reader
    R = {'run': run, 't0': _t0(run).timestamp()}
    R['n_rsus'] = sum(1 for line in open(os.path.join(run, 'scenario.resolved.yaml')) if 'position_m' in line)
    R['report'] = json.load(open(os.path.join(run, 'run-report.json')))['report']
    R['states'] = []       # (t_ns, time32, ctl or None)
    R['first'] = R['last'] = None
    R['creds'] = {}        # device -> [(t_ns, ctl_seq, provider, enrolment_id8, current_i)]
    R['certs'] = {}        # id8 -> hex, every certificate node.credentials carries
    R['owner'] = {}        # id8 -> device
    R['sec'] = []          # (t_ns, node, cert_i)
    R['enrol'] = []        # (type, from, to, t_ns, msg provider)
    R['ra_first'] = {}     # device -> first EeRaCertRequestSpdu t_ns
    R['allcerts'] = {}     # id8 -> hex, every certificate any pki.state carried
    R['comp_hist'] = {}    # (label, role) -> [(t_ns, cert id8)] when it changed
    R['cred_first'] = {}   # certificate id8 -> first node.credentials t_ns carrying it
    h = hashlib.sha256()
    with open(os.path.join(run, 'recording.mcap'), 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            h.update(ch.topic.encode())
            h.update(m.log_time.to_bytes(8, 'little'))
            h.update(len(m.data).to_bytes(8, 'little'))
            h.update(m.data)
            tp = ch.topic
            if tp == 'record/pki.state':
                q = json.loads(m.data)
                c = q.get('ctl')
                R['states'].append((q['t'], q.get('time32'), None if c is None else {
                    k: c.get(k) for k in ('sequence', 'quorum', 'electors', 'roots', 'effective', 'signers', 'history')}))
                if R['first'] is None:
                    R['first'] = q
                R['last_raw'] = m.data
                R['allcerts'].update(q.get('certificates') or {})
                for p in q.get('providers') or []:
                    for x in p.get('components') or []:
                        if x.get('instance') == 0 and x.get('cert'):
                            h = R['comp_hist'].setdefault((p['label'], x['role']), [])
                            if not h or h[-1][1] != x['cert']:
                                h.append((q['t'], x['cert']))
            elif tp == 'record/node.credentials':
                q = json.loads(m.data)
                R['creds'].setdefault(q['device'], []).append(
                    (q['t'], q.get('ctl_sequence'), q.get('provider'), q.get('enrolment'), q.get('current_i')))
                for k, v in (q.get('certificates') or {}).items():
                    R['certs'].setdefault(k, v)
                    R['owner'].setdefault(k, q['device'])
                    R['cred_first'].setdefault(k, q['t'])
            elif tp == 'record/node.security':
                q = json.loads(m.data)
                R['sec'].append((q['t'], q['node'], q.get('cert_i')))
            elif tp == 'record/pki.flow':
                q = json.loads(m.data)
                for d in q.get('pdus') or []:
                    if d['type'] in ('EeEcaCertRequestSpdu', 'EcaEeCertResponseSpdu'):
                        R['enrol'].append((d['type'], d['from'], d['to'], d['t'], q.get('provider')))
                    elif d['type'] == 'EeRaCertRequestSpdu' and d['from'].startswith('ee/'):
                        R['ra_first'].setdefault(d['from'], d['t'])
    R['last'] = json.loads(R.pop('last_raw'))
    R['digest'] = h.hexdigest()
    return R


def dec_cert(hx):
    return coer.decode('Certificate', bytes.fromhex(hx))


def name_of(c):
    i = c['toBeSigned']['id']
    return i[1] if i[0] == 'name' else None


def comps(state):
    """(label, role, instance) -> component dict."""
    return {(p['label'], x['role'], x['instance']): x for p in state['providers'] for x in p['components']}


def published():
    """Published certificate per (form, role): ISS preprod + production root, SAESOL production/test."""
    def chain_file(p):
        outer = coer.decode('Ieee1609Dot2Data', open(os.path.join(ARTEFACTS, p), 'rb').read())
        kind, body = coer.decode('ScmsPdu', outer['content'][1])['content'][1]
        return body['others'] if kind == 'certificateChain' else []
    test = chain_file('test-2025-07-17/test_cert_chain.oer')
    prod = chain_file('production-2025-08-13/cert-chain_0.oer')
    out = {}
    role_of = [('rootca', 'root'), ('ica.', 'ica'), ('eca.', 'eca'), ('pca.', 'aca'), ('aca.', 'aca'), ('ra.', 'ra'),
               ('la1.', 'la1'), ('la2.', 'la2'), ('ma.', 'ma'), ('crlg.', 'crlg'), ('pg', 'pg')]

    def role(n):
        n = n.split('.', 1)[0] + '.' if not n.startswith('v2x') else n
        for p, r in role_of:
            if n.startswith(p) or (p == 'pg' and 'pg' in n.split('.')[0]):
                return r
        if n.startswith('dot1.'):
            return 'eca'
        return None
    for c in test:
        n = name_of(c)
        if 'isscms' in n or 'ghsiss' in n:
            r = 'root' if 'rootca' in n else role(n)
            out.setdefault(('iss', r), c)
        elif 'saesoltech' in n:
            out.setdefault(('s2x', role(n)), c)
    for c in prod:   # production overrides test for SAESOL
        out[('s2x', role(name_of(c)))] = c
    return out


FORM = {'provider-a': 'iss', 'provider-b': 's2x'}
ISS_ROOT_PRODUCTION_YEARS = 70   # v2xrootca.ghsiss.com in the production CTL (gov L-1); the test root is 40 y


def secmgmt(c):
    for p in c['toBeSigned'].get('appPermissions') or []:
        if p['psid'] == 35 and p.get('ssp'):
            return p['ssp'][1]
    return None


# --- the rows -------------------------------------------------------------------------------
def r_G3(R):
    bad = [s for s in R['states'] if s[2] and s[2]['electors'] != 5]
    el = [g for g in R['first']['governance'] if g['role'] == 'elector']
    ok = not bad and len(el) == 5 and len({g['cert'] for g in el}) == 5
    return ok, f'{len(el)} elector entities, {len({g["cert"] for g in el})} certs, {len(bad)} states with electors != 5'


def r_G4(R):
    qs = {s[2]['quorum'] for s in R['states'] if s[2]}
    return qs == {3}, f'quorum values {sorted(qs)}'


def r_G5(R):
    hist = R['states'][-1][2]['history']
    n = [len(h['signers']) for h in hist]
    return all(x == 5 for x in n), f'signers per CTL {n}'


def root_certs(R, state):
    out = {}
    for (lab, role, _), x in comps(state).items():
        if role == 'root' and x['cert'] and x['cert'] in state['certificates']:
            out[lab] = state['certificates'][x['cert']]
    return out


def r_C1(R):
    roots = root_certs(R, R['first'])
    want = {prims.sha256(bytes.fromhex(h)).hex() for h in roots.values()}
    got = set(R['states'][0][2]['roots'])
    return want == got and len(want) == 2, f'CTL0 roots {len(got)}, recomputed {len(want)}, equal {want == got}'


def ctl_changes(R):
    out, prev = [], None
    for t, v, c in R['states']:
        if c and c['sequence'] != prev:
            out.append((t, v, c))
            prev = c['sequence']
    return out


def r_L6(R):
    ch = ctl_changes(R)
    if len(ch) < 2:
        return False, f'{len(ch)} CTLs seen'
    a, b = set(ch[0][2]['roots']), set(ch[1][2]['roots'])
    ok = ch[1][2]['sequence'] == ch[0][2]['sequence'] + 1 and a < b and len(b - a) == 1
    return ok, f'seq {ch[0][2]["sequence"]}->{ch[1][2]["sequence"]}, roots {len(a)}->{len(b)}, kept {a <= b}'


def r_C6(R):
    ch = ctl_changes(R)
    if len(ch) < 2:
        return False, 'no CTL 1'
    t0 = dt.datetime.fromtimestamp(R['t0'], dt.timezone.utc)
    dec = t0 + dt.timedelta(seconds=120)
    dl = add_business_days(dec, 3)
    pub = t0 + dt.timedelta(microseconds=ch[1][0] // 1000)
    return dec < pub <= dl, f'published {pub:%a %Y-%m-%d %H:%M:%S}Z (t={ch[1][0] / 1e9:.0f} s), deadline {dl:%a %Y-%m-%d %H:%M}Z'


def r_C9(R):
    ch = ctl_changes(R)
    if len(ch) < 2:
        return False, 'no CTL 1'
    pub32 = time32(R['t0'] + ch[1][0] / 1e9)
    e0, e1 = ch[0][2]['effective'], ch[1][2]['effective']
    return e1 == pub32 and e1 > e0, f'effective {e1}, Time32(publication) {pub32}, CTL0 {e0}'


def r_C12m(R):
    pubs = [(t, c['sequence']) for t, _, c in ctl_changes(R)]
    bad = 0
    for d, recs in R['creds'].items():
        s = [r[1] for r in recs if r[1] is not None]
        bad += sum(1 for a, b in zip(s, s[1:]) if b < a)
        for r in recs:
            cur = max((q for t, q in pubs if t <= r[0]), default=None)
            if r[1] is not None and cur is not None and r[1] > cur:
                bad += 1
    return bad == 0, f'{bad} violations over {len(R["creds"])} devices'


def record_installs(R):
    n = 0
    for recs in R['creds'].values():
        s = [r[1] for r in recs if r[1] is not None]
        n += sum(1 for a, b in zip(s, s[1:]) if b > a)
    return n


def r_C12r(R):
    m = R['report']['phase2']['trust_lists_installed']
    n = record_installs(R)
    return 0 <= m - n <= R['n_rsus'], f'trust_lists_installed {m}, record installs {n}, RSUs {R["n_rsus"]}'


def r_C12v(R):
    p = R['report']['phase2']
    hist = R['states'][-1][2]['history']
    per = len(hist[-1]['signers']) + hist[-1]['roots'] if len(hist) > 1 else 0
    return p['trust_list_verifications'] == p['trust_lists_installed'] * per, \
        f'verifications {p["trust_list_verifications"]} = installs {p["trust_lists_installed"]} x {per}?'


def r_Mctl(R):
    p = R['report']['phase2']
    ch = len(ctl_changes(R)) - 1
    seq = R['states'][-1][2]['sequence']
    el = {g['counters'].get('ctl_sequence') for g in R['last']['governance'] if g['role'] == 'elector'}
    ok = p['ctls_published'] == ch == 1 and p['gov_decisions'] == 1 and el == {seq}
    return ok, f'ctls_published {p["ctls_published"]}, changes {ch}, gov_decisions {p["gov_decisions"]}, elector ctl_sequence {sorted(el)} vs {seq}'


def component_rows(R):
    """[(label, role, decoded, published or None)] for every component certificate at t0."""
    pub = R.setdefault('_pub', published())
    out = []
    for (lab, role, inst), x in comps(R['first']).items():
        h = R['first']['certificates'].get(x['cert'] or '')
        if not h:
            continue
        out.append((lab, role, dec_cert(h), pub.get((FORM.get(lab), role))))
    return out


def _field_rows(R, fn, label):
    bad, n = [], 0
    for lab, role, c, p in component_rows(R):
        if p is None:
            continue
        n += 1
        a, b = fn(lab, role, c, p)
        if a != b:
            bad.append(f'{lab}/{role} {a} vs {b}')
    return not bad, f'{n} compared; ' + ('; '.join(bad[:6]) if bad else 'all equal')


def _dur(lab, role, c, p):
    got = tuple(c['toBeSigned']['validityPeriod']['duration'])
    want = tuple(p['toBeSigned']['validityPeriod']['duration'])
    if FORM.get(lab) == 'iss' and role == 'root':
        want = ('years', ISS_ROOT_PRODUCTION_YEARS)
    return got, want


def r_L1(R):
    return _field_rows(R, _dur, 'duration')


def r_L3(R):
    return _field_rows(R, lambda l, r, c, p: (c['toBeSigned']['crlSeries'], p['toBeSigned']['crlSeries']), 'crl')


def r_L4(R):
    return _field_rows(R, lambda l, r, c, p: (c['toBeSigned'].get('encryptionKey') is not None,
                                              p['toBeSigned'].get('encryptionKey') is not None), 'enc')


def r_L5(R):
    return _field_rows(R, lambda l, r, c, p: (c['toBeSigned'].get('region') is not None,
                                              p['toBeSigned'].get('region') is not None), 'region')


def r_L10(R):
    def f(lab, role, c, p):
        a, b = secmgmt(c), secmgmt(p)
        if a is None or b is None:
            return a, b
        if FORM.get(lab) == 's2x' and role not in ('la1', 'la2'):
            return a.hex(), b.hex()
        return a[:3].hex(), b[:3].hex()
    return _field_rows(R, f, 'ssp')


def r_Lchain(R):
    st = R['first']
    by = {}
    for h in st['certificates'].values():
        c = dec_cert(h)
        by[certs.hashed_id8(c)] = c
    anchors = {bytes.fromhex(x) for x in R['states'][0][2]['roots']}
    bad, n = [], 0
    for c in by.values():
        ch = [c]
        while ch[-1]['issuer'][0] != 'self' and ch[-1]['issuer'][1] in by and len(ch) < 6:
            ch.append(by[ch[-1]['issuer'][1]])
        n += 1
        errs = validate_chain(ch, anchors)
        if errs:
            bad.append(f'{name_of(c)}: {errs[0]}')
    return not bad, f'{n} chains; ' + ('; '.join(bad[:4]) if bad else '0 failures')


def r_Lnames(R):
    bad = []
    for lab, role, c, _ in component_rows(R):
        n = name_of(c) or ''
        if lab not in n.split('.'):
            bad.append(f'{lab}/{role}={n}')
    return not bad, f'{len(bad)} mismatches ' + ' '.join(bad[:4])


def r_Lone(R):
    bad, n = [], 0
    for (lab, role, inst), x in comps(R['first']).items():
        cc = (x.get('counters') or {}).get('certificate')
        if x['cert'] and cc:
            n += 1
            if cc != x['cert']:
                bad.append(f'{lab}/{role}')
    return not bad, f'{n} with both; differ: {len(bad)} ' + ' '.join(bad[:8])


def r_Lcnt(R):
    st = R['first']
    issued = {}
    for h in st['certificates'].values():
        c = dec_cert(h)
        k = 'self:' + certs.hashed_id8(c).hex() if c['issuer'][0] == 'self' else c['issuer'][1].hex()
        issued[k] = issued.get(k, 0) + 1
    bad, txt = [], []
    for p in st['providers']:
        cs = {x['role']: x for x in p['components'] if x['instance'] == 0}
        unrec = sum(1 for x in p['components'] if not x['cert'])
        for role in ('root', 'ica'):
            x = cs.get(role)
            if not x:
                continue
            obs = issued.get(x['cert'], 0) + (issued.get('self:' + x['cert'], 0) if role == 'root' else 0)
            got = x['counters'].get('certs_issued')
            txt.append(f'{p["label"]}/{role} {got} (records {obs}+{unrec})')
            if got is None or not obs <= got <= obs + unrec:
                bad.append(role)
    return not bad, ', '.join(txt)


def r_D1(R):
    acas = [x for (lab, role, _), x in comps(R['first']).items() if lab == 'provider-b' and role == 'aca']
    certd = [x for x in acas if x['cert']]
    return len(acas) == 2 and len(certd) == 2, f'{len(acas)} ACAs, {len(certd)} with a certificate'


def enrolments(R):
    """device -> (provider, decoded enrolment certificate)."""
    out = {}
    for d, recs in R['creds'].items():
        for r in recs:
            if r[3] and r[3] in R['certs']:
                out.setdefault(d, (r[2], dec_cert(R['certs'][r[3]])))
                break
    return out


def r_E1(R):
    e = enrolments(R)
    durs = {tuple(c['toBeSigned']['validityPeriod']['duration']) for _, c in e.values()}
    return durs == {('hours', 52560)}, f'{len(e)} certificates, durations {sorted(durs)}'


def eca_of(R):
    """label -> every ECA certificate id8 the provider used; (label, role) likewise for the root and ICA."""
    out = {}
    for (lab, role), hist in R['comp_hist'].items():
        if role == 'eca':
            out.setdefault(lab, set()).update(c for _, c in hist)
        if role in ('root', 'ica'):
            out.setdefault((lab, role), set()).update(c for _, c in hist)
    return out


def up(R, hx):
    """The issuer id8 (hex) of the certificate with id8 `hx`, from every pki.state certificate."""
    h = R['allcerts'].get(hx)
    if not h:
        return None
    c = dec_cert(h)
    return None if c['issuer'][0] == 'self' else c['issuer'][1].hex()


def r_E3(R):
    e, ids = enrolments(R), eca_of(R)
    bad = []
    for d, (lab, c) in e.items():
        t = c['toBeSigned']
        reg = t.get('region')
        regs = sorted(x[1] for x in reg[1]) if reg and reg[0] == 'identifiedRegion' else None
        eca = c['issuer'][1].hex() if c['issuer'][0] == 'sha256AndDigest' else ''
        ica = up(R, eca) or ''
        craca = ((up(R, ica) or '') if FORM.get(lab) == 'iss' else ica)[-6:]
        why = [k for k, ok in [('type', c['type'] == 'implicit'), ('crlSeries', t['crlSeries'] == 4),
                               ('region', regs == [124, 484, 840]), ('crp', bool(t.get('certRequestPermissions'))),
                               ('issuer', eca in ids.get(lab, ())),
                               ('cracaId', t['cracaId'].hex() == craca)] if not ok]
        if why:
            bad.append(f'{d}:{",".join(why)}')
    return not bad, f'{len(e)} certificates, {len(bad)} violations ' + ' '.join(bad[:4])


def r_E3n(R):
    e = enrolments(R)
    names = [name_of(c) for _, c in e.values()]
    wrong = [f'{d}({lab})={name_of(c)}' for d, (lab, c) in e.items() if not (name_of(c) or '').endswith('.' + lab)]
    return len(set(names)) == len(names) and not wrong, \
        f'{len(names)} names, {len(set(names))} distinct, {len(wrong)} name another provider ' + ' '.join(wrong[:2])


def r_E3k(R):
    e = enrolments(R)
    keys = [repr(c['toBeSigned']['verifyKeyIndicator']) for _, c in e.values()]
    return len(set(keys)) == len(keys), f'{len(keys)} keys, {len(set(keys))} distinct'


def r_Echain(R):
    e = enrolments(R)
    by = {}
    for h in R['allcerts'].values():
        c = dec_cert(h)
        by[certs.hashed_id8(c)] = c
    anchors = {bytes.fromhex(x) for _, _, c in R['states'] if c for x in c['roots']}
    bad = []
    for d, (lab, c) in e.items():
        ch = [c]
        while ch[-1]['issuer'][0] != 'self' and ch[-1]['issuer'][1] in by and len(ch) < 6:
            ch.append(by[ch[-1]['issuer'][1]])
        errs = validate_chain(ch, anchors)
        if len(ch) < 2:
            errs.append('no ECA')
        else:
            try:
                C, Q = certs.verification_key(c, ch[1])
                if not ec.on_curve(C, Q):
                    errs.append('key off the curve')
            except Exception as x:  # noqa: BLE001
                errs.append(f'reconstruction: {x}')
        if errs:
            bad.append(f'{d}: {errs[0]}')
    return not bad, f'{len(e)} chains, {len(bad)} failures ' + '; '.join(bad[:3])


def r_Eonce(R):
    per = {}
    for typ, fr, to, t, prov in R['enrol']:
        if typ == 'EeEcaCertRequestSpdu' and fr.startswith('ee/'):
            per.setdefault(fr, {'req': [], 'rsp': [], 'prov': set()})['req'].append(t)
        elif to.startswith('ee/'):
            p = per.setdefault(to, {'req': [], 'rsp': [], 'prov': set()})
            p['rsp'].append(t)
            p['prov'].add(prov)
    dev_prov = {d: recs[0][2] for d, recs in R['creds'].items()}
    bad = []
    for d, p in per.items():
        ok = len(p['req']) == 1 and len(p['rsp']) == 1 and p['req'][0] <= p['rsp'][0] \
            and (d not in R['ra_first'] or p['rsp'][0] <= R['ra_first'][d]) \
            and (d not in dev_prov or p['prov'] == {dev_prov[d]})
        if not ok:
            bad.append(d)
    return not bad, f'{len(per)} devices, {len(bad)} violations ' + ' '.join(bad[:4])


def r_Ecnt(R):
    by = {}
    for typ, fr, to, t, prov in R['enrol']:
        if typ == 'EcaEeCertResponseSpdu' and to.startswith('ee/'):
            by.setdefault(prov, set()).add(to)
    txt, bad = [], False
    for p in R['last']['providers']:
        eca = next(x for x in p['components'] if x['role'] == 'eca')
        a, b, c = eca['counters'].get('enrolment_certs_issued'), p['devices']['enrolled'], len(by.get(p['label'], ()))
        txt.append(f'{p["label"]}: issued {a}, enrolled {b}, records {c}')
        bad |= not (a == b == c)
    return not bad, '; '.join(txt)


def r_Edcm(R):
    """Every device's EeEcaCertRequestSpdu to its DCM is forwarded to its provider's ECA (DCM->ECA on pki.flow)."""
    reqs, fwd = {}, {}
    for typ, fr, to, t, prov in R['enrol']:
        if typ != 'EeEcaCertRequestSpdu':
            continue
        if fr.startswith('ee/') and '/dcm/' in to:
            reqs.setdefault(prov, []).append(t)
        elif '/dcm/' in fr and '/eca/' in to:
            fwd.setdefault(prov, []).append(t)
    txt, bad = [], False
    for p in sorted(set(reqs) | set(fwd), key=str):
        a, b = sorted(reqs.get(p, [])), sorted(fwd.get(p, []))
        ok = len(a) == len(b) and len(a) > 0 and all(x <= y for x, y in zip(a, b))
        bad |= not ok
        txt.append(f'{p}: {len(a)} EE->DCM, {len(b)} DCM->ECA')
    return bool(txt) and not bad, '; '.join(txt) or 'no enrolment requests'


def r_Lroll(R):
    """After CTL 1 publishes the home provider's new root: certificates first held after the
    publication chain to the new root, earlier ones to the old root, and the home ECA and ACA in
    pki.state are re-certified (Provider Req. 3.5.4)."""
    ch = ctl_changes(R)
    if len(ch) < 2:
        return False, 'no CTL 1'
    pub = ch[1][0]
    new = set(ch[1][2]['roots']) - set(ch[0][2]['roots'])
    if len(new) != 1:
        return False, f'{len(new)} roots added'
    def root_of(hx):
        seen = 0
        while hx and seen < 6:
            h = R['allcerts'].get(hx) or R['certs'].get(hx)
            if not h:
                return None
            c = dec_cert(h)
            if c['issuer'][0] == 'self':
                return prims.sha256(bytes.fromhex(h)).hex()
            hx, seen = c['issuer'][1].hex(), seen + 1
        return None
    after = before = 0
    bad = []
    for k, t in R['cred_first'].items():
        if R['creds'].get(R['owner'].get(k), [(0, 0, None)])[0][2] != 'provider-a':
            continue
        r = root_of(k)
        if t > pub:
            after += 1
            if r not in new:
                bad.append(f'after:{k}')
        else:
            before += 1
            if r in new or r is None:
                bad.append(f'before:{k}')
    moved = {role: [c for tt, c in R['comp_hist'].get(('provider-a', role), []) if tt >= pub] for role in ('eca', 'aca', 'ica')}
    recert = all(moved.values())
    ok = not bad and after > 0 and before > 0 and recert
    return ok, (f'publication t={pub / 1e9:.0f} s; {after} certificates held after it, {before} before; '
                f'{len(bad)} on the wrong root {" ".join(bad[:3])}; ECA/ACA/ICA re-certified after it: {recert}')


def r_K1(R):
    bad = [(t, v) for t, v, _ in R['states'] if v != time32(R['t0'] + t // 1_000_000_000)]
    return not bad, f'{len(R["states"])} records, {len(bad)} differ ' + (str(bad[0]) if bad else '')


def iperiod(unix):
    return int((unix - IPERIOD_EPOCH) // WEEK)


def r_K3(R):
    bad_c = sum(1 for recs in R['creds'].values() for r in recs
                if r[4] is not None and r[4] != iperiod(R['t0'] + r[0] / 1e9))
    bad_s = 0
    for t, node, ci in R['sec']:
        if ci is None:
            continue
        u = R['t0'] + t / 1e9
        i = iperiod(u)
        since = u - (IPERIOD_EPOCH + i * WEEK)
        if not (ci == i or (ci == i - 1 and since < 3600)):
            bad_s += 1
    n = sum(len(v) for v in R['creds'].values())
    return bad_c == 0 and bad_s == 0, f'current_i: {bad_c}/{n} differ; cert_i: {bad_s}/{len(R["sec"])} outside i or the overlap'


def r_K5(R):
    bad, n = [], 0
    for k, h in R['certs'].items():
        c = dec_cert(h)
        t = c['toBeSigned']
        if t['id'][0] != 'linkageData':
            continue
        n += 1
        st = t['validityPeriod']['start']
        u = st + TIME32_EPOCH - 5   # back to Unix (every start here is after 2017)
        i = iperiod(u)
        unit, v = t['validityPeriod']['duration']
        secs = v * {'seconds': 1, 'minutes': 60, 'hours': 3600, 'sixtyHours': 216000, 'years': 31556952}.get(unit, 0)
        # 169 h in any unit: CAMP-EE R1.1 2.1.5.3.2 gives minutes (constants.md cert_lifetime_minutes)
        ok = time32(IPERIOD_EPOCH + i * WEEK) == st and secs == 169 * 3600
        if not ok:
            bad.append(f'{k}:{st},{t["validityPeriod"]["duration"]}')
    return n > 0 and not bad, f'{n} pseudonym certificates, {len(bad)} violations ' + ' '.join(bad[:3])


def r_DET(R):
    B = R.get('_base')
    if B is None:
        return False, 'needs --base'
    return R['digest'] == B['digest'], f'{R["digest"][:16]} vs {B["digest"][:16]}'


ROWS = [
    ('GOV.G3.electors', r_G3), ('GOV.G4.quorum', r_G4), ('GOV.G5.signers', r_G5), ('GOV.C1.roots', r_C1),
    ('GOV.L6.rollover', r_L6), ('GOV.C6.deadline', r_C6), ('GOV.C9.effective', r_C9),
    ('GOV.C12.monotone', r_C12m), ('GOV.C12.record', r_C12r), ('GOV.C12.verifications', r_C12v),
    ('GOV.M.ctls_published', r_Mctl), ('GOV.L1.lifetimes', r_L1), ('GOV.L3.crlseries', r_L3),
    ('GOV.L4.enckey', r_L4), ('GOV.L5.region', r_L5), ('GOV.L10.ssp', r_L10), ('GOV.L.chain', r_Lchain),
    ('GOV.L.names', r_Lnames), ('GOV.L.onecert', r_Lone), ('GOV.L.counters', r_Lcnt), ('GOV.D1.acas', r_D1),
    ('GOV.E1.lifetime', r_E1), ('GOV.E3.profile', r_E3), ('GOV.E3.name', r_E3n), ('GOV.E3.key', r_E3k),
    ('GOV.E.chain', r_Echain), ('GOV.E.once', r_Eonce), ('GOV.E.count', r_Ecnt), ('GOV.E.dcm-hop', r_Edcm),
    ('GOV.L.rollover-wire', r_Lroll),
    ('GOV.K1.time32', r_K1), ('GOV.K3.iperiod', r_K3), ('GOV.K5.pseudonym', r_K5),
    ('GOV.DET.release', r_DET), ('GOV.DET.fixed', r_DET),
]


# --- faults: each mutates the loaded records so that its row must go red -------------------
def _reenc_cert(R, store, key, fn):
    c = dec_cert(store[key])
    fn(c)
    store[key] = coer.encode('Certificate', c).hex()


def _first_comp(R, lab, role):
    return next(x for x in R['first']['providers'][0 if lab == 'provider-a' else 1]['components'] if x['role'] == role)


def _enrol_key(R, provider='provider-b'):
    for d, recs in sorted(R['creds'].items()):
        for r in recs:
            if r[3] in R['certs'] and r[2] == provider:
                return r[3]


def _set_dur(c, d):
    c['toBeSigned']['validityPeriod']['duration'] = d


def _m_states(R, fn):
    R['states'] = [(t, v, (fn(dict(c)) if c else c)) for t, v, c in R['states']]


def m_G3(R):
    _m_states(R, lambda c: dict(c, electors=4))


def m_G4(R):
    _m_states(R, lambda c: dict(c, quorum=2))


def m_G5(R):
    t, v, c = R['states'][-1]
    h = copy.deepcopy(c['history'])
    h[-1]['signers'] = h[-1]['signers'][:3]
    R['states'][-1] = (t, v, dict(c, history=h))


def m_C1(R):
    t, v, c = R['states'][0]
    R['states'][0] = (t, v, dict(c, roots=c['roots'][:1] + ['00' * 32]))


def m_L6(R):
    _m_states(R, lambda c: dict(c, roots=c['roots'][1:]) if c['sequence'] == 1 else c)


def m_C6(R):
    ch = ctl_changes(R)
    t1 = ch[1][0]
    late = 4 * 86400 * 10**9   # CTL 1 first seen four days later: past the deadline
    R['states'] = [(t if t < t1 else t + late, v, c) for t, v, c in R['states']]


def m_C9(R):
    _m_states(R, lambda c: dict(c, effective=c['effective'] + 1) if c['sequence'] == 1 else c)


def m_C12m(R):
    d = next(iter(R['creds']))
    recs = R['creds'][d]
    R['creds'][d] = [(recs[0][0], 1) + recs[0][2:]] + [(r[0], 0) + r[2:] for r in recs[1:]] + \
        [(recs[-1][0] + 1, 0) + recs[-1][2:]]


def m_C12r(R):
    R['report']['phase2']['trust_lists_installed'] += R['n_rsus'] + record_installs(R) + 1


def m_C12v(R):
    R['report']['phase2']['trust_list_verifications'] += 1


def m_Mctl(R):
    R['report']['phase2']['ctls_published'] += 1


def m_L1(R):
    x = _first_comp(R, 'provider-b', 'eca')
    _reenc_cert(R, R['first']['certificates'], x['cert'], lambda c: _set_dur(c, ('years', 16)))


def m_L3(R):
    x = _first_comp(R, 'provider-b', 'ra')
    _reenc_cert(R, R['first']['certificates'], x['cert'], lambda c: c['toBeSigned'].__setitem__('crlSeries', 256))


def m_L4(R):
    x = _first_comp(R, 'provider-b', 'ra')
    _reenc_cert(R, R['first']['certificates'], x['cert'], lambda c: c['toBeSigned'].pop('encryptionKey'))


def m_L5(R):
    x = _first_comp(R, 'provider-a', 'ra')
    _reenc_cert(R, R['first']['certificates'], x['cert'], lambda c: c['toBeSigned'].pop('region'))


def m_L10(R):
    # The first form: ISS components at version 2 (`85 00 02`), as SAESOL's.
    x = _first_comp(R, 'provider-a', 'aca')

    def f(c):
        for p in c['toBeSigned']['appPermissions']:
            if p['psid'] == 35:
                p['ssp'] = ('opaque', b'\x85\x00\x02')
    _reenc_cert(R, R['first']['certificates'], x['cert'], f)


def m_Lchain(R):
    x = _first_comp(R, 'provider-b', 'eca')
    _reenc_cert(R, R['first']['certificates'], x['cert'], lambda c: c['toBeSigned']['validityPeriod'].__setitem__(
        'start', c['toBeSigned']['validityPeriod']['start'] + 1))


def m_Lnames(R):
    x = _first_comp(R, 'provider-b', 'eca')
    _reenc_cert(R, R['first']['certificates'], x['cert'], lambda c: c['toBeSigned'].__setitem__(
        'id', ('name', 'eca.provider-a')))


def m_Lone(R):
    for p in R['first']['providers']:
        for x in p['components']:
            if x['cert'] and x.get('counters', {}).get('certificate'):
                x['counters']['certificate'] = '00' * 8
                return


def m_Lcnt(R):
    _first_comp(R, 'provider-b', 'root')['counters']['certs_issued'] = 99


def m_D1(R):
    _first_comp(R, 'provider-b', 'aca')['cert'] = None


def m_E1(R):
    _reenc_cert(R, R['certs'], _enrol_key(R), lambda c: _set_dur(c, ('years', 6)))


def m_E3(R):
    _reenc_cert(R, R['certs'], _enrol_key(R), lambda c: c['toBeSigned'].__setitem__('crlSeries', 3))


def m_E3n(R):
    k = _enrol_key(R)
    _reenc_cert(R, R['certs'], k, lambda c: c['toBeSigned'].__setitem__('id', ('name', 'ee-dup.provider-b')))
    k2 = _enrol_key(R, 'provider-a')
    _reenc_cert(R, R['certs'], k2, lambda c: c['toBeSigned'].__setitem__('id', ('name', 'ee-dup.provider-b')))


def m_E3k(R):
    a, b = _enrol_key(R), _enrol_key(R, 'provider-a')
    vk = dec_cert(R['certs'][b])['toBeSigned']['verifyKeyIndicator']
    _reenc_cert(R, R['certs'], a, lambda c: c['toBeSigned'].__setitem__('verifyKeyIndicator', vk))


def m_Echain(R):
    _reenc_cert(R, R['certs'], _enrol_key(R), lambda c: c.__setitem__('issuer', ('sha256AndDigest', b'\x01' * 8)))


def m_Eonce(R):
    R['enrol'].append(R['enrol'][0])


def m_Ecnt(R):
    for p in R['last']['providers']:
        for x in p['components']:
            if x['role'] == 'eca':
                x['counters']['enrolment_certs_issued'] += 1
                return


def m_K1(R):
    t, v, c = R['states'][5]
    R['states'][5] = (t, v + 1, c)


def m_K3(R):
    d = next(iter(R['creds']))
    r = R['creds'][d][0]
    R['creds'][d][0] = r[:4] + (r[4] + 1,)


def m_K5(R):
    for k, h in R['certs'].items():
        if dec_cert(h)['toBeSigned']['id'][0] == 'linkageData':
            _reenc_cert(R, R['certs'], k, lambda c: _set_dur(c, ('minutes', 10080)))
            return


def m_Edcm(R):
    R['enrol'] = [e for e in R['enrol'] if not ('/dcm/' in e[1] and '/eca/' in e[2])]


def m_Lroll(R):
    # The first form: the PDU PKI fixed at run start, so a post-publication certificate is
    # on the old root: move its first sighting after the publication onto a pre-change one.
    ch = ctl_changes(R)
    pub = ch[1][0] if len(ch) > 1 else 0
    old = next((k for k, t in sorted(R['cred_first'].items(), key=lambda kv: kv[1]) if t <= pub
                and R['creds'].get(R['owner'].get(k), [(0, 0, None)])[0][2] == 'provider-a'), None)
    if old:
        R['cred_first'][old] = pub + 1


def m_DET(R):
    R['digest'] = '0' + R['digest'][1:]


MUT = {'GOV.G3.electors': m_G3, 'GOV.G4.quorum': m_G4, 'GOV.G5.signers': m_G5, 'GOV.C1.roots': m_C1,
       'GOV.L6.rollover': m_L6, 'GOV.C6.deadline': m_C6, 'GOV.C9.effective': m_C9,
       'GOV.C12.monotone': m_C12m, 'GOV.C12.record': m_C12r, 'GOV.C12.verifications': m_C12v,
       'GOV.M.ctls_published': m_Mctl, 'GOV.L1.lifetimes': m_L1, 'GOV.L3.crlseries': m_L3,
       'GOV.L4.enckey': m_L4, 'GOV.L5.region': m_L5, 'GOV.L10.ssp': m_L10, 'GOV.L.chain': m_Lchain,
       'GOV.L.names': m_Lnames, 'GOV.L.onecert': m_Lone, 'GOV.L.counters': m_Lcnt, 'GOV.D1.acas': m_D1,
       'GOV.E1.lifetime': m_E1, 'GOV.E3.profile': m_E3, 'GOV.E3.name': m_E3n, 'GOV.E3.key': m_E3k,
       'GOV.E.chain': m_Echain, 'GOV.E.once': m_Eonce, 'GOV.E.count': m_Ecnt, 'GOV.K1.time32': m_K1,
       'GOV.K3.iperiod': m_K3, 'GOV.K5.pseudonym': m_K5, 'GOV.DET.release': m_DET, 'GOV.DET.fixed': m_DET,
       'GOV.E.dcm-hop': m_Edcm, 'GOV.L.rollover-wire': m_Lroll}


def judge(R, only=None):
    out = []
    for rid, fn in ROWS:
        if only and rid != only:
            continue
        try:
            ok, txt = fn(R)
        except Exception as x:  # noqa: BLE001
            ok, txt = False, f'error {type(x).__name__}: {x}'
        out.append((rid, ok, txt))
    return out


def main(argv):
    run = argv[1]
    base = argv[argv.index('--base') + 1] if '--base' in argv else None
    R = load(run)
    if base:
        R['_base'] = {'digest': load(base)['digest']}
    if '--selftest' in argv:
        clean = {r: ok for r, ok, _ in judge(R)}
        red = 0
        for rid, _ in ROWS:
            if rid.startswith('GOV.DET') and not base:
                continue
            M = copy.deepcopy(R)
            MUT[rid](M)
            _, ok, txt = judge(M, rid)[0]
            red += not ok
            print(f'{rid:24} clean={"PASS" if clean[rid] else "FAIL"} mutated={"PASS" if ok else "FAIL"}'
                  f'{"  <-- STAYS GREEN" if ok else ""}  {txt[:110]}')
        print(f'selftest: {red} of {sum(1 for r, _ in ROWS if base or not r.startswith("GOV.DET"))} rows go red under their fault')
        return
    if '--mutate' in argv:
        rid = argv[argv.index('--mutate') + 1]
        MUT[rid](R)
        rows = judge(R, rid)
    else:
        rows = judge(R)
    for rid, ok, txt in rows:
        print(f'{rid:24} {"PASS" if ok else "FAIL"}  {txt}')
    print(f'{sum(ok for _, ok, _ in rows)}/{len(rows)} pass; record digest {R["digest"][:16]}')


if __name__ == '__main__':
    main(sys.argv)
