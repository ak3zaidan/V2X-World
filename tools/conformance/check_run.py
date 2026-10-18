#!/usr/bin/env python3
"""WP-F1: the conformance oracle. Decodes, verifies and decrypts every PDU of a conformance run.

    V2XW_CONFORMANCE_OUT=<dir> cargo test -p v2xw-engine --test conformance
    <oracle venv>/bin/python -I tools/conformance/check_run.py <dir> [--json OUT] [--mutate]

The capture (written by crates/v2xw-engine/tests/conformance.rs) is what a recording of the run
holds, plus the private keys a recipient opens its PDUs with. Every codec and cryptographic step
uses the WP-A0 oracle's primitives (asn1tools COER over the committed 1609.2 / 1609.2.1-2022
modules; the oracle's ECDSA, ECQV-1609, butterfly expansion, linkage values, ECIES, KDF2, MAC1
and AES-CCM, written from the standards without reading the Rust).

Independence. The rules below were derived by the WP-F1 agent from the standard text only: the
ASN.1 modules' own constraints and comments ([ASN-2.1] = tools/oracle/asn1/ieee1609.2.1-2022,
[ASN-2] = tools/oracle/asn1/ieee1609.2-2022), docs/design/scms-real/02 (sections 7, 8, 9, 10, 11,
12) and 03 (sections 3, 8, 10), and [SM-MBR] via tools/pdu_check/asn1/CvpdMbdWire.asn (a
structural restatement used for decoding only). The agent did not read the Rust that builds or
opens these PDUs; it wrote only the capture hooks in the engine test. No rule is taken from the
earlier tools/pdu_check drivers (whose rules were written by the agents that wrote the Rust).
Every rule is tagged with its source; rules the oracle did not have before are marked NEW in
docs/design/scms-real/06-conformance-run.md.

What is checked (each a disagreement when it fails):

* every SPDU: canonical COER of every layer (decode, re-encode, byte for byte);
  protocolVersion 3; ScmsPdu version 2; the inner PDU alternative its SPDU type names;
* signed layers ([ASN-2.1] Ieee1609Dot2Data-Signed, SignedCertRequest; 02 section 7): psid,
  the ABSENT header fields, SignerSingleCert / SignerSelf, the hash algorithm of the signer's
  curve, the 1609.2 digest H(H(tbs) || H(signer input)) and ECDSA under the signer's key
  (implicit keys reconstructed by ECQV-1609 under their issuer), the signer's chain to a root
  on a CTL the oracle itself verified, through the oracle's chain rules (02 section 12), and the
  signer's role (its SecurityMgmtSsp or CrlSsp);
* encrypted layers (02 section 8): one recipient of the form the SPDU type requires, its
  recipientId computed by the oracle, ECIES with the P1 of that form, AES-128-CCM, and the
  plaintext equal byte for byte to what the engine's recipient opened (pki.flow `opened`);
* the protocol's bindings: every requestHash names an SPDU of the right type in the capture;
  every AcaEeCertResponsePrivateSpdu is signed by the ACA that issued its certificate; the
  certificate's linkage value is PLV1 XOR PLV2 of the PLVs the RA forwarded for it, under its
  iCert; the RA's cocoon keys are the device's butterfly expansion (02 section 9.2), and the
  private key the device reconstructs (e*(a + f1) + r) matches the certificate's public key;
* misbehaviour reports ([SM-MBR] section 3; 03 section 8.1): encrypted to the MA certificate,
  signed under psid 0x26 by an implicit pseudonym with headerInfo {psid, generationTime}, every
  evidence BSM verifies under its signingCertificate;
* CRLs (02 section 11; 03 section 3.4): signed under psid 256 by a CrlSsp holder whose crls list
  the series, crlCraca the HashedId8 of the CRACA, nextCrl > issueDate; the devices the CRL's
  linkage seeds revoke (computed by the oracle over every pseudonym certificate it decrypted)
  are exactly the liars the engine's MA decided on, and none is honest;
* CTLs and chain files ([ASN-2.1] CertManagement; 03 section 10): every elector signature, the
  tbsCtlHash, the quorum of electors trusted by the previous CTL, the series and sequence;
* the air sample: each 1609.2 SPDU a node put on the air is signed data that verifies under a
  certificate that chains to a CTL root.

Prints one verdict line; with --json writes every disagreement and the per-kind counts.
--mutate applies each named capture fault and requires the checker to go red on it.
"""

import argparse
import collections
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ORACLE = os.path.join(HERE, '..', 'oracle')
sys.path.insert(0, ORACLE)

import asn1tools  # noqa: E402

from scms_oracle import butterfly, certs, coer, ec, ecqv, linkage, prims  # noqa: E402
from scms_oracle import chain as ochain  # noqa: E402

MBR_MODULE = os.path.join(HERE, '..', 'pdu_check', 'asn1', 'CvpdMbdWire.asn')
P256 = ec.P256
SECURITY_MGMT_PSID = 35   # [ASN-2.1] Protocol SecurityMgmtPsid
CRL_PSID = 256            # [ASN-2] Crl CrlPsid
MBR_PSID = 0x26           # [SM-MBR] section 1
BSM_PSID = 0x20


def _spec():
    parsed = asn1tools.parse_files(coer.module_files('2022') + [MBR_MODULE], encoding='cp1252')
    coer._fix_open_types(parsed)  # noqa: SLF001 - the oracle's own open-type rule
    return asn1tools.compile_dict(parsed, 'oer')


SPEC = _spec()


class Red(Exception):
    pass


def need(cond, what):
    if not cond:
        raise Red(what)


def canon(type_name, data):
    """[02 section 3] COER is canonical: decode, re-encode, compare byte for byte."""
    try:
        v = SPEC.decode(type_name, data, check_constraints=True)
    except Exception as e:  # noqa: BLE001 - any decoder error is a disagreement
        raise Red(f'{type_name} does not decode: {e}') from None
    need(SPEC.encode(type_name, v, check_constraints=True) == data, f'{type_name} not canonical COER')
    return v


def hid8(b):
    return prims.sha256(b)[-8:]


def cert_name(c):
    i = c['toBeSigned']['id']
    return i[1] if i[0] == 'name' else i[0]


# --------------------------------------------------------------------------------------------
# Trust: CTLs, the certificate store, chains


class Trust:
    """Roots the oracle verified on a CTL, and every certificate seen, by HashedId8."""

    def __init__(self):
        self.by_id8 = {}
        self.keys = {}
        self.roots = set()          # HashedId32 of approved roots, over every verified CTL
        self.ctl_log = []           # (provider, sequence, roots, electors, quorum)
        self.chains_ok = {}
        self.known = collections.Counter()

    def add(self, c):
        self.by_id8.setdefault(certs.hashed_id8(c), c)

    def add_hex(self, h):
        c = canon('Certificate', bytes.fromhex(h))
        self.add(c)
        return c

    def issuer(self, c):
        iss = c['issuer']
        if iss[0] == 'self':
            return None
        need(iss[0] == 'sha256AndDigest', f'issuer {iss[0]}')
        p = self.by_id8.get(iss[1])
        need(p is not None, f'issuer {iss[1].hex()} of {cert_name(c)} is not in the capture')
        return p

    def key(self, c):
        """(curve, Q); [02 section 6] an implicit key is e*P_U + Q_CA with e per 1609.2."""
        k = certs.hashed_id8(c)
        if k not in self.keys:
            if c['type'] == 'explicit':
                self.keys[k] = certs.verification_key(c)
            else:
                self.keys[k] = certs.verification_key(c, self.issuer(c))
        return self.keys[k]

    def chain_of(self, c):
        out, seen = [c], set()
        while out[-1]['issuer'][0] != 'self':
            k = certs.hashed_id8(out[-1])
            need(k not in seen, 'issuer loop')
            seen.add(k)
            out.append(self.issuer(out[-1]))
            need(len(out) <= 8, 'chain longer than 8')
        return out

    def trusted(self, c):
        """[02 section 12] a chain to a root on a verified CTL, every link consistent."""
        k = certs.hashed_id8(c)
        if k not in self.chains_ok:
            try:
                ch = self.chain_of(c)
                errs = validate_chain(ch, self.roots, self.known)
                self.chains_ok[k] = '; '.join(errs) if errs else ''
            except Red as e:
                self.chains_ok[k] = str(e)
        need(self.chains_ok[k] == '', f'chain of {cert_name(c)}: {self.chains_ok[k]}')


# The published production chain (SCMS Manager cert-chain_0.oer of 2025-08-13, sha256
# 'see 06-conformance-run.md section 4') carries an RA certificate whose only
# certIssuePermissions group is {subjectPermissions all, minChainLength 0, chainLengthRange 0,
# eeType app}. [ASN-2] PsidGroupPermissions: "A value of 0 for minChainLength is not permitted
# when this type appears in the certIssuePermissions field ...; a certificate that has a value
# of 0 for this field is invalid." The engine clones the deployment, so this is counted as a
# KNOWN deviation of the real artefact, not a disagreement, and only in exactly that form.
KNOWN_RA_GROUP = [(0, 0, 'all', (b'\x80', 8))]


def _group_key(g):
    return (g.get('minChainLength', 1), g.get('chainLengthRange', 0), g['subjectPermissions'][0],
            g.get('eeType', (b'\x80', 8)))


def _known_zero_min(c):
    t = c['toBeSigned']
    return (ssp_role(c) == 'ra' and [_group_key(g) for g in t.get('certIssuePermissions', [])] == KNOWN_RA_GROUP)


def _covering(issuer_t, psid):
    """The issuer certIssuePermissions group that covers `psid` (explicit first, else all)."""
    groups = issuer_t.get('certIssuePermissions', [])
    for g in groups:
        kind, body = g['subjectPermissions']
        if kind == 'explicit' and any(p['psid'] == psid for p in body):
            return g
    return next((g for g in groups if g['subjectPermissions'][0] == 'all'), None)


def chain_length_errors(sub, iss):
    """[ASN-2] PsidGroupPermissions; 02 section 12: per permission, against the issuer group
    that covers it, mcr_i <= mcr_s + 1 and mcr_i + clr_i >= mcr_s + clr_s + 1 (clr -1 is
    unbounded), with mcr_s = clr_s = 0 for appPermissions and certRequestPermissions entries."""
    st, it = sub['toBeSigned'], iss['toBeSigned']
    if not it.get('certIssuePermissions'):
        return ['issuer has no certIssuePermissions']
    items = []   # (psid or 'all', ms, cs, eeType bit)
    for p in st.get('appPermissions', []):
        items.append((p['psid'], 0, 0, 0x80))
    for g in st.get('certRequestPermissions', []):
        kind, body = g['subjectPermissions']
        for p in (body if kind == 'explicit' else [{'psid': 'all'}]):
            items.append((p['psid'], 0, 0, 0x40))
    for g in st.get('certIssuePermissions', []):
        if _known_zero_min(sub):
            continue
        kind, body = g['subjectPermissions']
        ms, cs = g.get('minChainLength', 1), g.get('chainLengthRange', 0)
        bits = g.get('eeType', (b'\x80', 8))[0][0]
        for p in (body if kind == 'explicit' else [{'psid': 'all'}]):
            for bit in (0x80, 0x40):
                if bits & bit:
                    items.append((p['psid'], ms, cs, bit))
    errs = []
    for psid, ms, cs, bit in items:
        g = (_covering(it, psid) if psid != 'all' else
             next((x for x in it['certIssuePermissions'] if x['subjectPermissions'][0] == 'all'), None))
        if g is None:
            errs.append(f'psid {psid}: no covering issuer group')
            continue
        mi, ci = g.get('minChainLength', 1), g.get('chainLengthRange', 0)
        if not (g.get('eeType', (b'\x80', 8))[0][0] & bit):
            errs.append(f'psid {psid}: issuer group eeType does not allow {"app" if bit == 0x80 else "enrol"}')
        if not mi <= ms + 1:
            errs.append(f'psid {psid}: issuer minChainLength {mi} > subordinate {ms} + 1')
        if ci != -1 and not (cs != -1 and mi + ci >= ms + cs + 1):
            errs.append(f'psid {psid}: issuer range {mi}+{ci} < subordinate {ms}+{cs}+1')
    return errs


def validate_chain(chain, roots, known):
    """The oracle's chain rules (chain.validate, 02 section 12) with the chain-length rule
    applied per covering group (chain.check_pair compares the largest groups, which wrongly
    fails the published root -> ICA pair) and the minChainLength-0 invalidity rule."""
    errs = []
    for n, c in enumerate(chain):
        for g in c['toBeSigned'].get('certIssuePermissions', []):
            if g.get('minChainLength', 1) == 0:
                if _known_zero_min(c):
                    known['RA certIssuePermissions minChainLength 0, as published'] += 1
                else:
                    errs.append(f'cert {n}: certIssuePermissions minChainLength 0 (invalid)')
        if n + 1 < len(chain):
            iss = chain[n + 1]
            if c['issuer'][0] == 'self' or c['issuer'][1] != certs.hashed_id8(iss):
                errs.append(f'cert {n}: issuer HashedId8 does not match cert {n + 1}')
                continue
            if c['type'] == 'explicit' and not certs.verify_explicit(c, iss):
                errs.append(f'cert {n}: signature does not verify')
            errs += [f'cert {n}: {e}' for e in ochain.check_pair(c, iss, n)
                     if not e.startswith('chain length') and e != 'issuer minChainLength 0']
            errs += [f'cert {n}: {e}' for e in chain_length_errors(c, iss)]
        else:
            if c['issuer'][0] != 'self':
                errs.append('chain does not end at a self-signed root')
            elif not certs.verify_explicit(c, None):
                errs.append('root self-signature does not verify')
            if prims.sha256(certs.encode_canonical(c)) not in roots:
                errs.append('root not on a verified CTL (HashedId32)')
    return errs


def verify_sig(trust, signer, tbs_bytes, sig, hash_id, self_signed_key=None):
    """[02 section 7] digest = H(H(data input) || H(signer input)); ECDSA (FIPS 186-4)."""
    if self_signed_key is None:
        C, Q = trust.key(signer)
        signer_input = certs.encode_canonical(signer)
    else:
        C, Q = self_signed_key
        signer_input = b''
    hname = certs.HASH_OF_CURVE[C.name]
    need(hash_id == ('sha256' if hname == 'SHA-256' else 'sha384'), f'hash {hash_id} for a {C.name} signer')
    digest = prims.digest_1609(hname, tbs_bytes, signer_input)
    r, s = certs.sig_rs(sig)
    need(prims.ecdsa_verify(C, Q, digest, r, s), 'signature does not verify')


def ssp_role(c, psid=SECURITY_MGMT_PSID):
    """[ASN-2.1] SecurityMgmtSsp: the CHOICE alternative names the component role."""
    for p in c['toBeSigned'].get('appPermissions', []):
        if p['psid'] == psid and 'ssp' in p:
            kind, raw = p['ssp']
            if kind == 'opaque':
                return canon('SecurityMgmtSsp', raw)[0] if psid == SECURITY_MGMT_PSID else raw
            return p['ssp']
    return None


# --------------------------------------------------------------------------------------------
# The wrappers of [ASN-2.1] Protocol


HEADER_ABSENT = ('generationTime', 'expiryTime', 'generationLocation', 'p2pcdLearningRequest',
                 'missingCrlIdentifier', 'encryptionKey')


def open_signed(trust, data, psid, header_absent=HEADER_ABSENT, want_role=None):
    """Ieee1609Dot2Data-Signed{T, psid}: returns (T's bytes, signer certificate, headerInfo)."""
    d = canon('Ieee1609Dot2Data', data)
    need(d['protocolVersion'] == 3, 'protocolVersion 3')
    need(d['content'][0] == 'signedData', f'content {d["content"][0]}, not signedData')
    sd = d['content'][1]
    hdr = sd['tbsData']['headerInfo']
    need(hdr['psid'] == psid, f'psid {hdr["psid"]:#x}, not {psid:#x}')
    for f in header_absent:
        need(f not in hdr, f'headerInfo.{f} present')
    kind, val = sd['signer']
    need(kind == 'certificate' and len(val) == 1, 'signer is not SignerSingleCert')
    signer = val[0]
    trust.add(signer)
    verify_sig(trust, signer, SPEC.encode('ToBeSignedData', sd['tbsData']), sd['signature'], sd['hashId'])
    trust.trusted(signer)
    if want_role is not None:
        need(ssp_role(signer) == want_role, f'signer role {ssp_role(signer)}, not {want_role}')
    pl = sd['tbsData']['payload']
    need('data' in pl and 'extDataHash' not in pl, 'payload is not data')
    inner = pl['data']
    need(inner['protocolVersion'] == 3, 'payload protocolVersion 3')
    need(inner['content'][0] == 'unsecuredData', 'payload is not unsecuredData')
    return inner['content'][1], signer, hdr


def scms_pdu(data, iface, alt):
    """ScmsPdu-Scoped {iface (WITH COMPONENTS {alt})}: version 2."""
    p = canon('ScmsPdu', data)
    need(p['version'] == 2, 'ScmsPdu version 2')
    need(p['content'][0] == iface, f'ScmsPdu {p["content"][0]}, not {iface}')
    need(p['content'][1][0] == alt, f'{iface} {p["content"][1][0]}, not {alt}')
    return p['content'][1][1]


def unsecured(data):
    d = canon('Ieee1609Dot2Data', data)
    need(d['protocolVersion'] == 3, 'protocolVersion 3')
    need(d['content'][0] == 'unsecuredData', f'content {d["content"][0]}, not unsecuredData')
    return d['content'][1]


def decrypt(data, recipient_form, find_key):
    """Ieee1609Dot2Data-Encrypted: one RecipientInfo of `recipient_form`; returns the plaintext.

    find_key(recipientId) -> key: the private scalar (PK forms, with P1) or the 16-byte
    symmetric key (symmRecipInfo). [02 section 8] ECIES (KDF2, MAC1) / AES-128-CCM, tag 16.
    """
    d = canon('Ieee1609Dot2Data', data)
    need(d['protocolVersion'] == 3, 'protocolVersion 3')
    need(d['content'][0] == 'encryptedData', f'content {d["content"][0]}, not encryptedData')
    ed = d['content'][1]
    need(len(ed['recipients']) == 1, f'{len(ed["recipients"])} recipients, not one')
    form, ri = ed['recipients'][0]
    need(form == recipient_form, f'recipient {form}, not {recipient_form}')
    ck, ct = ed['ciphertext']
    need(ck == 'aes128ccm', f'ciphertext {ck}')
    found = find_key(ri['recipientId'])
    need(found is not None, f'no key for recipientId {ri["recipientId"].hex()}')
    if form == 'symmRecipInfo':
        ek, enc = ri['encKey']
        need(ek == 'aes128ccm', f'symmRecipInfo encKey {ek}')
        k = prims.ccm_decrypt(found, enc['nonce'], enc['ccmCiphertext'])
        need(k is not None and len(k) == 16, 'the data key does not unwrap (CCM tag)')
    else:
        r, p1 = found
        ek, enc = ri['encKey']
        need(ek == 'eciesNistP256', f'encKey {ek}')
        V = certs.point_from(P256, enc['v'])
        k = prims.ecies_decrypt(r, V, enc['c'], enc['t'], p1)
        need(k is not None, 'ECIES tag does not verify')
    pt = prims.ccm_decrypt(k, ct['nonce'], ct['ccmCiphertext'])
    need(pt is not None, 'AES-CCM tag does not verify')
    return pt


def open_signed_cert_request(trust, data, self_key_of=None, want_signer_role=None):
    """Ieee1609Dot2Data-SignedCertRequest: returns (ScmsPdu bytes, decoded, signer or None)."""
    d = canon('Ieee1609Dot2Data', data)
    need(d['protocolVersion'] == 3, 'protocolVersion 3')
    need(d['content'][0] == 'signedCertificateRequest', f'content {d["content"][0]}')
    scr = canon('SignedCertificateRequest', d['content'][1])
    tbs = SPEC.encode('ScopedCertificateRequest', scr['tbsRequest'])
    kind, val = scr['signer']
    if kind == 'self':
        need(self_key_of is not None, 'SignerSelf where a certificate must sign')
        verify_sig(trust, None, tbs, scr['signature'], scr['hashAlgorithmId'],
                   self_signed_key=self_key_of(scr['tbsRequest']))
        return scr['tbsRequest'], None
    need(self_key_of is None, f'signer {kind} where SignerSelf is required')
    need(kind == 'certificate' and len(val) == 1, 'signer is not SignerSingleCert')
    signer = val[0]
    trust.add(signer)
    verify_sig(trust, signer, tbs, scr['signature'], scr['hashAlgorithmId'])
    trust.trusted(signer)
    if want_signer_role is not None:
        need(ssp_role(signer) == want_signer_role, f'signer role {ssp_role(signer)}')
    return scr['tbsRequest'], signer


# --------------------------------------------------------------------------------------------
# The run


class Run:
    def __init__(self, cap):
        self.cap = cap
        self.trust = Trust()
        self.keys = {k['provider']: k for k in json.load(open(os.path.join(cap, 'keys.json')))}
        self.gov = json.load(open(os.path.join(cap, 'gov.json')))
        self.truth = json.load(open(os.path.join(cap, 'truth.json')))
        self.pdus = [json.loads(line) for line in open(os.path.join(cap, 'pdus.jsonl'))]
        self.crls = json.load(open(os.path.join(cap, 'crls.json')))
        self.air = [json.loads(line) for line in open(os.path.join(cap, 'air.jsonl'))]
        self.cert_hex = json.load(open(os.path.join(cap, 'certs.json')))
        self.red = []
        self.ok = collections.Counter()
        self.notes = collections.Counter()
        # Built while checking.
        self.by_hash = {}          # HashedId8 of an SPDU as sent -> (kind, pdu)
        self.ra_aca_req = {}       # HashedId8 -> (lv, iCert, certEncKey point, B point, device, j)
        self.issued = []           # (device, i, j, certificate) of every pseudonym decrypted
        self.app_issued = []       # (device, certificate) of every application certificate
        self.enrolment_reqs = set()
        self.devices = {}          # node -> (provider, caterpillar dict)
        self.bf_index = {}         # (provider, i) -> {compressed B: (node, j), compressed J: ...}

    def disagree(self, where, what):
        self.red.append({'pdu': where, 'rule': what})

    # -- trust -------------------------------------------------------------------------------

    def load_trust(self):
        for k in self.keys.values():
            for h in k['pki'].values():
                self.trust.add_hex(h)
        for g in self.gov:
            for h in g['electors']:
                self.trust.add_hex(h)
        for h in self.cert_hex.values():
            self.trust.add_hex(h)
        for g in self.gov:
            self.check_ctls(g)
            for name in ('gccf', 'lccf'):
                self.check(f'{g["provider"]}/{name}', 'CertificateChainSpdu', lambda b, n=name: self.chain_file(b),
                           bytes.fromhex(g[name]))
        need(self.trust.roots, 'no CTL verified: nothing to trust')
        for k in self.keys.values():
            for d, v in k['devices'].items():
                self.devices[int(d)] = (k['provider'], v)

    def check_ctls(self, g):
        """[ASN-2.1] CertManagement MultiSignedCtl; 03 section 10: m-of-N electors."""
        trusted_electors, quorum, last_seq, series = None, None, None, None
        uniq = {c['sequence']: c for c in g['ctls']}
        for c in (uniq[k] for k in sorted(uniq)):
            where = f'{g["provider"]}/ctl/{c["sequence"]}'
            try:
                tbs, sigs = self.ctl(bytes.fromhex(c['hex']))
                need(tbs['sequenceNumber'] == c['sequence'], 'sequence')
                if last_seq is not None:
                    need(tbs['sequenceNumber'] == last_seq + 1, 'sequenceNumber is not the next')
                    need(tbs['ctlSeriesId'] == series, 'ctlSeriesId changed')
                electors_here = set(tbs['electorApprove'])
                # Genesis: its own electors; an update: the electors the previous CTL trusted.
                voters = electors_here if trusted_electors is None else trusted_electors
                q = tbs['quorum'] if quorum is None else quorum
                good = {h for h in sigs if h in voters}
                need(len(good) >= q, f'{len(good)} trusted elector signatures, quorum {q}')
                for h in tbs['electorRemove']:
                    need(h not in electors_here, 'an elector both approved and removed')
                for h in tbs['rootCaRemove']:
                    need(h not in tbs['rootCaApprove'], 'a root both approved and removed')
                self.trust.roots |= set(tbs['rootCaApprove'])
                self.trust.ctl_log.append((g['provider'], tbs['sequenceNumber'], len(tbs['rootCaApprove']),
                                           len(electors_here), tbs['quorum']))
                trusted_electors, quorum = electors_here, tbs['quorum']
                last_seq, series = tbs['sequenceNumber'], tbs['ctlSeriesId']
                self.ok['MultiSignedCtlSpdu'] += 1
            except Red as e:
                self.disagree(where, str(e))

    def ctl(self, data):
        body = scms_pdu(unsecured(data), 'cert', 'multiSignedCtl')
        need(body['type'] == 1, 'type fullIeeeCtl')
        tbs_bytes = body['tbsCtl']
        tbs = canon('FullIeeeTbsCtl', tbs_bytes)
        signed_by = set()
        for sig in body['signatures']:
            raw = SPEC.encode('Ieee1609Dot2Data', sig)
            inner, elector, _ = open_signed_elector(self.trust, raw)
            t = scms_pdu(inner, 'cert', 'tbsCtlSignature')
            need(t['tbsCtlHash'] == prims.sha384(tbs_bytes), 'tbsCtlHash is not SHA-384(tbsCtl)')
            need(t['ctlSeriesId'] == tbs['ctlSeriesId'] and t['sequenceNumber'] == tbs['sequenceNumber']
                 and t['ctlType'] == 1, 'the signature names another CTL')
            signed_by.add(prims.sha384(certs.encode_canonical(elector)))
        return tbs, signed_by

    def chain_file(self, data):
        body = scms_pdu(unsecured(data), 'cert', 'certificateChain')
        self.ctl(SPEC.encode('Ieee1609Dot2Data', body['homeCtl']))
        for c in body['others']:
            self.trust.add(c)
        for c in body['others']:
            if c['issuer'][0] != 'self' and c['type'] == 'explicit':
                need(certs.verify_explicit(c, self.trust.issuer(c)), f'{cert_name(c)} signature')
            self.trust.trusted(c)

    # -- dispatch ----------------------------------------------------------------------------

    def check(self, where, kind, fn, data):
        try:
            fn(data)
            self.ok[kind] += 1
        except Red as e:
            self.disagree(where, f'{kind}: {e}')
        except Exception as e:  # noqa: BLE001 - a crash on a PDU is a disagreement too
            self.disagree(where, f'{kind}: {type(e).__name__}: {e}')

    def provider_of(self, pdu):
        return pdu['id'].split('/')[0]

    def opened_matches(self, pdu, plain):
        """The engine's recipient opened the same bytes the oracle decrypted."""
        o = pdu.get('opened')
        if o is None:
            return
        need(o.get('hex') is not None, f'the engine failed to open it: {o.get("error")}')
        need(bytes.fromhex(o['hex']) == plain, 'the engine opened different plaintext')

    def run_pdus(self):
        for p in self.pdus:
            need(p.get('hex') is not None, f'{p["id"]} has no bytes')
            h = hid8(bytes.fromhex(p['hex']))
            self.by_hash[h] = (p['type'], p)
            # A relayed SPDU (the LOP, the DCM) travels several hops with the same bytes;
            # the device is the end entity at either end of one of them.
            for end in (p['from'], p['to']):
                if end.startswith('ee/'):
                    self.ends.setdefault(h, set()).add(int(end.split('/')[1]))
        handlers = {
            'EeEcaCertRequestSpdu': self.ee_eca_request,
            'EcaEeCertResponseSpdu': self.eca_ee_response,
            'EeRaCertRequestSpdu': self.ee_ra_request,
            'RaEeCertAckSpdu': self.ra_ee_ack,
            'EeRaDownloadRequestSpdu': self.download_request,
            'EeRaSuccessorEnrollmentCertRequestSpdu': self.successor_request,
            'RaEeEnrollmentCertAckSpdu': self.enrolment_ack,
            'EncryptedIndividualPLV': self.plv,
            'RaAcaCertRequestSpdu': self.ra_aca_request,
            'AcaRaCertResponseSpdu': self.aca_ra_response,
            'AcaEeCertResponsePrivateSpdu': self.private_response,
            'RaEeCertInfoSpdu': self.info_file,
            'SecuredCrl': self.secured_crl,
            'SecuredMisbehaviorReport': self.mbr,
        }
        # Requests before the responses that name them; the run order otherwise.
        order = ['EeEcaCertRequestSpdu', 'EcaEeCertResponseSpdu', 'EeRaCertRequestSpdu',
                 'EeRaSuccessorEnrollmentCertRequestSpdu', 'RaEeCertAckSpdu', 'RaEeEnrollmentCertAckSpdu',
                 'RaEeCertInfoSpdu', 'EeRaDownloadRequestSpdu', 'EncryptedIndividualPLV',
                 'RaAcaCertRequestSpdu', 'AcaRaCertResponseSpdu', 'AcaEeCertResponsePrivateSpdu',
                 'SecuredMisbehaviorReport', 'SecuredCrl']
        for kind in order + sorted(set(p['type'] for p in self.pdus) - set(order)):
            for p in self.pdus:
                if p['type'] != kind:
                    continue
                h = handlers.get(kind)
                if h is None:
                    self.disagree(p['id'], f'no rule for SPDU type {kind}')
                    continue
                self.check(p['id'], kind, lambda b, p=p, h=h: h(p, b), bytes.fromhex(p['hex']))

    # -- enrolment ([ASN-2.1] EcaEeInterface, Protocol EeEcaCertRequestSpdu) ------------------

    def ee_eca_request(self, p, b, inner=False):
        canonical = self.keys[self.provider_of(p)]['canonical'] if p else {}

        def self_key(tbs_request):
            req = tbs_request['content'][1][1]
            if 'canonicalId' in req:
                h = canonical.get(req['canonicalId'])
                need(h is not None, f'canonicalId {req["canonicalId"]} not registered')
                return P256, ec.decode_point(P256, bytes.fromhex(h))
            alg, pt = req['tbsCert']['verifyKeyIndicator'][1]
            C = certs.CURVE_OF_KEY[alg]
            return C, certs.point_from(C, pt)

        tbs_request, _ = open_signed_cert_request(self.trust, b, self_key_of=self_key)
        need(tbs_request['version'] == 2, 'ScmsPdu version 2')
        need(tbs_request['content'][0] == 'eca-ee' and tbs_request['content'][1][0] == 'eeEcaCertRequest',
             'not an eeEcaCertRequest')
        req = tbs_request['content'][1][1]
        need(req['version'] == 2, 'EeEcaCertRequest version 2')
        t = req['tbsCert']
        need(t['cracaId'] == b'\x00\x00\x00' and t['crlSeries'] == 0, 'cracaId/crlSeries not zero')
        need('appPermissions' not in t and 'certIssuePermissions' not in t and 'certRequestPermissions' in t,
             'permissions of an enrolment request')
        need(t['id'][0] != 'linkageData', 'linkageData id')
        self.enrolment_reqs.add(hid8(b))
        return req

    def eca_ee_response(self, p, b):
        inner, signer, _ = open_signed(self.trust, b, SECURITY_MGMT_PSID, want_role='eca')
        r = scms_pdu(inner, 'eca-ee', 'ecaEeCertResponse')
        need(r['version'] == 2, 'version 2')
        kind, _ = self.by_hash.get(r['requestHash'], (None, None))
        need(kind in ('EeEcaCertRequestSpdu', 'EeRaSuccessorEnrollmentCertRequestSpdu'),
             f'requestHash names {kind}')
        c = r['certificate']
        self.trust.add(c)
        need(c['issuer'] == ('sha256AndDigest', certs.hashed_id8(signer)), 'issued by another ECA')
        need(c['toBeSigned']['crlSeries'] == 4, 'enrolment crlSeries 4 ([SMP] Table 1)')
        for x in r['ecaCertChain']:
            self.trust.add(x)
        need(r['ecaCertChain'] and certs.hashed_id8(r['ecaCertChain'][0]) == certs.hashed_id8(signer),
             'ecaCertChain does not start at the ECA')
        need(r['ecaCertChain'][-1]['issuer'][0] == 'self', 'ecaCertChain does not reach the root')
        need((c['type'] == 'implicit') == ('privateKeyInfo' in r), 'privateKeyInfo iff implicit')
        self.trust.trusted(c)

    # -- the RA ([ASN-2.1] EeRaInterface) ------------------------------------------------------

    def ra_key(self, p):
        k = self.keys[self.provider_of(p)]['ra']
        ra = canon('Certificate', bytes.fromhex(k['cert']))
        rid, p1 = certs.hashed_id8(ra), prims.sha256(certs.encode_canonical(ra))
        r = int(k['enc_private'], 16)
        return lambda got: (r, p1) if got == rid else None

    def enrolment_signed(self, signer):
        need(signer is not None and ssp_role(signer) is None, 'not signed by an end-entity certificate')
        need(signer['toBeSigned']['crlSeries'] == 4, 'signer is not an enrolment certificate (crlSeries 4)')
        iss = self.trust.issuer(signer)
        need(ssp_role(iss) == 'eca', 'the enrolment certificate is not issued by an ECA')

    def ee_ra_request(self, p, b):
        plain = decrypt(b, 'certRecipInfo', self.ra_key(p))
        self.opened_matches(p, plain)
        tbs_request, signer = open_signed_cert_request(self.trust, plain)
        self.enrolment_signed(signer)
        req = tbs_request['content'][1][1]
        need(tbs_request['content'] [0] == 'ee-ra' and tbs_request['content'][1][0] == 'eeRaCertRequest',
             'not an eeRaCertRequest')
        need(req['version'] == 2, 'version 2')
        t = req['tbsCert']
        need(t['cracaId'] == b'\x00\x00\x00' and t['crlSeries'] == 0, 'cracaId/crlSeries not zero')
        need('appPermissions' in t and 'certIssuePermissions' not in t and 'certRequestPermissions' not in t,
             'permissions')
        need(t['verifyKeyIndicator'][0] == 'verificationKey', 'verifyKeyIndicator')
        ap = req.get('additionalParams')
        need(ap is not None and ap[0] in ('original', 'encryptionKey'), f'additionalParams {ap and ap[0]}')
        if ap[0] == 'original':
            # [02 section 9.1] the caterpillar: A (verifyKeyIndicator), ck, H, ek; must be the
            # device's own (the exported keys are the ones it uploaded).
            devs = self.ends.get(hid8(b), set())
            need(len(devs) == 1, f'the request travels from {len(devs)} devices')
            device = next(iter(devs))
            A = certs.point_from(P256, t['verifyKeyIndicator'][1][1])
            H = certs.point_from(P256, ap[1]['encryptionKey']['publicKey'][1])
            got = (A, bytes(ap[1]['signingExpansion'][1]), H, bytes(ap[1]['encryptionExpansion'][1]))
            need(got in self.caterpillars(device),
                 f'the caterpillar {p["from"]} uploaded is none of the keys it holds')

    def caterpillars(self, device):
        """(A, ck, H, ek) of every caterpillar the device holds: pseudonym and identification."""
        out = []
        if device in self.devices:
            c = self.devices[device][1]
            out.append((ec.base_mul(P256, int(c['a'], 16)), bytes.fromhex(c['ck']),
                        ec.base_mul(P256, int(c['p'], 16)), bytes.fromhex(c['ek'])))
        for k in self.keys.values():
            for a in k['apps']:
                if a['device'] == device and a.get('caterpillar'):
                    ca, cck, ch, cek = a['caterpillar']
                    out.append((ec.base_mul(P256, int(ca, 16)), bytes.fromhex(cck),
                                ec.base_mul(P256, int(ch, 16)), bytes.fromhex(cek)))
        return out

    def ra_ee_ack(self, p, b):
        inner, _, _ = open_signed(self.trust, b, SECURITY_MGMT_PSID, want_role='ra')
        a = scms_pdu(inner, 'ee-ra', 'raEeCertAck')
        need(a['version'] == 2, 'version 2')
        kind, req = self.by_hash.get(a['requestHash'], (None, None))
        need(kind == 'EeRaCertRequestSpdu', f'requestHash names {kind}')
        need(self.ends.get(hid8(b), set()) == self.ends.get(a['requestHash'], set()),
             'the ack goes to another device than the request came from')

    def enrolment_ack(self, p, b):
        inner, _, _ = open_signed(self.trust, b, SECURITY_MGMT_PSID, want_role='ra')
        a = scms_pdu(inner, 'ee-ra', 'raEeCertInfo')
        need('acpcTreeId' not in a, 'acpcTreeId present')
        kind, _ = self.by_hash.get(a['requestHash'], (None, None))
        need(kind == 'EeRaSuccessorEnrollmentCertRequestSpdu', f'requestHash names {kind}')

    def info_file(self, p, b):
        a = scms_pdu(unsecured(b), 'ee-ra', 'raEeCertInfo')
        need(a['version'] == 2 and 'acpcTreeId' not in a, 'RaEeCertInfoSpdu: version 2, no acpcTreeId')
        kind, _ = self.by_hash.get(a['requestHash'], (None, None))
        need(kind == 'EeRaCertRequestSpdu', f'requestHash names {kind}')

    def download_request(self, p, b):
        plain = decrypt(b, 'certRecipInfo', self.ra_key(p))
        self.opened_matches(p, plain)
        inner, signer, _ = open_signed(self.trust, plain, SECURITY_MGMT_PSID)
        self.enrolment_signed(signer)
        r = scms_pdu(inner, 'ee-ra', 'eeRaDownloadRequest')
        need(0 <= len(r['filename']) <= 255, 'filename length')

    def successor_request(self, p, b):
        plain = decrypt(b, 'certRecipInfo', self.ra_key(p))
        self.opened_matches(p, plain)
        tbs_request, signer = open_signed_cert_request(self.trust, plain)
        self.enrolment_signed(signer)
        need(tbs_request['content'][0] == 'ee-ra'
             and tbs_request['content'][1][0] == 'eeRaSuccessorEnrollmentCertRequest', 'alternative')
        inner = SPEC.encode('Ieee1609Dot2Data', tbs_request['content'][1][1])
        self.ee_eca_request(p, inner)

    # -- provisioning ([ASN-2.1] AcaRaInterface; 02 sections 9, 10) ----------------------------

    def la_key(self, p):
        """symmRecipInfo: recipientId = HashedId8 of COER SymmetricEncryptionKey (02 section 8.1)."""
        table = {}
        for e in self.keys[self.provider_of(p)]['la_aca']:
            k = bytes.fromhex(e['key'])
            rid = hid8(SPEC.encode('SymmetricEncryptionKey', ('aes128Ccm', k)))
            table[rid] = (e['la'], k)
        return table

    def open_plv(self, p, e):
        """EncryptedIndividualPLV: the PreLinkageValue, under the LA's key for the ACA."""
        need(e['version'] == 2, 'EncryptedIndividualPLV version 2')
        table = self.la_key(p)
        la_id = int.from_bytes(e['laId'], 'big')
        raw = SPEC.encode('Ieee1609Dot2Data', e['encPlv'])

        def find(rid):
            got = table.get(rid)
            need(got is None or got[0] == la_id, 'the PLV is sealed under another LA\'s key')
            return got and got[1]

        pt = decrypt(raw, 'symmRecipInfo', find)
        # [ASN-2.1] Ieee1609Dot2Data-Encrypted: Tbe "encapsulated in an Ieee1609Dot2Data of
        # type unsecured if Tbe is not already an Ieee1609Dot2Data".
        plv = unsecured(pt)
        need(len(plv) == 9, 'PreLinkageValue is not 9 bytes')
        return la_id, plv, pt

    def plv(self, p, b):
        e = canon('EncryptedIndividualPLV', b)
        _, _, pt = self.open_plv(p, e)
        o = p.get('opened')
        if o is not None:
            need(o.get('hex') is not None, f'the engine failed to open it: {o.get("error")}')
            need(bytes.fromhex(o['hex']) in (pt, unsecured(pt)), 'the engine opened different plaintext')

    def bf(self, provider, i):
        """Every device's cocoon keys for period i: B_ij = A + f1(ck,i,j)G, J_ij = H + f2(ek,i,j)G."""
        key = (provider, i)
        if key not in self.bf_index:
            idx = {}
            for node, (prov, cat) in self.devices.items():
                if prov != provider:
                    continue
                a, pp = int(cat['a'], 16), int(cat['p'], 16)
                ck, ek = bytes.fromhex(cat['ck']), bytes.fromhex(cat['ek'])
                for j in range(self.jmax):
                    b_ij = butterfly.expand_private(a, ck, butterfly.SIGN, i, j)
                    h_ij = butterfly.expand_private(pp, ek, butterfly.ENC, i, j)
                    idx[('B', ec.base_mul(P256, b_ij))] = (node, j, b_ij)
                    idx[('J', ec.base_mul(P256, h_ij))] = (node, j, h_ij)
            self.bf_index[key] = idx
        return self.bf_index[key]

    def ra_aca_request(self, p, b):
        tbs_request, signer = open_signed_cert_request(self.trust, b, want_signer_role='ra')
        need(tbs_request['content'][0] == 'aca-ra' and tbs_request['content'][1][0] == 'raAcaCertRequest',
             'not a raAcaCertRequest')
        r = tbs_request['content'][1][1]
        need(r['version'] == 2, 'version 2')
        t = r['tbsCert']
        need(t['cracaId'] == b'\x00\x00\x00' and t['crlSeries'] == 0, 'cracaId/crlSeries not zero')
        need('appPermissions' in t and 'certIssuePermissions' not in t and 'certRequestPermissions' not in t,
             'permissions')
        flags = r['flags']
        need(flags[1] == 8 and flags[0] == b'\x00', f'flags {flags}: butterflyExplicit and cubk clear')
        need('certEncKey' in r, 'certEncKey absent: not an AcaEeCertResponsePrivateSpdu request')
        if r['type'] == 'implicit':
            need(t['verifyKeyIndicator'][0] == 'reconstructionValue', 'implicit: reconstructionValue')
        enc = certs.point_from(P256, r['certEncKey']['publicKey'][1])
        B = certs.point_from(P256, t['verifyKeyIndicator'][1] if t['verifyKeyIndicator'][0] ==
                             'reconstructionValue' else t['verifyKeyIndicator'][1][1])
        rec = {'enc': enc, 'B': B, 'lv': None, 'i': None, 'device': None, 'j': None}
        if 'linkageInfo' in r:
            need(t['id'][0] == 'linkageData', 'linkageInfo without linkageData')
            i = t['id'][1]['iCert']
            plvs = {}
            for f in ('encPlv1', 'encPlv2'):
                la_id, plv, _ = self.open_plv(p, r['linkageInfo'][f])
                plvs[f] = (la_id, plv)
            need(plvs['encPlv1'][0] != plvs['encPlv2'][0], 'both PLVs from one LA')
            rec['lv'] = bytes(x ^ y for x, y in zip(plvs['encPlv1'][1], plvs['encPlv2'][1]))
            rec['i'] = i
            # [02 section 9.2] the RA expanded the device's caterpillar: B and J for one (i, j).
            idx = self.bf(self.provider_of(p), i)
            hb, hj = idx.get(('B', B)), idx.get(('J', enc))
            need(hb is not None, 'the cocoon signing key is no device\'s butterfly expansion')
            need(hj is not None and hj[:2] == hb[:2], 'the cocoon encryption key is not the same (device, i, j)')
            rec['device'], rec['j'], rec['b'], rec['h'] = hb[0], hb[1], hb[2], hj[2]
        self.ra_aca_req[hid8(b)] = rec

    def aca_ra_response(self, p, b):
        inner, signer, _ = open_signed(self.trust, b, SECURITY_MGMT_PSID, want_role='aca')
        r = scms_pdu(inner, 'aca-ra', 'acaRaCertResponse')
        need(r['version'] == 2, 'version 2')
        need(r['requestHash'] in self.ra_aca_req, 'requestHash names no RaAcaCertRequestSpdu')
        kind, body = r['acaResponse']
        need(kind == 'private', f'acaResponse {kind}')
        raw = SPEC.encode('Ieee1609Dot2Data', body)
        self.open_private(raw, self.ra_aca_req[r['requestHash']], signer, p)

    def private_response(self, p, b):
        """The copy the RA handed the device: must be an ACA's response to a request in the capture."""
        inner, signer, _ = open_signed(self.trust, b, SECURITY_MGMT_PSID, want_role='aca')
        need(any(self.private_seen.get(hid8(b), [])), 'the device got a response no ACA sent the RA')

    def open_private(self, raw, req, aca, p):
        """AcaEeCertResponsePrivateSpdu = Signed{Unsecured{Encrypted{ScmsPdu}}} ([ASN-2.1] note)."""
        inner, signer, _ = open_signed(self.trust, raw, SECURITY_MGMT_PSID, want_role='aca')
        need(certs.hashed_id8(signer) == certs.hashed_id8(aca), 'signed by another ACA than the response')
        rid = hid8(SPEC.encode('PublicEncryptionKey', {'supportedSymmAlg': 'aes128Ccm',
                                                       'publicKey': ('eciesNistP256',
                                                                     certs.point_value(P256, req['enc']))}))
        if req['device'] is not None:
            key = req['h']
        else:
            key = self.app_response_key(req)
        plain = decrypt(inner, 'rekRecipInfo', lambda got: (key, prims.sha256(b'')) if got == rid else None)
        r = scms_pdu(unsecured(plain), 'aca-ee', 'acaEeCertResponse')
        need(r['version'] == 2, 'version 2')
        c = r['certificate']
        self.trust.add(c)
        need(c['issuer'] == ('sha256AndDigest', certs.hashed_id8(signer)),
             'the response is not signed by the ACA that issued the certificate')
        self.trust.trusted(c)
        t = c['toBeSigned']
        need(c['type'] == 'implicit' and 'privateKeyInfo' in r, 'implicit, with privateKeyInfo')
        e = ecqv.e_1609(certs.tbs_bytes(c), certs.encode_canonical(signer), certs.encode_canonical(c))
        k_u = req['b'] if req['device'] is not None else req['k_u']
        d = ecqv.private_key(e, k_u, ec.os2i(r['privateKeyInfo']))
        need(ec.base_mul(P256, d) == self.trust.key(c)[1], 'the reconstructed private key does not match')
        if req['device'] is not None:
            need(t['id'][0] == 'linkageData', 'a pseudonym without linkageData')
            ld = t['id'][1]
            need(ld['iCert'] == req['i'], 'iCert is not the requested i')
            need(ld['linkage-value'] == req['lv'], 'linkage-value is not PLV1 XOR PLV2')
            need(t['crlSeries'] == 1, 'pseudonym crlSeries 1 ([SMP] Table 1)')
            self.issued.append((req['device'], req['i'], req['j'], c))
        else:
            need(t['crlSeries'] == 3, 'application certificate crlSeries 3 ([SMP] Table 1)')
            self.app_issued.append((req.get('app_device'), c))
        self.private_seen.setdefault(hid8(raw), []).append(True)

    def app_response_key(self, req):
        for k in self.keys.values():
            for a in k['apps']:
                r = int(a['response'], 16)
                if ec.base_mul(P256, r) == req['enc']:
                    k_u = int(a['k_u'], 16)
                    req['k_u'] = k_u
                    req['app_device'] = a['device']
                    return r
        raise Red('no exported application-certificate response key opens it')

    # -- misbehaviour ([SM-MBR] section 3; 03 section 8.1) -------------------------------------

    def mbr(self, p, b):
        k = self.keys[self.provider_of(p)]['ma']
        ma = canon('Certificate', bytes.fromhex(k['cert']))
        need(ssp_role(ma) == 'ma', 'the recipient is not an MA certificate')
        rid, p1 = certs.hashed_id8(ma), prims.sha256(certs.encode_canonical(ma))
        r = int(k['enc_private'], 16)
        plain = decrypt(b, 'certRecipInfo', lambda got: (r, p1) if got == rid else None)
        self.opened_matches(p, plain)
        inner, reporter, hdr = open_signed(self.trust, plain, MBR_PSID, header_absent=HEADER_ABSENT[1:])
        need(set(hdr) == {'psid', 'generationTime'}, f'headerInfo {sorted(hdr)}')
        need(reporter['type'] == 'implicit' and reporter['toBeSigned']['id'][0] == 'linkageData',
             'the reporter does not sign with a pseudonym')
        kind, top = canon('EndEntityMaInterfacePDU', inner)
        need(kind == 'eeMaApplicationSpecificMisbehaviorReport' and top['version'] == 1, 'report v1')
        need(top['psid'] == BSM_PSID, 'application psid 0x20')
        body = canon('Psid20Mbr', top['contents'])
        need(body['version'] == 1 and body['type'] in (1, 2, 3), f'type {body["type"]}')
        canon('MbrSubtypeList', body['subtype'])
        evs = ([canon('SignedBsmsWithCertificate', body['evidence'])] if body['type'] != 3 else
               list(canon('EvidenceObsImplausible', body['evidence']).values()))
        for ev in evs:
            need(len(ev['signedBsmList']) >= 1, 'empty evidence')
            sc = ev['signingCertificate']
            self.trust.add(sc)
            for bsm in ev['signedBsmList']:
                self.bsm(SPEC.encode('Ieee1609Dot2Data', bsm), must_signer=sc)
        self.reports = getattr(self, 'reports', 0) + 1

    def bsm(self, raw, must_signer=None):
        d = canon('Ieee1609Dot2Data', raw)
        need(d['content'][0] == 'signedData', 'a BSM is not signed data')
        sd = d['content'][1]
        kind, val = sd['signer']
        if kind == 'certificate':
            need(len(val) == 1, 'a BSM signed with a chain')
            signer = val[0]
            self.trust.add(signer)
        else:
            need(kind == 'digest', f'signer {kind}')
            signer = self.trust.by_id8.get(val)
            need(signer is not None, f'the digest {val.hex()} names no certificate in the capture')
        if must_signer is not None:
            need(certs.hashed_id8(signer) == certs.hashed_id8(must_signer), 'evidence signed by another certificate')
        verify_sig(self.trust, signer, SPEC.encode('ToBeSignedData', sd['tbsData']), sd['signature'], sd['hashId'])
        self.trust.trusted(signer)
        hdr = sd['tbsData']['headerInfo']
        psid = hdr['psid']
        need(any(a['psid'] == psid for a in signer['toBeSigned'].get('appPermissions', [])),
             f'psid {psid:#x} is not in the signer\'s appPermissions')
        # [02 section 12] the SPDU is consistent with its signing certificate in time: the
        # generationTime (Time64, microseconds) lies inside the validity period (Time32 s).
        if 'generationTime' in hdr:
            v0, v1 = ochain.validity(signer['toBeSigned'])
            g = hdr['generationTime'] / 1e6
            need(v0 <= g < v1, f'generationTime {g:.0f} outside the signer validity [{v0}, {v1})')
        return signer

    # -- CRLs ([ASN-2] Crl; 02 section 11) ------------------------------------------------------

    def secured_crl(self, p, b):
        inner, signer, _ = open_signed(self.trust, b, CRL_PSID)
        self.crl_contents(inner, signer)

    def crl_contents(self, inner, signer):
        c = canon('CrlContents', inner)
        need(c['version'] == 1, 'CrlContents version 1')
        need(c['nextCrl'] > c['issueDate'], 'nextCrl is not after issueDate')
        raw = ssp_role(signer, CRL_PSID)
        need(raw is not None, 'the signer holds no psid 256 SSP')
        ssp = canon('CrlSsp', raw)
        need(c['crlSeries'] in ssp['crls'], f'series {c["crlSeries"]} not in the signer\'s CrlSsp')
        craca = signer if ssp['associatedCraca'] == 'isCraca' else self.trust.issuer(signer)
        need(c['crlCraca'] == certs.hashed_id8(craca), 'crlCraca is not the CRACA\'s HashedId8')
        self.crl_seen = getattr(self, 'crl_seen', [])
        self.crl_seen.append(c)
        return c

    def check_crl_heads(self):
        for h in self.crls:
            self.check(f'crl/{h["id8"]}', 'SecuredCrl(head)',
                       lambda b: self.crl_contents(*open_signed(self.trust, b, CRL_PSID)[:2]),
                       bytes.fromhex(h['hex']))

    def enforcement(self):
        """Which devices the published linkage CRLs revoke, over every pseudonym decrypted."""
        revoked = set()
        for c in getattr(self, 'crl_seen', []):
            if c['typeSpecific'][0] not in ('fullLinkedCrl', 'deltaLinkedCrl'):
                continue
            for device, _i, _j, cert in self.issued:
                if device not in revoked and ochain.linkage_crl_revokes(c, cert):
                    revoked.add(device)
        liars = set(self.truth['liars'])
        decided = set(self.truth['revoked'])
        try:
            need(revoked, 'no published CRL revokes any certificate')
            need(not (revoked - liars), f'honest devices revoked: {sorted(revoked - liars)}')
            need(revoked == decided, f'the CRLs revoke {sorted(revoked)}, the MA decided {sorted(decided)}')
            provs = {self.devices[d][0] for d in revoked if d in self.devices}
            self.ok['enforcement'] += 1
            self.revoked_devices = sorted(revoked)
            self.revoked_providers = sorted(provs)
        except Red as e:
            self.disagree('enforcement', str(e))

    # -- the air -------------------------------------------------------------------------------

    def run_air(self):
        for f in self.air:
            self.check(f'air/{f["node"]}/{f["msg"]}', 'air', lambda b: self.bsm(b), bytes.fromhex(f['hex']))

    def completeness(self):
        """The capture holds what the engine says each run emitted ([ASN-2.1] SPDU names)."""
        try:
            by = collections.Counter((p['id'].split('/')[0], p['type']) for p in self.pdus)
            for r in self.truth['runs']:
                for kind, n in r['by_kind'].items():
                    need(by[(r['provider'], kind)] == n,
                         f'{r["provider"]}: {kind} emitted {n}, captured {by[(r["provider"], kind)]}')
            self.ok['completeness'] += 1
        except Red as e:
            self.disagree('completeness', str(e))

    def main(self):
        self.jmax = 20
        self.private_seen = {}
        self.ends = {}
        self.load_trust()
        self.completeness()
        self.run_pdus()
        self.check_crl_heads()
        self.enforcement()
        self.run_air()
        return self


def open_signed_elector(trust, raw):
    """CtlSignatureSpdu: Signed{tbsCtlSignature, 35} by a self-signed elector certificate."""
    d = canon('Ieee1609Dot2Data', raw)
    need(d['content'][0] == 'signedData', 'a CTL signature is not signed data')
    sd = d['content'][1]
    hdr = sd['tbsData']['headerInfo']
    need(hdr['psid'] == SECURITY_MGMT_PSID, 'CTL signature psid')
    for f in HEADER_ABSENT:
        need(f not in hdr, f'headerInfo.{f} present')
    kind, val = sd['signer']
    need(kind == 'certificate' and len(val) == 1, 'signer is not SignerSingleCert')
    elector = val[0]
    need(elector['issuer'][0] == 'self' and certs.verify_explicit(elector, None), 'elector self-signature')
    need(ssp_role(elector) == 'elector', f'signer role {ssp_role(elector)}, not elector')
    verify_sig(trust, elector, SPEC.encode('ToBeSignedData', sd['tbsData']), sd['signature'], sd['hashId'])
    pl = sd['tbsData']['payload']['data']
    need(pl['content'][0] == 'unsecuredData', 'payload is not unsecuredData')
    return pl['content'][1], elector, hdr


# --------------------------------------------------------------------------------------------
# Capture faults: each must turn the checker red (proves the rules can fail)


def _flip(h, at):
    b = bytearray(bytes.fromhex(h))
    b[at % len(b)] ^= 0x01
    return b.hex()


def _first(run, kind):
    return next(p for p in run.pdus if p['type'] == kind)


MUTATIONS = {
    'flip-last-byte-of-an-eca-response': lambda r: _first(r, 'EcaEeCertResponseSpdu').update(
        hex=_flip(_first(r, 'EcaEeCertResponseSpdu')['hex'], -1)),
    'flip-a-ciphertext-byte-of-a-ra-request': lambda r: _first(r, 'EeRaCertRequestSpdu').update(
        hex=_flip(_first(r, 'EeRaCertRequestSpdu')['hex'], -5)),
    'wrong-ra-private-key': lambda r: next(iter(r.keys.values()))['ra'].update(
        enc_private='%064x' % (int(next(iter(r.keys.values()))['ra']['enc_private'], 16) ^ 1)),
    'wrong-la-aca-key': lambda r: [e.update(key='00' * 16) for k in r.keys.values() for e in k['la_aca']],
    'wrong-device-caterpillar': lambda r: [v.update(ek='11' * 16) for k in r.keys.values()
                                           for v in k['devices'].values()],
    'engine-opened-other-plaintext': lambda r: next(p for p in r.pdus if (p.get('opened') or {}).get('hex'))
    ['opened'].update(hex='00'),
    'drop-a-captured-pdu': lambda r: r.pdus.remove(_first(r, 'RaEeCertAckSpdu')),
    'flip-a-crl-signature': lambda r: _first(r, 'SecuredCrl').update(hex=_flip(_first(r, 'SecuredCrl')['hex'], -2)),
    'flip-a-ctl-signature': lambda r: r.gov[0]['ctls'][-1].update(hex=_flip(r.gov[0]['ctls'][-1]['hex'], -3)),
    'an-honest-device-called-a-liar': lambda r: r.truth.update(revoked=r.truth['revoked'] + [-1]),
    'flip-an-air-frame-signature': lambda r: r.air[0].update(hex=_flip(r.air[0]['hex'], -1)),
    'flip-an-mbr-ciphertext': lambda r: _first(r, 'SecuredMisbehaviorReport').update(
        hex=_flip(_first(r, 'SecuredMisbehaviorReport')['hex'], -7)),
}


def run_mutations(cap):
    rows = []
    for name, f in MUTATIONS.items():
        r = Run(cap)
        try:
            f(r)
        except StopIteration:
            rows.append((name, None, 'nothing to mutate in this capture'))
            continue
        r.main()
        rows.append((name, len(r.red), r.red[0]['rule'] if r.red else ''))
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('capture')
    ap.add_argument('--json')
    ap.add_argument('--mutate', action='store_true')
    a = ap.parse_args()
    if a.mutate:
        rows = run_mutations(a.capture)
        bad = [r for r in rows if not r[1]]
        for name, n, first in rows:
            print(f'  {"RED " if n else "MISS"} {name}: {n} disagreements; first: {first[:110]}')
        print(f'mutate: {"PASS" if not bad else "FAIL"} ({len(rows) - len(bad)}/{len(rows)} faults red)')
        return 0 if not bad else 1
    r = Run(a.capture).main()
    total = sum(r.ok.values()) + len(r.red)
    if a.json:
        json.dump({'ok': r.ok, 'disagreements': r.red, 'ctls': r.trust.ctl_log,
                   'revoked': getattr(r, 'revoked_devices', []),
                   'revoked_providers': getattr(r, 'revoked_providers', []),
                   'pseudonyms': len(r.issued), 'app_certs': len(r.app_issued),
                   'known': r.trust.known},
                  open(a.json, 'w'), indent=1, default=str)
    for d in r.red[:12]:
        print(f'  RED {d["pdu"]}: {d["rule"][:160]}')
    kinds = ', '.join(f'{k} {v}' for k, v in sorted(r.ok.items()))
    print(f'conformance: {"PASS" if not r.red else "FAIL"} ({total - len(r.red)}/{total} checks; '
          f'{len(r.red)} disagreements; {kinds}; pseudonyms {len(r.issued)}, app certs {len(r.app_issued)}, '
          f'revoked {getattr(r, "revoked_devices", [])}; known deviations {dict(r.trust.known)})')
    return 0 if not r.red else 1


if __name__ == '__main__':
    sys.exit(main())
