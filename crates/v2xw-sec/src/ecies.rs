//! ECIES over NIST P-256 exactly as IEEE 1609.2 profiles it, used only to wrap a
//! 16-byte AES-128-CCM data-encryption key for one recipient (IEEE 1609.2a-2017
//! §5.3.5; S1 `EciesP256EncryptedKey`; docs/design/scms-real/02 §8.2).
//!
//! ```text
//! v  fresh scalar in [1, n−1];  V = v·G                   (sent as EciesP256EncryptedKey.v)
//! Z  = x(v·R)                                             (ECSVDP-DHC, cofactor 1; non-DHAES: V is not hashed)
//! K  = KDF2(Z, 48, P1);  K1 = K[0..16],  K2 = K[16..48]    (crate::kdf::kdf2_sha256)
//! c  = k XOR K1                                           (16 bytes)
//! t  = MAC1(K2, c) = HMAC-SHA-256(K2, c)[0..16]           (crate::kdf::mac1, P2 empty)
//! ```
//!
//! Decryption recomputes `Z = x(r·V)`, checks `t` in constant time, and only then
//! returns `k = c XOR K1`.
//!
//! `P1` binds the wrapped key to the recipient: it is SHA-256 of an input fixed by the
//! `RecipientInfo` alternative ([`p1`], [`RecipientInfoKind`]). The published vectors
//! (1609.2a D.6.2 and CAMP `ecies.txt`) **misprint P1** (the printed value is the AES key
//! `k`); the real values are in CAMP `ecies.py` and reproduce every printed `C` and `T`,
//! and `tests/vectors_1609_2a.rs` checks both that the real P1 passes and that the
//! printed one fails (02 §8.2 erratum).

use crate::ec::Point;
use crate::kdf::{kdf2_sha256, mac1, mac1_verify};
use p256::Scalar;
use p256::elliptic_curve::Field;
use sha2::{Digest, Sha256};

/// Length of the wrapped AES-128 key.
pub const KEY_LEN: usize = 16;
/// Length of `P1`: one SHA-256 output.
pub const P1_LEN: usize = 32;
/// Length of the MAC1 tag `t`.
pub const TAG_LEN: usize = 16;
/// KDF2 output length: K1 (16 bytes, XOR key) ‖ K2 (32 bytes, MAC key).
const KDF_LEN: usize = KEY_LEN + 32;

/// The three `RecipientInfo` alternatives that carry an ECIES-wrapped key
/// (`PKRecipientInfo`), each with its own `P1` input (02 §8.1 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipientInfoKind {
    /// `certRecipInfo`: the recipient's encryption key is in its certificate, and P1 is
    /// SHA-256 of that certificate's canonical COER encoding.
    CertRecipInfo,
    /// `signedDataRecipInfo`: the key came in `headerInfo.encryptionKey` of a signed
    /// SPDU, and P1 is SHA-256 of the COER `Ieee1609Dot2Data` that carried it
    /// (canonicalised per 1609.2 §6.3.4).
    SignedDataRecipInfo,
    /// `rekRecipInfo`: a response encryption key from an anonymous request; P1 is
    /// SHA-256 of the empty string.
    RekRecipInfo,
}

/// `P1` for a recipient of kind `kind`: SHA-256 of `input`, where `input` is the
/// canonical COER certificate for [`RecipientInfoKind::CertRecipInfo`], the COER
/// `Ieee1609Dot2Data` for [`RecipientInfoKind::SignedDataRecipInfo`], and ignored (the
/// hash of the empty string) for [`RecipientInfoKind::RekRecipInfo`].
pub fn p1(kind: RecipientInfoKind, input: &[u8]) -> [u8; P1_LEN] {
    let bytes: &[u8] = match kind {
        RecipientInfoKind::CertRecipInfo | RecipientInfoKind::SignedDataRecipInfo => input,
        RecipientInfoKind::RekRecipInfo => &[],
    };
    Sha256::digest(bytes).into()
}

/// An `EciesP256EncryptedKey`: the ephemeral point `v`, the wrapped key `c` and the tag
/// `t`. On the wire `v` is an `EccP256CurvePoint` (1609.2 senders use the compressed
/// form, [`Point::to_ecc_point`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EciesP256EncryptedKey {
    /// The sender's ephemeral public key `V = v·G`.
    pub v: Point,
    /// The wrapped key, `k XOR K1`.
    pub c: [u8; KEY_LEN],
    /// MAC1 of `c` under `K2`.
    pub t: [u8; TAG_LEN],
}

/// Why an ECIES operation produced no output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EciesError {
    /// The ephemeral or recipient private scalar is zero.
    #[error("ECIES private scalar is zero")]
    ZeroScalar,
    /// A public point is the identity, so there is no shared secret.
    #[error("ECIES shared point is the identity")]
    Identity,
    /// The tag `t` did not verify: wrong recipient key, wrong P1, or a tampered `v`/`c`/`t`.
    #[error("ECIES authentication tag did not verify")]
    AuthenticationFailed,
}

/// `K1 ‖ K2` from the shared point `S = d·Q`.
fn derive(shared: &Point, p1: &[u8; P1_LEN]) -> Result<[u8; KDF_LEN], EciesError> {
    let (z, _) = shared.xy().ok_or(EciesError::Identity)?;
    let mut k = [0u8; KDF_LEN];
    k.copy_from_slice(&kdf2_sha256(&z, p1, KDF_LEN));
    Ok(k)
}

/// Wraps the AES key `key` for the holder of `recipient` with the ephemeral private
/// scalar `ephemeral` (`v`).
///
/// `ephemeral` must be fresh and uniformly random for every encryption: the caller draws
/// it from its deterministic RNG stream, which is what keeps runs reproducible, and
/// reusing one leaks `K1` for every key wrapped under it.
pub fn encrypt(
    recipient: &Point,
    key: &[u8; KEY_LEN],
    p1: &[u8; P1_LEN],
    ephemeral: &Scalar,
) -> Result<EciesP256EncryptedKey, EciesError> {
    if bool::from(ephemeral.is_zero()) {
        return Err(EciesError::ZeroScalar);
    }
    if recipient.is_identity() {
        return Err(EciesError::Identity);
    }
    let k = derive(&recipient.mul(ephemeral), p1)?;
    let mut c = [0u8; KEY_LEN];
    for (i, b) in c.iter_mut().enumerate() {
        *b = key[i] ^ k[i];
    }
    let t = mac1(&k[KEY_LEN..], &c);
    Ok(EciesP256EncryptedKey {
        v: Point::mul_base(ephemeral),
        c,
        t,
    })
}

/// Unwraps `enc` with the recipient's private scalar `recipient_private` and the same
/// `p1` the sender used. Returns the AES key only if the tag verifies.
pub fn decrypt(
    recipient_private: &Scalar,
    enc: &EciesP256EncryptedKey,
    p1: &[u8; P1_LEN],
) -> Result<[u8; KEY_LEN], EciesError> {
    if bool::from(recipient_private.is_zero()) {
        return Err(EciesError::ZeroScalar);
    }
    if enc.v.is_identity() {
        return Err(EciesError::Identity);
    }
    let k = derive(&enc.v.mul(recipient_private), p1)?;
    if !mac1_verify(&k[KEY_LEN..], &enc.c, &enc.t) {
        return Err(EciesError::AuthenticationFailed);
    }
    let mut out = [0u8; KEY_LEN];
    for (i, b) in out.iter_mut().enumerate() {
        *b = enc.c[i] ^ k[i];
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rek_p1_is_the_hash_of_the_empty_string() {
        let e3b0 = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(p1(RecipientInfoKind::RekRecipInfo, b"ignored"), e3b0);
        assert_eq!(p1(RecipientInfoKind::CertRecipInfo, b""), e3b0);
        assert_ne!(p1(RecipientInfoKind::CertRecipInfo, b"cert"), e3b0);
    }

    #[test]
    fn round_trip_and_rejections() {
        let r = Scalar::from(0x1234_5678_u64);
        let recipient = Point::mul_base(&r);
        let key = [0xA5; 16];
        let p = p1(RecipientInfoKind::CertRecipInfo, b"some certificate");
        let enc = encrypt(&recipient, &key, &p, &Scalar::from(99u64)).unwrap();
        assert_eq!(decrypt(&r, &enc, &p).unwrap(), key);
        // Wrong P1, wrong key, tampered c or t.
        let other = p1(RecipientInfoKind::CertRecipInfo, b"another certificate");
        assert_eq!(
            decrypt(&r, &enc, &other),
            Err(EciesError::AuthenticationFailed)
        );
        assert_eq!(
            decrypt(&Scalar::from(7u64), &enc, &p),
            Err(EciesError::AuthenticationFailed)
        );
        let mut bad = enc.clone();
        bad.c[0] ^= 1;
        assert_eq!(decrypt(&r, &bad, &p), Err(EciesError::AuthenticationFailed));
        let mut bad = enc.clone();
        bad.t[15] ^= 1;
        assert_eq!(decrypt(&r, &bad, &p), Err(EciesError::AuthenticationFailed));
        assert_eq!(
            encrypt(&recipient, &key, &p, &Scalar::ZERO),
            Err(EciesError::ZeroScalar)
        );
    }
}
