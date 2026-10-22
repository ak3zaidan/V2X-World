#!/usr/bin/env python3
"""WP-B4: verify the SecuredCrls a v2xw-proto run issued, with the A0 oracle only.

    V2XW_PDU_OUT=<dir> cargo test -p v2xw-proto --test crl_issuance a_revocation_publishes
    <oracle venv>/bin/python -I tools/pdu_check/check_run_crl.py <dir>/crl

Checks (re-derived from 1609.2 Ieee1609Dot2Crl / Ieee1609Dot2CrlSsp and 02 §11 by the
WP-B4 agent; the primitives and the linkage rule are the oracle's): canonical COER
SecuredCrl; signedData sha256; headerInfo exactly {psid 256}; signer one attached
explicit certificate issued by the root or by the ICA (which the root issued), and whose
PSID-256 SSP is CrlSsp{version 1, issuerIsCraca, crls [4, 3, 1, 2]}; the signature;
CrlContents version 1, nextCrl > issueDate, crlSeries 1 is a fullLinkedCrl and is
permitted, crlCraca = HashedId8(the signer's issuer) (the issuerIsCraca rule); and the
oracle's linkage-CRL matching revokes
every certificate listed as revoked and none listed as not revoked. Prints one verdict.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'oracle'))

from scms_oracle import certs, chain, coer, prims  # noqa: E402


def main(d):
    m = json.load(open(os.path.join(d, 'manifest.json')))
    rows = []

    def ok(name, cond):
        rows.append((name, bool(cond)))

    cert = lambda h: coer.decode('Certificate', bytes.fromhex(h))  # noqa: E731
    root, ica = cert(m['root']), cert(m['ica'])
    ok('ICA issued by the root', certs.verify_explicit(ica, root))
    for e in m['crls']:
        n = e['file']
        b = open(os.path.join(d, n), 'rb').read()
        data = coer.decode('Ieee1609Dot2Data', b)
        ok(f'{n} canonical COER', coer.encode('Ieee1609Dot2Data', data) == b)
        ok(f'{n} is SecuredCrl', coer.encode('SecuredCrl', coer.decode('SecuredCrl', b)) == b)
        kind, sd = data['content']
        ok(f'{n} signedData sha256', kind == 'signedData' and sd['hashId'] == 'sha256')
        ok(f'{n} header psid 256 only', sd['tbsData']['headerInfo'] == {'psid': 256})
        skind, sval = sd['signer']
        ok(f'{n} one attached signer certificate', skind == 'certificate' and len(sval) == 1)
        signer = sval[0]
        # The CRL signer is issued by the root ([SMP] Table 1, ISS) or by the ICA (the
        # SAESOL production form); under issuerIsCraca its issuer is the CRACA (CF-CRACA).
        up = next((a for a in (root, ica) if signer['issuer'] == ('sha256AndDigest', certs.hashed_id8(a))), None)
        ok(f'{n} signer explicit, issued by the root or the ICA',
           signer['type'] == 'explicit' and up is not None and certs.verify_explicit(signer, up))
        up = up or ica
        ssp = None
        for p in signer['toBeSigned'].get('appPermissions', []):
            if p['psid'] == 256 and 'ssp' in p:
                ssp = coer.decode('CrlSsp', p['ssp'][1])
        ok(f'{n} CrlSsp issuerIsCraca 4,3,1,2',
           ssp == {'version': 1, 'associatedCraca': 'issuerIsCraca', 'crls': [4, 3, 1, 2]})
        tbs = coer.encode('ToBeSignedData', sd['tbsData'])
        C, Q = certs.verification_key(signer)
        r, s = certs.sig_rs(sd['signature'])
        ok(f'{n} signature', prims.ecdsa_verify(C, Q, prims.digest_1609('SHA-256', tbs, certs.encode_canonical(signer)), r, s))
        inner = sd['tbsData']['payload']['data']
        ok(f'{n} payload unsecuredData', inner['content'][0] == 'unsecuredData')
        c = coer.decode('CrlContents', inner['content'][1])
        ok(f'{n} CrlContents canonical', coer.encode('CrlContents', c) == inner['content'][1])
        ok(f'{n} version 1, nextCrl > issueDate', c['version'] == 1 and c['nextCrl'] > c['issueDate'])
        ok(f'{n} series {e["series"]} permitted', c['crlSeries'] == e['series'] and ssp and c['crlSeries'] in ssp['crls'])
        ok(f'{n} crlCraca = HashedId8(the signer\'s issuer)', c['crlCraca'] == certs.hashed_id8(up))
        want = 'deltaLinkedCrl' if e['delta'] else 'fullLinkedCrl'
        ok(f'{n} is {want}', c['typeSpecific'][0] == want)
        for k, h in enumerate(m['revoked']):
            ok(f'{n} revokes listed certificate {k}', chain.linkage_crl_revokes(c, cert(h)))
        for k, h in enumerate(m['not_revoked']):
            ok(f'{n} spares listed certificate {k}', not chain.linkage_crl_revokes(c, cert(h)))
    bad = [n for n, g in rows if not g]
    for n in bad:
        print(f'  RED {n}')
    print(f'run-crl-check: {"PASS" if not bad and rows else "FAIL"} ({len(rows) - len(bad)}/{len(rows)} checks)')
    return 0 if not bad and rows else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1]))
