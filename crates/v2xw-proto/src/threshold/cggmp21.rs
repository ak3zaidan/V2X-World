//! Threshold ECDSA — Canetti, Gennaro, Goldfeder, Makriyannis and Peled, "UC Non-Interactive,
//! Proactive, Threshold ECDSA with Identifiable Aborts" (CGGMP21, ePrint 2021/060).
//!
//! Its output is a plain ECDSA P-256 signature, so a certificate or CRL it signs verifies
//! with an unmodified IEEE 1609.2 stack — the reason to pay for it rather than FROST. The
//! non-interactive variant has three presigning rounds that do not depend on the message
//! and one online round (CGGMP21 Table 1); presignatures can be pooled ahead of time.
//!
//! # Numbers, and where they come from
//!
//! CGGMP21 publishes asymptotic sizes, not measurements. The measured figures here are
//! Silence Laboratories' "Presign calculator — DKLs23 vs CGGMP21" (2-of-3 on GCP n2d VMs,
//! 3072-bit Paillier; fetched 2026-10-07):
//!
//! * **bytes:** ≈ 88 KiB one-way per presignature, aggregate over the parties, so 45,056 B
//!   per party for its one peer, spread here evenly over the three presign rounds
//!   (15,019 B per peer per round). CGGMP21 Table 1's `65κ + 48ν` per pair at κ = 256,
//!   ν = 2048 gives 14,368 B — a lower bound without the proofs' full weight;
//! * **time:** a single presignature session took 2,646 ms at 4 vCPU; it is charged here
//!   as 882 ms of participant compute per presign round (an upper bound: the measured
//!   figure includes in-region network time), and the online round as the measured
//!   ~26 ms p50 per signature;
//! * the online share is one scalar, κ = 32 B (CGGMP21 Table 1: online `n × κ`).
//!
//! CGGMP21 is peer-to-peer; the session engine runs it as a star through the coordinator,
//! which relays each round's messages. That doubles each round's hops (latency) and the
//! relayed bytes, and is stated rather than hidden.

use v2xw_core::time::Duration;
use v2xw_sec::primitive::{PrimitiveId, PrimitiveOpKind};

use super::{
    Committee, Knowledge, ProtocolEntry, RoundMessage, RoundPlan, StarRound, ThresholdProtocol,
};
use crate::sizes::WireSize;
use crate::spec::OpDescriptor;

/// The registry id.
pub const ID: &str = "cggmp21-ecdsa";

/// The registry entry.
pub const ENTRY: ProtocolEntry = ProtocolEntry {
    id: ID,
    label: "Threshold ECDSA (CGGMP21) — 1609.2-compatible output",
    build: |p| Box::new(Cggmp21 {
        presigned: p.preprocessed,
    }),
};

const SILENCE: &str = "Silence Laboratories, Presign calculator, DKLs23 vs CGGMP21, 2-of-3 on GCP \
                       n2d, 3072-bit Paillier (fetched 2026-10-07)";
const TABLE1: &str = "CGGMP21 (ePrint 2021/060) Table 1";

/// Bytes one party sends one peer per presign round: 88 KiB aggregate per presignature
/// for a 2-party presign = 45,056 B per party, over three rounds.
pub const PRESIGN_PAIR_ROUND_BYTES: u32 = 45_056 / 3;
/// κ = 256 bits.
pub const KAPPA_BYTES: u32 = 32;
/// ν = 2048-bit Paillier modulus, as CGGMP21 Table 1 counts it.
pub const NU_BYTES: u32 = 256;
/// Participant compute per presign round per presignature: 2,646 ms / 3.
pub const PRESIGN_ROUND_TIME: Duration = Duration::from_millis(882);
/// Participant time per online signature share: the measured ~26 ms p50.
pub const ONLINE_TIME: Duration = Duration::from_millis(26);

/// CGGMP21 threshold ECDSA.
#[derive(Debug, Clone, Copy, Default)]
pub struct Cggmp21 {
    /// Whether presignatures come from a pool built ahead of time.
    pub presigned: bool,
}

impl ThresholdProtocol for Cggmp21 {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        ENTRY.label
    }

    fn standard(&self) -> &'static str {
        "CGGMP21, ePrint 2021/060 (non-interactive variant: 3 presign rounds + 1 online)"
    }

    fn sign_rounds(&self, signers: u16, batch: u32) -> Vec<StarRound> {
        let peers = u32::from(signers.saturating_sub(1)).max(1);
        let mut rounds = Vec::new();
        if !self.presigned {
            for (k, name) in ["cggmp21-presign-1", "cggmp21-presign-2", "cggmp21-presign-3"]
                .into_iter()
                .enumerate()
            {
                // Round 1's request carries nothing to relay; later ones relay the other
                // signers' messages from the round before.
                let request = if k == 0 {
                    WireSize::cited(4, "Ieee1609Dot2BaseTypes.asn Uint32: a batch count")
                } else {
                    WireSize::derived(
                        batch * peers * PRESIGN_PAIR_ROUND_BYTES,
                        "B · (s−1) · relayed peer messages of the previous round",
                        SILENCE,
                    )
                };
                rounds.push(StarRound {
                    name,
                    request,
                    response: WireSize::derived(
                        batch * peers * PRESIGN_PAIR_ROUND_BYTES,
                        "B · (s−1) · 45,056 B / 3",
                        SILENCE,
                    ),
                    member_ops: Vec::new(),
                    member_time: Some((PRESIGN_ROUND_TIME.saturating_mul(u64::from(batch)), SILENCE)),
                    coordinator_ops: Vec::new(),
                    verifies_shares: false,
                });
            }
        }
        rounds.push(StarRound {
            name: "cggmp21-online",
            request: WireSize::derived(batch * 32, "B · message digest", TABLE1),
            response: WireSize::derived(batch * KAPPA_BYTES, "B · σ_i (κ)", TABLE1),
            member_ops: Vec::new(),
            member_time: Some((ONLINE_TIME.saturating_mul(u64::from(batch)), SILENCE)),
            // Combine the shares and check the result as an ECDSA signature: a bad share
            // is then traced through the protocol's identifiable-abort proofs.
            coordinator_ops: vec![OpDescriptor::new(
                PrimitiveId::ECDSA_P256_SHA256,
                PrimitiveOpKind::Verify,
                batch,
            )],
            verifies_shares: true,
        });
        rounds
    }

    fn dkg_rounds(&self, committee: &Committee) -> Vec<RoundPlan> {
        (1..=3)
            .map(|index| RoundPlan {
                index,
                messages: (0..committee.n)
                    .map(|from| RoundMessage {
                        from,
                        to: None,
                        bytes: WireSize::derived(
                            4 * KAPPA_BYTES,
                            "n · 4κ incoming per party: 4κ per broadcast",
                            TABLE1,
                        ),
                    })
                    .collect(),
                compute: Vec::new(),
                attempts: 1,
            })
            .collect()
    }

    fn refresh_rounds(&self, committee: &Committee) -> Vec<RoundPlan> {
        let n = u32::from(committee.n);
        (1..=3)
            .map(|index| RoundPlan {
                index,
                messages: (0..committee.n)
                    .map(|from| RoundMessage {
                        from,
                        to: None,
                        bytes: WireSize::derived(
                            2 * n * KAPPA_BYTES + 5 * NU_BYTES,
                            "2nκ + (2m+5)ν with m = 0: a lower bound (m, the proof \
                             repetitions, is not fixed by the source held)",
                            TABLE1,
                        ),
                    })
                    .collect(),
                compute: Vec::new(),
                attempts: 1,
            })
            .collect()
    }

    fn signature_bytes(&self) -> WireSize {
        WireSize::cited(64, "an ECDSA P-256 signature (r, s), 66 B as 1609.2 COER")
    }

    fn verification_key_bytes(&self) -> WireSize {
        WireSize::cited(33, "SEC 1 §2.3.3: compressed P-256 point")
    }

    fn verifies_as(&self) -> &'static str {
        "ecdsa-p256"
    }

    fn identifiable_abort(&self) -> bool {
        true
    }

    fn knowledge(&self) -> Vec<Knowledge> {
        vec![
            Knowledge {
                party: "coordinator",
                learns: "the message and the combined signature; relayed ciphertexts it \
                         cannot open",
            },
            Knowledge {
                party: "signer",
                learns: "its key share, its Paillier key, and zero-knowledge proofs from the \
                         others; nothing about their shares",
            },
            Knowledge {
                party: "coalition of t or fewer",
                learns: "nothing that lets it sign (CGGMP21 UC security); a cheater is \
                         identified",
            },
            Knowledge {
                party: "verifier",
                learns: "an ordinary ECDSA P-256 signature: it cannot tell it was made by a \
                         committee",
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_presign_rounds_and_one_online_round() {
        let p = Cggmp21::default();
        let r = p.sign_rounds(2, 1);
        assert_eq!(r.len(), 4);
        assert_eq!(r[3].response.bytes(), 32);
        // 2 signers → one peer each; three rounds carry the measured 45,056 B (to the byte
        // that division by three loses).
        let presign: u32 = r[..3].iter().map(|x| x.response.bytes()).sum();
        assert_eq!(presign, 3 * (45_056 / 3));
        assert_eq!(Cggmp21 { presigned: true }.sign_rounds(2, 1).len(), 1);
        assert_eq!(p.verifies_as(), "ecdsa-p256");
    }
}
