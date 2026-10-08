//! The one place a signature scheme or a key-encapsulation scheme is described.
//!
//! A scheme is a *composition* of primitives from [`crate::primitive`]: ECDSA P-256 alone,
//! ML-DSA-44 alone, or the two concatenated. Everything the simulator needs to know about
//! it follows from that composition and the primitives' cited sizes and cost anchors:
//!
//! * the octets a signature and a certificate add over the P-256 baseline the node
//!   encodes ([`SignatureScheme::extra_octets`]), which is what the air interface carries
//!   and fragments;
//! * the hardware-profile operations a node is charged for signing and verifying
//!   ([`SignatureScheme::sign_op`], [`op_components`]), whose time comes from the profile's
//!   published figure or, failing that, from the catalogue anchor the profile names;
//! * what the credential system's entities pay and carry (`v2xw_proto::hybrid` reads the
//!   scheme's post-quantum part and its mode);
//! * the value a scenario writes in `security.signature` or `security.kem`, which the
//!   loader validates against [`SIGNATURE_IDS`] and [`KEM_IDS`], and which the settings
//!   window offers because the published schema is built from the same lists.
//!
//! # Adding a scheme
//!
//! `docs/design/extending-crypto.md` is the guide. In short: copy
//! [`template`] to a new module, fill in the `const`, declare the module below and add one
//! line to [`SIGNATURE_SCHEMES`] (or [`KEM_SCHEMES`]). If the scheme needs a primitive the
//! catalogue does not have, its module also declares the primitive's sizes and cited cost
//! rows in a `fn` named by [`SignatureScheme::defines`], and the catalogue picks it up.
//! Nothing in the engine, the node or the protocols changes. [`hybrid_mldsa65`] is the
//! worked example, made from the template.
//!
//! # What a mode means
//!
//! * [`SchemeMode::Classical`] — an elliptic-curve signature, butterfly key expansion
//!   available, broken by a quantum adversary.
//! * [`SchemeMode::Hybrid`] — the classical signature with a post-quantum one
//!   concatenated (05-protocols.md §5.3, "raw concat"); secure if either half is; both
//!   are made and both checked.
//! * [`SchemeMode::PostQuantum`] — the post-quantum signature alone. The certificate's
//!   verification key and its issuer's signature are the post-quantum ones, so the
//!   certificate *replaces* rather than adds its classical fields.
//!
//! In both post-quantum modes there is no butterfly: lattice and hash-based schemes have
//! no key addition, so a device generates one key pair per pseudonym certificate
//! (05-protocols.md §5.3).

use std::sync::LazyLock;

use serde::Serialize;

use crate::primitive::{PrimitiveCatalogue, PrimitiveDescriptor, PrimitiveId, PrimitiveOpKind};

pub mod classical;
pub mod hybrid;
pub mod hybrid_mldsa65;
pub mod kem;
pub mod post_quantum;
pub mod template;

/// What kind of protection a scheme gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SchemeMode {
    /// Elliptic-curve only.
    Classical,
    /// Classical and post-quantum, concatenated.
    Hybrid,
    /// Post-quantum only.
    PostQuantum,
}

impl SchemeMode {
    /// The mode's stable name, as metrics and recordings carry it.
    pub const fn as_str(self) -> &'static str {
        match self {
            SchemeMode::Classical => "classical",
            SchemeMode::Hybrid => "hybrid",
            SchemeMode::PostQuantum => "post-quantum",
        }
    }
}

/// One primitive inside a scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Part {
    /// The primitive, whose sizes and cost anchors the catalogue holds.
    pub primitive: PrimitiveId,
    /// The prefix of the hardware-profile operation ids this part is charged as: a profile
    /// publishes `<prefix>-sign`, `<prefix>-verify` and `<prefix>-keygen`.
    pub op_prefix: &'static str,
    /// Whether this part is the post-quantum one.
    pub post_quantum: bool,
}

/// A signature scheme: what `security.signature` names.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SignatureScheme {
    /// The scenario's name for it, e.g. `ml-dsa-44`. Stable: recordings carry it.
    pub id: &'static str,
    /// A plain name for the settings window.
    pub label: &'static str,
    /// Classical, hybrid or post-quantum.
    pub mode: SchemeMode,
    /// What a signature is made of, in wire order. Sizes add; operation costs add.
    pub parts: &'static [Part],
    /// The standard (or draft) the scheme follows.
    pub standard: &'static str,
    /// Whether `real` crypto mode can execute it. Only ECDSA P-256 can today.
    pub real_crypto: bool,
    /// A primitive this scheme introduces to the catalogue, for a scheme whose primitive
    /// is not one of 04-models.md §9.4's. `None` for every built-in scheme.
    #[serde(skip)]
    pub defines: Option<fn() -> PrimitiveDescriptor>,
    /// Anything a reader of a result should know.
    pub notes: &'static str,
}

/// The classical baseline the node encodes: an ECDSA P-256 signature as carried in IEEE
/// 1609.2 (66 B: choice tag, `rSig` as a compressed point, `sSig`).
pub const BASELINE_SIG_OCTETS: u32 = 66;
/// The baseline certificate's verification key: a compressed P-256 point (SEC 1 §2.3.3).
pub const BASELINE_KEY_OCTETS: u32 = 33;

impl SignatureScheme {
    /// The classical part, if the scheme has one.
    pub fn classical_part(&self) -> Option<&'static Part> {
        self.parts_static().iter().find(|p| !p.post_quantum)
    }

    /// The post-quantum part, if the scheme has one.
    pub fn post_quantum_part(&self) -> Option<&'static Part> {
        self.parts_static().iter().find(|p| p.post_quantum)
    }

    fn parts_static(&self) -> &'static [Part] {
        self.parts
    }

    /// The octets one signature occupies in the envelope: the parts' encoded signatures,
    /// concatenated.
    pub fn sig_octets(&self) -> u32 {
        let c = PrimitiveCatalogue::standard();
        self.parts
            .iter()
            .filter_map(|p| c.get(p.primitive))
            .map(PrimitiveDescriptor::envelope_sig_bytes)
            .sum()
    }

    /// The octets of the verification key a certificate carries.
    pub fn key_octets(&self) -> u32 {
        let c = PrimitiveCatalogue::standard();
        self.parts
            .iter()
            .filter_map(|p| c.get(p.primitive))
            .map(|d| d.pk_bytes)
            .sum()
    }

    /// Octets this scheme adds over the P-256 SPDU the node encodes:
    /// `(per signature, per attached certificate)`.
    ///
    /// A certificate carries the holder's key and its issuer's signature, so it grows by
    /// both differences. Saturating: no registered scheme is smaller than P-256.
    pub fn extra_octets(&self) -> (u32, u32) {
        let per_sig = self.sig_octets().saturating_sub(BASELINE_SIG_OCTETS);
        let per_key = self.key_octets().saturating_sub(BASELINE_KEY_OCTETS);
        (per_sig, per_key + per_sig)
    }

    /// The NIST security category the scheme gives against a quantum adversary: the
    /// post-quantum part's, and 0 for a classical scheme.
    pub fn quantum_level(&self) -> u8 {
        let c = PrimitiveCatalogue::standard();
        self.post_quantum_part()
            .and_then(|p| c.get(p.primitive))
            .map_or(0, |d| d.security_level)
    }

    /// Whether a device can use butterfly key expansion under this scheme: only when
    /// every part is elliptic-curve.
    pub fn butterfly(&self) -> bool {
        self.mode == SchemeMode::Classical
    }

    /// The hardware-profile operation a node signs with.
    pub fn sign_op(&self) -> &'static str {
        ops_of(self.id).map_or("ecdsa-p256-sign", |o| o.sign.as_str())
    }

    /// The hardware-profile operation a node verifies with.
    pub fn verify_op(&self) -> &'static str {
        ops_of(self.id).map_or("ecdsa-p256-verify", |o| o.verify.as_str())
    }

    /// The hardware-profile operation a node generates a key pair with.
    pub fn keygen_op(&self) -> &'static str {
        ops_of(self.id).map_or("ecdsa-p256-keygen", |o| o.keygen.as_str())
    }
}

/// A key-encapsulation scheme: what `security.kem` names, used wherever the credential
/// system encrypts to an authority or to a device (the PCA's request, the device's batch).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct KemScheme {
    /// The scenario's name for it, e.g. `ml-kem-768`.
    pub id: &'static str,
    /// A plain name for the settings window.
    pub label: &'static str,
    /// Classical, hybrid or post-quantum.
    pub mode: SchemeMode,
    /// Whether the ECIES P-256 half is present (IEEE 1609.2 `EciesP256EncryptedKey`).
    pub ecies: bool,
    /// The post-quantum KEM, if any.
    pub post_quantum: Option<PrimitiveId>,
    /// The standard it follows.
    pub standard: &'static str,
    /// Anything a reader of a result should know.
    pub notes: &'static str,
}

/// ECIES's ephemeral public key in a 1609.2 `EciesP256EncryptedKey`: a compressed point.
pub const ECIES_EPHEMERAL_OCTETS: u32 = 33;
/// The wrapped AES-128 key and its tag in a 1609.2 `EciesP256EncryptedKey`: 16 + 16.
pub const WRAPPED_KEY_AND_TAG_OCTETS: u32 = 32;

impl KemScheme {
    /// The octets of one encrypted-key structure: each half's encapsulation (the ECIES
    /// ephemeral point, the ML-KEM ciphertext), then the wrapped AES key and its tag. 65
    /// for ECIES alone, which is `EciesP256EncryptedKey` (`Ieee1609Dot2BaseTypes.asn`).
    /// The post-quantum layouts follow the same shape and are derived, not standardised:
    /// no 1609.2 or TS 103 097 structure for ML-KEM exists yet.
    pub fn wrapped_key_octets(&self) -> u32 {
        let pq = self
            .post_quantum
            .and_then(|p| PrimitiveCatalogue::standard().get(p))
            .map_or(0, |d| d.sig_bytes.nominal());
        let ecies = if self.ecies {
            ECIES_EPHEMERAL_OCTETS
        } else {
            0
        };
        ecies + pq + WRAPPED_KEY_AND_TAG_OCTETS
    }
}

// --------------------------------------------------------------------------------------
// The registry. Adding a scheme is one line here and one module above.
// --------------------------------------------------------------------------------------

/// Every signature scheme a scenario can name, in the order the settings window offers
/// them.
pub const SIGNATURE_SCHEMES: &[&SignatureScheme] = &[
    &classical::ECDSA_P256,
    &classical::ECDSA_BRAINPOOL_P256R1,
    &classical::ECDSA_P384,
    &hybrid::FALCON512_ECDSA_P256,
    &hybrid::MLDSA44_ECDSA_P256,
    &hybrid_mldsa65::SCHEME,
    &post_quantum::ML_DSA_44,
    &post_quantum::ML_DSA_65,
    &post_quantum::ML_DSA_87,
    &post_quantum::FALCON_512,
    &post_quantum::FALCON_1024,
    &post_quantum::SLH_DSA_SHA2_128S,
    &post_quantum::SLH_DSA_SHA2_128F,
];

/// Every key-encapsulation scheme a scenario can name.
pub const KEM_SCHEMES: &[&KemScheme] = &[
    &kem::ECIES_P256,
    &kem::ML_KEM_512,
    &kem::ML_KEM_768,
    &kem::ML_KEM_1024,
    &kem::ECIES_P256_ML_KEM_768,
];

const N_SIG: usize = SIGNATURE_SCHEMES.len();
const N_KEM: usize = KEM_SCHEMES.len();

/// The values `security.signature` accepts — built from [`SIGNATURE_SCHEMES`] at compile
/// time, so the loader, the published schema and the settings window cannot disagree with
/// the registry.
pub const SIGNATURE_IDS: [&str; N_SIG] = {
    let mut out = [""; N_SIG];
    let mut i = 0;
    while i < N_SIG {
        out[i] = SIGNATURE_SCHEMES[i].id;
        i += 1;
    }
    out
};

/// The values `security.kem` accepts.
pub const KEM_IDS: [&str; N_KEM] = {
    let mut out = [""; N_KEM];
    let mut i = 0;
    while i < N_KEM {
        out[i] = KEM_SCHEMES[i].id;
        i += 1;
    }
    out
};

/// The signature scheme a `security.signature` value names.
pub fn signature(id: &str) -> Option<&'static SignatureScheme> {
    SIGNATURE_SCHEMES.iter().copied().find(|s| s.id == id)
}

/// The KEM a `security.kem` value names.
pub fn kem(id: &str) -> Option<&'static KemScheme> {
    KEM_SCHEMES.iter().copied().find(|s| s.id == id)
}

/// Every primitive a registered scheme introduces, for the catalogue.
pub fn contributed_primitives() -> Vec<PrimitiveDescriptor> {
    SIGNATURE_SCHEMES
        .iter()
        .filter_map(|s| s.defines.map(|f| f()))
        .collect()
}

struct Ops {
    sign: String,
    verify: String,
    keygen: String,
    parts: Vec<[String; 3]>,
}

static OPS: LazyLock<Vec<(&'static str, Ops)>> = LazyLock::new(|| {
    SIGNATURE_SCHEMES
        .iter()
        .map(|s| {
            let parts: Vec<[String; 3]> = s
                .parts
                .iter()
                .map(|p| {
                    [
                        format!("{}-sign", p.op_prefix),
                        format!("{}-verify", p.op_prefix),
                        format!("{}-keygen", p.op_prefix),
                    ]
                })
                .collect();
            let ops = if let [only] = parts.as_slice() {
                Ops {
                    sign: only[0].clone(),
                    verify: only[1].clone(),
                    keygen: only[2].clone(),
                    parts,
                }
            } else {
                Ops {
                    sign: format!("{}-sign", s.id),
                    verify: format!("{}-verify", s.id),
                    keygen: format!("{}-keygen", s.id),
                    parts,
                }
            };
            (s.id, ops)
        })
        .collect()
});

fn ops_of(id: &str) -> Option<&'static Ops> {
    OPS.iter().find(|(s, _)| *s == id).map(|(_, o)| o)
}

/// The component operations of a compound hardware-profile operation — a hybrid's
/// `hybrid-mldsa44-ecdsa-p256-sign` is `ml-dsa-44-sign` and `ecdsa-p256-sign` — or `None`
/// for an operation that is not compound. A profile charges a compound operation as the
/// sum of its components, and needs every one of them.
pub fn op_components(op: &str) -> Option<Vec<&'static str>> {
    for (_, o) in OPS.iter() {
        if o.parts.len() < 2 {
            continue;
        }
        let k = if op == o.sign {
            0
        } else if op == o.verify {
            1
        } else if op == o.keygen {
            2
        } else {
            continue;
        };
        return Some(o.parts.iter().map(|p| p[k].as_str()).collect());
    }
    None
}

/// The catalogue primitive and operation kind a single hardware-profile operation id
/// stands for, e.g. `ml-dsa-65-verify` → (`primitive/ml-dsa-65`, verify). How a hardware
/// profile with no published figure for an operation falls back to the catalogue anchor
/// it names. A part whose `op_prefix` is its own primitive's name wins over one charged
/// as another's (brainpool is charged as `ecdsa-p256`).
pub fn primitive_of_op(op: &str) -> Option<(PrimitiveId, PrimitiveOpKind)> {
    let (prefix, kind) = if let Some(p) = op.strip_suffix("-sign") {
        (p, PrimitiveOpKind::Sign)
    } else if let Some(p) = op.strip_suffix("-verify") {
        (p, PrimitiveOpKind::Verify)
    } else if let Some(p) = op.strip_suffix("-keygen") {
        (p, PrimitiveOpKind::KeyGen)
    } else {
        return None;
    };
    let mut fallback = None;
    for s in SIGNATURE_SCHEMES {
        for p in s.parts {
            if p.op_prefix != prefix {
                continue;
            }
            let name = p.primitive.as_str().trim_start_matches("primitive/");
            if name.starts_with(prefix) {
                return Some((p.primitive, kind));
            }
            fallback.get_or_insert((p.primitive, kind));
        }
    }
    fallback
}

/// The first of `candidates` on which `primitive`'s `kind` has a *published* figure, else
/// the first on which it has a derived one — how a role picks which of a device's
/// libraries prices a primitive (the Cohda MK6 publishes ML-DSA under Botan and Falcon
/// under liboqs).
pub fn priced_on(
    primitive: PrimitiveId,
    kind: PrimitiveOpKind,
    candidates: &[&'static str],
) -> Option<&'static str> {
    let c = PrimitiveCatalogue::standard();
    let d = c.get(primitive)?;
    candidates
        .iter()
        .copied()
        .find(|p| d.cost_duration(kind, p, c).is_some() && !d.cost_is_derived(kind, p, c))
        .or_else(|| {
            candidates
                .iter()
                .copied()
                .find(|p| d.cost_duration(kind, p, c).is_some())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::profiles;

    #[test]
    fn ids_are_unique_and_match_the_registry_order() {
        let mut seen = std::collections::BTreeSet::new();
        for (i, s) in SIGNATURE_SCHEMES.iter().enumerate() {
            assert_eq!(SIGNATURE_IDS[i], s.id);
            assert!(seen.insert(s.id), "duplicate scheme id {}", s.id);
        }
        for (i, k) in KEM_SCHEMES.iter().enumerate() {
            assert_eq!(KEM_IDS[i], k.id);
            assert!(seen.insert(k.id), "duplicate scheme id {}", k.id);
        }
    }

    /// Every part names a primitive the catalogue has, and every mode matches its parts.
    #[test]
    fn every_scheme_is_well_formed() {
        let c = PrimitiveCatalogue::standard();
        for s in SIGNATURE_SCHEMES {
            assert!(!s.parts.is_empty(), "{}", s.id);
            for p in s.parts {
                assert!(c.get(p.primitive).is_some(), "{}: {}", s.id, p.primitive);
            }
            let pq = s.parts.iter().filter(|p| p.post_quantum).count();
            let cl = s.parts.len() - pq;
            let expected = match (cl > 0, pq > 0) {
                (true, false) => SchemeMode::Classical,
                (true, true) => SchemeMode::Hybrid,
                (false, true) => SchemeMode::PostQuantum,
                (false, false) => unreachable!(),
            };
            assert_eq!(s.mode, expected, "{}", s.id);
        }
    }

    /// The octets the engine's previous hand-written table carried, now derived.
    #[test]
    fn the_derived_octets_reproduce_the_published_sizes() {
        let o = |id| signature(id).expect("registered").extra_octets();
        assert_eq!(o("ecdsa-p256"), (0, 0));
        assert_eq!(o("ecdsa-brainpoolp256r1"), (0, 0));
        assert_eq!(o("ecdsa-p384"), (32, 16 + 32));
        assert_eq!(o("hybrid-falcon512-ecdsa-p256"), (666, 897 + 666));
        assert_eq!(o("hybrid-mldsa44-ecdsa-p256"), (2_420, 1_312 + 2_420));
        // Pure ML-DSA-44 replaces the 66-byte ECDSA signature and the 33-byte key
        // [FIPS 204 Table 2: pk 1,312, sig 2,420].
        assert_eq!(o("ml-dsa-44"), (2_420 - 66, (1_312 - 33) + (2_420 - 66)));
        // SLH-DSA-SHA2-128s: 7,856-byte signature, 32-byte key [FIPS 205 Table 2]; its
        // key is a byte smaller than the compressed point, which saturates to zero.
        assert_eq!(o("slh-dsa-sha2-128s"), (7_856 - 66, 7_856 - 66));
        assert_eq!(o("ml-dsa-87"), (4_627 - 66, (2_592 - 33) + (4_627 - 66)));
    }

    #[test]
    fn a_kem_wrap_is_ecies_shaped() {
        assert_eq!(kem("ecies-p256").expect("kem").wrapped_key_octets(), 65);
        // ML-KEM-768 ciphertext 1,088 B [FIPS 203 Table 3] + wrapped key and tag.
        assert_eq!(
            kem("ml-kem-768").expect("kem").wrapped_key_octets(),
            1_088 + 32
        );
        assert_eq!(
            kem("hybrid-ecies-p256-ml-kem-768")
                .expect("kem")
                .wrapped_key_octets(),
            33 + 1_088 + 32
        );
    }

    #[test]
    fn compound_ops_name_their_components() {
        let h = signature("hybrid-mldsa44-ecdsa-p256").expect("registered");
        assert_eq!(h.sign_op(), "hybrid-mldsa44-ecdsa-p256-sign");
        assert_eq!(
            op_components(h.verify_op()).expect("compound"),
            vec!["ecdsa-p256-verify", "ml-dsa-44-verify"]
        );
        let p = signature("ml-dsa-65").expect("registered");
        assert_eq!(p.sign_op(), "ml-dsa-65-sign");
        assert!(op_components(p.sign_op()).is_none());
        assert_eq!(
            primitive_of_op("ml-dsa-65-verify"),
            Some((PrimitiveId::ML_DSA_65, PrimitiveOpKind::Verify))
        );
        assert_eq!(
            primitive_of_op("ecdsa-p256-sign"),
            Some((PrimitiveId::ECDSA_P256_SHA256, PrimitiveOpKind::Sign))
        );
    }

    /// Every post-quantum part of every scheme has a sign and a verify time on the
    /// profiles the credential system charges — published or derived, never invented.
    #[test]
    fn every_post_quantum_part_is_priced_on_the_v2x_profiles() {
        let c = PrimitiveCatalogue::standard();
        for s in SIGNATURE_SCHEMES {
            let Some(p) = s.post_quantum_part() else {
                continue;
            };
            let d = c.get(p.primitive).expect("present");
            for op in [PrimitiveOpKind::Sign, PrimitiveOpKind::Verify] {
                // The backend anchor, and whichever of the Cohda unit's two libraries
                // prices it.
                let t = d.cost_duration(op, profiles::CORTEX_A76_PI5_LIBOQS, c);
                assert!(
                    t.is_some_and(|t| t.as_nanos() > 0),
                    "{} {op:?} Pi 5: {t:?}",
                    s.id
                );
                let mk6 = priced_on(
                    p.primitive,
                    op,
                    &[profiles::COHDA_MK6_BOTAN, profiles::COHDA_MK6_LIBOQS],
                );
                assert!(mk6.is_some(), "{} {op:?} on a Cohda MK6", s.id);
            }
        }
    }

    /// The derivation is the documented ratio and is reported as derived.
    #[test]
    fn a_ratio_derived_cost_is_the_cycle_ratio_and_says_so() {
        let c = PrimitiveCatalogue::standard();
        let m65 = c.get(PrimitiveId::ML_DSA_65).expect("present");
        let m44 = c.get(PrimitiveId::ML_DSA_44).expect("present");
        let p = profiles::CORTEX_A76_PI5_LIBOQS;
        let v44 = m44
            .cost_duration(PrimitiveOpKind::Verify, p, c)
            .expect("published");
        let v65 = m65
            .cost_duration(PrimitiveOpKind::Verify, p, c)
            .expect("derived");
        // pqm4 m4f verify cycles: 2,415,944 / 1,421,623.
        let expected = u128::from(v44.as_nanos()) * 2_415_944 / 1_421_623;
        assert_eq!(u128::from(v65.as_nanos()), expected);
        assert!(m65.cost_is_derived(PrimitiveOpKind::Verify, p, c));
        assert!(!m44.cost_is_derived(PrimitiveOpKind::Verify, p, c));
        // ML-KEM-768 decapsulation on a Pi 5 is published (Table 8: 9,614.3/s).
        let k = c.get(PrimitiveId::ML_KEM_768).expect("present");
        let dec = k
            .cost_duration(PrimitiveOpKind::Verify, p, c)
            .expect("published");
        assert_eq!(dec.as_nanos(), 104_012);
        assert!(!k.cost_is_derived(PrimitiveOpKind::Verify, p, c));
    }
}
