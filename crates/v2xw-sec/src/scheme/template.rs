//! TEMPLATE — copy this file to add a signature scheme. It is compiled (so it cannot rot)
//! and deliberately **not** registered.
//!
//! Steps (the full guide, with a worked example, is `docs/design/extending-crypto.md`):
//!
//! 1. Copy this file to `crates/v2xw-sec/src/scheme/<your_scheme>.rs` and rename the
//!    `const`.
//! 2. Fill in every field below. `id` is what a scenario writes in `security.signature`
//!    and what recordings carry: lower-case, stable, unique.
//! 3. List the scheme's parts in wire order. Each part names a primitive from
//!    `crate::primitive` and the prefix of the hardware-profile operations it is charged
//!    as (`<prefix>-sign`, `<prefix>-verify`, `<prefix>-keygen`).
//! 4. If a primitive is new to the catalogue, write a `fn` returning its
//!    `PrimitiveDescriptor` — sizes from the standard, one `CostRow` per published
//!    benchmark, each with its citation — and set `defines: Some(that_fn)`. The commented
//!    block at the end of this file is the shape. If a profile publishes nothing for it,
//!    give it a `cost_ratio` to a sibling with a shared reference platform instead of
//!    inventing a number.
//! 5. In `scheme/mod.rs`: `pub mod <your_scheme>;` and one line in `SIGNATURE_SCHEMES`.
//! 6. Run `cargo test -p v2xw-sec scheme` — the registry tests check the id is unique,
//!    the mode matches the parts, every part is in the catalogue and every post-quantum
//!    part is priced on the V2X profiles.
//!
//! That is all. The scenario loader, the published schema (and so the settings window),
//! the node's signing cost, the over-the-air octets, the fragmenter, the credential
//! system's sizes and charges, and the run's recorded scheme all read the registry.

use super::{Part, SchemeMode, SignatureScheme};
use crate::primitive::PrimitiveId;

/// The template. Not registered: copy it.
pub const TEMPLATE: SignatureScheme = SignatureScheme {
    // What a scenario writes in `security.signature`.
    id: "template-copy-me",
    // What the settings window shows.
    label: "Template (copy me)",
    // Classical, Hybrid or PostQuantum — must match the parts (a registry test checks).
    mode: SchemeMode::PostQuantum,
    // Wire order. Sizes and operation costs of the parts add.
    parts: &[Part {
        primitive: PrimitiveId::ML_DSA_44,
        op_prefix: "ml-dsa-44",
        post_quantum: true,
    }],
    // The standard or draft, with its clause.
    standard: "FIPS 204 (2024)",
    // Only true if `crate::crypto::Real` can execute every part.
    real_crypto: false,
    // `Some(fn)` if a part's primitive is new to the catalogue (step 4).
    defines: None,
    // What a reader of a result should know: derived figures, drafts, caveats.
    notes: "Template only.",
};

// Step 4, for a primitive the catalogue does not have — the shape, to adapt:
//
// pub const MY_PRIMITIVE: PrimitiveId = PrimitiveId("primitive/my-scheme-1");
//
// pub fn my_primitive() -> crate::primitive::PrimitiveDescriptor {
//     use crate::primitive::*;
//     use v2xw_core::card::{Source, SourceKind};
//     PrimitiveDescriptor {
//         id: MY_PRIMITIVE,
//         family: PrimitiveFamily::Signature,
//         pk_bytes: /* from the specification, cited */,
//         pk_bytes_uncompressed: None,
//         sk_bytes: /* … */,
//         sig_bytes: SizeSpec::Fixed(/* … */),
//         sig_bytes_encoded: None,
//         shared_secret_bytes: None,
//         cert_bytes: None,
//         security_level: /* NIST category */,
//         cost: CostTable::new(vec![/* one CostRow per published benchmark */]),
//         cost_components: Vec::new(),
//         cost_proxy: None,
//         cost_ratio: None, // or Some(CostRatio { base, reference }) for derived rows
//         sources: vec![Source::new(SourceKind::Standard, "…")],
//         notes: vec!["…"],
//     }
// }
//
// …and in the scheme: `defines: Some(my_primitive)`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_is_not_registered() {
        assert!(super::super::signature(TEMPLATE.id).is_none());
    }
}
