"""ECQV implicit certificates (SEC 4 v1.0 sections 3.3-3.6) and the IEEE 1609.2 profile (02 section 6).

SEC 4 mode:  e = leftmost floor(log2 n) bits of H(CertU) = SHA-256(CertU) >> 1 on P-256.
1609 mode:   e = SHA-256( SHA-256(COER tbsCertificate) || SHA-256(COER issuer certificate) ), full 256 bits.

    issuance:  P_U = R_U + k*G;  r = e*k + d_CA mod n      (retry when e*P_U + Q_CA = O)
    reception: d_U = e*k_U + r mod n
    any verifier: Q_U = e*P_U + Q_CA
"""

from . import ec, faults
from .prims import sha256

C = ec.P256


def e_sec4(cert_u):
    h = int.from_bytes(sha256(cert_u), 'big')
    return h if faults.on('ecqv-no-sec4-truncate') else h >> 1


def e_1609(tbs_coer, issuer_cert_coer, whole_cert_coer=None):
    if faults.on('ecqv-1609-whole-cert') and whole_cert_coer is not None:
        h = int.from_bytes(sha256(whole_cert_coer), 'big')
    else:
        h = int.from_bytes(sha256(sha256(tbs_coer) + sha256(issuer_cert_coer)), 'big')
    return h >> 1 if faults.on('ecqv-1609-sec4-truncate') else h


def issue(R_U, d_ca, k, e_of_pu):
    """e_of_pu(P_U) -> e (the caller encodes the certificate around P_U). Returns (P_U, r, e)."""
    P_U = ec.add(C, R_U, ec.base_mul(C, k))
    e = e_of_pu(P_U)
    Q_ca = ec.base_mul(C, d_ca)
    if ec.add(C, ec.mul(C, e, P_U), Q_ca) is None:
        raise ValueError('e*P_U + Q_CA is the point at infinity: retry with a new k (SEC 4 3.4)')
    r = (e * k + d_ca) % C.n
    return P_U, r, e


def private_key(e, k_u, r):
    return (e * k_u + r) % C.n


def public_key(e, P_U, Q_ca):
    return ec.add(C, ec.mul(C, e, P_U), Q_ca)
