//! Pure post-quantum schemes, at the NIST parameter sets: the post-quantum signature alone,
//! and certificates whose key and issuer signature are post-quantum.
//!
//! Sizes are the standards' (FIPS 204 Table 2, FIPS 205 Table 2, PQClean's padded Falcon);
//! costs are the catalogue's cited anchors, derived by cycle ratio where a profile
//! publishes nothing for a parameter set (`v2xw_sec::primitive::CostRatio`).

use super::{Part, SchemeMode, SignatureScheme};
use crate::primitive::PrimitiveId;

const fn pq(primitive: PrimitiveId, op_prefix: &'static str) -> Part {
    Part {
        primitive,
        op_prefix,
        post_quantum: true,
    }
}

/// ML-DSA-44, NIST category 2.
pub const ML_DSA_44: SignatureScheme = SignatureScheme {
    id: "ml-dsa-44",
    label: "Post-quantum: ML-DSA-44",
    mode: SchemeMode::PostQuantum,
    parts: &[pq(PrimitiveId::ML_DSA_44, "ml-dsa-44")],
    standard: "FIPS 204 (2024), ML-DSA-44",
    real_crypto: false,
    defines: None,
    notes: "pk 1,312 B, sig 2,420 B [FIPS 204 Table 2].",
};

/// ML-DSA-65, NIST category 3.
pub const ML_DSA_65: SignatureScheme = SignatureScheme {
    id: "ml-dsa-65",
    label: "Post-quantum: ML-DSA-65",
    mode: SchemeMode::PostQuantum,
    parts: &[pq(PrimitiveId::ML_DSA_65, "ml-dsa-65")],
    standard: "FIPS 204 (2024), ML-DSA-65",
    real_crypto: false,
    defines: None,
    notes: "pk 1,952 B, sig 3,309 B [FIPS 204 Table 2]. Cortex-A and Cohda times are derived \
            from ML-DSA-44 by the pqm4 cycle ratio.",
};

/// ML-DSA-87, NIST category 5.
pub const ML_DSA_87: SignatureScheme = SignatureScheme {
    id: "ml-dsa-87",
    label: "Post-quantum: ML-DSA-87",
    mode: SchemeMode::PostQuantum,
    parts: &[pq(PrimitiveId::ML_DSA_87, "ml-dsa-87")],
    standard: "FIPS 204 (2024), ML-DSA-87",
    real_crypto: false,
    defines: None,
    notes: "pk 2,592 B, sig 4,627 B [FIPS 204 Table 2]. Times derived as for ML-DSA-65.",
};

/// Falcon-512 (FN-DSA), padded signature, NIST category 1.
pub const FALCON_512: SignatureScheme = SignatureScheme {
    id: "falcon-512",
    label: "Post-quantum: Falcon-512 (FN-DSA)",
    mode: SchemeMode::PostQuantum,
    parts: &[pq(PrimitiveId::FALCON_512, "falcon-512")],
    standard: "Falcon specification v1.2; FN-DSA is draft FIPS 206 (status UNVERIFIED)",
    real_crypto: false,
    defines: None,
    notes: "pk 897 B, padded sig 666 B [PQClean falcon-padded-512]. Key generation is slow \
            (tens of ms on a Cortex-A72), which matters with no butterfly.",
};

/// Falcon-1024 (FN-DSA), padded signature, NIST category 5.
pub const FALCON_1024: SignatureScheme = SignatureScheme {
    id: "falcon-1024",
    label: "Post-quantum: Falcon-1024 (FN-DSA)",
    mode: SchemeMode::PostQuantum,
    parts: &[pq(PrimitiveId::FALCON_1024, "falcon-1024")],
    standard: "Falcon specification v1.2; draft FIPS 206",
    real_crypto: false,
    defines: None,
    notes: "pk 1,793 B, padded sig 1,280 B [PQClean falcon-padded-1024]. Cortex-A and Cohda \
            times derived from Falcon-512 by the Cortex-M7 cycle ratio.",
};

/// SLH-DSA-SHA2-128s, NIST category 1, small signatures and slow signing.
pub const SLH_DSA_SHA2_128S: SignatureScheme = SignatureScheme {
    id: "slh-dsa-sha2-128s",
    label: "Post-quantum: SLH-DSA-SHA2-128s",
    mode: SchemeMode::PostQuantum,
    parts: &[pq(PrimitiveId::SLH_DSA_SHA2_128S, "slh-dsa-sha2-128s")],
    standard: "FIPS 205 (2024), SLH-DSA-SHA2-128s",
    real_crypto: false,
    defines: None,
    notes: "pk 32 B, sig 7,856 B [FIPS 205 Table 2]; hash-based, so its security rests on \
            SHA-2 alone. Signing takes about a second on a vehicle-class core: an authority \
            scheme, not a V2V one.",
};

/// SLH-DSA-SHA2-128f, NIST category 1, fast signing and large signatures.
pub const SLH_DSA_SHA2_128F: SignatureScheme = SignatureScheme {
    id: "slh-dsa-sha2-128f",
    label: "Post-quantum: SLH-DSA-SHA2-128f",
    mode: SchemeMode::PostQuantum,
    parts: &[pq(PrimitiveId::SLH_DSA_SHA2_128F, "slh-dsa-sha2-128f")],
    standard: "FIPS 205 (2024), SLH-DSA-SHA2-128f",
    real_crypto: false,
    defines: None,
    notes: "pk 32 B, sig 17,088 B [FIPS 205 Table 2]: eight frames' worth of signature.",
};
