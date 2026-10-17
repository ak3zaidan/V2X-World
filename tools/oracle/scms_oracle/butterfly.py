"""Butterfly key expansion (IEEE 1609.2.1 9.3; 02 section 9.2) and the three ways an ACA uses a cocoon key.

    x_sign = 00000000 || i || j || 00000000          x_enc = FFFFFFFF || i || j || 00000000
    f_int(x) = (AES_k(x+1) ^ (x+1)) || (AES_k(x+2) ^ (x+2)) || (AES_k(x+3) ^ (x+3)),  +1..+3 mod 2^128
    f(x) = f_int(x) mod n
"""

from . import ec, faults
from .prims import aes_block

SIGN = 'signing'
ENC = 'encryption'


def x_input(kind, i, j):
    sign_pre, enc_pre = b'\x00' * 4, b'\xff' * 4
    if faults.on('bf-swap-prefix'):
        sign_pre, enc_pre = enc_pre, sign_pre
    pre = sign_pre if kind == SIGN else enc_pre
    return pre + i.to_bytes(4, 'big') + j.to_bytes(4, 'big') + b'\x00' * 4


def f_int(key, x):
    xi = int.from_bytes(x, 'big')
    out, trace = b'', []
    for d in (1, 2, 3):
        xd = ((xi + d) % (1 << 128)).to_bytes(16, 'big')
        a = aes_block(key, xd)
        out += bytes(p ^ q for p, q in zip(a, xd))
        trace.append((xd, a))
    return out, trace


def f(key, kind, i, j, C=ec.P256):
    fi, _ = f_int(key, x_input(kind, i, j))
    v = int.from_bytes(fi, 'big')
    return v if faults.on('bf-no-reduce') else v % C.n


def expand_private(seed, key, kind, i, j, C=ec.P256):
    return (seed + f(key, kind, i, j, C)) % C.n


def expand_public(seed_pub, key, kind, i, j, C=ec.P256):
    return ec.add(C, seed_pub, ec.base_mul(C, f(key, kind, i, j, C)))


# --- ACA side (02 section 9.3) ---------------------------------------------------------------

def explicit_cert_key(C, B, c):
    """Explicit butterfly certificate: ACA certifies U = B + c*G and returns c (privateKeyInfo)."""
    return ec.add(C, B, ec.base_mul(C, c))


def explicit_private(C, b, c):
    return (b + c) % C.n
