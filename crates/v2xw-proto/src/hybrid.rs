//! Post-quantum credentials: what `security.signature`'s hybrid and pure post-quantum
//! schemes, and `security.kem`'s post-quantum KEMs, cost the credential system, in bytes
//! and in time.
//!
//! The scheme itself is described once, in `v2xw_sec::scheme`; this module reads its
//! post-quantum part and its mode, so a scheme registered there is usable by the SCMS and
//! the CCMS without an edit here.
//!
//! A hybrid signature is the ECDSA P-256 signature with a post-quantum one concatenated
//! (05-protocols.md §5.3, "raw concat"); a pure one is the post-quantum signature alone.
//! Nothing in IEEE 1609.2 or TS 103 097 standardises either yet, so this is the design
//! every published post-quantum V2X proposal converges on, and the consequences below
//! follow from it rather than from any deployment:
//!
//! * **Hybrid: every signature is two signatures.** Each signed message between an
//!   authority and a device, and between two authorities, carries the post-quantum
//!   signature as well, and whoever makes or checks it pays both operations. **Pure:** the
//!   post-quantum signature replaces the ECDSA one, and only it is paid. The authorities'
//!   figures come from [`BACKEND_PQ_PROFILE`], the devices' from the per-scheme device
//!   profile.
//! * **Every certificate grows** by the holder's post-quantum public key and the issuer's
//!   post-quantum signature (less, under a pure scheme, the P-256 key and signature they
//!   replace). Pseudonym batches, chain files, trust lists and CRLs grow with them.
//! * **There is no post-quantum butterfly.** Butterfly key expansion (and ECQV) rest on
//!   elliptic-curve key addition, which lattice and hash-based schemes do not have: a
//!   device cannot hand the RA one caterpillar key from which the PCA derives a thousand
//!   unlinkable post-quantum keys. The device therefore generates one post-quantum key
//!   pair per pseudonym certificate itself and uploads each public key encrypted to the
//!   PCA (so the RA, which sees the whole request, still cannot link the certificates):
//!   the request grows by one wrapped key per certificate and the device pays one key
//!   generation per certificate. This is the straightforward construction, not a standard
//!   one; a real design might instead let the PCA generate the keys and encrypt the
//!   private halves, which moves the key-generation cost to the PCA and gives the PCA the
//!   private keys — exactly what the butterfly design was built to avoid.
//! * **A post-quantum KEM** (`security.kem`, ML-KEM per FIPS 203) replaces — or, hybrid,
//!   joins — ECIES wherever something is encrypted to the PCA or to a device: each
//!   encrypted-key structure grows from 65 B to the ML-KEM ciphertext plus the wrapped key
//!   and tag, and each encryption and decryption pays an encapsulation or decapsulation.
//!
//! Cost anchors, and where they are weaker than they look (said plainly, not hidden):
//!
//! * Authorities: [`BACKEND_PQ_PROFILE`], a Raspberry Pi 5 running liboqs (arXiv
//!   2503.10238 Table 8), the one profile in the catalogue that publishes signing,
//!   verification *and* key generation for ML-DSA-44, Falcon-512 and ML-KEM-512/768 as a
//!   time. A server is faster, so the authorities' post-quantum time is an upper bound;
//!   the x86 figures in the catalogue are cycle counts without a clock and cannot be
//!   charged. Other parameter sets are derived by the `pqm4` cycle ratio and flagged.
//! * Devices: the Cohda MK6 figures of NDSS 2024 Table V (ML-DSA-44 under Botan,
//!   Falcon-512 and SPHINCS+ under liboqs), the same unit whose ECDSA figure the
//!   credential kernel charges; other parameter sets derived as above.
//! * Device key generation: no MK6 figure is published, so it is the Raspberry Pi 4's
//!   (Cortex-A72, liboqs, arXiv 2503.10238 Table 8). The MK6's application processor is
//!   no faster, so this is a lower bound. Falcon-512 key generation there takes 42 ms,
//!   which is why a Falcon batch is expensive to request.

use v2xw_sec::primitive::{PrimitiveCatalogue, PrimitiveId, PrimitiveOpKind, profiles};
use v2xw_sec::scheme::{self, SchemeMode};

use crate::kernel::{KemCharge, PqCharge};
use crate::sizes::{
    AES128_KEY_BYTES, EC_POINT_COMPRESSED_BYTES, ECDSA_P256_SIG_COER_BYTES,
    ECIES_P256_ENCRYPTED_KEY_BYTES,
};

/// Where the authorities' post-quantum time comes from. See the module notes.
pub const BACKEND_PQ_PROFILE: &str = profiles::CORTEX_A76_PI5_LIBOQS;
/// Where a device's post-quantum key-generation time comes from. See the module notes.
pub const DEVICE_PQ_KEYGEN_PROFILE: &str = profiles::CORTEX_A72_PI4_LIBOQS;
/// The device libraries a device's post-quantum operations may be priced from, in
/// preference order: the Cohda MK6 publishes ML-DSA under Botan and Falcon and SPHINCS+
/// under liboqs (NDSS 2024 Table V).
pub const DEVICE_PQ_PROFILES: [&str; 2] = [profiles::COHDA_MK6_BOTAN, profiles::COHDA_MK6_LIBOQS];

/// An AES-CCM nonce: IEEE 1609.2 `AesCcmCiphertext`.
const CCM_NONCE_BYTES: u32 = 12;

/// The post-quantum half of a `security.signature` scheme — hybrid or pure — as the
/// credential system pays it. `None` for a classical scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HybridScheme {
    /// The scenario's name for it, e.g. `hybrid-mldsa44-ecdsa-p256` or `ml-dsa-65`.
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
    /// True when the scheme is post-quantum alone: no ECDSA half is made, checked or
    /// carried, and a certificate's key and issuer signature are the post-quantum ones.
    pub pure: bool,
    /// What one uploaded post-quantum key is wrapped in: the KEM's encrypted-key
    /// structure, 65 B for ECIES ([`KemSpec::wrapped_key_bytes`] otherwise).
    pub upload_wrap_bytes: u32,
}

impl HybridScheme {
    /// The scheme a `security.signature` value names, `None` for a classical one. Read
    /// from the registry (`v2xw_sec::scheme`).
    #[must_use]
    pub fn from_signature(signature: &str) -> Option<HybridScheme> {
        let s = scheme::signature(signature)?;
        let part = s.post_quantum_part()?;
        let d = PrimitiveCatalogue::standard().get(part.primitive)?;
        let device_profile =
            scheme::priced_on(part.primitive, PrimitiveOpKind::Verify, &DEVICE_PQ_PROFILES)
                .unwrap_or(profiles::COHDA_MK6_BOTAN);
        Some(HybridScheme {
            id: s.id,
            pq: part.primitive,
            sig_bytes: d.envelope_sig_bytes(),
            pk_bytes: d.pk_bytes,
            device_profile,
            pure: s.mode == SchemeMode::PostQuantum,
            upload_wrap_bytes: ECIES_P256_ENCRYPTED_KEY_BYTES,
        })
    }

    /// The same scheme with its key uploads wrapped by `kem` rather than ECIES.
    #[must_use]
    pub const fn with_kem(mut self, kem: Option<&KemSpec>) -> HybridScheme {
        if let Some(k) = kem {
            self.upload_wrap_bytes = k.wrapped_key_bytes;
        }
        self
    }

    /// What every signature in an envelope grows by: the post-quantum signature, less the
    /// ECDSA one it replaces under a pure scheme.
    #[must_use]
    pub const fn envelope_extra_bytes(&self) -> u32 {
        if self.pure {
            self.sig_bytes.saturating_sub(ECDSA_P256_SIG_COER_BYTES)
        } else {
            self.sig_bytes
        }
    }

    /// What a certificate grows by: the holder's post-quantum key and the issuer's
    /// post-quantum signature — less the P-256 key and signature they replace under a
    /// pure scheme.
    #[must_use]
    pub const fn cert_extra_bytes(&self) -> u32 {
        if self.pure {
            self.pk_bytes.saturating_sub(EC_POINT_COMPRESSED_BYTES)
                + self.sig_bytes.saturating_sub(ECDSA_P256_SIG_COER_BYTES)
        } else {
            self.pk_bytes + self.sig_bytes
        }
    }

    /// One post-quantum public key as a device uploads it for a pseudonym certificate,
    /// encrypted to the PCA: the key, the wrapped AES key, the CCM nonce and tag.
    #[must_use]
    pub const fn key_upload_bytes(&self) -> u32 {
        self.pk_bytes + self.upload_wrap_bytes + CCM_NONCE_BYTES + AES128_KEY_BYTES
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
            pure: self.pure,
        }
    }

    /// The modelled time of one operation of `kind` for an authority or a device, `None`
    /// when the catalogue publishes no figure for it on that profile.
    #[must_use]
    pub fn op_time(
        &self,
        kind: PrimitiveOpKind,
        device: bool,
    ) -> Option<v2xw_core::time::Duration> {
        let c = self.charge(device);
        let profile = if kind == PrimitiveOpKind::KeyGen {
            c.keygen_profile
        } else {
            c.profile
        };
        crate::spec::OpDescriptor::new(self.pq, kind, 1).duration(profile)
    }
}

/// The post-quantum KEM `security.kem` names, as the credential system pays it. `None`
/// for ECIES alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KemSpec {
    /// The scenario's name for it.
    pub id: &'static str,
    /// The ML-KEM parameter set.
    pub primitive: PrimitiveId,
    /// Whether the ECIES half remains.
    pub ecies: bool,
    /// One encrypted-key structure: 65 B for ECIES, the ML-KEM ciphertext + 32 for a pure
    /// KEM, both encapsulations + 32 for a hybrid one (`v2xw_sec::scheme::KemScheme`).
    pub wrapped_key_bytes: u32,
}

impl KemSpec {
    /// The KEM a `security.kem` value names; `None` for ECIES alone.
    #[must_use]
    pub fn from_kem(kem: &str) -> Option<KemSpec> {
        let k = scheme::kem(kem)?;
        let primitive = k.post_quantum?;
        Some(KemSpec {
            id: k.id,
            primitive,
            ecies: k.ecies,
            wrapped_key_bytes: k.wrapped_key_octets(),
        })
    }

    /// What one encrypted key adds over the 65-byte ECIES structure.
    #[must_use]
    pub const fn extra_over_ecies(&self) -> u32 {
        self.wrapped_key_bytes
            .saturating_sub(ECIES_P256_ENCRYPTED_KEY_BYTES)
    }

    /// The charge an authority or a device pays.
    #[must_use]
    pub fn charge(&self, device: bool) -> KemCharge {
        let profile = if device {
            scheme::priced_on(self.primitive, PrimitiveOpKind::Verify, &DEVICE_PQ_PROFILES)
                .unwrap_or(profiles::COHDA_MK6_BOTAN)
        } else {
            BACKEND_PQ_PROFILE
        };
        KemCharge {
            primitive: self.primitive,
            profile,
            ecies: self.ecies,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_hybrids_have_their_published_sizes_and_the_classical_curves_none() {
        let m = HybridScheme::from_signature("hybrid-mldsa44-ecdsa-p256").expect("hybrid");
        assert_eq!((m.sig_bytes, m.pk_bytes), (2_420, 1_312));
        assert!(!m.pure);
        let f = HybridScheme::from_signature("hybrid-falcon512-ecdsa-p256").expect("hybrid");
        assert_eq!((f.sig_bytes, f.pk_bytes), (666, 897));
        assert!(HybridScheme::from_signature("ecdsa-p256").is_none());
        assert!(HybridScheme::from_signature("ecdsa-p384").is_none());
        // The device profile is the library that publishes the primitive.
        assert_eq!(m.device_profile, profiles::COHDA_MK6_BOTAN);
        assert_eq!(f.device_profile, profiles::COHDA_MK6_LIBOQS);
    }

    #[test]
    fn a_pure_scheme_replaces_rather_than_adds() {
        let p = HybridScheme::from_signature("ml-dsa-44").expect("pq");
        assert!(p.pure);
        assert_eq!(p.envelope_extra_bytes(), 2_420 - 66);
        assert_eq!(p.cert_extra_bytes(), (1_312 - 33) + (2_420 - 66));
        let h = HybridScheme::from_signature("hybrid-mldsa44-ecdsa-p256").expect("hybrid");
        assert_eq!(h.envelope_extra_bytes(), 2_420);
        assert_eq!(h.cert_extra_bytes(), 1_312 + 2_420);
    }

    #[test]
    fn an_ml_kem_wrap_replaces_the_65_byte_ecies_structure() {
        assert!(KemSpec::from_kem("ecies-p256").is_none());
        let k = KemSpec::from_kem("ml-kem-768").expect("kem");
        assert_eq!(k.wrapped_key_bytes, 1_088 + 32);
        assert_eq!(k.extra_over_ecies(), 1_088 + 32 - 65);
        let h = HybridScheme::from_signature("ml-dsa-44")
            .expect("pq")
            .with_kem(Some(&k));
        assert_eq!(h.key_upload_bytes(), 1_312 + 1_120 + 12 + 16);
    }

    /// Every operation the credential kernel charges has a figure on the profile it is
    /// charged against, so no post-quantum operation is silently free — for every
    /// post-quantum scheme the registry holds.
    #[test]
    fn every_charged_operation_has_a_time() {
        for s in v2xw_sec::scheme::SIGNATURE_SCHEMES {
            let Some(h) = HybridScheme::from_signature(s.id) else {
                continue;
            };
            for device in [false, true] {
                for kind in [PrimitiveOpKind::Sign, PrimitiveOpKind::Verify] {
                    let t = h.op_time(kind, device);
                    assert!(
                        t.is_some_and(|t| t.as_nanos() > 0),
                        "{} {kind:?} device={device}: {t:?}",
                        s.id
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
