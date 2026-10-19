#!/usr/bin/env python3
"""Crypto-formats validation: every engine certificate's ToBeSignedCertificate profile against
the published US certificate of the same role, field by field, with the A0 oracle's codec.

    V2XW_SIZES_OUT=f.json cargo test -p v2xw-proto --test camp_sizes
    <venv>/bin/python -I tools/realworld/profiles.py f.json third_party/scms-artefacts/files

Independent of the Rust: the expectations are the published certificates (SCMS Manager chain
files, fetched by tools/scms-artefacts/fetch.py, never committed) and the rules of
docs/design/scms-real/validation/crypto-formats.md section 2, each cited there. For each engine
component certificate the tool finds the published certificates whose host label is the same role
and compares: type, the issuer's role, crlSeries, the CRACA's role, the duration (must be one a
published certificate of the role has, or a documented choice), appPermissions PSIDs with each
SecurityMgmtSsp/CrlSsp choice, certIssuePermissions / certRequestPermissions groups, encryptionKey
presence and the verification key's curve. End-entity certificates (no published sample) are checked
against the profile rules ([SMP] Table 1 and section 6.3.3, CAMP Table 3, 03 section 7).
Prints one line per certificate and a verdict.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'oracle'))

from scms_oracle import certs, coer  # noqa: E402

UNIT = {'seconds': 1, 'minutes': 60, 'hours': 3600, 'sixtyHours': 216000, 'years': 31556952}
# Role of a host label (Provider Req. section 3.3.1.1 names, ISS pca/dot1.eca variants).
ROLE = {'rootca': 'root', 'v2xrootca-test': 'root', 'root': 'root', 'ica': 'ica', 'eca': 'eca',
        'dot1': 'eca', 'aca': 'aca', 'pca': 'aca', 'ra': 'ra', 'ma': 'ma', 'la1': 'la', 'la2': 'la',
        'crlg': 'crlg', 'pg': 'pg', 'v2xpg-test': 'pg', 'dcm': 'dcm'}
# Choices recorded in the validation document (finding id -> what differs on purpose).
DOCUMENTED = {
    ('crlg', 'issuer'): 'CF-CRACA', ('crlg', 'crlSeries'): 'CF-CRACA', ('crlg', 'craca'): 'CF-CRACA',
    ('la', 'craca'): 'CF-CRACA', ('ra', 'craca'): 'CF-CRACA',
}
# Lifetimes chosen where no published certificate fits (role -> (seconds, finding id)).
DOCUMENTED_LIFE = {'la': (25 * 31556952, 'CF-LIFE-LA')}


def name(c):
    t = c['toBeSigned']
    return t['id'][1] if t['id'][0] == 'name' else None


def role_of(c):
    n = name(c)
    return ROLE.get(n.split('.')[0]) if n else None


SSP_CHOICE = ['elector', 'root', 'pg', 'ica', 'eca', 'aca', 'crl', 'dcm', 'la', 'lop', 'ma', 'ra', 'ee', 'dc']


def ssp_choice(p):
    if 'ssp' not in p:
        return f'{p["psid"]:#x}'
    k, v = p['ssp']
    if p['psid'] == 0x23:
        # The SecurityMgmtSsp CHOICE tag only: ISS's certificates carry the 2020 SSP version 1.
        return f'0x23:{SSP_CHOICE[v[0] & 0x3f]}'
    if p['psid'] == 0x100:
        return f'0x100:{v[2:].hex()}'  # CrlSsp after preamble and version
    return f'{p["psid"]:#x}:{k}'


def groups(gs):
    out = []
    for g in gs or []:
        sp = g['subjectPermissions']
        s = 'all' if sp[0] == 'all' else tuple(sorted(x['psid'] for x in sp[1]))
        out.append((s, g.get('minChainLength', 1), g.get('chainLengthRange', 0),
                    g.get('eeType', (b'\x80', 8))[0][0]))
    return tuple(sorted(out, key=str))


def profile(c, by_id):
    t = c['toBeSigned']
    iss = c['issuer']
    issuer = 'self' if iss[0] == 'self' else role_of(by_id[iss[1]]) if iss[1] in by_id else '?'
    craca = next((role_of(x) for h, x in by_id.items() if h[-3:] == t['cracaId']), None)
    vki = t['verifyKeyIndicator']
    u, n = t['validityPeriod']['duration']
    return {
        'type': c['type'], 'issuer': issuer, 'crlSeries': t['crlSeries'],
        'craca': craca if t['cracaId'] != b'\0\0\0' else 'none',
        'seconds': n * UNIT[u],
        'app': tuple(sorted(ssp_choice(p) for p in t.get('appPermissions', []))),
        'cip': groups(t.get('certIssuePermissions')), 'crp': groups(t.get('certRequestPermissions')),
        'enc': 'encryptionKey' in t,
        'key': 'reconstruction' if vki[0] == 'reconstructionValue' else vki[1][0],
    }


def published(root):
    out = []
    for dp, _, fs in os.walk(root):
        for f in fs:
            if not f.endswith('.oer') or 'elector' in f:
                continue
            try:
                outer = coer.decode('Ieee1609Dot2Data', open(os.path.join(dp, f), 'rb').read())
            except Exception:  # noqa: BLE001 - a bare root certificate file (also in the chain file)
                continue
            if outer['content'][0] != 'unsecuredData':
                continue
            pdu = coer.decode('ScmsPdu', outer['content'][1])
            kind, body = pdu['content'][1]
            if kind == 'certificateChain':
                out += body['others']
    return out


SECOND_WEEK = 7 * 86400


def ee_rules(role, p):
    """[SMP] Table 1, section 6.3.3; 03 section 7; CAMP Table 3: what each EE certificate is."""
    want = {
        'obe-pseudonym': dict(type='implicit', issuer='aca', crlSeries=1, seconds=SECOND_WEEK + 3600),
        'obe-identification': dict(type='implicit', issuer='aca', crlSeries=3, seconds=30 * 86400 + 3600),
        'rse-application': dict(type='implicit', issuer='aca', crlSeries=3, seconds=SECOND_WEEK + 3600),
        'obe-enrolment': dict(type='implicit', issuer='eca', crlSeries=4),
        'rse-enrolment': dict(type='implicit', issuer='eca', crlSeries=4),
    }[role]
    bad = [f'{k}={p[k]} (want {v})' for k, v in want.items() if p[k] != v]
    if p['cip']:
        bad.append('an end entity issues nothing')
    if role.endswith('enrolment') and (p['app'] or not p['crp']):
        bad.append('enrolment: certRequestPermissions only ([SMP] 4.4.1)')
    if not role.endswith('enrolment') and (not p['app'] or p['crp']):
        bad.append('authorization: appPermissions only')
    return bad


def main(sizes, root):
    m = json.load(open(sizes))
    eng = {k: coer.decode('Certificate', bytes.fromhex(v)) for k, v in m['certs'].items()}
    pub = published(root)
    by_id = {certs.hashed_id8(c): c for c in list(eng.values()) + pub}
    pub_by_role = {}
    for c in pub:
        pub_by_role.setdefault(role_of(c), []).append(profile(c, by_id))
    bad_total = checked = 0
    for k, c in sorted(eng.items()):
        p = profile(c, by_id)
        r = role_of(c)
        notes = []
        if r is None:
            bad = ee_rules(k, p)
            ref = 'profile rules'
        else:
            refs = pub_by_role.get(r)
            if not refs:
                print(f'  {k}: no published {r} certificate (UNVERIFIABLE)')
                continue
            best = None
            for q in refs:
                diff = [f for f in ('type', 'issuer', 'crlSeries', 'craca', 'app', 'cip', 'crp', 'enc', 'key')
                        if p[f] != q[f] and (r, f) not in DOCUMENTED]
                noted = sorted({DOCUMENTED[(r, f)] for f in ('issuer', 'crlSeries', 'craca')
                                if p[f] != q[f] and (r, f) in DOCUMENTED})
                if p['seconds'] not in {x['seconds'] for x in refs}:
                    if DOCUMENTED_LIFE.get(r, (None,))[0] == p['seconds']:
                        noted.append(DOCUMENTED_LIFE[r][1])
                    else:
                        diff.append('seconds')
                diff = diff + [f'#{n}' for n in noted]
                if best is None or len(diff) < len(best[0]):
                    best = (diff, q)
            notes = [f[1:] for f in best[0] if f.startswith('#')]
            best = ([f for f in best[0] if not f.startswith('#')], best[1])
            bad = [f'{f}={p[f]} (published {best[1][f]})' for f in best[0] if f != 'seconds']
            if 'seconds' in best[0]:
                bad.append(f'lifetime {p["seconds"] / 31556952:.2f} y (published '
                           f'{sorted(round(x["seconds"] / 31556952, 2) for x in refs)})')
            ref = f'nearest published {r}' + (f' (documented: {", ".join(notes)})' if notes else '')
        checked += 1
        bad_total += bool(bad)
        print(f'  {"RED " if bad else "ok  "}{k} vs {ref}' + (': ' + '; '.join(bad) if bad else ''))
    ok = checked and not bad_total
    print(f'profiles: {"PASS" if ok else "FAIL"} ({checked - bad_total}/{checked} engine certificates '
          f'match their published role or profile rule; {len(pub)} published certificates)')
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1], sys.argv[2]))
