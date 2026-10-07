"""Independent references for the security layer.

Three operations, chosen with the "op" key of the JSON on stdin:

* "ecdsa": verify P-256 signatures with OpenSSL (through the `cryptography` package) and
  produce OpenSSL signatures for the Rust side to verify.
* "spdu": parse IEEE 1609.2 SignedData from its COER bytes, written here from the
  ASN.1 of IEEE 1609.2-2016 clause 6.3 and the canonical OER rules of ITU-T X.696,
  sharing nothing with the Rust envelope, and verify its signature with OpenSSL over
  H(H(tbsData) || H(signer certificate)) [1609.2 clause 5.3.1].
* "butterfly": check butterfly key-expansion identities with a pure-Python P-256
  implementation (affine Weierstrass arithmetic over the NIST curve parameters of
  FIPS 186-4 D.1.2.3), independent of both the Rust `p256` crate and OpenSSL.
"""

import hashlib
import json
import sys

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, utils

# FIPS 186-4 D.1.2.3, curve P-256.
P = 0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF
A = P - 3
B = 0x5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B
N = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
GX = 0x6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296
GY = 0x4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5


def h(b: bytes) -> bytes:
    return hashlib.sha256(b).digest()


# ---------------------------------------------------------------------------------------
# Pure-Python P-256
# ---------------------------------------------------------------------------------------

def on_curve(pt):
    if pt is None:
        return True
    x, y = pt
    return (y * y - (x * x * x + A * x + B)) % P == 0


def add(p1, p2):
    if p1 is None:
        return p2
    if p2 is None:
        return p1
    (x1, y1), (x2, y2) = p1, p2
    if x1 == x2 and (y1 + y2) % P == 0:
        return None
    if p1 == p2:
        lam = (3 * x1 * x1 + A) * pow(2 * y1, -1, P) % P
    else:
        lam = (y2 - y1) * pow(x2 - x1, -1, P) % P
    x3 = (lam * lam - x1 - x2) % P
    return (x3, (lam * (x1 - x3) - y1) % P)


def mul(k, pt):
    acc = None
    k %= N
    while k:
        if k & 1:
            acc = add(acc, pt)
        pt = add(pt, pt)
        k >>= 1
    return acc


def decompress(b: bytes):
    if len(b) == 65 and b[0] == 4:
        return (int.from_bytes(b[1:33], "big"), int.from_bytes(b[33:], "big"))
    x = int.from_bytes(b[1:33], "big")
    y2 = (x * x * x + A * x + B) % P
    y = pow(y2, (P + 1) // 4, P)
    if (y & 1) != (b[0] & 1):
        y = P - y
    return (x, y)


# ---------------------------------------------------------------------------------------
# ECDSA through OpenSSL
# ---------------------------------------------------------------------------------------

def verify(pub33: bytes, digest: bytes, r: int, s: int) -> bool:
    key = ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), pub33)
    try:
        key.verify(utils.encode_dss_signature(r, s), digest, ec.ECDSA(utils.Prehashed(hashes.SHA256())))
        return True
    except InvalidSignature:
        return False


def op_ecdsa(req):
    results = []
    for v in req["verify"]:
        results.append(verify(bytes.fromhex(v["pub"]), bytes.fromhex(v["digest"]), int(v["r"], 16), int(v["s"], 16)))
    signed = []
    for d in req["sign"]:
        key = ec.generate_private_key(ec.SECP256R1())
        der = key.sign(bytes.fromhex(d), ec.ECDSA(utils.Prehashed(hashes.SHA256())))
        r, s = utils.decode_dss_signature(der)
        pub = key.public_key().public_bytes(serialization.Encoding.X962, serialization.PublicFormat.CompressedPoint)
        signed.append({"pub": pub.hex(), "digest": d, "r": "%064x" % r, "s": "%064x" % s})
    return {"verify": results, "signed": signed}


# ---------------------------------------------------------------------------------------
# IEEE 1609.2 SignedData, COER
# ---------------------------------------------------------------------------------------

class Coer:
    def __init__(self, b: bytes):
        self.b = b
        self.i = 0

    def octets(self, n):
        out = self.b[self.i:self.i + n]
        if len(out) != n:
            raise ValueError("truncated at %d" % self.i)
        self.i += n
        return out

    def u8(self):
        return self.octets(1)[0]

    def length(self):
        first = self.u8()
        if first < 0x80:
            return first
        return int.from_bytes(self.octets(first & 0x7F), "big")

    def tag(self):
        t = self.u8()
        if t & 0xC0 != 0x80:
            raise ValueError("not a context tag: %02x" % t)
        return t & 0x3F


def parse_spdu(raw: bytes):
    r = Coer(raw)
    out = {}
    out["protocolVersion"] = r.u8()
    content = r.tag()
    out["content"] = content
    if content != 1:
        raise ValueError("not signedData")
    out["hashId"] = r.u8()
    tbs_start = r.i
    # SignedDataPayload: extensible, two OPTIONAL roots.
    pre = r.u8()
    if pre & 0x80:
        raise ValueError("SignedDataPayload extension present")
    if pre & 0x40:
        out["innerVersion"] = r.u8()
        inner = r.tag()
        if inner != 0:
            raise ValueError("inner content is not unsecuredData")
        out["payload"] = r.octets(r.length()).hex()
    if pre & 0x20:
        raise ValueError("extDataHash not expected")
    # HeaderInfo: extensible, six OPTIONAL roots.
    pre = r.u8()
    if pre & 0x80:
        raise ValueError("HeaderInfo extension present")
    out["psid"] = int.from_bytes(r.octets(r.length()), "big")
    if pre & 0x40:
        out["generationTime"] = int.from_bytes(r.octets(8), "big")
    if pre & 0x20:
        out["expiryTime"] = int.from_bytes(r.octets(8), "big")
    if pre & 0x10:
        loc = r.octets(10)
        out["generationLocation"] = [int.from_bytes(loc[0:4], "big", signed=True), int.from_bytes(loc[4:8], "big", signed=True), int.from_bytes(loc[8:10], "big")]
    if pre & 0x0E:
        raise ValueError("unexpected HeaderInfo optional: %02x" % pre)
    tbs = raw[tbs_start:r.i]
    signer = r.tag()
    out["signerChoice"] = signer
    if signer == 0:
        out["signerDigest"] = r.octets(8).hex()
    else:
        raise ValueError("signer choice %d not handled" % signer)
    sig = r.tag()
    if sig != 0:
        raise ValueError("not ecdsaNistP256Signature")
    point = r.tag()
    if point != 0:
        raise ValueError("rSig is not x-only")
    rr = int.from_bytes(r.octets(32), "big")
    ss = int.from_bytes(r.octets(32), "big")
    out["trailing"] = len(raw) - r.i
    return out, tbs, rr, ss


def op_spdu(req):
    results = []
    for item in req["items"]:
        try:
            parsed, tbs, rr, ss = parse_spdu(bytes.fromhex(item["spdu"]))
            cert = bytes.fromhex(item["cert"])
            parsed["expectedDigest"] = h(cert)[-8:].hex()  # HashedId8: low-order 8 octets
            digest = h(h(tbs) + h(cert))
            parsed["signatureValid"] = verify(bytes.fromhex(item["pub"]), digest, rr, ss)
            results.append(parsed)
        except Exception as e:  # noqa: BLE001
            results.append({"error": repr(e)})
    return {"items": results}


# ---------------------------------------------------------------------------------------
# Butterfly key expansion
# ---------------------------------------------------------------------------------------

def op_butterfly(req):
    G = (GX, GY)
    results = []
    for it in req["items"]:
        a_pub = decompress(bytes.fromhex(it["A"]))
        f = int(it["f"], 16)
        cocoon = decompress(bytes.fromhex(it["B"]))
        c = int(it["c"], 16)
        certified = decompress(bytes.fromhex(it["certified"]))
        d = int(it["d"], 16)
        results.append({
            "on_curve": on_curve(a_pub) and on_curve(cocoon) and on_curve(certified),
            "cocoon": add(a_pub, mul(f, G)) == cocoon,
            "certified": add(cocoon, mul(c, G)) == certified,
            "private": mul(d, G) == certified,
        })
    return {"items": results}


def main():
    req = json.load(sys.stdin)
    op = req["op"]
    out = {"ecdsa": op_ecdsa, "spdu": op_spdu, "butterfly": op_butterfly}[op](req)
    json.dump(out, sys.stdout)


if __name__ == "__main__":
    main()
