#!/usr/bin/env python3
"""Independent check of the WP-A5 Rust-emitted CRL/CTL vectors with the A0 oracle.

    <venv>/bin/python -I tools/scms-artefacts/xcheck_crl.py [DIR]

DIR defaults to crates/v2xw-sec/tests/vectors/rust/wp-a5 (written by
crates/v2xw-sec/tests/crl_ctl_vectors.rs). Everything here goes through the oracle's own
asn1tools COER, hashing, ECDSA and linkage code (tools/oracle/scms_oracle); nothing calls
the Rust. This driver only uses the oracle, it does not extend it. Prints one verdict
line, plus the failing checks.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, '..', '..'))
sys.path.insert(0, os.path.join(REPO, 'tools', 'oracle'))

from scms_oracle import certs, chain, coer, prims, real  # noqa: E402


def main():
    d = sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO, 'crates', 'v2xw-sec', 'tests', 'vectors', 'rust', 'wp-a5')
    rows = []

    def ok(name, cond, detail=''):
        rows.append((name, bool(cond), detail))

    def raw(n):
        return open(os.path.join(d, n), 'rb').read()

    def cert(n):
        b = raw(n)
        c = coer.decode('Certificate', b)
        ok(f'{n} re-encodes', coer.encode('Certificate', c) == b)
        return c

    root, crlg, aca = cert('root.cert.oer'), cert('crlg.cert.oer'), cert('aca.cert.oer')
    revoked, past = cert('pseudonym_revoked.cert.oer'), cert('pseudonym_past_imax.cert.oer')
    ok('root self-signed', certs.verify_explicit(root, None))
    ok('crlg issued by root', certs.verify_explicit(crlg, root))
    ok('aca issued by root', certs.verify_explicit(aca, root))
    ok('pseudonyms issued by aca', certs.verify_explicit(revoked, aca) and certs.verify_explicit(past, aca))

    def ssp(c, psid):
        for p in c['toBeSigned'].get('appPermissions', []):
            if p['psid'] == psid and 'ssp' in p:
                return coer.decode('CrlSsp', p['ssp'][1])
        return None

    ok('CrlSsp{1,isCraca,[256]} encodes 00 01 00 01 01 01 00',
       coer.encode('CrlSsp', {'version': 1, 'associatedCraca': 'isCraca', 'crls': [256]}) == bytes.fromhex('00010001010100'))
    s = ssp(crlg, 256)
    ok('crlg CrlSsp', s == {'version': 1, 'associatedCraca': 'isCraca', 'crls': [1, 2, 3, 4]}, repr(s))

    def crl(n):
        b = raw(n)
        data = coer.decode('Ieee1609Dot2Data', b)
        ok(f'{n} re-encodes', coer.encode('Ieee1609Dot2Data', data) == b)
        ok(f'{n} is SecuredCrl', coer.encode('SecuredCrl', coer.decode('SecuredCrl', b)) == b)
        kind, sd = data['content']
        ok(f'{n} signedData sha256', kind == 'signedData' and sd['hashId'] == 'sha256')
        ok(f'{n} header psid 256 only', sd['tbsData']['headerInfo'] == {'psid': 256}, repr(sd['tbsData']['headerInfo']))
        signer = sd['signer']
        ok(f'{n} signer is crlg', signer[0] == 'certificate' and coer.encode('Certificate', signer[1][0]) == raw('crlg.cert.oer'))
        tbs = coer.encode('ToBeSignedData', sd['tbsData'])
        digest = prims.digest_1609('SHA-256', tbs, certs.encode_canonical(crlg))
        C, Q = certs.verification_key(crlg)
        r, s_ = certs.sig_rs(sd['signature'])
        ok(f'{n} signature', prims.ecdsa_verify(C, Q, digest, r, s_))
        inner = sd['tbsData']['payload']['data']
        ok(f'{n} payload unsecuredData', inner['content'][0] == 'unsecuredData')
        contents = coer.decode('CrlContents', inner['content'][1])
        ok(f'{n} CrlContents re-encodes', coer.encode('CrlContents', contents) == inner['content'][1])
        ok(f'{n} version 1, nextCrl > issueDate', contents['version'] == 1 and contents['nextCrl'] > contents['issueDate'])
        ok(f'{n} crlCraca = HashedId8(crlg) (isCraca)', contents['crlCraca'] == certs.hashed_id8(crlg))
        ok(f'{n} series permitted', contents['crlSeries'] in s['crls'])
        return contents

    c1 = crl('crl_series1_linkage.oer')
    c2 = crl('crl_series2_hash_delta.oer')
    ok('series 1 is fullLinkedCrl', c1['crlSeries'] == 1 and c1['typeSpecific'][0] == 'fullLinkedCrl')
    ok('series 2 is deltaHashCrl', c2['crlSeries'] == 2 and c2['typeSpecific'][0] == 'deltaHashCrl')
    ok('linkage CRL revokes the revoked pseudonym', chain.linkage_crl_revokes(c1, revoked))
    ok('linkage CRL spares the certificate past iMax', not chain.linkage_crl_revokes(c1, past))
    entries = c2['typeSpecific'][1]['entries']
    ok('hash CRL revokes the ACA', chain.hash_crl_revokes(entries, aca, 0))
    ok('hash CRL spares the CRL signer', not chain.hash_crl_revokes(entries, crlg, 0))

    for n in ('ctl_3of5.oer', 'ccf.oer'):
        for name, good, detail in real.check_file(os.path.join(d, n)):
            ok(f'{n}: {name}', good, detail)

    b = raw('composite_crl.oer')
    data = coer.decode('Ieee1609Dot2Data', b)
    pdu = coer.decode('ScmsPdu', data['content'][1])
    kind, body = pdu['content'][1]
    ok('composite is compositeCrl', kind == 'compositeCrl')
    ok('composite carries both CRLs',
       [coer.encode('Ieee1609Dot2Data', x) for x in body['crl']] == [raw('crl_series1_linkage.oer'), raw('crl_series2_hash_delta.oer')])
    home = body['homeCtl']
    ok('composite home CTL is the CTL file', coer.encode('Ieee1609Dot2Data', home) == raw('ctl_3of5.oer'))

    bad = [r for r in rows if not r[1]]
    print(f"xcheck_crl: {'PASS' if not bad else 'FAIL'} ({len(rows) - len(bad)}/{len(rows)} checks)")
    for name, _, detail in bad:
        print(f'  FAIL {name} {detail}')
    return 0 if not bad else 1


if __name__ == '__main__':
    sys.exit(main())
