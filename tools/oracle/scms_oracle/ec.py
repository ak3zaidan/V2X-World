"""Prime-field short-Weierstrass curves P-256 and P-384 (FIPS 186-4 D.1.2.3 / D.1.2.4), affine arithmetic.

Written for clarity, not speed or side-channel resistance: this is a test oracle.  ``cross_check``
compares k*G with the ``cryptography`` package so the hand-written arithmetic cannot drift unnoticed.
"""

from dataclasses import dataclass

from . import faults


@dataclass(frozen=True)
class Curve:
    name: str
    p: int
    a: int
    b: int
    n: int
    gx: int
    gy: int
    size: int  # field element length in bytes

    @property
    def G(self):
        return (self.gx, self.gy)


P256 = Curve(
    'P-256',
    p=0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF,
    a=-3,
    b=0x5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B,
    n=0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551,
    gx=0x6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296,
    gy=0x4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5,
    size=32)

P384 = Curve(
    'P-384',
    p=0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFFFF0000000000000000FFFFFFFF,
    a=-3,
    b=0xB3312FA7E23EE7E4988E056BE3F82D19181D9C6EFE8141120314088F5013875AC656398D8A2ED19D2A85C8EDD3EC2AEF,
    n=0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC7634D81F4372DDF581A0DB248B0A77AECEC196ACCC52973,
    gx=0xAA87CA22BE8B05378EB1C71EF320AD746E1D3B628BA79B9859F741E082542A385502F25DBF55296C3A545E3872760AB7,
    gy=0x3617DE4A96262C6F5D9E98BF9292DC29F8F41DBD289A147CE9DA3113B5F0B8C00A60B1CE1D7E819D7A431D7C90EA0E5F,
    size=48)

CURVES = {'P-256': P256, 'P-384': P384}
INF = None


def on_curve(C, P):
    if P is INF:
        return True
    x, y = P
    return 0 <= x < C.p and 0 <= y < C.p and (y * y - (x * x * x + C.a * x + C.b)) % C.p == 0


def neg(C, P):
    return INF if P is INF else (P[0], (-P[1]) % C.p)


def add(C, P, Q):
    if P is INF:
        return Q
    if Q is INF:
        return P
    x1, y1 = P
    x2, y2 = Q
    if x1 == x2:
        if (y1 + y2) % C.p == 0:
            return INF
        a = 0 if faults.on('ec-double-bug') else C.a
        lam = (3 * x1 * x1 + a) * pow(2 * y1, -1, C.p) % C.p
    else:
        lam = (y2 - y1) * pow(x2 - x1, -1, C.p) % C.p
    x3 = (lam * lam - x1 - x2) % C.p
    return (x3, (lam * (x1 - x3) - y1) % C.p)


def mul(C, k, P):
    k %= C.n
    R = INF
    Q = P
    while k:
        if k & 1:
            R = add(C, R, Q)
        Q = add(C, Q, Q)
        k >>= 1
    return R


def base_mul(C, k):
    return mul(C, k, C.G)


def i2os(x, n):
    return x.to_bytes(n, 'big')


def os2i(b):
    return int.from_bytes(b, 'big')


def compress(C, P):
    """SEC 1 compressed encoding: 02/03 || x.  (1609.2 compressed-y-0 / compressed-y-1 carry the same x.)"""
    return bytes([2 + (P[1] & 1)]) + i2os(P[0], C.size)


def uncompressed(C, P):
    return b'\x04' + i2os(P[0], C.size) + i2os(P[1], C.size)


def sqrt_mod(C, v):
    # p = 3 mod 4 for both curves
    r = pow(v, (C.p + 1) // 4, C.p)
    if r * r % C.p != v % C.p:
        raise ValueError('not a quadratic residue')
    return r


def decompress(C, y_bit, x):
    if not 0 <= x < C.p:
        raise ValueError('x out of range')
    y = sqrt_mod(C, (x * x * x + C.a * x + C.b) % C.p)
    if (y & 1) != y_bit:
        y = C.p - y
    P = (x, y)
    assert on_curve(C, P)
    return P


def decode_point(C, data):
    if data[0] in (2, 3) and len(data) == 1 + C.size:
        return decompress(C, data[0] - 2, os2i(data[1:]))
    if data[0] == 4 and len(data) == 1 + 2 * C.size:
        P = (os2i(data[1:1 + C.size]), os2i(data[1 + C.size:]))
        if not on_curve(C, P):
            raise ValueError('point not on curve')
        return P
    raise ValueError('bad point encoding')


def cross_check(samples=8, seed=1):
    """k*G here equals k*G from the cryptography package (OpenSSL) for P-256 and P-384."""
    import random
    from cryptography.hazmat.primitives.asymmetric import ec as cec
    rnd = random.Random(seed)
    for C, cc in ((P256, cec.SECP256R1()), (P384, cec.SECP384R1())):
        for _ in range(samples):
            k = rnd.randrange(1, C.n)
            pub = cec.derive_private_key(k, cc).public_key().public_numbers()
            if base_mul(C, k) != (pub.x, pub.y):
                return False
    return True
