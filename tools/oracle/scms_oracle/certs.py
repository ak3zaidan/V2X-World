"""1609.2 certificates on asn1tools values: canonical encoding, hashes, explicit signatures, ECQV keys.

Values are the Python form asn1tools uses for the 1609.2 ASN.1: SEQUENCE -> dict, CHOICE -> (name, value),
OCTET STRING -> bytes, BIT STRING -> (bytes, nbits).

Rules (02 sections 3, 4, 5, 6, 7):
* canonical form before hashing: every EccP256/384CurvePoint compressed, EcdsaP256/384Signature.rSig
  x-only;
* the whole-certificate hash (for HashedId8 / HashedId3 / HashedId10) is SHA-256 of the canonical COER
  certificate, or SHA-384 when the certificate's issuer field is sha384AndDigest (self-signed: the
  `self` HashAlgorithm);
* explicit signature: ECDSA over H( H(COER tbs) || H(COER canonical issuer cert, or '' if self) ) with
  H the issuer-key hash (SHA-256 for P-256, SHA-384 for P-384);
* implicit: e = SHA-256( SHA-256(COER tbs) || SHA-256(COER canonical issuer cert) ), Q = e*P_U + Q_issuer.
"""

import copy

from . import coer, ec, ecqv, prims

CURVE_OF_KEY = {'ecdsaNistP256': ec.P256, 'ecdsaBrainpoolP256r1': None, 'ecdsaNistP384': ec.P384,
                'ecdsaBrainpoolP384r1': None, 'eciesNistP256': ec.P256}
HASH_OF_CURVE = {'P-256': 'SHA-256', 'P-384': 'SHA-384'}


def point_from(C, v):
    kind, data = v
    if kind == 'x-only':
        raise ValueError('x-only point has no y')
    if kind in ('compressed-y-0', 'compressed-y-1'):
        return ec.decompress(C, 0 if kind.endswith('0') else 1, ec.os2i(data))
    if kind == 'fill':
        raise ValueError('fill')
    if kind == 'uncompressed-P256' or kind.startswith('uncompressed'):
        return (ec.os2i(data['x']), ec.os2i(data['y']))
    raise ValueError(kind)


def point_value(C, P, compressed=True):
    if compressed:
        return ('compressed-y-%d' % (P[1] & 1), ec.i2os(P[0], C.size))
    return ('uncompressed-P%d' % (C.size * 8), {'x': ec.i2os(P[0], C.size), 'y': ec.i2os(P[1], C.size)})


def _canon_point(C, v):
    kind, data = v
    if kind.startswith('uncompressed'):
        return point_value(C, (ec.os2i(data['x']), ec.os2i(data['y'])))
    return v


def _canon_sig(sig):
    alg, s = sig
    C = ec.P384 if 'P384' in alg or 'p384' in alg else ec.P256
    r = s['rSig']
    if r[0] != 'x-only':
        if r[0].startswith('uncompressed'):
            x = r[1]['x']
        elif r[0] in ('compressed-y-0', 'compressed-y-1'):
            x = r[1]
        else:
            raise ValueError('rSig ' + r[0])
        s = dict(s, rSig=('x-only', x))
    return (alg, s)


def canonical(cert):
    """Canonical copy of a Certificate value (02 section 3)."""
    c = copy.deepcopy(cert)
    t = c['toBeSigned']
    vki = t['verifyKeyIndicator']
    if vki[0] == 'verificationKey':
        alg, pt = vki[1]
        C = CURVE_OF_KEY.get(alg)
        if C is not None:
            t['verifyKeyIndicator'] = ('verificationKey', (alg, _canon_point(C, pt)))
    else:
        t['verifyKeyIndicator'] = ('reconstructionValue', _canon_point(ec.P256, vki[1]))
    if 'encryptionKey' in t:
        ek = t['encryptionKey']
        alg, pt = ek['publicKey']
        C = CURVE_OF_KEY.get(alg)
        if C is not None:
            t['encryptionKey'] = dict(ek, publicKey=(alg, _canon_point(C, pt)))
    if 'signature' in c:
        c['signature'] = _canon_sig(c['signature'])
    return c


def encode(cert, edition='2022'):
    return coer.encode('Certificate', cert, edition)


def encode_canonical(cert, edition='2022'):
    return encode(canonical(cert), edition)


def tbs_bytes(cert, edition='2022'):
    return coer.encode('ToBeSignedCertificate', canonical(cert)['toBeSigned'], edition)


def cert_hash_alg(cert):
    iss = cert['issuer']
    if iss[0] == 'sha384AndDigest' or (iss[0] == 'self' and iss[1] == 'sha384'):
        return 'SHA-384'
    return 'SHA-256'


def whole_hash(cert, edition='2022'):
    return prims.HASHES[cert_hash_alg(cert)](encode_canonical(cert, edition))


def hashed_id8(cert, edition='2022'):
    return whole_hash(cert, edition)[-8:]


def hashed_id3(cert, edition='2022'):
    return whole_hash(cert, edition)[-3:]


def hashed_id10(cert, edition='2022'):
    return whole_hash(cert, edition)[-10:]


def verification_key(cert, issuer=None, edition='2022'):
    """(curve, Q) of the certificate's key; implicit certificates need their issuer (ECQV-1609)."""
    vki = cert['toBeSigned']['verifyKeyIndicator']
    if vki[0] == 'verificationKey':
        alg, pt = vki[1]
        C = CURVE_OF_KEY[alg]
        return C, point_from(C, pt)
    if issuer is None:
        raise ValueError('implicit certificate needs its issuer')
    Ci, Qi = verification_key(issuer, None, edition)
    e = ecqv.e_1609(tbs_bytes(cert, edition), encode_canonical(issuer, edition), encode_canonical(cert, edition))
    PU = point_from(ec.P256, vki[1])
    return ec.P256, ecqv.public_key(e, PU, Qi)


def sig_rs(sig):
    alg, s = sig
    r = s['rSig']
    x = r[1] if r[0] in ('x-only', 'compressed-y-0', 'compressed-y-1') else r[1]['x']
    return ec.os2i(x), ec.os2i(s['sSig'])


def verify_explicit(cert, issuer=None, edition='2022'):
    """Signature on an explicit certificate; issuer None means self-signed."""
    if 'signature' not in cert:
        return False
    signer = cert if issuer is None else issuer
    C, Q = verification_key(signer, None, edition)
    hname = HASH_OF_CURVE[C.name]
    signer_input = b'' if issuer is None else encode_canonical(issuer, edition)
    digest = prims.digest_1609(hname, tbs_bytes(cert, edition), signer_input)
    r, s = sig_rs(cert['signature'])
    return prims.ecdsa_verify(C, Q, digest, r, s)


def signed_data_digest(hash_name, tbs_data_bytes, signer_cert_canonical_bytes):
    return prims.digest_1609(hash_name, tbs_data_bytes, signer_cert_canonical_bytes)
