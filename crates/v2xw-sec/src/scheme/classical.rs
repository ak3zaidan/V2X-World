//! The classical schemes IEEE 1609.2's `Signature` CHOICE admits.

use super::{Part, SchemeMode, SignatureScheme};
use crate::primitive::PrimitiveId;

/// ECDSA over NIST P-256 — what deployed 1609.2 and TS 103 097 stacks sign with.
pub const ECDSA_P256: SignatureScheme = SignatureScheme {
    id: "ecdsa-p256",
    label: "ECDSA P-256 (deployed today)",
    mode: SchemeMode::Classical,
    parts: &[Part {
        primitive: PrimitiveId::ECDSA_P256_SHA256,
        op_prefix: "ecdsa-p256",
        post_quantum: false,
    }],
    standard: "IEEE 1609.2-2016 §6.3.29 (EcdsaP256Signature); FIPS 186-4",
    real_crypto: true,
    defines: None,
    notes: "The measured envelope (04-models.md §9.1) is built around this signature.",
};

/// ECDSA over brainpoolP256r1.
pub const ECDSA_BRAINPOOL_P256R1: SignatureScheme = SignatureScheme {
    id: "ecdsa-brainpoolp256r1",
    label: "ECDSA brainpoolP256r1",
    mode: SchemeMode::Classical,
    parts: &[Part {
        primitive: PrimitiveId::ECDSA_BRAINPOOL_P256R1,
        // No shipped profile publishes a brainpool figure; the curves have the same field
        // size and operation count, so it is charged as P-256.
        op_prefix: "ecdsa-p256",
        post_quantum: false,
    }],
    standard: "IEEE 1609.2-2016 §6.3.29; RFC 5639",
    real_crypto: false,
    defines: None,
    notes: "Charged the P-256 time: no shipped hardware profile publishes a brainpool figure.",
};

/// ECDSA over NIST P-384.
pub const ECDSA_P384: SignatureScheme = SignatureScheme {
    id: "ecdsa-p384",
    label: "ECDSA P-384",
    mode: SchemeMode::Classical,
    parts: &[Part {
        primitive: PrimitiveId::ECDSA_P384,
        // Its own operations: a profile that publishes P-384 figures charges them, and
        // one that does not charges P-256 (`v2xw_node::profile::HardwareProfile::priced_op`).
        op_prefix: "ecdsa-p384",
        post_quantum: false,
    }],
    standard: "IEEE 1609.2-2016 §6.3.29 (EcdsaP384Signature); SEC 1 §2.3.3",
    real_crypto: false,
    defines: None,
    notes: "Sizes are P-384's (98-byte signature, 49-byte key); time is the profile's P-384 figure, or P-256 where it publishes none.",
};
