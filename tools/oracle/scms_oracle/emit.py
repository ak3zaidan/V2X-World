"""Emit oracle-generated vectors for the Rust tests (deterministic: fixed seeds, no clock, no network).

Files (JSON, hex strings lowercase) under crates/v2xw-sec/tests/vectors/oracle/, plus MANIFEST.json with
the sha256 of each.  ``verify-emitted`` regenerates in memory and compares byte for byte, so a file
edited by hand, or an oracle change that alters an output, is caught.

These vectors are dual-implementation checks, not official vectors: they are only as right as the
oracle, which is itself checked against the official corpus (selftest) and the published SCMS
artefacts (real).
"""

import datetime as dt
import hashlib
import json
import os
import random

from . import butterfly, certs, coer, ec, ecqv, linkage, prims, time1609

C = ec.P256
VERSION = 1


def hx(b):
    return b.hex()


def pt(P, C=C):
    return {'x': '%0*x' % (2 * C.size, P[0]), 'y': '%0*x' % (2 * C.size, P[1]), 'compressed': ec.compress(C, P).hex()}


def scalar(rnd, C=C):
    return rnd.randrange(1, C.n)


# ----------------------------------------------------------------------------------------------
# certificate builders (1609.2-2022 values)

def key_value(C, P):
    alg = 'ecdsaNistP256' if C is ec.P256 else 'ecdsaNistP384'
    return ('verificationKey', (alg, certs.point_value(C, P)))


def tbs(name_or_linkage, Pkey, start, hours, crl_series=4, craca=b'\0\0\0', app=(32,), issue=None, C=C, implicit=False):
    t = {'id': name_or_linkage, 'cracaId': craca, 'crlSeries': crl_series,
         'validityPeriod': {'start': start, 'duration': hours if isinstance(hours, tuple) else ('hours', hours)}}
    if app:
        t['appPermissions'] = [{'psid': p} for p in app]
    if issue:
        t['certIssuePermissions'] = issue
    t['verifyKeyIndicator'] = ('reconstructionValue', certs.point_value(ec.P256, Pkey)) if implicit else key_value(C, Pkey)
    return t


def sign_explicit(tbs_value, issuer_cert, d_issuer, k, C_issuer, self_hash='sha256'):
    cert = {'version': 3, 'type': 'explicit',
            'issuer': ('self', self_hash) if issuer_cert is None else
            (('sha384AndDigest' if certs.cert_hash_alg(issuer_cert) == 'SHA-384' else 'sha256AndDigest'), certs.hashed_id8(issuer_cert)),
            'toBeSigned': tbs_value}
    hname = certs.HASH_OF_CURVE[C_issuer.name]
    tb = coer.encode('ToBeSignedCertificate', tbs_value)
    signer_input = b'' if issuer_cert is None else certs.encode_canonical(issuer_cert)
    digest = prims.digest_1609(hname, tb, signer_input)
    r, s, _ = prims.ecdsa_sign(C_issuer, d_issuer, digest, k)
    alg = 'ecdsaNistP256Signature' if C_issuer is ec.P256 else 'ecdsaNistP384Signature'
    cert['signature'] = (alg, {'rSig': ('x-only', ec.i2os(r, C_issuer.size)), 'sSig': ec.i2os(s, C_issuer.size)})
    return cert, digest


ALL_ISSUE = [{'subjectPermissions': ('all', None), 'minChainLength': 1, 'chainLengthRange': 0,
              'eeType': (b'\x80', 8)}]
START = 679737605  # 2025-07-16 08:00:00 UTC in Time32 (see time1609.KNOWN)


def make_root(rnd):
    d = scalar(rnd)
    t = tbs(('name', 'rootca.oracle.example'), ec.base_mul(C, d), START, ('years', 20), app=(35,),
            issue=[{'subjectPermissions': ('all', None), 'minChainLength': 2, 'chainLengthRange': -1, 'eeType': (b'\xc0', 8)}])
    cert, _ = sign_explicit(t, None, d, scalar(rnd), C)
    return cert, d


def make_aca(rnd, root, d_root):
    d = scalar(rnd)
    t = tbs(('name', 'aca.oracle.example'), ec.base_mul(C, d), START, ('years', 5), app=(35,), issue=ALL_ISSUE,
            craca=certs.hashed_id3(root))
    cert, _ = sign_explicit(t, root, d_root, scalar(rnd), C)
    return cert, d


# ----------------------------------------------------------------------------------------------

def gen_time():
    rnd = random.Random(101)
    vec = [{'utc': t.isoformat(), 'time32': v, 'source': 'derived known instant'} for t, v in time1609.KNOWN]
    for _ in range(40):
        t = dt.datetime(2004, 1, 1, tzinfo=dt.timezone.utc) + dt.timedelta(seconds=rnd.randrange(0, 23 * 365 * 86400),
                                                                          microseconds=rnd.randrange(0, 10 ** 6))
        vec.append({'utc': t.isoformat(), 'time32': time1609.time32(t), 'time64': time1609.time64(t), 'source': 'oracle'})
    return {'kind': 'time32-time64-tai', 'leap_table': 'IERS Bulletin C 72 (valid to 2027-06-28)', 'vectors': vec}


def gen_hashedid():
    rnd = random.Random(102)
    vec = []
    for n in range(20):
        data = rnd.randbytes(rnd.randrange(0, 300))
        vec.append({'data': hx(data), 'sha256': hx(prims.sha256(data)), 'sha384': hx(prims.sha384(data)),
                    'hashed_id3': hx(prims.hashed_id(data, 3)), 'hashed_id8': hx(prims.hashed_id(data, 8)),
                    'hashed_id10': hx(prims.hashed_id(data, 10)), 'hashed_id48_sha384': hx(prims.hashed_id(data, 48, 'SHA-384'))})
    return {'kind': 'hashedid', 'vectors': vec}


def gen_ccm():
    rnd = random.Random(103)
    vec = []
    for n in range(30):
        k, nonce = rnd.randbytes(16), rnd.randbytes(12)
        p = rnd.randbytes(rnd.choice([0, 1, 15, 16, 17, 31, 32, 33, 100, 255, 1000]))
        vec.append({'key': hx(k), 'nonce': hx(nonce), 'plaintext': hx(p), 'ciphertext_tag': hx(prims.ccm_encrypt(k, nonce, p))})
    return {'kind': 'aes-128-ccm-1609', 'parameters': 'nonce 12, tag 16, no AAD', 'vectors': vec}


def gen_ecies(root, aca):
    """ECIES with each P1 rule: certRecipInfo (P1 = SHA-256 of the recipient cert), signedDataRecipInfo
    (SHA-256 of an Ieee1609Dot2Data), rekRecipInfo (SHA-256(''))."""
    rnd = random.Random(104)
    cert_bytes = certs.encode_canonical(aca)
    sdata = coer.encode('Ieee1609Dot2Data', {'protocolVersion': 3, 'content': ('unsecuredData', b'oracle signed data stand-in')})
    vec = []
    for kind, enc in (('certRecipInfo', cert_bytes), ('signedDataRecipInfo', sdata), ('rekRecipInfo', b'')):
        for _ in range(4):
            r, v, k = scalar(rnd), scalar(rnd), rnd.randbytes(16)
            R = ec.base_mul(C, r)
            p1 = prims.ecies_p1(kind, enc)
            V, c, t = prims.ecies_encrypt(R, k, p1, v)
            vec.append({'recipient_kind': kind, 'p1_input': hx(enc), 'p1': hx(p1), 'r': '%064x' % r, 'R': pt(R), 'v': '%064x' % v,
                        'k': hx(k), 'V': pt(V), 'c': hx(c), 't': hx(t), 'recipient_id_hashed_id8': hx(prims.hashed_id(enc, 8)) if enc else None})
    return {'kind': 'ecies-p256-1609', 'vectors': vec}


def gen_ecdsa(root):
    rnd = random.Random(105)
    vec = []
    signer_cert = certs.encode_canonical(root)
    for Cv in (ec.P256, ec.P384):
        hname = certs.HASH_OF_CURVE[Cv.name]
        for n in range(6):
            d, k = scalar(rnd, Cv), scalar(rnd, Cv)
            tbs_data = rnd.randbytes(rnd.randrange(10, 200))
            signer = b'' if n % 3 == 0 else signer_cert
            digest = prims.digest_1609(hname, tbs_data, signer)
            r, s, R = prims.ecdsa_sign(Cv, d, digest, k)
            vec.append({'curve': Cv.name, 'hash': hname, 'd': '%0*x' % (2 * Cv.size, d), 'Q': pt(ec.base_mul(Cv, d), Cv),
                        'k': '%0*x' % (2 * Cv.size, k), 'tbs_data': hx(tbs_data), 'signer_input': hx(signer),
                        'digest': hx(digest), 'r': '%0*x' % (2 * Cv.size, r), 's': '%0*x' % (2 * Cv.size, s), 'R': pt(R, Cv)})
    return {'kind': 'ecdsa-1609-digest', 'rule': 'digest = H(H(tbsData) || H(signer cert COER or empty)); r = x(kG) mod n', 'vectors': vec}


def gen_certs_ecqv(root, d_root, aca, d_aca, count=12):
    """Implicit pseudonym certificates issued by an explicit ACA (ECQV-1609), with all intermediate values."""
    rnd = random.Random(106)
    aca_bytes = certs.encode_canonical(aca)
    vec = []
    for n in range(count):
        ku, k = scalar(rnd), scalar(rnd)
        RU = ec.base_mul(C, ku)
        lvv = rnd.randbytes(9)
        box = {}

        def e_of(PU):
            t = tbs(('linkageData', {'iCert': n, 'linkage-value': lvv}), PU, START + 3600 * n, 168, implicit=True,
                    craca=certs.hashed_id3(root))
            box['tbs'] = t
            cert = {'version': 3, 'type': 'implicit', 'issuer': ('sha256AndDigest', certs.hashed_id8(aca)), 'toBeSigned': t}
            box['cert'] = cert
            return ecqv.e_1609(coer.encode('ToBeSignedCertificate', t), aca_bytes, certs.encode(cert))

        PU, r, e = ecqv.issue(RU, d_aca, k, e_of)
        du = ecqv.private_key(e, ku, r)
        QU = ecqv.public_key(e, PU, ec.base_mul(C, d_aca))
        assert ec.base_mul(C, du) == QU
        assert certs.verification_key(box['cert'], aca)[1] == QU
        cb = certs.encode(box['cert'])
        vec.append({'tbs_coer': hx(coer.encode('ToBeSignedCertificate', box['tbs'])), 'cert_coer': hx(cb), 'issuer_coer': hx(aca_bytes),
                    'k_u': '%064x' % ku, 'R_U': pt(RU), 'k': '%064x' % k, 'P_U': pt(PU), 'e': '%064x' % e,
                    'r': '%064x' % r, 'd_u': '%064x' % du, 'Q_U': pt(QU), 'cert_hashed_id8': hx(certs.hashed_id8(box['cert']))})
    return {'kind': 'ecqv-1609-implicit-certificates',
            'rule': 'e = SHA-256(SHA-256(COER tbs) || SHA-256(COER issuer cert)) full 256 bits; P_U = R_U + kG; r = ek + d_CA; '
                    'd_U = e k_U + r; Q_U = e P_U + Q_CA',
            'root_coer': hx(certs.encode_canonical(root)), 'aca_coer': hx(aca_bytes),
            'aca_public': pt(ec.base_mul(C, d_aca)), 'vectors': vec}


def gen_chain(root, d_root, aca, d_aca):
    """Explicit chain root -> ACA -> explicit EE, with COER bytes, HashedIds and the signed digests."""
    rnd = random.Random(107)
    vec = []
    for n in range(4):
        d = scalar(rnd)
        t = tbs(('name', f'ee{n}.oracle.example'), ec.base_mul(C, d), START + 86400, 168, app=(32, 35), craca=certs.hashed_id3(root))
        cert, digest = sign_explicit(t, aca, d_aca, scalar(rnd), C)
        assert certs.verify_explicit(cert, aca)
        vec.append({'cert_coer': hx(certs.encode(cert)), 'tbs_coer': hx(coer.encode('ToBeSignedCertificate', t)), 'signed_digest': hx(digest),
                    'hashed_id8': hx(certs.hashed_id8(cert)), 'hashed_id10': hx(certs.hashed_id10(cert)), 'd': '%064x' % d})
    return {'kind': 'explicit-chain', 'root_coer': hx(certs.encode(root)), 'root_hashed_id8': hx(certs.hashed_id8(root)),
            'root_hashed_id32': hx(prims.sha256(certs.encode_canonical(root))),
            'aca_coer': hx(certs.encode(aca)), 'aca_hashed_id8': hx(certs.hashed_id8(aca)), 'vectors': vec}


def gen_butterfly(aca, d_aca):
    rnd = random.Random(108)
    vec = []
    aca_bytes = certs.encode_canonical(aca)
    for n in range(8):
        a, h = scalar(rnd), scalar(rnd)
        ck, ek = rnd.randbytes(16), rnd.randbytes(16)
        i, j = rnd.randrange(0, 1 << 16), rnd.randrange(0, 40)
        A, Hp = ec.base_mul(C, a), ec.base_mul(C, h)
        b = butterfly.expand_private(a, ck, butterfly.SIGN, i, j)
        B = butterfly.expand_public(A, ck, butterfly.SIGN, i, j)
        hh = butterfly.expand_private(h, ek, butterfly.ENC, i, j)
        J = butterfly.expand_public(Hp, ek, butterfly.ENC, i, j)
        c = scalar(rnd)
        U = butterfly.explicit_cert_key(C, B, c)
        k = scalar(rnd)
        lvv = rnd.randbytes(9)
        box = {}

        def e_of(PU):
            t = tbs(('linkageData', {'iCert': i, 'linkage-value': lvv}), PU, START, 168, implicit=True)
            box['cert'] = {'version': 3, 'type': 'implicit', 'issuer': ('sha256AndDigest', certs.hashed_id8(aca)), 'toBeSigned': t}
            return ecqv.e_1609(coer.encode('ToBeSignedCertificate', t), aca_bytes)

        PU, r, e = ecqv.issue(B, d_aca, k, e_of)
        d_impl = ecqv.private_key(e, b, r)
        assert ec.base_mul(C, d_impl) == certs.verification_key(box['cert'], aca)[1]
        vec.append({'i': i, 'j': j, 'a': '%064x' % a, 'A': pt(A), 'ck': hx(ck), 'h': '%064x' % h, 'H': pt(Hp), 'ek': hx(ek),
                    'x_sign': hx(butterfly.x_input(butterfly.SIGN, i, j)), 'x_enc': hx(butterfly.x_input(butterfly.ENC, i, j)),
                    'f_sign': '%064x' % butterfly.f(ck, butterfly.SIGN, i, j), 'f_enc': '%064x' % butterfly.f(ek, butterfly.ENC, i, j),
                    'b': '%064x' % b, 'B': pt(B), 'h_ij': '%064x' % hh, 'J': pt(J),
                    'explicit': {'c': '%064x' % c, 'U': pt(U), 'u': '%064x' % butterfly.explicit_private(C, b, c)},
                    'implicit': {'k': '%064x' % k, 'cert_coer': hx(certs.encode(box['cert'])), 'P_U': pt(PU), 'e': '%064x' % e,
                                 'r': '%064x' % r, 'd': '%064x' % d_impl}})
    return {'kind': 'butterfly-expansion-and-issuance', 'aca_coer': hx(aca_bytes), 'aca_public': pt(ec.base_mul(C, d_aca)),
            'vectors': vec}


def gen_linkage():
    rnd = random.Random(109)
    vec = []
    for n in range(10):
        la1, la2 = rnd.randrange(0, 1 << 16), rnd.randrange(0, 1 << 16)
        s1, s2 = rnd.randbytes(16), rnd.randbytes(16)
        i = rnd.randrange(0, 6)
        js = [rnd.randrange(0, 40) for _ in range(3)]
        vec.append({'la_id1': la1, 'la_id2': la2, 'ls1_0': hx(s1), 'ls2_0': hx(s2), 'i': i,
                    'ls1_i': hx(linkage.seed_at(la1, s1, i)), 'ls2_i': hx(linkage.seed_at(la2, s2, i)),
                    'lv': {str(j): hx(linkage.lv(la1, s1, la2, s2, i, j)) for j in js}})
    return {'kind': 'linkage-values', 'vectors': vec}


def generate():
    rnd = random.Random(100)
    root, d_root = make_root(rnd)
    aca, d_aca = make_aca(rnd, root, d_root)
    return {
        'time.json': gen_time(),
        'hashedid.json': gen_hashedid(),
        'ccm.json': gen_ccm(),
        'ecies.json': gen_ecies(root, aca),
        'ecdsa_1609.json': gen_ecdsa(root),
        'chain_explicit.json': gen_chain(root, d_root, aca, d_aca),
        'ecqv_1609.json': gen_certs_ecqv(root, d_root, aca, d_aca),
        'butterfly.json': gen_butterfly(aca, d_aca),
        'linkage.json': gen_linkage(),
    }


def render(doc):
    doc = dict({'generator': f'tools/oracle (scms_oracle.emit v{VERSION})', 'edition': '1609.2-2022'}, **doc)
    return (json.dumps(doc, indent=1) + '\n').encode()


def main(out, check_only=False):
    files = {name: render(doc) for name, doc in generate().items()}
    manifest = {'generator': f'tools/oracle (scms_oracle.emit v{VERSION})',
                'files': {n: hashlib.sha256(b).hexdigest() for n, b in sorted(files.items())}}
    files['MANIFEST.json'] = (json.dumps(manifest, indent=1) + '\n').encode()
    if check_only:
        bad = []
        for n, b in files.items():
            p = os.path.join(out, n)
            if not os.path.exists(p) or open(p, 'rb').read() != b:
                bad.append(n)
        print(f'verify-emitted: {"PASS" if not bad else "FAIL"} ({len(files) - len(bad)}/{len(files)} files identical'
              f'{"; differ: " + ", ".join(bad) if bad else ""})')
        return 0 if not bad else 1
    os.makedirs(out, exist_ok=True)
    for n, b in files.items():
        with open(os.path.join(out, n), 'wb') as f:
            f.write(b)
    print(f'emit: wrote {len(files)} files to {os.path.relpath(out)}')
    return 0
