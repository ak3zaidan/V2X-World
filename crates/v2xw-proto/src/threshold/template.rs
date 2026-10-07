//! TEMPLATE — copy this file to add an interactive multi-party protocol. Compiled, so it
//! cannot rot, and deliberately **not** registered.
//!
//! Steps (the full guide, with a worked example, is `docs/design/extending-crypto.md`):
//!
//! 1. Copy to `crates/v2xw-proto/src/threshold/<your_protocol>.rs`; rename `ID`, `ENTRY`
//!    and the struct.
//! 2. `sign_rounds(signers, batch)`: one [`StarRound`] per round of the online operation,
//!    in order. For each: the request the coordinator sends every selected participant,
//!    the response each sends back (payloads only — the session adds the envelope), what
//!    a participant computes (`member_ops` from the primitive catalogue, or
//!    `member_time` for a measured protocol-level time), what the coordinator computes
//!    once the round is in, and whether it checks each response there (`verifies_shares`,
//!    which is where a malicious participant is caught). Every size cites its source.
//!    `batch` is how many messages one session signs; scale per-message fields by it.
//! 3. `dkg_rounds` / `refresh_rounds`: the peer-to-peer setup rounds, for the report.
//! 4. `identifiable_abort`, `verifies_as`, `knowledge`: what the protocol guarantees.
//! 5. In `threshold/mod.rs`: `pub mod <your_protocol>;` and `<your_protocol>::ENTRY` in
//!    `THRESHOLD_PROTOCOLS`.
//! 6. `cargo test -p v2xw-proto threshold` checks the id is unique, it builds under its
//!    own id, and it has rounds and a knowledge statement.
//!
//! That is all: `security.threshold.protocol` accepts the id, the settings window offers
//! it, the SCMS runs its PCA, CRL Generator or linkage release on it through
//! [`super::session`], and the run's metrics count its sessions, rounds, bytes, retries
//! and failures.

use super::{Committee, Knowledge, ProtocolEntry, RoundPlan, StarRound, ThresholdProtocol};
use crate::sizes::WireSize;

/// The id a scenario names.
pub const ID: &str = "template-copy-me";

/// The registry entry — add it to `THRESHOLD_PROTOCOLS` in your copy.
pub const ENTRY: ProtocolEntry = ProtocolEntry {
    id: ID,
    label: "Template (copy me)",
    build: |_| Box::new(TemplateProtocol),
};

/// The template.
#[derive(Debug, Clone, Copy, Default)]
pub struct TemplateProtocol;

impl ThresholdProtocol for TemplateProtocol {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        ENTRY.label
    }

    fn standard(&self) -> &'static str {
        "the specification and clause"
    }

    fn sign_rounds(&self, _signers: u16, batch: u32) -> Vec<StarRound> {
        vec![StarRound {
            name: "template-round",
            request: WireSize::cited(batch * 32, "the specification: request size"),
            response: WireSize::cited(batch * 32, "the specification: response size"),
            member_ops: Vec::new(),
            member_time: None,
            coordinator_ops: Vec::new(),
            verifies_shares: true,
        }]
    }

    fn dkg_rounds(&self, _committee: &Committee) -> Vec<RoundPlan> {
        Vec::new()
    }

    fn refresh_rounds(&self, _committee: &Committee) -> Vec<RoundPlan> {
        Vec::new()
    }

    fn signature_bytes(&self) -> WireSize {
        WireSize::cited(64, "the specification: output size")
    }

    fn verification_key_bytes(&self) -> WireSize {
        WireSize::cited(33, "the specification: group key size")
    }

    fn verifies_as(&self) -> &'static str {
        "what a verifier checks the output as"
    }

    fn identifiable_abort(&self) -> bool {
        true
    }

    fn knowledge(&self) -> Vec<Knowledge> {
        vec![Knowledge {
            party: "coordinator",
            learns: "what the specification says it learns",
        }]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_template_is_not_registered() {
        assert!(!super::super::THRESHOLD_IDS.contains(&super::ID));
    }
}
