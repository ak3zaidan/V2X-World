#!/usr/bin/env python3
"""WP-A4: original-variant butterfly response vectors, composed from the WP-A0 oracle's primitives.

For each vector of tests/vectors/oracle/butterfly.json (cocoon encryption key J, its private h_ij),
build the encrypted layer of AcaEeCertResponsePrivateSpdu with the oracle's own ECIES, CCM, EC and
asn1tools COER (02 sections 8.1, 9.4):

    Ieee1609Dot2Data { 3, encryptedData { recipients { rekRecipInfo { HashedId8(COER PublicEncryptionKey(J)),
        eciesNistP256 { V, c, t } (P1 = SHA-256("")) } }, aes128ccm { nonce, CCM(data key, plaintext) } } }

The plaintext is COER Ieee1609Dot2Data { 3, unsecuredData (the vector's implicit certificate) }.
Randomness (data key, ECIES v, nonce) is fixed by random.Random(4401) and written out, so the Rust
encoder must reproduce the bytes exactly and the Rust decoder must recover the plaintext with h_ij.

This script composes oracle primitives; it does not change the oracle (00 section 4.1). It also
decrypts every output again with the oracle's ECIES and CCM before writing.

    <venv>/bin/python -I crates/v2xw-sec/tests/data/gen_butterfly_response_vectors.py [--check]
"""

import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
sys.path.insert(0, os.path.join(ROOT, 'tools', 'oracle'))

from scms_oracle import certs, coer, ec, prims  # noqa: E402

C = ec.P256
SRC = os.path.join(HERE, '..', 'vectors', 'oracle', 'butterfly.json')
OUT = os.path.join(HERE, '..', 'vectors', 'wpa4_butterfly_response.json')


def pt(v):
    return (int(v['x'], 16), int(v['y'], 16))


def build(J, plaintext, dkey, v, nonce):
    pek = coer.encode('PublicEncryptionKey', {'supportedSymmAlg': 'aes128Ccm',
                                              'publicKey': ('eciesNistP256', certs.point_value(C, J))})
    rid = prims.sha256(pek)[-8:]
    V, c, t = prims.ecies_encrypt(J, dkey, prims.ecies_p1('rekRecipInfo'), v)
    ct = prims.ccm_encrypt(dkey, nonce, plaintext)
    data = {'protocolVersion': 3, 'content': ('encryptedData', {
        'recipients': [('rekRecipInfo', {'recipientId': rid, 'encKey': ('eciesNistP256', {
            'v': certs.point_value(C, V), 'c': c, 't': t})})],
        'ciphertext': ('aes128ccm', {'nonce': nonce, 'ccmCiphertext': ct})})}
    return pek, rid, coer.encode('Ieee1609Dot2Data', data)


def reopen(h, spdu):
    d = coer.decode('Ieee1609Dot2Data', spdu)
    enc = d['content'][1]
    pk = enc['recipients'][0][1]
    k = pk['encKey'][1]
    dkey = prims.ecies_decrypt(h, certs.point_from(C, k['v']), k['c'], k['t'], prims.ecies_p1('rekRecipInfo'))
    if dkey is None:
        return None
    ccm = enc['ciphertext'][1]
    return prims.ccm_decrypt(dkey, ccm['nonce'], ccm['ccmCiphertext'])


def main():
    src = json.load(open(SRC))
    rnd = random.Random(4401)
    out = []
    for n, b in enumerate(src['vectors']):
        J, h = pt(b['J']), int(b['h_ij'], 16)
        assert ec.base_mul(C, h) == J
        plaintext = coer.encode('Ieee1609Dot2Data', {'protocolVersion': 3, 'content': (
            'unsecuredData', bytes.fromhex(b['implicit']['cert_coer']))})
        dkey, nonce = rnd.randbytes(16), rnd.randbytes(12)
        v = rnd.randrange(1, C.n)
        pek, rid, spdu = build(J, plaintext, dkey, v, nonce)
        assert reopen(h, spdu) == plaintext
        assert reopen((h + 1) % C.n, spdu) is None, 'a wrong h_ij must not open the wrap'
        out.append({'source_index': n, 'i': b['i'], 'j': b['j'], 'h_ij': b['h_ij'], 'J': b['J'],
                    'public_encryption_key_coer': pek.hex(), 'recipient_id': rid.hex(),
                    'data_key': dkey.hex(), 'ecies_v': '%064x' % v, 'nonce': nonce.hex(),
                    'plaintext': plaintext.hex(), 'spdu_coer': spdu.hex()})
    doc = {'generator': 'crates/v2xw-sec/tests/data/gen_butterfly_response_vectors.py (WP-A4, oracle primitives)',
           'source': 'tests/vectors/oracle/butterfly.json', 'edition': '1609.2-2022',
           'kind': 'butterfly-original-response-encryption', 'vectors': out}
    text = json.dumps(doc, indent=1) + '\n'
    if '--check' in sys.argv:
        same = open(OUT).read() == text
        print('butterfly response vectors:', 'PASS (file reproduces)' if same else 'FAIL (file differs)')
        return 0 if same else 1
    open(OUT, 'w').write(text)
    print(f'butterfly response vectors: wrote {len(out)}; each reopened with the oracle')
    return 0


if __name__ == '__main__':
    sys.exit(main())
