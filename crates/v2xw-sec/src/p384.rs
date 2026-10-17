//! ECDSA over NIST P-384 with SHA-384: the elector curve of the US SCMS (the electors
//! on the published CTLs sign with P-384, docs/design/scms-real/01 §3.1), and SHA-384,
//! the hash behind `HashedId48` and `sha384AndDigest` (02 §2, §4).
//!
//! FIPS 186-4 ECDSA, through RustCrypto `p384`. Signing is deterministic (RFC 6979), so a
//! run's signatures are a pure function of its keys and messages (ADR 0004).
//! Verification takes the 1609.2 view of a signature, the two 48-byte integers `r` and
//! `s` (`EcdsaP384Signature` carries `r` as the x coordinate of `rSig`), and returns a
//! plain `bool` for every malformed input rather than an error, because a signature and
//! a key off the wire are data. Checked against NIST CAVP 186-4 `SigVer.rsp`
//! `[P-384,SHA-384]` and `SHA384ShortMsg.rsp` in `tests/cavp.rs`.

use ::p384::ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier};
use ::p384::ecdsa::{Signature, SigningKey, VerifyingKey};
use sha2::{Digest, Sha384};

/// Length of a P-384 scalar, field element and SHA-384 output.
pub const LEN: usize = 48;

/// SHA-384 of `bytes`.
pub fn sha384(bytes: &[u8]) -> [u8; LEN] {
    Sha384::digest(bytes).into()
}

/// An ECDSA P-384 signature as the two big-endian integers `r` and `s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P384Signature {
    /// `r`, the x coordinate of `k·G` reduced mod n.
    pub r: [u8; LEN],
    /// `s`.
    pub s: [u8; LEN],
}

/// A P-384 signing key.
#[derive(Clone)]
pub struct P384SigningKey(SigningKey);

impl core::fmt::Debug for P384SigningKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("P384SigningKey(..)")
    }
}

impl P384SigningKey {
    /// The key whose private scalar is the big-endian `d`; `None` unless `1 ≤ d < n`.
    pub fn from_be48(d: &[u8; LEN]) -> Option<Self> {
        SigningKey::from_bytes(d.into()).ok().map(Self)
    }

    /// The public key as a SEC 1 point: 49 bytes compressed, or 97 uncompressed.
    pub fn public_sec1(&self, compressed: bool) -> Vec<u8> {
        self.0
            .verifying_key()
            .to_encoded_point(compressed)
            .as_bytes()
            .to_vec()
    }

    /// Signs a 48-byte SHA-384 digest (RFC 6979 nonce, low-s not enforced: FIPS 186-4
    /// and 1609.2 accept either `s`).
    pub fn sign_digest(&self, digest: &[u8; LEN]) -> P384Signature {
        let sig: Signature = self
            .0
            .sign_prehash(digest)
            .expect("a 48-byte prehash is always signable");
        let (r, s) = sig.split_bytes();
        P384Signature {
            r: r.into(),
            s: s.into(),
        }
    }

    /// Signs `msg`: [`Self::sign_digest`] of [`sha384`]`(msg)`.
    pub fn sign(&self, msg: &[u8]) -> P384Signature {
        self.sign_digest(&sha384(msg))
    }
}

/// Verifies `sig` over the 48-byte `digest` under the SEC 1 public key `public_sec1`.
/// `false` for an invalid point, `r` or `s` outside `[1, n−1]`, or a bad signature.
pub fn verify_digest(public_sec1: &[u8], digest: &[u8; LEN], sig: &P384Signature) -> bool {
    let Ok(vk) = VerifyingKey::from_sec1_bytes(public_sec1) else {
        return false;
    };
    let Ok(sig) = Signature::from_scalars(sig.r, sig.s) else {
        return false;
    };
    vk.verify_prehash(digest, &sig).is_ok()
}

/// Verifies `sig` over `msg`: [`verify_digest`] of [`sha384`]`(msg)`.
pub fn verify(public_sec1: &[u8], msg: &[u8], sig: &P384Signature) -> bool {
    verify_digest(public_sec1, &sha384(msg), sig)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_round_trip_and_rejections() {
        let mut d = [0u8; LEN];
        d[LEN - 1] = 42;
        let key = P384SigningKey::from_be48(&d).unwrap();
        let sig = key.sign(b"elector ballot");
        // Deterministic (RFC 6979).
        assert_eq!(sig, key.sign(b"elector ballot"));
        for compressed in [true, false] {
            let pk = key.public_sec1(compressed);
            assert_eq!(pk.len(), if compressed { 49 } else { 97 });
            assert!(verify(&pk, b"elector ballot", &sig));
            assert!(!verify(&pk, b"elector ballot!", &sig));
        }
        let pk = key.public_sec1(true);
        let mut bad = sig;
        bad.s[LEN - 1] ^= 1;
        assert!(!verify(&pk, b"elector ballot", &bad));
        assert!(!verify(&pk[..48], b"elector ballot", &sig));
        assert!(P384SigningKey::from_be48(&[0u8; LEN]).is_none());
    }
}
