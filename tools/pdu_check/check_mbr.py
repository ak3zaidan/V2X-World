#!/usr/bin/env python3
"""WP-B4: decode and verify the US misbehaviour reports a v2xw-proto test emitted.

    V2XW_PDU_OUT=<dir> cargo test -p v2xw-proto --test mbr corpus_for_the_oracle
    <oracle venv>/bin/python -I tools/pdu_check/check_mbr.py <dir>/mbr

Every codec and cryptographic step uses the WP-A0 oracle (asn1tools COER over the
committed 1609.2 modules plus tools/pdu_check/asn1/CvpdMbdWire.asn; the oracle's ECDSA,
ECQV-1609, ECIES and AES-CCM). Independence caveat, as for check_pdus.py: this driver was
written by the WP-B4 agent that wrote the Rust, so the rules it applies are re-derived
from [SM-MBR] v1.0 §3 and [SM-MBR-ASN] v1.1 by the same hand; the primitives are not.

Per report:
* outer: canonical COER Ieee1609Dot2Data encryptedData, one certRecipInfo addressed to
  the MA certificate's HashedId8, ECIES (P1 = SHA-256 of the canonical MA certificate)
  and AES-128-CCM decrypt;
* signed layer: canonical COER, protocolVersion 3, headerInfo = {psid 0x26,
  generationTime} exactly, signer = one certificate (the reporter's implicit pseudonym
  certificate, reconstructed under its ACA), the 1609.2 signature verifies, payload is
  unsecuredData;
* the payload is canonical COER EndEntityMaInterfacePDU with version 1, psid 32, and
  every open type holds exactly one canonical value of the type its object set selects;
  types and subtypes are in the v1.1 sets, NULL params are empty, and the evidence
  counts hold (type 1: >= 2 BSMs, type 2: exactly 1, type 3: one suspect + one reporter);
* every suspect BSM is signedData over unsecuredData with headerInfo {psid 0x20,
  generationTime}, signed by signingCertificate (attached or by digest), and verifies
  under it; the result must equal the manifest's evidence_ok (a forged-evidence report is
  expected to fail here and only here).

Prints one verdict line.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, '..', 'oracle'))

import asn1tools  # noqa: E402

import check_pdus as cp  # noqa: E402
from scms_oracle import certs, coer  # noqa: E402

need = cp.need

MODULE = os.path.join(HERE, 'asn1', 'CvpdMbdWire.asn')


def mbr_spec():
    parsed = asn1tools.parse_files(coer.module_files('2022') + [MODULE], encoding='cp1252')
    coer._fix_open_types(parsed)  # noqa: SLF001 - the oracle's own open-type rule
    return asn1tools.compile_dict(parsed, 'oer')


SPEC = mbr_spec()


def canon(type_name, data):
    v = SPEC.decode(type_name, data, check_constraints=True)
    need(SPEC.encode(type_name, v, check_constraints=True) == data, f'{type_name} not canonical COER')
    return v


def open_signed(data, psid, pki, signer_must=None):
    d = canon('Ieee1609Dot2Data', data)
    need(d['protocolVersion'] == 3, 'protocolVersion 3')
    need(d['content'][0] == 'signedData', 'signedData')
    sd = d['content'][1]
    hdr = sd['tbsData']['headerInfo']
    need(hdr.get('psid') == psid, f'psid {psid:#x}')
    need(set(hdr) == {'psid', 'generationTime'}, f'header fields {sorted(hdr)}')
    kind, val = sd['signer']
    if signer_must is None:
        need(kind == 'certificate' and len(val) == 1, 'signer: the end-entity certificate only')
        signer = val[0]
    else:
        signer = signer_must
        if kind == 'certificate':
            need(len(val) == 1 and val[0] == signer_must, 'attached signer is signingCertificate')
        else:
            need(kind == 'digest' and val == certs.hashed_id8(signer_must), 'digest names signingCertificate')
    C, Q = pki.key(signer)
    cp.verify_sig(C, Q, coer.encode('ToBeSignedData', sd['tbsData']), certs.encode_canonical(signer),
                  sd['signature'])
    inner = sd['tbsData']['payload']['data']
    need(inner['content'][0] == 'unsecuredData', 'payload is unsecuredData')
    return inner['content'][1], signer, hdr['generationTime']


SUBTYPES = {
    1: {1: None, 2: 'ParamsRandomPosition'},
    2: {1: 'ParamsThreshold', 2: 'ParamsThreshold', 3: None},
    3: {1: 'ParamsObsDistance'},
}


def evidence_ok(e, pki):
    need(len(e['signedBsmList']) >= 1, 'signedBsmList SIZE (1..MAX)')
    try:
        for b in e['signedBsmList']:
            raw = SPEC.encode('Ieee1609Dot2Data', b)
            open_signed(raw, 0x20, pki, signer_must=e['signingCertificate'])
        return True
    except cp.Red:
        return False


def check(data, entry, m, pki):
    ma = pki.by_name[m['ma_cert']]
    plain = cp.decrypt_to(data, ma, int(m['ma_enc_private'], 16))
    payload, signer, _t = open_signed(plain, 0x26, pki)
    need(certs.encode_canonical(signer) == certs.encode_canonical(
        coer.decode('Certificate', bytes.fromhex(entry['reporter_cert']))), 'reporter certificate')
    need(signer['type'] == 'implicit', 'the reporter signs with an implicit pseudonym certificate')
    kind, top = canon('EndEntityMaInterfacePDU', payload)
    need(kind == 'eeMaApplicationSpecificMisbehaviorReport', 'report alternative')
    need(top['version'] == 1, 'version 1')
    need(top['psid'] == 32, 'application psid 0x20')
    need(top['generationTime'] == entry['generation_time'], 'generationTime')
    inner = canon('Psid20Mbr', top['contents'])
    need(inner['version'] == 1, 'Psid20Mbr version 1')
    t = inner['type']
    need(t in SUBTYPES and t == entry['type'], f'type {t}')
    subs = canon('MbrSubtypeList', inner['subtype'])
    got = []
    for s in subs:
        need(s['subtype'] in SUBTYPES[t], f'subtype {s["subtype"]} of type {t}')
        ptype = SUBTYPES[t][s['subtype']]
        if ptype is None:
            need(s['params'] == b'', 'NULL params are empty')
        else:
            canon(ptype, s['params'])
        got.append(s['subtype'])
    need(got == entry['subtypes'], 'subtypes')
    if t == 3:
        ev = canon('EvidenceObsImplausible', inner['evidence'])
        need(len(ev['suspectBsm']['signedBsmList']) == 1 and len(ev['reporterBsm']['signedBsmList']) == 1,
             'ObsImplausible: one suspect and one reporter BSM')
        suspect = ev['suspectBsm']
        need(evidence_ok(ev['reporterBsm'], pki), 'reporter BSM verifies')
    else:
        suspect = canon('SignedBsmsWithCertificate', inner['evidence'])
        n = len(suspect['signedBsmList'])
        need(n >= 2 if t == 1 else n == 1, f'evidence count {n} for type {t}')
    need(evidence_ok(suspect, pki) == entry['evidence_ok'], 'suspect evidence verdict')


def main(d):
    m = json.load(open(os.path.join(d, 'manifest.json')))
    pki = cp.Pki(m['certs'])
    bad = 0
    for e in m['reports']:
        try:
            check(open(os.path.join(d, e['file']), 'rb').read(), e, m, pki)
        except Exception as ex:  # noqa: BLE001 - any failure is red
            bad += 1
            print(f'  RED {e["file"]}: {type(ex).__name__}: {ex}')
    n = len(m['reports'])
    ok = n and not bad
    print(f'mbr-check: {"PASS" if ok else "FAIL"} ({n - bad}/{n} reports decode and verify)')
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1]))
