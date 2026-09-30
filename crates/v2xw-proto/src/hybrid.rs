//! Hybrid post-quantum credentials: what `security.signature`'s hybrid schemes cost the
//! credential system, in bytes and in time.
//!
//! A hybrid signature is the ECDSA P-256 signature with a post-quantum one concatenated
//! (05-protocols.md §5.3, "raw concat"). Nothing in IEEE 1609.2 or TS 103 097 standardises
//! one yet, so this is the design every published hybrid-V2X proposal converges on, and the
//! consequences below follow from it rather than from any deployment:
//!
//! * **Every signature is two signatures.** Each signed message between an authority and a
//!   device, and between two authorities, carries the post-quantum signature as well, and
//!   whoever makes or checks it pays both operations. The authorities' figures come from
//!   [`BACKEND_PQ_PROFILE`], the devices' from the per-scheme device profile.
//! * **Every certificate grows** by the holder's post-quantum public key and the issuer's
//!   post-quantum signature. Pseudonym batches, chain files, trust lists and CRLs grow with
//!   them.
//! * **There is no post-quantum butterfly.** Butterfly key expansion (and ECQV) rest on
//!   elliptic-curve key addition, which lattice schemes do not have: a device cannot hand
//!   the RA one caterpillar key from which the PCA derives a thousand unlinkable
//!   post-quantum keys. The device therefore generates one post-quantum key pair per
//!   pseudonym certificate itself and uploads each public key encrypted to the PCA (so the
//!   RA, which sees the whole request, still cannot link the certificates): the request
//!   grows by one wrapped key per certificate and the device pays one key generation per
//!   certificate. This is the straightforward construction, not a standard one; a real
//!   design might instead let the PCA generate the keys and encrypt the private halves,
//!   which moves the key-generation cost to the PCA and gives the PCA the private keys —
//!   exactly what the butterfly design was built to avoid.
//!
//! Cost anchors, and where they are weaker than they look (said plainly, not hidden):
//!
//! * Authorities: [`BACKEND_PQ_PROFILE`], a Raspberry Pi 5 running liboqs (arXiv
//!   2503.10238 Table 8), the one profile in the catalogue that publishes signing,
//!   verification *and* key generation for both ML-DSA-44 and Falcon-512 as a time. A
//!   server is faster, so the authorities' post-quantum time is an upper bound; the x86
//!   figures in the catalogue are cycle counts without a clock and cannot be charged.
//! * Devices: the Cohda MK6 figures of NDSS 2024 Table V (ML-DSA-44 under Botan,
//!   Falcon-512 under liboqs), the same unit whose ECDSA figure the credential kernel
//!   charges.
//! * Device key generation: no MK6 figure is published, so it is the Raspberry Pi 4's
//!   (Cortex-A72, liboqs, arXiv 2503.10238 Table 8). The MK6's application processor is
//!   no faster, so this is a lower bound. Falcon-512 key generation there takes 42 ms,
//!   which is why a Falcon batch is expensive to request.

use v2xw_sec::primitive::{PrimitiveCatalogue, PrimitiveId, PrimitiveOpKind, profiles};

use crate::kernel::PqCharge;
use crate::sizes::{AES128_KEY_BYTES, ECIES_P256_ENCRYPTED_KEY_BYTES};

/// Where the authorities' post-quantum time comes from. See the module notes.
pub const BACKEND_PQ_PROFILE: &str = profiles::CORTEX_A76_PI5_LIBOQS;
/// Where a device's post-quantum key-generation time comes from. See the module notes.
pub const DEVICE_PQ_KEYGEN_PROFILE: &str = profiles::CORTEX_A72_PI4_LIBOQS;

/// An AES-CCM nonce: IEEE 1609.2 `AesCcmCiphertext`.
const CCM_NONCE_BYTES: u32 = 12;

/// The post-quantum half of a hybrid `security.signature` scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HybridScheme {
    /// The scenario's name for it, e.g. `hybrid-mldsa44-ecdsa-p256`.
    pub id: &'static str,
    /// The post-quantum primitive.
    pub pq: PrimitiveId,
    /// Its signature, as carried: 2,420 B for ML-DSA-44 (FIPS 204 Table 2), 666 B for
    /// Falcon-512 padded (PQClean `falcon-padded-512`).
    pub sig_bytes: u32,
    /// Its public key: 1,312 B for ML-DSA-44, 897 B for Falcon-512.
    pub pk_bytes: u32,
    /// Where a device's post-quantum signing and verification time comes from.
    pub device_profile: &'static str,
}

impl HybridScheme {
    /// The hybrid scheme a `security.signature` value names, `None` for a classical one.
    #[must_use]
    pub fn from_signature(signature: &str) -> Option<HybridScheme> {
        let (id, pq, device_profile) = match signature {
            "hybrid-mldsa44-ecdsa-p256" => (
                "hybrid-mldsa44-ecdsa-p256",
                PrimitiveId::ML_DSA_44,
                profiles::COHDA_MK6_BOTAN,
            ),
            "hybrid-falcon512-ecdsa-p256" => (
                "hybrid-falcon512-ecdsa-p256",
                PrimitiveId::FALCON_512,
                profiles::COHDA_MK6_LIBOQS,
            ),
            _ => return None,
        };
        let d = PrimitiveCatalogue::standard().get(pq)?;
        Some(HybridScheme {
            id,
            pq,
            sig_bytes: d.envelope_sig_bytes(),
            pk_bytes: d.pk_bytes,
            device_profile,
        })
    }

    /// What a certificate grows by: the holder's post-quantum key and the issuer's
    /// post-quantum signature.
    #[must_use]
    pub const fn cert_extra_bytes(&self) -> u32 {
        self.pk_bytes + self.sig_bytes
    }

    /// One post-quantum public key as a device uploads it for a pseudonym certificate,
    /// encrypted to the PCA: the key, an ECIES-wrapped AES key, the CCM nonce and tag.
    #[must_use]
    pub const fn key_upload_bytes(&self) -> u32 {
        self.pk_bytes + ECIES_P256_ENCRYPTED_KEY_BYTES + CCM_NONCE_BYTES + AES128_KEY_BYTES
    }

    /// The charge an entity pays for the post-quantum half: an authority's, or a device's.
    #[must_use]
    pub const fn charge(&self, device: bool) -> PqCharge {
        PqCharge {
            primitive: self.pq,
            profile: if device {
                self.device_profile
            } else {
                BACKEND_PQ_PROFILE
            },
            keygen_profile: if device {
                DEVICE_PQ_KEYGEN_PROFILE
            } else {
                BACKEND_PQ_PROFILE
            },
        }
    }

    /// The modelled time of one operation of `kind` for an authority or a device, `None`
    /// when the catalogue publishes no figure for it on that profile.
    #[must_use]
    pub fn op_time(&self, kind: PrimitiveOpKind, device: bool) -> Option<v2xw_core::time::Duration> {
        let c = self.charge(device);
        let profile = if kind == PrimitiveOpKind::KeyGen {
            c.keygen_profile
        } else {
            c.profile
        };
        crate::spec::OpDescriptor::new(self.pq, kind, 1).duration(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_hybrids_have_their_published_sizes_and_the_classical_curves_none() {
        let m = HybridScheme::from_signature("hybrid-mldsa44-ecdsa-p256").expect("hybrid");
        assert_eq!((m.sig_bytes, m.pk_bytes), (2_420, 1_312));
        let f = HybridScheme::from_signature("hybrid-falcon512-ecdsa-p256").expect("hybrid");
        assert_eq!((f.sig_bytes, f.pk_bytes), (666, 897));
        assert!(HybridScheme::from_signature("ecdsa-p256").is_none());
        assert!(HybridScheme::from_signature("ecdsa-p384").is_none());
    }

    /// Every operation the credential kernel charges has a figure on the profile it is
    /// charged against, so no post-quantum operation is silently free.
    #[test]
    fn every_charged_operation_has_a_published_time() {
        for s in ["hybrid-mldsa44-ecdsa-p256", "hybrid-falcon512-ecdsa-p256"] {
            let h = HybridScheme::from_signature(s).expect("hybrid");
            for device in [false, true] {
                for kind in [PrimitiveOpKind::Sign, PrimitiveOpKind::Verify, PrimitiveOpKind::KeyGen] {
                    let t = h.op_time(kind, device);
                    assert!(
                        t.is_some_and(|t| t.as_nanos() > 0),
                        "{s} {kind:?} device={device}: {t:?}"
                    );
                }
            }
        }
        // Falcon-512 key generation is the slow one: tens of milliseconds on a device.
        let f = HybridScheme::from_signature("hybrid-falcon512-ecdsa-p256").expect("hybrid");
        let kg = f.op_time(PrimitiveOpKind::KeyGen, true).expect("figure");
        assert!(kg.as_nanos() > 10_000_000, "{kg:?}");
    }
}
