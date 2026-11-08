"""1609.2 primitives written from the standards (02 sections 2-8), independent of the engine.

Hash, HMAC and the KDF are written out here (on hashlib's compression function) rather than taken
from a library, so the fault hooks can reach inside them; ``cross_check_libs`` compares each with the
``cryptography`` / stdlib implementation.  AES itself (the block function) comes from
``cryptography``; CCM is written out over it per NIST SP 800-38C.
"""

import hashlib

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

from . import ec, faults

# ------------------------------------------------------------------------------------------------
# hashes and HashedId (1609.2 section 6.3 / ASN.1 HashedIdN comments)


def sha256(b):
    if faults.on('sha256-is-sha224'):
        return hashlib.sha224(b).digest() + b'\0' * 4
    return hashlib.sha256(b).digest()


def sha384(b):
    return hashlib.sha384(b).digest()


HASHES = {'SHA-256': sha256, 'SHA-384': sha384, 'sha256': sha256, 'sha384': sha384}


def hashed_id(data, n, hash_name='SHA-256'):
    """HashedIdN: the low-order (last) N bytes of the hash of the (canonical COER) encoding."""
    h = HASHES[hash_name](data)
    if faults.on('hashedid-high-bytes'):
        return h[:n]
    return h[-n:]


# ------------------------------------------------------------------------------------------------
# HMAC (RFC 2104) and MAC1 (1609.2a 5.3.5: HMAC-SHA-256, 128-bit tag)


def hmac_sha256(key, msg):
    block = 64
    if len(key) > block:
        key = sha256(key)
    key = key.ljust(block, b'\0')
    ipad = 0x35 if faults.on('hmac-wrong-pad') else 0x36
    inner = sha256(bytes(k ^ ipad for k in key) + msg)
    return sha256(bytes(k ^ 0x5C for k in key) + inner)


def mac1(key, msg):
    t = hmac_sha256(key, msg)
    return t if faults.on('mac1-no-truncate') else t[:16]


# ------------------------------------------------------------------------------------------------
# KDF2 (IEEE 1363a 13.2) with SHA-256 = ANSI X9.63 KDF


def kdf2(z, length, p=b'', hash_fn=sha256):
    out = b''
    counter = 0 if faults.on('kdf2-counter-0') else 1
    while len(out) < length:
        out += hash_fn(z + counter.to_bytes(4, 'big') + p)
        counter += 1
    return out[:length]


# ------------------------------------------------------------------------------------------------
# AES-128 block and AES-128-CCM (SP 800-38C), 1609.2: nonce 12, tag 16, no AAD


def aes_block(key, block):
    e = Cipher(algorithms.AES(key), modes.ECB()).encryptor()
    return e.update(block) + e.finalize()


def _xor(a, b):
    return bytes(x ^ y for x, y in zip(a, b))


def _ccm_core(key, nonce, payload, adata, tlen):
    n = len(nonce)
    q = 15 - n
    if not 7 <= n <= 13 or tlen not in (4, 6, 8, 10, 12, 14, 16):
        raise ValueError('bad CCM parameters')
    if len(payload) >= 1 << (8 * q):
        raise ValueError('payload too long for nonce length')
    t_enc = 8 if faults.on('ccm-flags-tag8') else tlen
    flags = (0x40 if adata else 0) | (((t_enc - 2) // 2) << 3) | (q - 1)
    b0 = bytes([flags]) + nonce + len(payload).to_bytes(q, 'big')
    blocks = [b0]
    if adata:
        a = len(adata)
        if a < 0xFF00:
            enc = a.to_bytes(2, 'big')
        elif a < 1 << 32:
            enc = b'\xff\xfe' + a.to_bytes(4, 'big')
        else:
            enc = b'\xff\xff' + a.to_bytes(8, 'big')
        ad = enc + adata
        ad += b'\0' * (-len(ad) % 16)
        blocks += [ad[i:i + 16] for i in range(0, len(ad), 16)]
    pl = payload + b'\0' * (-len(payload) % 16)
    blocks += [pl[i:i + 16] for i in range(0, len(pl), 16)]
    y = b'\0' * 16
    for b in blocks:
        y = aes_block(key, _xor(y, b))
    tag = y[:tlen]

    def ctr(i):
        return bytes([q - 1]) + nonce + i.to_bytes(q, 'big')

    first = 2 if faults.on('ccm-counter-1') else 1
    stream = b''
    i = first
    while len(stream) < len(payload):
        stream += aes_block(key, ctr(i))
        i += 1
    s0 = aes_block(key, ctr(0))
    return _xor(payload, stream), _xor(tag, s0)


def ccm_encrypt(key, nonce, plaintext, adata=b'', tlen=16):
    c, t = _ccm_core(key, nonce, plaintext, adata, tlen)
    return c + t


def ccm_decrypt(key, nonce, ct, adata=b'', tlen=16):
    """Returns the plaintext, or None when the tag does not verify."""
    if len(ct) < tlen:
        return None
    c, t = ct[:-tlen], ct[-tlen:]
    n = len(nonce)
    q = 15 - n

    def ctr(i):
        return bytes([q - 1]) + nonce + i.to_bytes(q, 'big')

    first = 2 if faults.on('ccm-counter-1') else 1
    stream = b''
    i = first
    while len(stream) < len(c):
        stream += aes_block(key, ctr(i))
        i += 1
    p = _xor(c, stream)
    _, t2 = _ccm_core(key, nonce, p, adata, tlen)
    import hmac as _h
    return p if _h.compare_digest(t, t2) else None


# ------------------------------------------------------------------------------------------------
# ECIES (1609.2a 5.3.5), P-256, non-DHAES; wraps a 16-byte AES key


def ecies_encrypt(R, k, p1, v):
    """R: recipient public point; k: 16-byte AES key; p1: KDF parameter; v: ephemeral private key."""
    C = ec.P256
    V = ec.base_mul(C, v)
    z = ec.i2os(ec.mul(C, v, R)[0], 32)
    kin = (ec.compress(C, V) + p1) if faults.on('ecies-dhaes') else p1
    K = kdf2(z, 48, kin)
    k1, k2 = K[:16], K[16:48]
    c = _xor(k, k1)
    t = mac1(k2, c)
    return V, c, t


def ecies_decrypt(r, V, c, t, p1):
    C = ec.P256
    if not ec.on_curve(C, V) or V is None:
        return None
    z = ec.i2os(ec.mul(C, r, V)[0], 32)
    kin = (ec.compress(C, V) + p1) if faults.on('ecies-dhaes') else p1
    K = kdf2(z, 48, kin)
    import hmac as _h
    if not _h.compare_digest(mac1(K[16:48], c), t):
        return None
    return _xor(c, K[:16])


def ecies_p1(recipient_kind, encoded=b''):
    """P1 per RecipientInfo kind (02 section 8.1): the SHA-256 of the recipient's encoding, or of ''."""
    if recipient_kind in ('certRecipInfo', 'signedDataRecipInfo'):
        return sha256(encoded)
    if recipient_kind == 'rekRecipInfo':
        return sha256(b'')
    raise ValueError(recipient_kind)


# ------------------------------------------------------------------------------------------------
# ECDSA (FIPS 186-4 6.4) over a pre-computed digest, P-256 / P-384


def _digest_int(C, digest):
    e = ec.os2i(digest)
    nbits = C.n.bit_length()
    if len(digest) * 8 > nbits:
        e >>= len(digest) * 8 - nbits
    return e


def ecdsa_sign(C, d, digest, k):
    """Deterministic given k (the caller supplies the nonce). Returns (r, s, R)."""
    e = _digest_int(C, digest)
    R = ec.base_mul(C, k)
    r = R[0] % C.n
    s = pow(k, -1, C.n) * (e + r * d) % C.n
    if r == 0 or s == 0:
        raise ValueError('bad nonce')
    return r, s, R


def ecdsa_verify(C, Q, digest, r, s):
    if faults.on('ecdsa-accept-any'):
        return True
    if not faults.on('ecdsa-skip-range'):
        if not (1 <= r < C.n and 1 <= s < C.n):
            return False
    if Q is None or not ec.on_curve(C, Q):
        return False
    e = _digest_int(C, digest)
    try:
        w = pow(s, -1, C.n)
    except ValueError:
        return False
    X = ec.add(C, ec.base_mul(C, e * w % C.n), ec.mul(C, r * w % C.n, Q))
    if X is None:
        return False
    return X[0] % C.n == r


def digest_1609(hash_name, tbs_data, signer_input):
    """1609.2 5.3.1: H( H(COER tbsData) || H(signer cert COER, or '' for self) )."""
    H = HASHES[hash_name]
    return H(H(tbs_data) + H(signer_input))


# ------------------------------------------------------------------------------------------------


def cross_check_libs():
    """Hand-written HMAC/KDF2/CCM equal the stdlib / cryptography implementations on random inputs."""
    import hmac as _h
    import random
    from cryptography.hazmat.primitives import hashes
    from cryptography.hazmat.primitives.ciphers.aead import AESCCM
    from cryptography.hazmat.primitives.kdf.x963kdf import X963KDF
    rnd = random.Random(7)
    for _ in range(20):
        key = rnd.randbytes(rnd.choice([4, 16, 20, 64, 131]))
        msg = rnd.randbytes(rnd.randrange(0, 200))
        if hmac_sha256(key, msg) != _h.new(key, msg, hashlib.sha256).digest():
            return 'hmac'
        z = rnd.randbytes(32)
        p = rnd.randbytes(rnd.choice([0, 16, 32]))
        n = rnd.choice([16, 48, 128])
        if kdf2(z, n, p) != X963KDF(hashes.SHA256(), n, p or None).derive(z):
            return 'kdf2'
        k = rnd.randbytes(16)
        nonce = rnd.randbytes(12)
        pt = rnd.randbytes(rnd.randrange(0, 300))
        if ccm_encrypt(k, nonce, pt) != AESCCM(k, 16).encrypt(nonce, pt, None):
            return 'ccm'
        if ccm_decrypt(k, nonce, AESCCM(k, 16).encrypt(nonce, pt, None)) != pt:
            return 'ccm-dec'
    return None
