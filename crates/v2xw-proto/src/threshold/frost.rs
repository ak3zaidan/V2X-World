//! FROST — two-round threshold Schnorr signing, RFC 9591, ciphersuite FROST(P-256, SHA-256).
//!
//! Round structure and sizes, all from RFC 9591 with the P-256 ciphersuite's element size
//! `Ne = 33` and scalar size `Ns = 32` (§6.4):
//!
//! | Round | Coordinator → participant | Participant → coordinator | Participant computes |
//! |---|---|---|---|
//! | 1, commit (§5.1) | "send commitments for `B` messages" | `B` × (hiding + binding nonce commitment) = `B` × 66 | 2 base-point multiplications per message |
//! | 2, sign (§5.2) | the signing package: per message, its 32-byte digest and every signer's identifier (32) and commitments (66) | `B` × one signature share (32) | the group commitment, `s` multiplications per message |
//!
//! The coordinator then aggregates (§5.3), verifying every share first (§5.4: three
//! multiplications per share) so a participant that sends an invalid share is identified
//! — FROST is identifiable-abort when the coordinator verifies shares. The signature is
//! `R ‖ z`, 33 + 32 = 65 bytes, and verifies as a plain Schnorr signature under the group
//! key: **not** as an IEEE 1609.2 ECDSA signature. A PCA that issued FROST-signed
//! certificates would need verifiers that accept Schnorr — the reason threshold ECDSA
//! (`cggmp21`) exists here too.
//!
//! With preprocessing (RFC 9591 §5.1: commitments may be generated in advance and stored)
//! the online signing is round 2 alone.
//!
//! Key generation is not in RFC 9591 (its Appendix C uses a trusted dealer); the DKG here
//! is the Pedersen DKG with proofs of knowledge of Komlo and Goldberg, "FROST: Flexible
//! Round-Optimized Schnorr Threshold Signatures", SAC 2020, Figure 1: one broadcast of
//! `t + 1` coefficient commitments and a Schnorr proof of knowledge, then one private
//! share to every other participant. Refresh is Herzberg et al. (CRYPTO 1995): a fresh
//! sharing of zero with the same message shape.
//!
//! Compute is charged in P-256 scalar multiplications, against the ECQV descriptor whose
//! declared cost proxy in `v2xw-sec` is the ECDSA P-256 anchor — the same convention the
//! SCMS run uses for every scalar multiplication. Hashes are counted and charged zero, as
//! everywhere (no profile publishes a SHA-256 anchor).

use v2xw_sec::primitive::{PrimitiveId, PrimitiveOpKind};

use super::{
    Committee, Knowledge, ProtocolEntry, RoundMessage, RoundPlan, StarRound, ThresholdProtocol,
};
use crate::sizes::WireSize;
use crate::spec::OpDescriptor;

/// The registry id.
pub const ID: &str = "frost-p256";

/// The registry entry.
pub const ENTRY: ProtocolEntry = ProtocolEntry {
    id: ID,
    label: "FROST threshold Schnorr (RFC 9591, P-256)",
    build: |p| {
        Box::new(Frost {
            preprocessed: p.preprocessed,
        })
    },
};

/// `Ne`, a serialized P-256 element: RFC 9591 §6.4.
pub const NE: u32 = 33;
/// `Ns`, a serialized P-256 scalar: RFC 9591 §6.4.
pub const NS: u32 = 32;
/// One participant's commitment pair (hiding, binding): RFC 9591 §5.1.
pub const COMMITMENT_BYTES: u32 = 2 * NE;
/// The message each participant signs: the 32-byte SHA-256 digest of the to-be-signed
/// structure. **Assumption**: a real deployment might ship the whole TBS certificate so
/// each member can check policy itself, which would grow round 2's request.
pub const MESSAGE_BYTES: u32 = 32;
/// A batch count on the commit request: a `Uint32`, `Ieee1609Dot2BaseTypes.asn`.
const COUNT_BYTES: u32 = 4;

const RFC9591: &str = "RFC 9591 §5, §6.4 (FROST(P-256, SHA-256): Ne = 33, Ns = 32)";
const KG20: &str = "Komlo & Goldberg, SAC 2020, Figure 1 (Pedersen DKG with proofs of knowledge)";
const HJKY95: &str = "Herzberg et al., CRYPTO 1995: refresh is a new sharing of zero";

/// A P-256 scalar multiplication, charged as the SCMS run charges one.
pub(crate) const fn mults(n: u32) -> OpDescriptor {
    OpDescriptor::new(PrimitiveId::ECQV_P256, PrimitiveOpKind::Sign, n)
}

/// SHA-256 invocations: counted, charged zero (no profile publishes an anchor).
pub(crate) const fn hashes(n: u32) -> OpDescriptor {
    OpDescriptor::new(PrimitiveId::SHA_256, PrimitiveOpKind::Verify, n)
}

/// FROST(P-256, SHA-256).
#[derive(Debug, Clone, Copy, Default)]
pub struct Frost {
    /// Whether round 1's commitments come from a preprocessed pool.
    pub preprocessed: bool,
}

impl ThresholdProtocol for Frost {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        ENTRY.label
    }

    fn standard(&self) -> &'static str {
        RFC9591
    }

    fn sign_rounds(&self, signers: u16, batch: u32) -> Vec<StarRound> {
        let s = u32::from(signers);
        let mut rounds = Vec::new();
        if !self.preprocessed {
            rounds.push(StarRound {
                name: "frost-commit",
                request: WireSize::cited(COUNT_BYTES, "Ieee1609Dot2BaseTypes.asn Uint32"),
                response: WireSize::derived(
                    batch * COMMITMENT_BYTES,
                    "B · (hiding + binding commitment) = B · 2 · Ne",
                    RFC9591,
                ),
                member_ops: vec![mults(2 * batch)],
                member_time: None,
                coordinator_ops: Vec::new(),
                verifies_shares: false,
            });
        }
        rounds.push(StarRound {
            name: "frost-sign",
            request: WireSize::derived(
                batch * (MESSAGE_BYTES + s * (NS + COMMITMENT_BYTES)),
                "B · (message digest + s · (identifier Ns + commitments 2·Ne))",
                RFC9591,
            ),
            response: WireSize::derived(batch * NS, "B · signature share (Ns)", RFC9591),
            // Binding factors (s + 1 hashes), challenge (1 hash), the group commitment
            // (s multiplications) per message.
            member_ops: vec![mults(batch * s), hashes(batch * (s + 2))],
            member_time: None,
            // Group commitment (s) and share verification (3 per share) per message.
            coordinator_ops: vec![mults(batch * (s + 3 * s)), hashes(batch * (s + 2))],
            verifies_shares: true,
        });
        rounds
    }

    fn dkg_rounds(&self, committee: &Committee) -> Vec<RoundPlan> {
        let n = committee.n;
        let t1 = u32::from(committee.quorum());
        vec![
            RoundPlan {
                index: 1,
                messages: (0..n)
                    .map(|from| RoundMessage {
                        from,
                        to: None,
                        bytes: WireSize::derived(
                            t1 * NE + NE + NS,
                            "(t+1) coefficient commitments + proof of knowledge (R, μ)",
                            KG20,
                        ),
                    })
                    .collect(),
                compute: vec![mults(t1 + 1)],
                attempts: 1,
            },
            RoundPlan {
                index: 2,
                messages: (0..n)
                    .flat_map(|from| {
                        (0..n)
                            .filter(move |to| *to != from)
                            .map(move |to| RoundMessage {
                                from,
                                to: Some(to),
                                bytes: WireSize::derived(NS, "one secret share f_i(ℓ)", KG20),
                            })
                    })
                    .collect(),
                // Verify every other party's proof (2) and share against its commitments
                // (t + 1).
                compute: vec![mults(u32::from(n.saturating_sub(1)) * (2 + t1))],
                attempts: 1,
            },
        ]
    }

    fn refresh_rounds(&self, committee: &Committee) -> Vec<RoundPlan> {
        let n = committee.n;
        let t1 = u32::from(committee.quorum());
        vec![
            RoundPlan {
                index: 1,
                messages: (0..n)
                    .map(|from| RoundMessage {
                        from,
                        to: None,
                        bytes: WireSize::derived(
                            t1 * NE,
                            "(t+1) commitments to a sharing of zero",
                            HJKY95,
                        ),
                    })
                    .collect(),
                compute: vec![mults(t1)],
                attempts: 1,
            },
            RoundPlan {
                index: 2,
                messages: (0..n)
                    .flat_map(|from| {
                        (0..n)
                            .filter(move |to| *to != from)
                            .map(move |to| RoundMessage {
                                from,
                                to: Some(to),
                                bytes: WireSize::derived(NS, "one share of zero", HJKY95),
                            })
                    })
                    .collect(),
                compute: vec![mults(u32::from(n.saturating_sub(1)) * t1)],
                attempts: 1,
            },
        ]
    }

    fn signature_bytes(&self) -> WireSize {
        WireSize::derived(NE + NS, "R (Ne) ‖ z (Ns)", RFC9591)
    }

    fn verification_key_bytes(&self) -> WireSize {
        WireSize::cited(NE, "RFC 9591 §6.4: a compressed P-256 element")
    }

    fn verifies_as(&self) -> &'static str {
        "Schnorr over P-256 (RFC 9591) — not an IEEE 1609.2 signature"
    }

    fn identifiable_abort(&self) -> bool {
        true
    }

    fn knowledge(&self) -> Vec<Knowledge> {
        vec![
            Knowledge {
                party: "coordinator",
                learns: "the message, every signer's commitments and signature shares, and \
                         the group signature; no key share (RFC 9591 §5)",
            },
            Knowledge {
                party: "signer",
                learns: "the message and the commitment list; its own key share and nothing \
                         about another's",
            },
            Knowledge {
                party: "coalition of t or fewer",
                learns: "nothing about the group signing key beyond its public key \
                         (RFC 9591 §7); it cannot sign",
            },
            Knowledge {
                party: "verifier",
                learns: "a Schnorr signature under the group key; not which members signed",
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_rounds_or_one_preprocessed_with_the_rfc_sizes() {
        let f = Frost::default();
        let r = f.sign_rounds(3, 1);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].response.bytes(), 66);
        assert_eq!(r[1].response.bytes(), 32);
        // 32 + 3 · (32 + 66)
        assert_eq!(r[1].request.bytes(), 32 + 3 * 98);
        assert_eq!(f.signature_bytes().bytes(), 65);
        let p = Frost { preprocessed: true };
        assert_eq!(p.sign_rounds(3, 1).len(), 1);
        // A batch scales every per-message field.
        assert_eq!(f.sign_rounds(3, 20)[0].response.bytes(), 20 * 66);
    }
}
