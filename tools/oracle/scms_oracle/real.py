"""Real-artefact cross-check: published SCMS Manager CTL and certificate-chain files (01 sections 2.2, 3.1).

``oracle.py real FILE...`` decodes each file as Ieee1609Dot2Data -> ScmsPdu, requires both layers to
re-encode byte for byte (COER canonical check), then
* certificate chains: links every certificate to its issuer by HashedId8 and verifies every explicit
  signature (self-signed roots with an empty signer input); implicit certificates are reconstructed
  (ECQV-1609) and reported;
* multi-signed CTLs: for each elector signature, verifies the elector's self-signed certificate, the
  SignedData signature over H(H(tbsData) || H(elector cert)), and that tbsCtlHash = SHA-384(tbsCtl).

The files are not vendored here (WP-A5 adds them under third_party/scms-artefacts with provenance).
"""

import hashlib

from . import certs, coer, ec, prims

# (sha256 of the published file, first differing byte of the ScmsPdu re-encoding) -> explanation
KNOWN_NONCANONICAL = {
    ('9dbc552e16d3dae96377c5fab094ffeeb3a289444cbece2ed218331aa598414c', 7841):
        'SCMS_Manager-Test_Cert_Chain.oer (test CTL zip of 2025-07-17): one aca.v2x-scms.saesoltech.io '
        'certificate encodes the ToBeSignedCertificate extension-presence bitmap as 02 06 80 (2 additions) '
        'where the 1609.2-2022 module has 4 (02 04 80). Its ICA signature verifies over the canonical '
        '2022 re-encoding, so the issuer signed the canonical form; only the transmitted bytes differ.',
}


def _name(c):
    return c['toBeSigned']['id'][1] if c['toBeSigned']['id'][0] == 'name' else c['toBeSigned']['id'][0]


def check_file(path):
    rows = []
    data = open(path, 'rb').read()
    outer = coer.decode('Ieee1609Dot2Data', data)
    rows.append(('Ieee1609Dot2Data re-encode', coer.encode('Ieee1609Dot2Data', outer) == data, f'{len(data)} B'))
    if outer['content'][0] != 'unsecuredData':
        rows.append(('content', False, outer['content'][0]))
        return rows
    inner = outer['content'][1]
    pdu = coer.decode('ScmsPdu', inner)
    re = coer.encode('ScmsPdu', pdu)
    if re == inner:
        rows.append(('ScmsPdu re-encode', True, f'{len(inner)} B'))
    else:
        i = next((i for i in range(min(len(re), len(inner))) if re[i] != inner[i]), min(len(re), len(inner)))
        sha = hashlib.sha256(data).hexdigest()
        known = KNOWN_NONCANONICAL.get((sha, i))
        rows.append(('ScmsPdu re-encode', known is not None,
                     f'differs at byte {i}' + (f' (known: {known})' if known else '')))
    kind, body = pdu['content'][1]
    if kind == 'certificateChain':
        rows += _chain(body['others'])
    elif kind == 'multiSignedCtl':
        rows += _ctl(body)
    else:
        rows.append(('kind', False, kind))
    return rows


def _chain(cs):
    rows = []
    by_id = {certs.hashed_id8(c): c for c in cs}
    for c in cs:
        rows.append((f'{_name(c)} canonical as published', certs.encode_canonical(c) == certs.encode(c), ''))
        iss = c['issuer']
        if iss[0] == 'self':
            rows.append((f'{_name(c)} self-signature', certs.verify_explicit(c, None), ''))
            continue
        parent = by_id.get(iss[1])
        if parent is None:
            rows.append((f'{_name(c)} issuer {iss[1].hex()} in file', False, 'issuer not present'))
            continue
        if c['type'] == 'explicit':
            rows.append((f'{_name(c)} signed by {_name(parent)}', certs.verify_explicit(c, parent), ''))
        else:
            C, Q = certs.verification_key(c, parent)
            rows.append((f'{_name(c)} implicit key reconstructed from {_name(parent)}', ec.on_curve(C, Q), ''))
    return rows


def _ctl(m):
    rows = []
    tbs_ctl = m['tbsCtl']
    for n, sig in enumerate(m['signatures']):
        sd = sig['content'][1]
        elector = sd['signer'][1][0]
        rows.append((f'elector {n} {_name(elector)} self-signature', certs.verify_explicit(elector, None), ''))
        C, Q = certs.verification_key(elector)
        hname = certs.HASH_OF_CURVE[C.name]
        tbs = coer.encode('ToBeSignedData', sd['tbsData'])
        digest = prims.digest_1609(hname, tbs, certs.encode_canonical(elector))
        r, s = certs.sig_rs(sd['signature'])
        rows.append((f'elector {n} CTL signature ({hname})', prims.ecdsa_verify(C, Q, digest, r, s), ''))
        payload = sd['tbsData']['payload']['data']['content'][1]
        p = coer.decode('ScmsPdu', payload)
        tbs_sig = p['content'][1][1]
        rows.append((f'elector {n} tbsCtlHash = SHA-384(tbsCtl)', tbs_sig['tbsCtlHash'] == prims.sha384(tbs_ctl), ''))
    return rows
