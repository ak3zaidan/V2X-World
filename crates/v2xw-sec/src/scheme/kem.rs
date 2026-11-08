//! Key-encapsulation schemes for the credential system's encryption to an authority and to
//! a device: ECIES P-256 (IEEE 1609.2), ML-KEM (FIPS 203), and the two combined.

use super::{KemScheme, SchemeMode};
use crate::primitive::PrimitiveId;

/// ECIES over P-256 — IEEE 1609.2 `EciesP256EncryptedKey`, 65 B.
pub const ECIES_P256: KemScheme = KemScheme {
    id: "ecies-p256",
    label: "ECIES P-256 (deployed today)",
    mode: SchemeMode::Classical,
    ecies: true,
    post_quantum: None,
    standard: "IEEE 1609.2-2016 §5.3.5 (ECIES), Ieee1609Dot2BaseTypes.asn",
    notes: "33 B ephemeral point + 16 B wrapped AES-128 key + 16 B tag.",
};

/// ML-KEM-512, NIST category 1.
pub const ML_KEM_512: KemScheme = KemScheme {
    id: "ml-kem-512",
    label: "Post-quantum: ML-KEM-512",
    mode: SchemeMode::PostQuantum,
    ecies: false,
    post_quantum: Some(PrimitiveId::ML_KEM_512),
    standard: "FIPS 203 (2024), ML-KEM-512",
    notes: "ciphertext 768 B, encapsulation key 800 B [FIPS 203 Table 3].",
};

/// ML-KEM-768, NIST category 3.
pub const ML_KEM_768: KemScheme = KemScheme {
    id: "ml-kem-768",
    label: "Post-quantum: ML-KEM-768",
    mode: SchemeMode::PostQuantum,
    ecies: false,
    post_quantum: Some(PrimitiveId::ML_KEM_768),
    standard: "FIPS 203 (2024), ML-KEM-768",
    notes: "ciphertext 1,088 B, encapsulation key 1,184 B [FIPS 203 Table 3].",
};

/// ML-KEM-1024, NIST category 5.
pub const ML_KEM_1024: KemScheme = KemScheme {
    id: "ml-kem-1024",
    label: "Post-quantum: ML-KEM-1024",
    mode: SchemeMode::PostQuantum,
    ecies: false,
    post_quantum: Some(PrimitiveId::ML_KEM_1024),
    standard: "FIPS 203 (2024), ML-KEM-1024",
    notes: "ciphertext 1,568 B [FIPS 203 Table 3]; times derived from ML-KEM-768 by the \
            pqm4 cycle ratio.",
};

/// ECIES P-256 and ML-KEM-768 combined: the AES key is derived from both shared secrets.
pub const ECIES_P256_ML_KEM_768: KemScheme = KemScheme {
    id: "hybrid-ecies-p256-ml-kem-768",
    label: "Hybrid: ECIES P-256 + ML-KEM-768",
    mode: SchemeMode::Hybrid,
    ecies: true,
    post_quantum: Some(PrimitiveId::ML_KEM_768),
    standard: "FIPS 203 + IEEE 1609.2 ECIES; combiner shape as draft-ietf-tls-ecdhe-mlkem",
    notes: "Both encapsulations travel; no 1609.2 structure for it exists yet (derived).",
};
