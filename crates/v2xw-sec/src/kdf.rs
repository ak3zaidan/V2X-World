//! KDF2 and MAC1, the two symmetric building blocks of IEEE 1609.2 ECIES.
//!
//! Both are specified in IEEE Std 1609.2a-2017 §5.3.5 (S4 in
//! docs/design/scms-real/02 §0) and checked against its Annex D.6.3 (MAC1, 7 vectors)
//! and D.6.4 (KDF2, 4 vectors) in `tests/vectors_1609_2a.rs`:
//!
//! - **KDF2** is IEEE 1363a §13.2 KDF2 with SHA-256, which is the ANSI X9.63 KDF:
//!   `K = SHA-256(Z ‖ 00000001 ‖ P) ‖ SHA-256(Z ‖ 00000002 ‖ P) ‖ …`, truncated to the
//!   requested length, with a 32-bit big-endian counter starting at 1.
//! - **MAC1** is HMAC-SHA-256 with the tag truncated to its leftmost 128 bits
//!   (`tBits = 128`); in ECIES the key is 256 bits and there is no `P2`.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

/// Length of a MAC1 tag in 1609.2: 128 bits.
pub const MAC1_TAG_LEN: usize = 16;

/// KDF2 (IEEE 1363a §13.2) over SHA-256 = ANSI X9.63 KDF: derives `len` bytes from the
/// shared secret `z` and the key-derivation parameter `p` (`P1` in ECIES).
///
/// `len` is bounded by the X9.63 limit `32 · (2³² − 1)` bytes, far above anything 1609.2
/// asks for (48 bytes in ECIES); a larger request panics rather than wraps the counter.
pub fn kdf2_sha256(z: &[u8], p: &[u8], len: usize) -> Vec<u8> {
    let blocks = len.div_ceil(32);
    assert!(
        u32::try_from(blocks).is_ok(),
        "KDF2 output length exceeds 32 * (2^32 - 1) bytes"
    );
    let mut out = Vec::with_capacity(blocks * 32);
    for counter in 1..=blocks as u32 {
        let mut h = Sha256::new();
        h.update(z);
        h.update(counter.to_be_bytes());
        h.update(p);
        out.extend_from_slice(&h.finalize());
    }
    out.truncate(len);
    out
}

/// MAC1 (1609.2a §5.3.5): HMAC-SHA-256 of `msg` under `key`, truncated to the leftmost
/// 128 bits.
pub fn mac1(key: &[u8], msg: &[u8]) -> [u8; MAC1_TAG_LEN] {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
    m.update(msg);
    let full = m.finalize().into_bytes();
    let mut tag = [0u8; MAC1_TAG_LEN];
    tag.copy_from_slice(&full[..MAC1_TAG_LEN]);
    tag
}

/// Constant-time MAC1 check: `true` iff `tag` is the MAC1 of `msg` under `key`.
pub fn mac1_verify(key: &[u8], msg: &[u8], tag: &[u8; MAC1_TAG_LEN]) -> bool {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
    m.update(msg);
    m.verify_truncated_left(tag).is_ok()
}
