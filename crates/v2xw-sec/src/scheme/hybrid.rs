//! Hybrid schemes: the ECDSA P-256 signature with a post-quantum one concatenated
//! (05-protocols.md §5.3, "raw concat"). No 1609.2 or TS 103 097 structure standardises
//! one yet; this is the construction every published hybrid-V2X proposal converges on.

use super::{Part, SchemeMode, SignatureScheme};
use crate::primitive::PrimitiveId;

/// The ECDSA P-256 half every hybrid here carries.
pub const ECDSA_HALF: Part = Part {
    primitive: PrimitiveId::ECDSA_P256_SHA256,
    op_prefix: "ecdsa-p256",
    post_quantum: false,
};

/// Falcon-512 (padded, 666 B) + ECDSA P-256.
pub const FALCON512_ECDSA_P256: SignatureScheme = SignatureScheme {
    id: "hybrid-falcon512-ecdsa-p256",
    label: "Hybrid: ECDSA P-256 + Falcon-512",
    mode: SchemeMode::Hybrid,
    parts: &[
        ECDSA_HALF,
        Part {
            primitive: PrimitiveId::FALCON_512,
            op_prefix: "falcon-512",
            post_quantum: true,
        },
    ],
    standard: "Falcon specification v1.2 / draft FIPS 206 (FN-DSA); PQClean falcon-padded-512",
    real_crypto: false,
    defines: None,
    notes: "Raw concatenation; the NDSS 2024 Partially-Hybrid fragmenter carries its certificate.",
};

/// ML-DSA-44 (2,420 B) + ECDSA P-256.
pub const MLDSA44_ECDSA_P256: SignatureScheme = SignatureScheme {
    id: "hybrid-mldsa44-ecdsa-p256",
    label: "Hybrid: ECDSA P-256 + ML-DSA-44",
    mode: SchemeMode::Hybrid,
    parts: &[
        ECDSA_HALF,
        Part {
            primitive: PrimitiveId::ML_DSA_44,
            op_prefix: "ml-dsa-44",
            post_quantum: true,
        },
    ],
    standard: "FIPS 204 (ML-DSA-44) + IEEE 1609.2 ECDSA P-256, concatenated",
    real_crypto: false,
    defines: None,
    notes: "Raw concatenation; draft-ietf-lamps-pq-composite-sigs encodes the pair in \
            1,377 / 2,492 B.",
};
