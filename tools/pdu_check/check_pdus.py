#!/usr/bin/env python3
"""WP-B1: decode and verify every IEEE 1609.2.1 SPDU a v2xw-proto flow test emitted.

    V2XW_PDU_OUT=<dir> cargo test -p v2xw-proto --test pdu_layer
    <oracle venv>/bin/python -I tools/pdu_check/check_pdus.py <dir>

Every cryptographic and codec step uses the WP-A0 oracle's primitives (asn1tools COER over
the committed 1609.2 / 1609.2.1-2022 modules; the oracle's ECDSA, ECQV-1609, ECIES, KDF2,
MAC1 and AES-CCM, written from the standards without reading the Rust). Independence
caveat: this driver itself was written by the WP-B1 agent that wrote the Rust under test,
so the *rules it applies* are not independent; the primitives are. A later oracle agent
should re-derive these rules from 03 §3 alone.

Checks per SPDU: COER canonical round trip of Ieee1609Dot2Data and of the ScmsPdu;
protocolVersion 3, ScmsPdu version 2; for signed data: headerInfo is psid 35 only and the
signer is exactly one certificate, which chains (explicit: issuer signature; implicit:
ECQV-1609 reconstruction under its issuer) to the emitted PKI; the 1609.2 signature
verifies; for cert requests: SignerSelf verifies under the requested verificationKey,
SignerSingleCert under the (reconstructed) enrolment key; for encrypted data: one
certRecipInfo addressed to the RA certificate, ECIES (P1 = SHA-256(canonical RA cert)) and
AES-128-CCM decrypt; the expected inner PDU choice; and the RA ack's requestHash equals
the low 8 bytes of SHA-256 over the request SPDU as sent. Prints one verdict line.

WP-B2 additions (written by the WP-B2 agent, same independence caveat; the rules are
re-derived from 03 §3.2–§5 and [SMP] §6.3.1, not from the Rust):

    V2XW_PDU_OUT=<dir> cargo test -p v2xw-proto --test enrolment a_successor_requested

* EeEcaCertRequestSpdu: with canonicalId, verified under the registered canonical key
  (manifest "canonical"), and canonicalId must equal SHA-256 of the COER
  PublicVerificationKey as upper-case hex (the assumed text form); the tbsCert is the US
  enrolment profile (id.name, cracaId 000000, crlSeries 4, identifiedRegion countryOnly
  840/124/484, no appPermissions / certIssuePermissions, one certRequestPermissions group
  with subjectPermissions all).
* EcaEeCertResponseSpdu: requestHash names a request the ECA received; the certificate is
  the profile and exactly the validity, name and region requested.
* EeRaSuccessorEnrollmentCertRequestSpdu: decrypts to the RA; the outer signer is an
  enrolment certificate the ECA issued; the inner EeEcaCertRequestSpdu is self-signed by a
  new key, keeps name and region, and starts exactly when the signer's validity ends.
* RaEeEnrollmentCertAckSpdu: RA-signed raEeCertInfo for a successor request as sent.
* EeRaDownloadRequestSpdu: decrypts to the RA, signed by an enrolment certificate, and
  names `<requestHash, 16 upper-case hex>.zip` of a successor request.
"""

import hashlib
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'oracle'))

from scms_oracle import butterfly, certs, coer, ec, ecqv, prims  # noqa: E402
import re  # noqa: E402


class Red(Exception):
    pass


def need(cond, what):
    if not cond:
        raise Red(what)


def decode_canonical(type_name, data):
    v = coer.decode(type_name, data)
    need(coer.encode(type_name, v) == data, f'{type_name} not canonical COER')
    return v


class Pki:
    def __init__(self, certs_hex):
        self.by_id8 = {}
        self.by_name = {}
        for name, h in certs_hex.items():
            c = coer.decode('Certificate', bytes.fromhex(h))
            self.by_id8[certs.hashed_id8(c)] = c
            self.by_name[name] = c

    def role(self, r):
        """The one certificate of role `r` (the first label of its name): the ICA is
        `ica.provider-a` under the provider's own anchors and `ica.governance` under the
        governance root (WIRE)."""
        names = [n for n in self.by_name if n.split('.')[0] == r]
        need(len(names) == 1, f'exactly one {r} certificate in the PKI, not {names}')
        return self.by_name[names[0]]

    def chain_ok(self, c, depth=0):
        need(depth < 6, 'chain too long')
        kind, val = c['issuer']
        if kind == 'self':
            need(certs.hashed_id8(c) in self.by_id8, 'self-signed signer is not the emitted root')
            need(certs.verify_explicit(c, None), 'root self-signature')
            return
        issuer = self.by_id8.get(val)
        need(issuer is not None, 'signer issuer is not in the PKI')
        if c['type'] == 'explicit':
            need(certs.verify_explicit(c, issuer), 'explicit certificate signature under its issuer')
        self.chain_ok(issuer, depth + 1)

    def key(self, c):
        """(curve, Q) of a signer certificate, after checking its chain."""
        self.chain_ok(c)
        if c['type'] == 'explicit':
            return certs.verification_key(c)
        return certs.verification_key(c, self.by_id8[c['issuer'][1]])


def verify_sig(C, Q, tbs_bytes, signer_input, sig):
    h = certs.HASH_OF_CURVE[C.name]
    r, s = certs.sig_rs(sig)
    need(prims.ecdsa_verify(C, Q, prims.digest_1609(h, tbs_bytes, signer_input), r, s), 'signature')


def open_signed(data, pki):
    d = decode_canonical('Ieee1609Dot2Data', data)
    need(d['protocolVersion'] == 3, 'protocolVersion')
    need(d['content'][0] == 'signedData', 'expected signedData')
    sd = d['content'][1]
    hdr = sd['tbsData']['headerInfo']
    need(hdr.get('psid') == 35, 'psid 35')
    need(set(hdr.keys()) == {'psid'}, f'header fields other than psid: {sorted(hdr)}')
    need(sd['signer'][0] == 'certificate' and len(sd['signer'][1]) == 1, 'SignerSingleCert')
    signer = sd['signer'][1][0]
    C, Q = pki.key(signer)
    verify_sig(C, Q, coer.encode('ToBeSignedData', sd['tbsData']), certs.encode_canonical(signer), sd['signature'])
    inner = sd['tbsData']['payload']['data']
    need(inner['content'][0] == 'unsecuredData', 'payload is unsecuredData')
    return inner['content'][1], signer


def scms_pdu(b):
    p = decode_canonical('ScmsPdu', b)
    need(p['version'] == 2, 'ScmsPdu version 2')
    return p['content']


def decrypt_to(data, rcert, r):
    d = decode_canonical('Ieee1609Dot2Data', data)
    need(d['content'][0] == 'encryptedData', 'expected encryptedData')
    enc = d['content'][1]
    need(len(enc['recipients']) == 1, 'one recipient')
    kind, ri = enc['recipients'][0]
    need(kind == 'certRecipInfo', 'certRecipInfo')
    need(ri['recipientId'] == certs.hashed_id8(rcert), 'recipientId is the RA certificate HashedId8')
    alg, k = ri['encKey']
    need(alg == 'eciesNistP256', 'eciesNistP256')
    V = certs.point_from(ec.P256, k['v'])
    p1 = prims.ecies_p1('certRecipInfo', certs.encode_canonical(rcert))
    key = prims.ecies_decrypt(r, V, k['c'], k['t'], p1)
    need(key is not None, 'ECIES unwrap')
    calg, ct = enc['ciphertext']
    need(calg == 'aes128ccm', 'aes128ccm')
    plain = prims.ccm_decrypt(key, ct['nonce'], ct['ccmCiphertext'])
    need(plain is not None, 'CCM tag')
    return plain


def open_cert_request(data, pki, canonical=None):
    """(content, signer cert or None, canonical id or None). A self-signed request with
    canonicalId is verified under the registered canonical key (WP-B2), else under the
    requested verificationKey."""
    d = decode_canonical('Ieee1609Dot2Data', data)
    need(d['content'][0] == 'signedCertificateRequest', 'expected signedCertificateRequest')
    scr = decode_canonical('SignedCertificateRequest', d['content'][1])
    tbs = coer.encode('ScmsPdu', scr['tbsRequest'])
    scms_pdu(tbs)
    kind, req = scr['tbsRequest']['content'][1]
    if scr['signer'][0] == 'self':
        cid = req.get('canonicalId')
        if cid is not None:
            need(canonical is not None and cid in canonical, 'canonicalId registered')
            raw = bytes.fromhex(canonical[cid])
            C = ec.P256
            Q = ec.decompress(C, raw[0] & 1, ec.os2i(raw[1:]))
            # The canonical identity is SHA-256 of the COER PublicVerificationKey (assumed
            # text form: 64 upper-case hex digits).
            pvk = coer.encode('PublicVerificationKey', ('ecdsaNistP256', certs.point_value(C, Q)))
            need(hashlib.sha256(pvk).hexdigest().upper() == cid, 'canonicalId is HashedId32 of the key')
        else:
            vki = req['tbsCert']['verifyKeyIndicator']
            need(vki[0] == 'verificationKey', 'self-signed request carries a verificationKey')
            C = certs.CURVE_OF_KEY[vki[1][0]]
            Q = certs.point_from(C, vki[1][1])
        verify_sig(C, Q, tbs, b'', scr['signature'])
        return scr['tbsRequest']['content'], None, cid
    need(scr['signer'][0] == 'certificate' and len(scr['signer'][1]) == 1, 'SignerSingleCert')
    signer = scr['signer'][1][0]
    C, Q = pki.key(signer)
    verify_sig(C, Q, tbs, certs.encode_canonical(signer), scr['signature'])
    return scr['tbsRequest']['content'], signer, None


YEAR_S = 31556952
UNIT_S = {'microseconds': 1e-6, 'milliseconds': 1e-3, 'seconds': 1, 'minutes': 60, 'hours': 3600,
          'sixtyHours': 216000, 'years': YEAR_S}


def validity(tbs):
    v = tbs['validityPeriod']
    unit, n = v['duration']
    return v['start'], v['start'] + int(n * UNIT_S[unit])


def check_us_enrolment_tbs(t):
    """[SMP] §6.3.1 / 03 §3.3, re-derived from the profile text, not from the Rust."""
    need(t['id'][0] == 'name' and len(t['id'][1]) > 0, 'id.name present')
    need(t['cracaId'] == b'\x00\x00\x00', 'cracaId 000000')
    need(t['crlSeries'] == 4, 'crlSeries 4')
    reg = t.get('region')
    need(reg is not None and reg[0] == 'identifiedRegion' and len(reg[1]) > 0, 'identifiedRegion')
    for r in reg[1]:
        need(r[0] == 'countryOnly' and r[1] in (840, 124, 484), f'region {r} is USA/Canada/Mexico')
    need('appPermissions' not in t, 'appPermissions absent')
    need('certIssuePermissions' not in t, 'certIssuePermissions absent')
    crp = t.get('certRequestPermissions')
    need(crp is not None and len(crp) == 1 and crp[0]['subjectPermissions'][0] == 'all',
         'certRequestPermissions: one group, subjectPermissions all')


def hash8(b):
    return hashlib.sha256(b).digest()[-8:]


def id8(b):
    return hashlib.sha256(b).digest()[-8:]


def symm_open(enc_data, keys, la_id):
    """WP-B3a: SymmEncryptedSingleRecipient under LA la_id's key shared with the ACA."""
    need(len(enc_data['recipients']) == 1, 'one RecipientInfo')
    kind, ri = enc_data['recipients'][0]
    need(kind == 'symmRecipInfo', 'symmRecipInfo')
    key = bytes.fromhex(keys[str(la_id)])
    need(ri['recipientId'] == id8(coer.encode('SymmetricEncryptionKey', ('aes128Ccm', key))),
         'recipientId is the HashedId8 of the shared key')
    alg, wrapped = ri['encKey']
    need(alg == 'aes128ccm', 'encKey aes128ccm')
    dk = prims.ccm_decrypt(key, wrapped['nonce'], wrapped['ccmCiphertext'])
    need(dk is not None and len(dk) == 16, 'data key unwraps')
    calg, ct = enc_data['ciphertext']
    need(calg == 'aes128ccm', 'aes128ccm')
    plain = prims.ccm_decrypt(dk, ct['nonce'], ct['ccmCiphertext'])
    need(plain is not None, 'CCM tag')
    return plain


def open_plv(e, m):
    need(e['version'] == 2, 'EncryptedIndividualPLV version 2')
    la_id = int.from_bytes(e['laId'], 'big')
    encp = e['encPlv']
    need(encp['content'][0] == 'encryptedData', 'encPlv is encryptedData')
    plv = symm_open(encp['content'][1], m['la_aca_keys'], la_id)
    need(len(plv) == 9, 'a PreLinkageValue is 9 bytes')
    return la_id, plv


def check(kind, data, pki, m, state):
    canonical = m.get('canonical', {})
    issued = state.setdefault('enrolment_certs', {})   # HashedId8 -> certificate
    requests = state.setdefault('enrol_requests', {})  # HashedId8 -> (tbsCert, canonical?)
    if kind == 'EeEcaCertRequestSpdu':
        (iface, (c, req)), signer, cid = open_cert_request(data, pki, canonical)
        need((iface, c) == ('eca-ee', 'eeEcaCertRequest') and signer is None, 'self-signed eeEcaCertRequest')
        need(req['type'] == 'implicit', 'implicit enrolment requested')
        check_us_enrolment_tbs(req['tbsCert'])
        requests[hash8(data)] = (req['tbsCert'], cid)
        state['enrol_req'] = data
    elif kind == 'EcaEeCertResponseSpdu':
        payload, signer = open_signed(data, pki)
        need(certs.hashed_id8(signer) == certs.hashed_id8(pki.by_name['eca.provider-a']), 'signed by the ECA')
        iface, (c, resp) = scms_pdu(payload)
        need((iface, c) == ('eca-ee', 'ecaEeCertResponse'), 'ecaEeCertResponse')
        need(resp['requestHash'] in requests, 'requestHash of a request the ECA received')
        need(len(resp.get('privateKeyInfo', b'')) == 32, 'privateKeyInfo')
        enrol = resp['certificate']
        need(enrol['type'] == 'implicit', 'implicit enrolment certificate')
        pki.key(enrol)  # ECQV reconstruction under the ECA
        t = enrol['toBeSigned']
        check_us_enrolment_tbs(t)
        rt, _ = requests[resp['requestHash']]
        need(validity(t) == validity(rt) and t['id'] == rt['id'] and t.get('region') == rt.get('region'),
             'issued as requested')
        issued[certs.hashed_id8(enrol)] = enrol
        state['enrol_cert'] = enrol
    elif kind == 'EeRaCertRequestSpdu':
        ra = pki.by_name[m['ra_cert']]
        plain = decrypt_to(data, ra, ec.os2i(bytes.fromhex(m['ra_enc_private'])))
        (iface, (c, req)), signer, _ = open_cert_request(plain, pki)
        need((iface, c) == ('ee-ra', 'eeRaCertRequest'), 'eeRaCertRequest')
        need(signer is not None and certs.hashed_id8(signer) in issued,
             'signed by an enrolment certificate the ECA issued')
        need(req['additionalParams'][0] == 'original', 'original butterfly')
        need(req['tbsCert']['cracaId'] == b'\x00\x00\x00' and req['tbsCert']['crlSeries'] == 0, 'cracaId/crlSeries')
        state.setdefault('requests', []).append(data)
    elif kind == 'RaEeCertAckSpdu':
        payload, signer = open_signed(data, pki)
        need(certs.hashed_id8(signer) == certs.hashed_id8(pki.by_name['ra.provider-a']), 'signed by the RA')
        iface, (c, ack) = scms_pdu(payload)
        need((iface, c) == ('ee-ra', 'raEeCertAck'), 'raEeCertAck')
        need(ack['requestHash'] in [hash8(r) for r in state['requests']], 'requestHash of a request as sent')
        need('firstI' in ack, 'firstI present for butterfly')
    elif kind == 'EeRaSuccessorEnrollmentCertRequestSpdu':
        ra = pki.by_name[m['ra_cert']]
        plain = decrypt_to(data, ra, ec.os2i(bytes.fromhex(m['ra_enc_private'])))
        (iface, (c, inner)), signer, _ = open_cert_request(plain, pki)
        need((iface, c) == ('ee-ra', 'eeRaSuccessorEnrollmentCertRequest'), 'eeRaSuccessorEnrollmentCertRequest')
        need(signer is not None and certs.hashed_id8(signer) in issued,
             'outer signature by the current enrolment certificate')
        inner_bytes = coer.encode('Ieee1609Dot2Data', inner)
        (i2, (c2, req)), s2, cid = open_cert_request(inner_bytes, pki, canonical)
        need((i2, c2) == ('eca-ee', 'eeEcaCertRequest') and s2 is None and cid is None,
             'inner request self-signed by the new key')
        t, cur = req['tbsCert'], signer['toBeSigned']
        check_us_enrolment_tbs(t)
        need(validity(t)[0] == validity(cur)[1], 'successor starts when the current certificate ends')
        need(t['id'] == cur['id'] and t.get('region') == cur.get('region'), 'same name and region')
        need(t['verifyKeyIndicator'][1] != cur['verifyKeyIndicator'][1], 'a new key')
        requests[hash8(inner_bytes)] = (t, None)
        state.setdefault('successors', []).append(data)
    elif kind == 'RaEeEnrollmentCertAckSpdu':
        payload, signer = open_signed(data, pki)
        need(certs.hashed_id8(signer) == certs.hashed_id8(pki.by_name['ra.provider-a']), 'signed by the RA')
        iface, (c, info) = scms_pdu(payload)
        need((iface, c) == ('ee-ra', 'raEeCertInfo') and 'acpcTreeId' not in info, 'raEeCertInfo, no ACPC')
        need(info['requestHash'] in [hash8(r) for r in state.get('successors', [])],
             'requestHash of a successor request as sent')
        need(info['nextDlTime'] > info['generationTime'], 'nextDlTime after generation')
    elif kind == 'EncryptedIndividualPLV':
        open_plv(decode_canonical('EncryptedIndividualPLV', data), m)
    elif kind == 'RaAcaCertRequestSpdu':
        (iface, (c, req)), signer, _ = open_cert_request(data, pki)
        need((iface, c) == ('aca-ra', 'raAcaCertRequest'), 'raAcaCertRequest')
        need(signer is not None and certs.hashed_id8(signer) == certs.hashed_id8(pki.by_name['ra.provider-a']),
             'signed by the RA')
        need(req['type'] == 'implicit', 'implicit')
        need(req['flags'][0] == b'\x00', 'flags clear (original butterfly, not cubk)')
        need('certEncKey' in req and 'linkageInfo' in req, 'private response requested with linkageInfo')
        tbs = req['tbsCert']
        need(tbs['cracaId'] == b'\x00\x00\x00' and tbs['crlSeries'] == 0, 'cracaId 000000 / crlSeries 0')
        need(tbs['id'][0] == 'linkageData', 'id is linkageData')
        # 1609.2.1-2022 Ieee1609Dot2Dot1AcaRaInterface, RaAcaCertRequest @param tbsCert:
        # "If type is implicit, the request is valid only if tbsCert.verifyKeyIndicator is
        # a reconstructionValue" (WP-P2: one builder for every certificate type).
        need(tbs['verifyKeyIndicator'][0] == 'reconstructionValue', 'B_ij as reconstructionValue')
        (l1, p1), (l2, p2) = (open_plv(req['linkageInfo'][k], m) for k in ('encPlv1', 'encPlv2'))
        need(l1 != l2, 'PLVs from two LAs')
        lv = bytes(a ^ b for a, b in zip(p1, p2))
        state.setdefault('aca_req', {})[id8(data)] = {'lv': lv, 'i': tbs['id'][1]['iCert'],
                                                      'B': tbs['verifyKeyIndicator'][1],
                                                      'J': req['certEncKey']['publicKey'][1]}
    elif kind == 'AcaRaCertResponseSpdu':
        payload, signer = open_signed(data, pki)
        need(certs.hashed_id8(signer) == certs.hashed_id8(pki.by_name['aca.provider-a']), 'signed by the ACA')
        iface, (c, resp) = scms_pdu(payload)
        need((iface, c) == ('aca-ra', 'acaRaCertResponse'), 'acaRaCertResponse')
        req = state.get('aca_req', {}).get(resp['requestHash'])
        need(req is not None, 'requestHash names a RaAcaCertRequestSpdu as sent')
        alt, inner = resp['acaResponse']
        need(alt == 'private', 'acaResponse.private')
        state.setdefault('private', {})[coer.encode('AcaEeCertResponsePrivateSpdu', inner)] = req
    elif kind == 'AcaEeCertResponsePrivateSpdu':
        k = state.get('n_private', 0)
        state['n_private'] = k + 1
        i, j = m['private_ij'][k]
        dev = m['device']
        a, pp = (ec.os2i(bytes.fromhex(dev[x])) for x in ('a', 'p'))
        ck, ek = (bytes.fromhex(dev[x]) for x in ('ck', 'ek'))
        h = butterfly.expand_private(pp, ek, butterfly.ENC, i, j)
        b = butterfly.expand_private(a, ck, butterfly.SIGN, i, j)
        req = state.get('private', {}).get(data)
        need(req is not None, 'the file is the private response the ACA sent the RA')
        J = ec.base_mul(ec.P256, h)
        need(certs.point_from(ec.P256, req['J']) == J, 'certEncKey is J_ij = h_ij*G')
        need(certs.point_from(ec.P256, req['B']) == ec.base_mul(ec.P256, b), 'tbsCert key is B_ij = b_ij*G')
        payload, signer = open_signed(data, pki)
        d = decode_canonical('Ieee1609Dot2Data', payload)
        need(d['content'][0] == 'encryptedData', 'encrypted to the EE')
        enc = d['content'][1]
        need(len(enc['recipients']) == 1 and enc['recipients'][0][0] == 'rekRecipInfo', 'one rekRecipInfo')
        ri = enc['recipients'][0][1]
        pek = coer.encode('PublicEncryptionKey', {'supportedSymmAlg': 'aes128Ccm',
                                                  'publicKey': ('eciesNistP256', certs.point_value(ec.P256, J))})
        need(ri['recipientId'] == id8(pek), 'recipientId is HashedId8(COER PublicEncryptionKey J_ij)')
        V = certs.point_from(ec.P256, ri['encKey'][1]['v'])
        dk = prims.ecies_decrypt(h, V, ri['encKey'][1]['c'], ri['encKey'][1]['t'], prims.ecies_p1('rekRecipInfo'))
        need(dk is not None, 'ECIES unwrap with h_ij')
        ct = enc['ciphertext'][1]
        plain = prims.ccm_decrypt(dk, ct['nonce'], ct['ccmCiphertext'])
        need(plain is not None, 'CCM tag')
        u = decode_canonical('Ieee1609Dot2Data', plain)
        need(u['content'][0] == 'unsecuredData', 'inner unsecuredData')
        iface, (c, resp) = scms_pdu(u['content'][1])
        need((iface, c) == ('aca-ee', 'acaEeCertResponse') and resp['version'] == 2, 'acaEeCertResponse v2')
        cert = resp['certificate']
        need(cert['type'] == 'implicit', 'implicit pseudonym certificate')
        need(cert['issuer'] == ('sha256AndDigest', certs.hashed_id8(signer)),
             'same-ACA rule: the outer signer issued the certificate')
        tbs = cert['toBeSigned']
        need(tbs['id'][0] == 'linkageData' and tbs['id'][1]['iCert'] == i == req['i'], 'iCert is i')
        need(tbs['id'][1]['linkage-value'] == req['lv'], 'lv = plv1 XOR plv2')
        need(tbs['crlSeries'] == 1, 'crlSeries 1')
        # WP-B4: the CRL signer is issuerIsCraca under the ICA (published US CRL-signer
        # CrlSsp), so the CRACA a pseudonym certificate names is the ICA.
        need(tbs['cracaId'] == certs.hashed_id8(pki.role('ica'))[-3:], 'cracaId = ICA HashedId3')
        r = ec.os2i(resp['privateKeyInfo'])
        e = ecqv.e_1609(certs.tbs_bytes(cert), certs.encode_canonical(signer))
        dd = (e * b + r) % ec.P256.n
        _, Q = certs.verification_key(cert, signer)
        need(ec.base_mul(ec.P256, dd) == Q, 'd*G = e*P_U + Q_ACA')
    elif kind == 'EeRaDownloadRequestSpdu':
        ra = pki.by_name[m['ra_cert']]
        plain = decrypt_to(data, ra, ec.os2i(bytes.fromhex(m['ra_enc_private'])))
        payload, signer = open_signed(plain, pki)
        need(certs.hashed_id8(signer) in issued, 'signed by an enrolment certificate')
        iface, (c, dl) = scms_pdu(payload)
        need((iface, c) == ('ee-ra', 'eeRaDownloadRequest'), 'eeRaDownloadRequest')
        fn = dl['filename']
        if re.fullmatch(r'[0-9A-F]{16}\.zip', fn):
            # WP-B2: a successor enrolment certificate download.
            names = [hash8(r).hex().upper() + '.zip' for r in state.get('successors', [])]
            need(fn in names, f'file {fn} is a successor request hash')
        else:
            # WP-B3a: a pseudonym batch download.
            need('enrol_cert' in state and coer.encode('Certificate', signer) == coer.encode('Certificate', state['enrol_cert']),
                 'signed by the enrolment certificate')
            need(re.fullmatch(r'[0-9A-F]{16}_[0-9A-F]{1,8}\.zip', fn) is not None, f'batch file name {fn}')
            need(fn[:16] == id8(state['requests'][0]).hex().upper(), 'named by the request hash')
    else:
        raise Red(f'unknown kind {kind}')


def main(d):
    m = json.load(open(os.path.join(d, 'manifest.json')))
    pki = Pki(m['certs'])
    state, bad, total, cert_bad = {}, 0, 0, 0
    for name in m['certs']:
        try:
            pki.chain_ok(pki.by_name[name])
        except Exception as e:  # noqa: BLE001 - any failure is a red verdict
            cert_bad += 1
            print(f'  RED certificate {name}: {type(e).__name__}: {e}')
    for p in m['pdus']:
        total += 1
        try:
            check(p['kind'], open(os.path.join(d, p['file']), 'rb').read(), pki, m, state)
        except Exception as e:  # noqa: BLE001 - a decode error is as red as a bad signature
            bad += 1
            print(f'  RED {p["file"]}: {type(e).__name__}: {e}')
    ok = not bad and not cert_bad and total
    print(f'pdu-check: {"PASS" if ok else "FAIL"} ({total - bad}/{total} SPDUs verify, '
          f'{len(m["certs"]) - cert_bad}/{len(m["certs"])} PKI certificates chain)')
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1]))
