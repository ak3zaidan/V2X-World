#!/usr/bin/env python3
"""WP-B3b: decode and verify every SPDU of the RSU application / OBU identification
certificate exchange, and re-derive every certificate key, with the WP-A0 oracle.

    V2XW_APPCERT_OUT=<dir> cargo test -p v2xw-proto --test appcert
    <oracle venv>/bin/python -I tools/pdu_check/check_appcerts.py <dir>

Primitives are the oracle's (asn1tools COER over the committed 1609.2 / 1609.2.1-2022
modules, ECDSA, ECQV-1609, ECIES, AES-CCM, butterfly expansion), written from the standards
without reading the Rust. Independence caveat (as check_pdus.py): the *rules* below were
written by the WP-B3b agent that wrote the Rust; the primitives are independent.

Per SPDU kind:
  EeRaCertRequestSpdu   certRecipInfo to the RA, ECIES + CCM; SignedCertificateRequest by
                        an implicit enrolment certificate chaining (ECQV) to the ECA; type
                        implicit; cracaId 000000, crlSeries 0; id not linkageData;
                        additionalParams encryptionKey (plain) or original (butterfly).
  RaEeCertAckSpdu       RA-signed; requestHash = low 8 bytes of SHA-256 of a request sent;
                        firstI present iff that request was butterfly.
  RaAcaCertRequestSpdu  SignedCertificateRequest by the RA certificate; type implicit; flags
                        all clear; linkageInfo absent; certEncKey present; reconstructionValue;
                        for plain requests R and certEncKey equal the EE's; for butterfly they
                        equal the oracle's own expansion of (A, ck) and (H, ek) at (i, 0).
  AcaRaCertResponseSpdu ACA-signed; requestHash of an RaAcaCertRequestSpdu; acaResponse is
                        private and byte-identical to a file the RA served.
  AcaEeCertResponsePrivateSpdu
                        signed by the ACA; rekRecipInfo to the EE's response key, decrypts;
                        acaEeCertResponse; the certificate is implicit, issued by the same ACA,
                        crlSeries 3, cracaId = HashedId3(ICA), id none, validity hours(169)
                        or hours(721); d = e*k_U + r (oracle ECQV) satisfies d*G = e*P_U + Q_ACA.
  EeRaDownloadRequestSpdu
                        to the RA; signed by an enrolment certificate; filename
                        <requestHash>_<i>.zip naming a request sent.
Prints one verdict line.
"""

import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, '..', 'oracle'))

import check_pdus as cp  # noqa: E402
from scms_oracle import butterfly, certs, coer, ec, ecqv, prims  # noqa: E402

need, Red = cp.need, cp.Red
C = ec.P256


def h8(b):
    return prims.hashed_id(b, 8)


def open_signed_any(data, pki):
    """EncryptedSigned{T, 35} = Signed{Encrypted{T}, 35}: the signed payload is
    unsecuredData CONTAINING the encrypted Ieee1609Dot2Data (1609.2.1 Protocol module)."""
    d = cp.decode_canonical('Ieee1609Dot2Data', data)
    need(d['protocolVersion'] == 3 and d['content'][0] == 'signedData', 'signedData')
    sd = d['content'][1]
    hdr = sd['tbsData']['headerInfo']
    need(set(hdr.keys()) == {'psid'} and hdr['psid'] == 35, 'header psid 35 only')
    need(sd['signer'][0] == 'certificate' and len(sd['signer'][1]) == 1, 'SignerSingleCert')
    signer = sd['signer'][1][0]
    Cs, Q = pki.key(signer)
    need(sd['hashId'] == cp.HASH_ID[Cs.name], "CF-RULE: SignedData.hashId is the signer key's hash")
    cp.verify_sig(Cs, Q, coer.encode('ToBeSignedData', sd['tbsData']), certs.encode_canonical(signer),
                  sd['signature'])
    data_ = sd['tbsData']['payload']['data']
    need(data_['content'][0] == 'unsecuredData', 'Signed payload is unsecuredData')
    return cp.decode_canonical('Ieee1609Dot2Data', data_['content'][1]), signer


def rek_decrypt(inner, priv):
    need(inner['content'][0] == 'encryptedData', 'encryptedData')
    enc = inner['content'][1]
    need(len(enc['recipients']) == 1, 'one recipient')
    kind, ri = enc['recipients'][0]
    need(kind == 'rekRecipInfo', 'rekRecipInfo')
    P = ec.base_mul(C, priv)
    pek = coer.encode('PublicEncryptionKey', {'supportedSymmAlg': 'aes128Ccm',
                                              'publicKey': ('eciesNistP256', certs.point_value(C, P))})
    need(ri['recipientId'] == h8(pek), 'rekRecipInfo recipientId = HashedId8(COER PublicEncryptionKey)')
    alg, k = ri['encKey']
    need(alg == 'eciesNistP256', 'eciesNistP256')
    V = certs.point_from(C, k['v'])
    key = prims.ecies_decrypt(priv, V, k['c'], k['t'], prims.ecies_p1('rekRecipInfo'))
    need(key is not None, 'ECIES unwrap')
    calg, ct = enc['ciphertext']
    need(calg == 'aes128ccm', 'aes128ccm')
    plain = prims.ccm_decrypt(key, ct['nonce'], ct['ccmCiphertext'])
    need(plain is not None, 'CCM tag')
    return plain


def pek_point(pek):
    need(pek['supportedSymmAlg'] == 'aes128Ccm', 'aes128Ccm')
    need(pek['publicKey'][0] == 'eciesNistP256', 'eciesNistP256')
    return certs.point_from(C, pek['publicKey'][1])


def check(kind, data, pki, m, st):
    ra = pki.by_name['ra.provider-a']
    aca = pki.by_name['aca.provider-a']
    ica = pki.role('ica')
    if kind == 'EeRaCertRequestSpdu':
        plain = cp.decrypt_to(data, ra, ec.os2i(bytes.fromhex(m['ra_enc_private'])))
        (iface, (c, req)), signer, _ = cp.open_cert_request(plain, pki)
        need((iface, c) == ('ee-ra', 'eeRaCertRequest'), 'eeRaCertRequest')
        need(signer is not None and signer['type'] == 'implicit', 'signed by an implicit enrolment certificate')
        need(signer['issuer'] == ('sha256AndDigest', certs.hashed_id8(pki.by_name['eca.provider-a'])), 'issued by the ECA')
        need(req['type'] == 'implicit', 'implicit requested')
        cp.v2(req, 'EeRaCertRequest')
        t = req['tbsCert']
        cp.tbs_request_permissions(t, 'EeRaCertRequest')
        need(t['cracaId'] == b'\x00\x00\x00' and t['crlSeries'] == 0, 'cracaId 000000, crlSeries 0')
        need(t['id'][0] != 'linkageData', 'id is not linkageData')
        vk = t['verifyKeyIndicator']
        need(vk[0] == 'verificationKey' and vk[1][0] == 'ecdsaNistP256', 'verificationKey P-256')
        ap = req['additionalParams']
        need(ap[0] in ('encryptionKey', 'original'), 'encryptionKey or original butterfly')
        st['requests'][h8(data)] = {'req': req, 'A': certs.point_from(C, vk[1][1]), 'signer': signer}
    elif kind == 'RaEeCertAckSpdu':
        payload, signer = cp.open_signed(data, pki)
        need(certs.hashed_id8(signer) == certs.hashed_id8(ra), 'signed by the RA')
        iface, (c, ack) = cp.scms_pdu(payload)
        need((iface, c) == ('ee-ra', 'raEeCertAck'), 'raEeCertAck')
        cp.v2(ack, 'RaEeCertAck')
        r = st['requests'].get(ack['requestHash'])
        need(r is not None, 'requestHash of a request sent')
        need(('firstI' in ack) == (r['req']['additionalParams'][0] == 'original'), 'firstI iff butterfly')
    elif kind == 'RaAcaCertRequestSpdu':
        (iface, (c, req)), signer, _ = cp.open_cert_request(data, pki)
        need((iface, c) == ('aca-ra', 'raAcaCertRequest'), 'raAcaCertRequest')
        need(signer is not None and certs.hashed_id8(signer) == certs.hashed_id8(ra), 'SignerSingleCert = RA')
        need(req['type'] == 'implicit', 'implicit')
        cp.v2(req, 'RaAcaCertRequest')
        need(req['flags'][0] == b'\x00' and req['flags'][1] == 8, 'flags clear')
        need('linkageInfo' not in req and 'certEncKey' in req, 'no linkageInfo, certEncKey present')
        t = req['tbsCert']
        need(t['cracaId'] == b'\x00\x00\x00' and t['crlSeries'] == 0, 'cracaId/crlSeries 0')
        cp.tbs_request_permissions(t, 'RaAcaCertRequest')
        need(t['verifyKeyIndicator'][0] == 'reconstructionValue', 'reconstructionValue')
        R = certs.point_from(C, t['verifyKeyIndicator'][1])
        J = pek_point(req['certEncKey'])
        ok = False
        for r in st['requests'].values():
            ap = r['req']['additionalParams']
            if ap[0] == 'encryptionKey':
                ok = ok or (R == r['A'] and J == pek_point(ap[1]))
            else:
                b = ap[1]
                H = pek_point(b['encryptionKey'])
                for s in m['secrets']:
                    i = s['period']
                    if (R == butterfly.expand_public(r['A'], b['signingExpansion'][1], butterfly.SIGN, i, 0)
                            and J == butterfly.expand_public(H, b['encryptionExpansion'][1], butterfly.ENC, i, 0)):
                        ok = True
        need(ok, 'R and certEncKey follow from an EE request (oracle expansion for butterfly)')
        st['aca_requests'].add(h8(data))
    elif kind == 'AcaRaCertResponseSpdu':
        payload, signer = cp.open_signed(data, pki)
        need(certs.hashed_id8(signer) == certs.hashed_id8(aca), 'signed by the ACA')
        iface, (c, resp) = cp.scms_pdu(payload)
        need((iface, c) == ('aca-ra', 'acaRaCertResponse'), 'acaRaCertResponse')
        cp.v2(resp, 'AcaRaCertResponse')
        need(resp['requestHash'] in st['aca_requests'], 'requestHash of an RaAcaCertRequestSpdu')
        need(resp['acaResponse'][0] == 'private', 'private response')
        st['private_inner'].append(coer.encode('Ieee1609Dot2Data', resp['acaResponse'][1]))
    elif kind == 'AcaEeCertResponsePrivateSpdu':
        need(data in st['private_inner'], 'the RA served the ACA response untouched')
        inner, signer = open_signed_any(data, pki)
        need(certs.hashed_id8(signer) == certs.hashed_id8(aca), 'signed by the ACA')
        last = None
        for s in m['secrets']:
            try:
                plain = rek_decrypt(inner, ec.os2i(bytes.fromhex(s['response'])))
            except Red as e:
                last = e
                continue
            break
        else:
            raise Red(f'no response key decrypts it ({last})')
        u = cp.decode_canonical('Ieee1609Dot2Data', plain)
        need(u['content'][0] == 'unsecuredData', 'inner unsecuredData')
        iface, (c, r) = cp.scms_pdu(u['content'][1])
        need((iface, c) == ('aca-ee', 'acaEeCertResponse'), 'acaEeCertResponse')
        cp.v2(r, 'AcaEeCertResponse')
        need(len(r.get('privateKeyInfo', b'')) == 32, 'CF-RULE: privateKeyInfo present (implicit)')
        cert = r['certificate']
        tb = cert['toBeSigned']
        need(cert['type'] == 'implicit', 'implicit')
        need(cert['issuer'] == ('sha256AndDigest', certs.hashed_id8(signer)), 'issued by the ACA that signed')
        need(tb['crlSeries'] == 3, 'crlSeries 3')
        need(tb['cracaId'] == certs.hashed_id3(ica), 'cracaId = HashedId3(ICA)')
        need(tb['id'] == ('none', None), 'id none (no LinkageData)')
        dur = tb['validityPeriod']['duration']
        need(dur in (('hours', 169), ('hours', 721)), f'validity {dur}')
        if s['caterpillar']:
            a, ck, p, ek = s['caterpillar']
            b = butterfly.expand_private(ec.os2i(bytes.fromhex(a)), bytes.fromhex(ck), butterfly.SIGN, s['period'], 0)
            hh = butterfly.expand_private(ec.os2i(bytes.fromhex(p)), bytes.fromhex(ek), butterfly.ENC, s['period'], 0)
            need(b == ec.os2i(bytes.fromhex(s['k_u'])) and hh == ec.os2i(bytes.fromhex(s['response'])),
                 'oracle butterfly expansion gives the EE keys')
            need(dur == ('hours', 721), 'identification: 1 month + 1 h')
        else:
            need(dur == ('hours', 169), 'RSU application: 169 h')
        e = ecqv.e_1609(certs.tbs_bytes(cert), certs.encode_canonical(aca))
        d = ecqv.private_key(e, ec.os2i(bytes.fromhex(s['k_u'])), ec.os2i(r['privateKeyInfo']))
        _, Q = certs.verification_key(cert, aca)
        need(ec.base_mul(C, d) == Q, 'd*G = e*P_U + Q_ACA')
        st['keys'] += 1
    elif kind == 'EeRaDownloadRequestSpdu':
        plain = cp.decrypt_to(data, ra, ec.os2i(bytes.fromhex(m['ra_enc_private'])))
        payload, signer = cp.open_signed(plain, pki)
        need(signer['type'] == 'implicit', 'signed by an enrolment certificate')
        iface, (c, dl) = cp.scms_pdu(payload)
        need((iface, c) == ('ee-ra', 'eeRaDownloadRequest'), 'eeRaDownloadRequest')
        mm = re.fullmatch(r'([0-9A-F]{16})_([0-9A-F]+)\.zip', dl['filename'])
        need(mm is not None, 'filename <requestHash>_<i>.zip')
        need(bytes.fromhex(mm.group(1)) in st['requests'], 'names a request sent')
    else:
        raise Red(f'unknown kind {kind}')


def main(d):
    m = json.load(open(os.path.join(d, 'manifest.json')))
    pki = cp.Pki(m['certs'])
    st = {'requests': {}, 'aca_requests': set(), 'private_inner': [], 'keys': 0}
    bad = total = 0
    for p in m['pdus']:
        if p['kind'] in ('EeEcaCertRequestSpdu', 'EcaEeCertResponseSpdu'):
            continue  # enrolment: check_pdus.py
        total += 1
        try:
            check(p['kind'], open(os.path.join(d, p['file']), 'rb').read(), pki, m, st)
        except Exception as e:  # noqa: BLE001 - any failure is red
            bad += 1
            print(f'  RED {p["file"]}: {type(e).__name__}: {e}')
    want = len(m['installed'])
    ok = not bad and total and st['keys'] == want
    print(f'appcert-check: {"PASS" if ok else "FAIL"} ({total - bad}/{total} SPDUs verify, '
          f'{st["keys"]}/{want} certificate keys re-derived)')
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1]))
