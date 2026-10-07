//! `security.signature`: what the chosen signature scheme adds to every signed message.
//!
//! The node's security stack signs and verifies with ECDSA P-256 (`v2xw-node`'s
//! `SIGN_PRIMITIVE`), and a real SPDU is encoded around that signature. IEEE 1609.2's
//! `Signature` CHOICE has alternatives for NIST P-256 and P-384 and brainpool P-256/P-384,
//! and **no** post-quantum alternative is standardised (`v2xw_sec::SigToken`'s refusal
//! says so). So the other schemes are modelled by what they change on the air — the
//! octets — over the P-256 SPDU the node really built:
//!
//! | `security.signature` | Per signature | Per attached certificate | Source |
//! |---|---:|---:|---|
//! | `ecdsa-p256` | 0 | 0 | the measured envelope (04-models.md §9.1) |
//! | `ecdsa-brainpoolp256r1` | 0 | 0 | same field sizes: a 256-bit curve's `r`, `s` and compressed point (IEEE 1609.2 `EcdsaP256Signature`, `EccP256CurvePoint`) |
//! | `ecdsa-p384` | +32 | +48 | 48-byte `r` and `s` against 32 (+32); a 49-byte compressed key against 33 (+16) and the issuer's P-384 signature (+32) — `EcdsaP384Signature`, SEC 1 §2.3.3, derived |
//! | `hybrid-falcon512-ecdsa-p256` | +666 | +1,563 | the ECDSA signature stays and a Falcon-512 signature is concatenated (666 B padded, PQClean `falcon-padded-512`); the certificate adds the Falcon public key (897 B) and a Falcon signature by its issuer (666 B) — 04-models.md §9.4, 05-protocols.md §5.3 ("raw concat", derived) |
//! | `hybrid-mldsa44-ecdsa-p256` | +2,420 | +3,732 | ML-DSA-44 signature 2,420 B, public key 1,312 B [FIPS 204 Table 2], concatenated the same way |
//! | `ml-dsa-44` (pure) | +2,354 | +3,633 | the ML-DSA-44 signature and key *replace* the ECDSA ones: (2,420 − 66) and (1,312 − 33) + (2,420 − 66) |
//!
//! These rows are no longer written here: `v2xw_sec::scheme` derives every scheme's octets
//! from its parts' cited sizes, and the table above is what that derivation gives (its
//! tests pin it). The full list — ML-DSA-44/65/87, Falcon-512/1024, SLH-DSA-128s/128f, pure
//! and hybrid — is the registry's.
//!
//! The post-quantum half's signing and verification *time* is charged elsewhere: on the
//! node by `v2xw_node::profile::signature_ops` (both halves' published costs), and in the
//! credential system by `v2xw_proto::hybrid`. The Partially-Hybrid design of NDSS 2024
//! that fragments the hybrid certificate across a certificate cycle is `net.fragmenter`'s
//! job. A hybrid SPDU above the network
//! layer's MTU is refused before the MAC and counted (`net_mtu_refusals`), which is the
//! air-interface consequence 05-protocols.md §5.3 describes.

/// The schemes `security.signature` accepts, in the order the page offers them — the
/// scheme registry's list (`v2xw_sec::scheme::SIGNATURE_IDS`), so a scheme registered there
/// is selectable here without an edit.
pub const SIGNATURES: [&str; v2xw_sec::scheme::SIGNATURE_IDS.len()] =
    v2xw_sec::scheme::SIGNATURE_IDS;

/// The schemes `security.kem` accepts (`v2xw_sec::scheme::KEM_IDS`).
pub const KEMS: [&str; v2xw_sec::scheme::KEM_IDS.len()] = v2xw_sec::scheme::KEM_IDS;

/// Octets a scheme adds over the P-256 SPDU: `(per signature, per attached certificate)`,
/// derived by the registry from the scheme's parts.
#[must_use]
pub fn extra_octets(signature: &str) -> (u32, u32) {
    v2xw_sec::scheme::signature(signature).map_or((0, 0), |s| s.extra_octets())
}

/// The SPDU, envelope and certificate octets of a message signed under `signature`, from
/// the P-256 message the node built.
#[must_use]
pub fn resize(
    signature: &str,
    spdu: u32,
    envelope: Option<u32>,
    cert: Option<u32>,
    certificate_attached: bool,
) -> (u32, Option<u32>, Option<u32>) {
    let (per_sig, per_cert) = extra_octets(signature);
    let cert_extra = if certificate_attached { per_cert } else { 0 };
    let extra = per_sig + cert_extra;
    (
        spdu.saturating_add(extra),
        envelope.map(|e| e.saturating_add(extra)),
        cert.map(|c| {
            if c > 0 {
                c.saturating_add(cert_extra)
            } else {
                c
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p256_is_unchanged_and_a_hybrid_grows_by_its_published_sizes() {
        assert_eq!(
            resize("ecdsa-p256", 200, Some(93), Some(0), false),
            (200, Some(93), Some(0))
        );
        // A digest-signed BSM under ML-DSA-44 hybrid: +2,420.
        assert_eq!(
            resize("hybrid-mldsa44-ecdsa-p256", 200, Some(93), Some(0), false),
            (2_620, Some(2_513), Some(0))
        );
        // With the certificate attached, the certificate's own growth too.
        let (spdu, _, cert) = resize(
            "hybrid-falcon512-ecdsa-p256",
            300,
            Some(200),
            Some(117),
            true,
        );
        assert_eq!(spdu, 300 + 666 + 897 + 666);
        assert_eq!(cert, Some(117 + 897 + 666));
    }
}
