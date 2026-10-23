#!/usr/bin/env python3
"""The engine's component certificates, provider by provider, against the published chains.

Independent of the Rust: a run's provisioning SPDUs (`pki.flow` PDU hex in
`<run-dir>/recording.mcap`: RaEeCertAck, EcaEeCertResponse, AcaEeCertResponsePrivate) are
decoded with the A0 oracle's COER codec (tools/oracle/scms_oracle/coer.py), every certificate
inside is broken into its toBeSigned fields, and each component certificate is compared with
the published certificate of the same role:
  * provider-a (the ISS form, governance::CertProfile::Iss) with the ISS pre-production chain
    in the 2025-07-17 test chain file (`*.preprod.v2x.isscms.com`, `v2xrootca-test.ghsiss.com`);
  * provider-b (the SAESOL form) with the production chain file (`*.prod.s2x.io`).
A role passes when the two have the same field set, every field but the name has the same
encoded size, and the engine's size is the published size plus the name-length difference.
Then the size difference between the two providers' SPDUs is the published certificates' own
(testing/matrix.md open item 5, realism rows Z1/Z3/Z4).

The published files are not committed (All Rights Reserved; tools/scms-artefacts/fetch.py).

Usage (project venv):
  python -I tools/realworld/component_certs.py <run-dir> [--artefacts DIR] [--json OUT]
Exit 1 when a role is RED. `--mutate` drops the ISS region from the engine's RA and the
encryption key from its PCA and must turn both RED (exit 0 when it does).
"""

import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'oracle'))
from scms_oracle import certs, coer  # noqa: E402

ARTEFACTS = os.path.join(HERE, '..', '..', 'third_party', 'scms-artefacts', 'files')
CHAINS = ('test-2025-07-17/test_cert_chain.oer', 'production-2025-08-13/cert-chain_0.oer')
FIELDS = [('id', 'CertificateId'), ('validityPeriod', 'ValidityPeriod'),
          ('region', 'GeographicRegion'), ('assuranceLevel', 'SubjectAssurance'),
          ('appPermissions', 'SequenceOfPsidSsp'),
          ('certIssuePermissions', 'SequenceOfPsidGroupPermissions'),
          ('certRequestPermissions', 'SequenceOfPsidGroupPermissions'),
          ('encryptionKey', 'PublicEncryptionKey'), ('verifyKeyIndicator', 'VerificationKeyIndicator')]
PDUS = ('RaEeCertAckSpdu', 'EcaEeCertResponseSpdu', 'AcaEeCertResponsePrivateSpdu')
# Engine host label -> the published certificate of the same role.
PAIRS = {
    'provider-a': {'ra': 'ra.preprod.v2x.isscms.com', 'aca': 'pca.preprod.v2x.isscms.com',
                   'eca': 'dot1.eca.preprod.v2x.isscms.com', 'ica': 'ica.preprod.v2x.isscms.com',
                   'rootca': 'v2xrootca-test.ghsiss.com'},
    'provider-b': {'ra': 'ra.prod.s2x.io', 'aca': 'aca.prod.s2x.io', 'eca': 'eca.prod.s2x.io',
                   'ica': 'ica.prod.s2x.io', 'rootca': 'rootca.prod.s2x.io'},
}


def name_of(c):
    i = c['toBeSigned']['id']
    return str(i[1]) if i[0] == 'name' else i[0]


def shape(c):
    t = c['toBeSigned']
    return {'name': name_of(c), 'size': len(certs.encode(c)),
            'fields': {k: len(coer.encode(T, t[k])) for k, T in FIELDS if k in t}}


def published(root):
    out = {}
    for f in CHAINS:
        outer = coer.decode('Ieee1609Dot2Data', open(os.path.join(root, f), 'rb').read())
        pdu = coer.decode('ScmsPdu', outer['content'][1])
        for c in pdu['content'][1][1]['others']:
            s = shape(c)
            out.setdefault(s['name'], []).append(s)
    return out


def walk(v, out):
    if isinstance(v, dict):
        if 'toBeSigned' in v and 'issuer' in v and 'type' in v:
            out.append(v)
            return
        for x in v.values():
            walk(x, out)
    elif isinstance(v, (list, tuple)):
        for x in v:
            walk(x, out)
    elif isinstance(v, (bytes, bytearray)) and len(v) > 60:
        # An OCTET STRING or open type that holds a nested structure (the ECA response's
        # ScmsPdu inside its signed data): taken only when it re-encodes byte for byte.
        for t in ('Ieee1609Dot2Data', 'ScmsPdu'):
            try:
                x = coer.decode(t, bytes(v))
            except Exception:
                continue
            if coer.encode(t, x) == bytes(v):
                walk(x, out)
                break


def engine(run_dir):
    from mcap.reader import make_reader
    first = {}
    with open(os.path.join(run_dir, 'recording.mcap'), 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            if ch.topic != 'record/pki.flow':
                continue
            for d in json.loads(m.data).get('pdus') or []:
                k = (d['id'].split('/')[0], d['type'])
                if d['type'] in PDUS and d.get('hex') and k not in first:
                    first[k] = d
    spdus, found = {}, {}
    for (prov, typ), d in sorted(first.items()):
        data = bytes.fromhex(d['hex'])
        v = coer.decode('Ieee1609Dot2Data', data)
        if coer.encode('Ieee1609Dot2Data', v) != data:
            raise SystemExit(f'RED {prov} {typ}: does not re-encode byte for byte under the oracle')
        cs = []
        walk(v, cs)
        spdus[(prov, typ)] = (len(data), [name_of(c) for c in cs])
        for c in cs:
            s = shape(c)
            found.setdefault(s['name'], s)
    return spdus, found


def judge(found, pub):
    rows = []
    for name, e in sorted(found.items()):
        role, _, rest = name.partition('.')
        owner = rest.split('.')[0]
        if owner not in PAIRS or role not in PAIRS[owner]:
            continue
        label = PAIRS[owner][role]
        # The production chain holds two aca.prod.s2x.io certificates (157 and 162 B); the
        # role passes if one of them matches.
        best = None
        for p in pub.get(label, []):
            ef, pf = e['fields'], p['fields']
            same_set = set(ef) == set(pf)
            diffs = {k: (ef.get(k), pf.get(k)) for k in set(ef) | set(pf) if k != 'id' and ef.get(k) != pf.get(k)}
            predicted = p['size'] - pf['id'] + ef['id']
            ok = same_set and not diffs and predicted == e['size']
            r = dict(owner=owner, role=role, engine=e['size'], published=label, published_size=p['size'],
                     name=(ef['id'], pf['id']), predicted=predicted, same_set=same_set, diffs=diffs, ok=ok)
            if best is None or (ok and not best['ok']):
                best = r
        if best is None:
            best = dict(owner=owner, role=role, engine=e['size'], published=label, ok=False, diffs='not published')
        rows.append(best)
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('run')
    ap.add_argument('--artefacts', default=ARTEFACTS)
    ap.add_argument('--json')
    ap.add_argument('--mutate', action='store_true')
    a = ap.parse_args()
    pub = published(a.artefacts)
    spdus, found = engine(a.run)
    for (prov, typ), (n, names) in spdus.items():
        print(f'{prov} {typ}: {n} B, certificates {", ".join(names)}')
    if a.mutate:
        for n, s in found.items():
            if n.startswith('ra.provider-a') and 'region' in s['fields']:
                s['size'] -= s['fields'].pop('region')
            if n.startswith('aca.provider-a') and 'encryptionKey' in s['fields']:
                s['size'] -= s['fields'].pop('encryptionKey')
    rows = judge(found, pub)
    red = 0
    for r in rows:
        red += not r['ok']
        print(f"{'ok ' if r['ok'] else 'RED'} {r['owner']} {r['role']:6s} engine {r['engine']} B vs "
              f"{r['published']} {r.get('published_size', '-')} B; name {r.get('name')} -> predicted "
              f"{r.get('predicted', '-')}; field set {'same' if r.get('same_set') else 'DIFFERS'}"
              + (f"; differing {r['diffs']}" if r.get('diffs') else ''))
    if a.json:
        json.dump({'spdus': {f'{p} {t}': v for (p, t), v in spdus.items()}, 'roles': rows},
                  open(a.json, 'w'), indent=1, default=str)
    print(f'{len(rows) - red}/{len(rows)} component roles match the published certificate field for field'
          + (f'; {red} RED' if red else ''))
    if a.mutate:
        sys.exit(0 if red >= 2 else 1)
    sys.exit(1 if red or not rows else 0)


if __name__ == '__main__':
    main()
