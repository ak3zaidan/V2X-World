//! t-of-n linkage authorities: the linkage seeds secret-shared over `n` servers, any `t` of
//! which release a revoked device's seeds — instead of CAMP's two-of-two, where both
//! Linkage Authorities must answer.
//!
//! # The construction, and the assumption it rests on
//!
//! A CRL entry needs `ls₁(i)` and `ls₂(i)`, the two chains' seeds at the revocation period
//! (Brecht 2018 §VI-D). In the CAMP design each lives at one LA, so revocation needs both
//! LAs up, and either alone can refuse. Here each seed is Shamir-shared (Shamir, "How to
//! share a secret", CACM 1979) over the committee with Feldman commitments (Feldman, FOCS
//! 1987), so any `t` members' shares reconstruct it, a member's share is checked against
//! the public commitments before use (identifiable abort), and `t − 1` colluding members
//! learn nothing.
//!
//! **Assumption.** A hash chain is not linear, so shares of `ls(0)` do not give shares of
//! `ls(i)`. The construction modelled here is the dealer one: when a chain is allocated,
//! its LA computes `ls(i)` for every period it will ever be asked for, shares each, hands
//! the shares out and erases the chain. Forward-only revocation is kept (releasing
//! `ls(i)` still reveals nothing before `i`), at the price of per-period share storage at
//! each member. A dealer-free version needs a distributed PRF evaluation per period — more
//! rounds, the same interface.
//!
//! Sizes: a share is a P-256 scalar (32 B) with its index (`Uint16`); a seed is 16 B. Each
//! release request names the chain (the linkage-chain identifier parameter) and the period.

use super::frost::mults;
use super::{
    Committee, Knowledge, ProtocolEntry, RoundMessage, RoundPlan, StarRound, ThresholdProtocol,
};
use crate::sizes::{LINKAGE_SEED_BYTES, TIME32_BYTES, WireSize};

/// The registry id.
pub const ID: &str = "shamir-feldman";

/// The registry entry.
pub const ENTRY: ProtocolEntry = ProtocolEntry {
    id: ID,
    label: "t-of-n secret release (Shamir + Feldman), for linkage seeds",
    build: |_| Box::new(ShamirFeldman),
};

/// A share: a P-256 scalar and its 16-bit index.
pub const SHARE_BYTES: u32 = 32 + 2;
/// A linkage-chain identifier as it crosses MA → LA: the `linkage_chain_identifier_bytes`
/// default of `SizeParams` (16 B, a todo-calibrate card parameter).
const LCI_BYTES: u32 = 16;

const WHY: &str = "Shamir, CACM 1979; Feldman, FOCS 1987; share = P-256 scalar (SEC 1) + Uint16 index";

/// Shamir sharing with Feldman verification.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShamirFeldman;

impl ThresholdProtocol for ShamirFeldman {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        ENTRY.label
    }

    fn standard(&self) -> &'static str {
        "Shamir 1979 secret sharing with Feldman 1987 verifiable shares (dealer model)"
    }

    /// One round: `batch` is the number of secrets released (two seeds per revocation).
    fn sign_rounds(&self, signers: u16, batch: u32) -> Vec<StarRound> {
        let t = u32::from(signers);
        vec![StarRound {
            name: "linkage-share",
            request: WireSize::derived(
                batch * (LCI_BYTES + TIME32_BYTES),
                "B · (linkage-chain identifier + i-period)",
                WHY,
            ),
            response: WireSize::derived(batch * SHARE_BYTES, "B · (scalar share + index)", WHY),
            member_ops: Vec::new(),
            member_time: None,
            // Feldman: share·G against Σ C_k · idx^k, about t multiplications per share,
            // for every share of every secret; Lagrange interpolation is scalar
            // arithmetic, negligible beside it.
            coordinator_ops: vec![mults(batch * t * (t + 1))],
            verifies_shares: true,
        }]
    }

    fn dkg_rounds(&self, committee: &Committee) -> Vec<RoundPlan> {
        let t1 = u32::from(committee.quorum());
        vec![RoundPlan {
            index: 1,
            messages: (0..committee.n)
                .map(|to| RoundMessage {
                    from: super::COORDINATOR,
                    to: Some(to),
                    bytes: WireSize::derived(
                        SHARE_BYTES + t1 * 33,
                        "one share per period per chain + the t Feldman commitments",
                        WHY,
                    ),
                })
                .collect(),
            compute: vec![mults(t1)],
            attempts: 1,
        }]
    }

    fn refresh_rounds(&self, committee: &Committee) -> Vec<RoundPlan> {
        super::frost::Frost::default().refresh_rounds(committee)
    }

    fn signature_bytes(&self) -> WireSize {
        WireSize::cited(LINKAGE_SEED_BYTES, "Brecht 2018 §V-B: a 16-byte linkage seed")
    }

    fn verification_key_bytes(&self) -> WireSize {
        WireSize::cited(33, "a Feldman commitment: a compressed P-256 point")
    }

    fn verifies_as(&self) -> &'static str {
        "a linkage seed, checked against the Feldman commitments"
    }

    fn identifiable_abort(&self) -> bool {
        true
    }

    fn knowledge(&self) -> Vec<Knowledge> {
        vec![
            Knowledge {
                party: "misbehaviour authority",
                learns: "the revoked chain's seed at the revocation period, once t shares \
                         arrive; nothing before it (forward-only, Brecht 2018 §VI-D)",
            },
            Knowledge {
                party: "member",
                learns: "one share per period per chain; a lone member cannot link anything",
            },
            Knowledge {
                party: "coalition of t − 1",
                learns: "nothing about any seed (Shamir 1979); unlike two-of-two, no single \
                         operator can block a revocation either",
            },
        ]
    }
}
