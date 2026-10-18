#!/usr/bin/env python3
"""WP-F2: certificate sizes, the engine's and the published ones, against CAMP Release 1.2.2 Table 3.

Independent of the Rust: every certificate is decoded with the A0 oracle's COER codec
(tools/oracle/scms_oracle/coer.py, asn1tools over the 1609.2-2022 modules), re-encoded (it must
come back byte for byte), and broken into its fields, each field encoded on its own, so a size
difference is attributed to the fields that cause it.

Inputs:
  ENGINE   the JSON that ``V2XW_SIZES_OUT=<file> cargo test -p v2xw-proto --test camp_sizes``
           writes (engine certificates and SPDUs from the real encoder).
  --real   third_party/scms-artefacts/files (fetched by tools/scms-artefacts/fetch.py; not
           committed, All Rights Reserved). Optional: without it only the engine side runs.

Checks (each prints RED on failure; the last line is the verdict):
  * every certificate decodes and re-encodes byte for byte under the oracle (engine and real);
  * the engine's component certificates use the same field set as the published certificate
    of the same role (the same toBeSigned members present), so their sizes differ only by
    the values (names, permissions), not by a missing or extra field;
  * each engine end-entity certificate's size equals its field sum (no unexplained bytes).

Usage (project venv):
  python -I tools/realworld/sizes.py ENGINE.json [--real DIR] [--json OUT] [-v]
"""

import argparse
import glob
import json
import re
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'oracle'))
from scms_oracle import coer  # noqa: E402

# CAMP VSC5, "SCMS PoC Implementation - EE Requirements and Specifications Supporting SCMS
# Software Release 1.2.2", 2016-11-15, Table 3 "PoC Certificate Expiration Timelines",
# column "Example Size in Bytes (Certs are Not Fixed Size)", PDF pp. 52-54 (printed 34-36).
# Transcribed 2026-10-18 (03 §12 carries the same values).
CAMP_TABLE_3 = {
    'obe-enrolment': 87, 'obe-pseudonym': 86, 'obe-identification': 89, 'rse-enrolment': 87,
    'rse-application': 89, 'dcm': 219, 'eca': 150, 'ra': 217, 'la': 205, 'pca': 216, 'ica': 195,
    'ma': 205, 'crlg': 190, 'pg': 172, 'root': 211, 'elector': 166,
}

TBS_FIELDS = [  # (member, ASN.1 type for encoding it alone)
    ('id', 'CertificateId'), ('cracaId', 'HashedId3'), ('crlSeries', 'CrlSeries'),
    ('validityPeriod', 'ValidityPeriod'), ('region', 'GeographicRegion'),
    ('assuranceLevel', 'SubjectAssurance'), ('appPermissions', 'SequenceOfPsidSsp'),
    ('certIssuePermissions', 'SequenceOfPsidGroupPermissions'),
    ('certRequestPermissions', 'SequenceOfPsidGroupPermissions'),
    ('canRequestRollover', None), ('encryptionKey', 'PublicEncryptionKey'),
    ('verifyKeyIndicator', 'VerificationKeyIndicator'), ('flags', None),
    ('appExtensions', 'SequenceOfAppExtensions'), ('certIssueExtensions', 'SequenceOfCertIssueExtensions'),
    ('certRequestExtension', 'SequenceOfCertRequestExtensions'),
]


def role_of(name):
    """The SCMS role a certificate name stands for, by its dot/dash tokens."""
    for tok in re.split(r'[.-]', name.lower()):
        t = tok[3:] if tok.startswith('v2x') else tok
        if t in ('rootca', 'rca', 'root'):
            return 'root'
        if t in ('la', 'la1', 'la2'):
            return 'la'
        if t in ('aca', 'pca'):
            return 'pca'
        if t.startswith('elector'):
            return 'elector'
        if t in ('ica', 'eca', 'ra', 'ma', 'crlg', 'pg', 'dcm'):
            return t
    return '?'


# A provider's choice, not a format rule: ISS pre-production puts an identifiedRegion on every
# sub-CA and the production s2x chain on none; some ACAs carry the 2022 flags extension.
PROVIDER_OPTIONAL = {'region', 'flags'}


def strip(members):
    return [m for m in members if m not in PROVIDER_OPTIONAL]


def engine_role(key):
    if '/' not in key:
        return key
    tail = key.split('/', 1)[1]
    r = role_of(tail)
    return 'pca' if r == 'pca' else r


def fields(cert):
    """Field name -> encoded bytes of that member, plus the framing remainder."""
    tbs = cert['toBeSigned']
    out = {}
    for m, t in TBS_FIELDS:
        if m not in tbs:
            continue
        if t is None:
            out[m] = None  # NULL or a bit string inside the bitmap: no separate encoding
            continue
        out[m] = len(coer.encode(t, tbs[m]))
    out['issuer'] = len(coer.encode('IssuerIdentifier', cert['issuer']))
    if 'signature' in cert:
        out['signature'] = len(coer.encode('Signature', cert['signature']))
    return out


def describe(cert):
    tbs = cert['toBeSigned']
    d = {'type': cert['type'], 'id': tbs['id'][0] if tbs['id'][0] != 'name' else 'name:' + tbs['id'][1]}
    if 'region' in tbs:
        d['region'] = tbs['region'][0]
    if 'appPermissions' in tbs:
        d['psids'] = [p['psid'] for p in tbs['appPermissions']]
        d['ssps'] = sum(1 for p in tbs['appPermissions'] if 'ssp' in p)
    if 'assuranceLevel' in tbs:
        d['assurance'] = tbs['assuranceLevel'].hex()
    vk = tbs['verifyKeyIndicator']
    d['key'] = vk[0] if vk[0] == 'reconstructionValue' else vk[1][0]
    return d


def analyse(name, b, bad, verbose):
    try:
        c = coer.decode('Certificate', b)
    except Exception as e:  # noqa: BLE001
        bad.append(f'{name}: oracle cannot decode ({type(e).__name__})')
        return None
    if coer.encode('Certificate', c) != b:
        bad.append(f'{name}: oracle re-encoding differs')
    f = fields(c)
    known = sum(v for v in f.values() if v)
    row = {'name': name, 'len': len(b), 'fields': f, 'framing': len(b) - known, 'desc': describe(c),
           'members': sorted(k for k in c['toBeSigned'])}
    if verbose:
        print(f'  {name}: {len(b)} B {row["desc"]} {f} framing {row["framing"]}')
    return row


def real_certs(root):
    out = []
    for path in sorted(glob.glob(os.path.join(root, '**', '*'), recursive=True)):
        if not os.path.isfile(path):
            continue
        data = open(path, 'rb').read()
        try:
            outer = coer.decode('Ieee1609Dot2Data', data)
            pdu = coer.decode('ScmsPdu', outer['content'][1])
            kind, body = pdu['content'][1]
        except Exception:  # noqa: BLE001
            try:  # a bare certificate file (roots, electors)
                coer.decode('Certificate', data)
                out.append((os.path.relpath(path, root), data))
            except Exception:  # noqa: BLE001
                pass
            continue
        if kind == 'certificateChain':
            for c in body['others']:
                nm = c['toBeSigned']['id'][1] if c['toBeSigned']['id'][0] == 'name' else '?'
                out.append((os.path.relpath(path, root) + ':' + nm, coer.encode('Certificate', c)))
        elif kind == 'multiSignedCtl':
            for k, sig in enumerate(body['signatures']):
                el = sig['content'][1]['signer'][1][0]
                out.append((os.path.relpath(path, root) + f':elector{k}', coer.encode('Certificate', el)))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('engine')
    ap.add_argument('--real')
    ap.add_argument('--json')
    ap.add_argument('-v', '--verbose', action='store_true')
    a = ap.parse_args()
    bad = []
    eng = json.load(open(a.engine))
    rows = {}
    for k, h in eng['certs'].items():
        r = analyse(k, bytes.fromhex(h), bad, a.verbose)
        if r:
            r['role'] = engine_role(k)
            r['camp'] = CAMP_TABLE_3.get(r['role'])
            rows[k] = r
    real = {}
    if a.real and os.path.isdir(a.real):
        seen = set()
        for nm, b in real_certs(a.real):
            if b in seen:
                continue
            seen.add(b)
            r = analyse(nm, b, bad, a.verbose)
            if r:
                tail = nm.split(':')[-1] if ':' in nm else os.path.basename(nm)
                r['role'] = 'elector' if 'elector' in nm else ('root' if '/roots/' in nm else role_of(tail))
                r['camp'] = CAMP_TABLE_3.get(r['role'])
                key = nm
                while key in real:  # two certificates of one name (an ACA renewal)
                    key += "'"
                real[key] = r
        # The engine's component profile must have the same members as a published one.
        for k, r in rows.items():
            if '/' not in k or r['role'] in ('?',):
                continue
            same = [x for x in real.values() if x['role'] == r['role']]
            if not same:
                continue
            exact = [x['name'] for x in same if x['members'] == r['members']]
            loose = [x['name'] for x in same if strip(x['members']) == strip(r['members'])]
            r['same_fields_as'] = exact
            r['same_fields_but_region_flags_as'] = [n for n in loose if n not in exact]
            # Where the field set is the same, every field but the name must encode to the
            # same length as in some published certificate: the size difference is the name.
            def sans_id(f):
                return {m: v for m, v in f.items() if m != 'id'}
            twins = [x['name'] for x in same
                     if x['members'] == r['members'] and sans_id(x['fields']) == sans_id(r['fields'])
                     and x['framing'] == r['framing']]
            r['same_encoding_but_name_as'] = twins
            if exact and not twins:
                bad.append(f'{k}: same fields as {exact[0]} but some field other than the name '
                           'encodes to a different length')
            if not loose:
                bad.append(f'{k}: members {r["members"]} match no published {r["role"]}, '
                           'even ignoring region and flags')
    for k, r in rows.items():
        if '/' in k:
            continue
        if r['framing'] < 0 or r['framing'] > 8:
            bad.append(f'{k}: {r["framing"]} B not attributed to a field')
    # What-if, for the reconciliation with CAMP (report §3): the engine's pseudonym with a
    # USA-only identifiedRegion, the region CAMP's CSR rule names (SCMS-1.2.2 §5.2.6.3.2).
    if 'obe-pseudonym' in eng['certs']:
        c = coer.decode('Certificate', bytes.fromhex(eng['certs']['obe-pseudonym']))
        c['toBeSigned']['region'] = ('identifiedRegion', [('countryOnly', 840)])
        rows['obe-pseudonym']['with_usa_region'] = len(coer.encode('Certificate', c))
    if a.json:
        json.dump({'engine': rows, 'real': real, 'camp_table_3': CAMP_TABLE_3,
                   'spdus': eng.get('spdus', {})}, open(a.json, 'w'), indent=1, default=str)
    for b in bad:
        print('RED', b)
    print(f'sizes: {"PASS" if not bad else "FAIL"} ({len(rows)} engine and {len(real)} published '
          f'certificates decoded by the oracle; {len(bad)} failures)')
    return 0 if not bad else 1


if __name__ == '__main__':
    sys.exit(main())
