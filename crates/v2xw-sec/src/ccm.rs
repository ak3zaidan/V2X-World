//! AES-128-CCM as IEEE 1609.2 uses it: NIST SP 800-38C / RFC 3610 with a 12-byte nonce,
//! a 16-byte tag and **no associated data** (IEEE 1609.2a-2017 §5.3.8 and Annex D.6.1;
//! S1 `One28BitCcmCiphertext`; docs/design/scms-real/02 §8.3).
//!
//! With `n = 12` the length field is `q = 15 − n = 3` bytes, so a plaintext is at most
//! 2²⁴ − 1 bytes. The ciphertext is `C ‖ T`, 16 bytes longer than the plaintext: the
//! `ccmCiphertext` of `One28BitCcmCiphertext`.
//!
//! Checked against all six 1609.2a D.6.1 vectors (`tests/vectors_1609_2a.rs`) and NIST
//! CAVP CCM samples (`tests/cavp.rs`, through [`seal_raw`]/[`open_raw`], which accept the
//! other SP 800-38C nonce lengths and associated data that CAVP exercises).

use ::ccm::aead::AeadInPlace;
use ::ccm::aead::KeyInit;
use ::ccm::aead::generic_array::GenericArray;
use ::ccm::consts::{U7, U8, U9, U10, U11, U12, U13, U16};
use aes::Aes128;

/// AES-128 key length.
pub const KEY_LEN: usize = 16;
/// 1609.2 CCM nonce length.
pub const NONCE_LEN: usize = 12;
/// 1609.2 CCM tag length.
pub const TAG_LEN: usize = 16;
/// Largest plaintext a 12-byte-nonce CCM can carry (q = 3).
pub const MAX_PLAINTEXT_LEN: usize = (1 << 24) - 1;

/// Why a CCM operation produced no output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CcmError {
    /// The tag did not verify: the ciphertext, nonce or key is not the one sealed.
    #[error("AES-CCM authentication failed")]
    AuthenticationFailed,
    /// The input is shorter than a tag.
    #[error("AES-CCM ciphertext shorter than the 16-byte tag")]
    TooShort,
    /// The plaintext does not fit the nonce's length field.
    #[error("AES-CCM plaintext too long for the nonce length")]
    TooLong,
    /// A nonce length outside SP 800-38C's 7..=13 bytes.
    #[error("AES-CCM nonce length {0} is outside 7..=13")]
    BadNonceLength(usize),
}

/// Encrypts `plaintext` under `key` and `nonce` with 1609.2's parameters, returning
/// `ciphertext ‖ tag` (`plaintext.len() + 16` bytes).
pub fn encrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
) -> Result<Vec<u8>, CcmError> {
    seal_raw(key, nonce, &[], plaintext)
}

/// Decrypts and authenticates `ciphertext ‖ tag` under `key` and `nonce` with 1609.2's
/// parameters. Nothing is returned unless the tag verifies.
pub fn decrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext_and_tag: &[u8],
) -> Result<Vec<u8>, CcmError> {
    open_raw(key, nonce, &[], ciphertext_and_tag)
}

macro_rules! by_nonce_len {
    ($len:expr, $n:ident => $body:expr) => {
        match $len {
            7 => {
                type $n = U7;
                $body
            }
            8 => {
                type $n = U8;
                $body
            }
            9 => {
                type $n = U9;
                $body
            }
            10 => {
                type $n = U10;
                $body
            }
            11 => {
                type $n = U11;
                $body
            }
            12 => {
                type $n = U12;
                $body
            }
            13 => {
                type $n = U13;
                $body
            }
            other => Err(CcmError::BadNonceLength(other)),
        }
    };
}

/// SP 800-38C CCM-AES-128 with a 16-byte tag, any nonce length 7..=13 and associated
/// data `aad`: the general mode, used to check this module against NIST CAVP. 1609.2
/// itself always calls [`encrypt`].
pub fn seal_raw(
    key: &[u8; KEY_LEN],
    nonce: &[u8],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CcmError> {
    by_nonce_len!(nonce.len(), N => {
        let cipher = ::ccm::Ccm::<Aes128, U16, N>::new(GenericArray::from_slice(key));
        let mut buf = plaintext.to_vec();
        let tag = cipher
            .encrypt_in_place_detached(GenericArray::from_slice(nonce), aad, &mut buf)
            .map_err(|_| CcmError::TooLong)?;
        buf.extend_from_slice(&tag);
        Ok(buf)
    })
}

/// The inverse of [`seal_raw`].
pub fn open_raw(
    key: &[u8; KEY_LEN],
    nonce: &[u8],
    aad: &[u8],
    ciphertext_and_tag: &[u8],
) -> Result<Vec<u8>, CcmError> {
    if ciphertext_and_tag.len() < TAG_LEN {
        return Err(CcmError::TooShort);
    }
    let (ct, tag) = ciphertext_and_tag.split_at(ciphertext_and_tag.len() - TAG_LEN);
    by_nonce_len!(nonce.len(), N => {
        let cipher = ::ccm::Ccm::<Aes128, U16, N>::new(GenericArray::from_slice(key));
        let mut buf = ct.to_vec();
        cipher
            .decrypt_in_place_detached(
                GenericArray::from_slice(nonce),
                aad,
                &mut buf,
                GenericArray::from_slice(tag),
            )
            .map_err(|_| CcmError::AuthenticationFailed)?;
        Ok(buf)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tamper() {
        let key = [7u8; 16];
        let nonce = [9u8; 12];
        let ct = encrypt(&key, &nonce, b"hello 1609.2").unwrap();
        assert_eq!(ct.len(), 12 + TAG_LEN);
        assert_eq!(decrypt(&key, &nonce, &ct).unwrap(), b"hello 1609.2");
        for i in 0..ct.len() {
            let mut bad = ct.clone();
            bad[i] ^= 1;
            assert_eq!(
                decrypt(&key, &nonce, &bad),
                Err(CcmError::AuthenticationFailed)
            );
        }
        assert_eq!(decrypt(&key, &nonce, &ct[..15]), Err(CcmError::TooShort));
        assert_eq!(
            seal_raw(&key, &[0; 6], &[], b""),
            Err(CcmError::BadNonceLength(6))
        );
    }
}
