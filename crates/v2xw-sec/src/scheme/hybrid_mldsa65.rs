//! Worked example of `template.rs`: ECDSA P-256 + ML-DSA-65, added by copying the template,
//! filling it in and adding one registry line. `docs/design/extending-crypto.md` walks
//! through it, and `crates/v2xw-engine/tests/pq_threshold.rs` checks it is selectable end
//! to end — loaded from a scenario, offered by the published schema, charged on the node,
//! carried on the air and in the credential system.
//!
//! Sizes: ML-DSA-65 public key 1,952 B and signature 3,309 B [FIPS 204 Table 2],
//! concatenated after the 33-byte key and 66-byte signature of ECDSA P-256. Costs: the
//! sum of the two halves, the ML-DSA-65 half derived from ML-DSA-44 by the pqm4 Cortex-M4
//! cycle ratio on profiles that publish no ML-DSA-65 figure.

use super::hybrid::ECDSA_HALF;
use super::{Part, SchemeMode, SignatureScheme};
use crate::primitive::PrimitiveId;

/// ECDSA P-256 + ML-DSA-65.
pub const SCHEME: SignatureScheme = SignatureScheme {
    id: "hybrid-mldsa65-ecdsa-p256",
    label: "Hybrid: ECDSA P-256 + ML-DSA-65",
    mode: SchemeMode::Hybrid,
    parts: &[
        ECDSA_HALF,
        Part {
            primitive: PrimitiveId::ML_DSA_65,
            op_prefix: "ml-dsa-65",
            post_quantum: true,
        },
    ],
    standard: "FIPS 204 (ML-DSA-65) + IEEE 1609.2 ECDSA P-256, concatenated",
    real_crypto: false,
    defines: None,
    notes: "Raw concatenation (05-protocols.md §5.3). The ML-DSA-65 time on Cortex-A and \
            Cohda profiles is derived from ML-DSA-44 by the pqm4 cycle ratio.",
};
