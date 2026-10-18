#!/usr/bin/env python3
"""Measures the TLS handshake flights of an EE <-> SCMS session (WP-D3).

The US profile: TLS 1.2 or 1.3, server-only X.509 authentication, ECDHE-ECDSA suites
only, no session resumption ([SMP] §2.1.18, §3.2; [SM-EESEC] §5.2.3; 03 §2). This runs a
real handshake between two OpenSSL endpoints over memory BIOs and counts the bytes of
each flight (TLS records, before TCP/IP headers). Nothing goes on the network.

Assumptions (labelled in docs/design/scms-real/constants.md):
  * the server sends a two-certificate chain, leaf + issuing CA, both ECDSA P-256
    (no provider publishes its TLS chain), with a neutral 26-character host name;
  * the client is OpenSSL's default ClientHello restricted to the profile's suites
    (an OBU's own TLS stack is not published);
  * no session tickets (resumption is not supported, so none is issued).

Usage: python -I tls_handshake_sizes.py   -> one line per version, then a verdict.
"""
import datetime
import os
import ssl
import sys
import tempfile

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID

HOST = "ra.provider-a.scms.example"  # 26 characters, neutral (owner decision 3)


def _cert(subject, issuer, key, issuer_key, ca, san=None):
    now = datetime.datetime(2026, 1, 6, tzinfo=datetime.timezone.utc)
    b = (
        x509.CertificateBuilder()
        .subject_name(x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, subject)]))
        .issuer_name(x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, issuer)]))
        .public_key(key.public_key())
        .serial_number(0x1D3 if ca else 0x1D4)
        .not_valid_before(now)
        .not_valid_after(now + datetime.timedelta(days=397))
        .add_extension(x509.BasicConstraints(ca=ca, path_length=None), critical=True)
    )
    if san:
        b = b.add_extension(x509.SubjectAlternativeName([x509.DNSName(san)]), critical=False)
    return b.sign(issuer_key, hashes.SHA256())


def _chain(d):
    ka = ec.derive_private_key(0xA11CE, ec.SECP256R1())
    kl = ec.derive_private_key(0xB0B, ec.SECP256R1())
    ca = _cert("SCMS TLS Issuing CA", "SCMS TLS Issuing CA", ka, ka, True)
    leaf = _cert(HOST, "SCMS TLS Issuing CA", kl, ka, False, HOST)

    def pem(c):
        return c.public_bytes(serialization.Encoding.PEM)

    with open(os.path.join(d, "chain.pem"), "wb") as f:
        f.write(pem(leaf) + pem(ca))
    with open(os.path.join(d, "key.pem"), "wb") as f:
        f.write(kl.private_bytes(serialization.Encoding.PEM,
                                 serialization.PrivateFormat.PKCS8,
                                 serialization.NoEncryption()))
    with open(os.path.join(d, "ca.pem"), "wb") as f:
        f.write(pem(ca))
    der = serialization.Encoding.DER
    return len(leaf.public_bytes(der)), len(ca.public_bytes(der))


def handshake(d, version):
    sctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    sctx.load_cert_chain(os.path.join(d, "chain.pem"), os.path.join(d, "key.pem"))
    cctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    cctx.load_verify_locations(os.path.join(d, "ca.pem"))
    for c in (sctx, cctx):
        c.minimum_version = c.maximum_version = version
        c.options |= ssl.OP_NO_TICKET
        # [SM-EESEC] §5.2.3: ECDHE_ECDSA with AES-128/256 in GCM or CBC (TLS 1.2).
        c.set_ciphers("ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:"
                      "ECDHE-ECDSA-AES128-SHA256:ECDHE-ECDSA-AES256-SHA384")
    sctx.num_tickets = 0
    ci, co, si, so = ssl.MemoryBIO(), ssl.MemoryBIO(), ssl.MemoryBIO(), ssl.MemoryBIO()
    cli = cctx.wrap_bio(ci, co, server_hostname=HOST)
    srv = sctx.wrap_bio(si, so, server_side=True)
    flights, done = [], {"c": False, "s": False}

    def step(end, name, out, peer_in):
        try:
            end.do_handshake()
            done[name] = True
        except ssl.SSLWantReadError:
            pass
        data = out.read()
        if data:
            flights.append((name, len(data)))
            peer_in.write(data)

    for _ in range(10):
        step(cli, "c", co, si)
        step(srv, "s", so, ci)
        if done["c"] and done["s"]:
            break
    step(cli, "c", co, si)
    return cli.version(), cli.cipher()[0], flights


def main():
    with tempfile.TemporaryDirectory() as d:
        leaf, ca = _chain(d)
        out = {}
        for v in (ssl.TLSVersion.TLSv1_3, ssl.TLSVersion.TLSv1_2):
            ver, suite, fl = handshake(d, v)
            out[ver] = fl
            print(f"{ver} {suite} flights={fl} leaf_der={leaf} ca_der={ca} "
                  f"openssl='{ssl.OPENSSL_VERSION}'")
    t13, t12 = out["TLSv1.3"], out["TLSv1.2"]
    ok = ([s for s, _ in t13] == ["c", "s", "c"]
          and [s for s, _ in t12] == ["c", "s", "c", "s"])
    print("VERDICT", "ok" if ok else "UNEXPECTED FLIGHT SHAPE")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
