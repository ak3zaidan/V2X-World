//! Interactive multi-party protocols: the extension point, the shipped protocols, and the
//! session engine that runs them over the modelled backend.
//!
//! A threshold protocol replaces one server's private-key operation by a committee of `n`
//! servers of which any `t` (the *quorum*) must act. The simulator does not execute the
//! group arithmetic; it executes everything that makes a threshold deployment cost what it
//! costs and fail how it fails:
//!
//! * the **rounds** — each a request from the coordinator to every selected participant
//!   and a response back, over a real [`crate::net::Link`] that charges latency and bytes
//!   (I-P1);
//! * the **messages and their sizes per round**, from the protocol's specification;
//! * the **compute per round**, charged to each participant's service queue from the
//!   primitive catalogue's cited anchors, or as a measured per-item time where the
//!   protocol was only ever benchmarked as a unit (CGGMP21's Paillier work);
//! * **what each party learns** ([`ThresholdProtocol::knowledge`]);
//! * **slow, offline and malicious participants** — a slow one delays its round, an
//!   offline one is timed out and replaced, a malicious one returns an invalid share that
//!   the coordinator detects (identifiable abort) and excludes, restarting the session;
//!   fewer than `t` live honest members fail the session.
//!
//! # Adding a protocol
//!
//! `docs/design/extending-crypto.md` is the guide. Copy [`template`], implement
//! [`ThresholdProtocol`] (its required methods are the specification's numbers), add a
//! [`ProtocolEntry`] to [`THRESHOLD_PROTOCOLS`]. A scenario can then name it in
//! `security.threshold.protocol`, the settings window offers it, the SCMS runs its PCA,
//! CRL Generator and linkage authorities on it, and the run's metrics count its rounds,
//! bytes and failures. [`session`] is protocol-agnostic and needs no change.
//!
//! # What ships
//!
//! | id | What | Source |
//! |---|---|---|
//! | `frost-p256` | FROST two-round threshold Schnorr, P-256 ciphersuite | RFC 9591 |
//! | `cggmp21-ecdsa` | threshold ECDSA (verifies as plain ECDSA P-256, so 1609.2-compatible) | CGGMP21, ePrint 2021/060 |
//! | `umbrella` | a t-of-n umbrella authority: one threshold signature covers `k` leaf pseudonyms | 05-protocols.md §5.4 (placeholder binding) |
//! | `shamir-feldman` | t-of-n release of a secret-shared value (the linkage seeds) | Shamir 1979; Feldman 1987 |

use core::fmt::Debug;

use v2xw_core::ids::NodeId;
use v2xw_core::time::Duration;

use crate::sizes::WireSize;
use crate::spec::{Centrality, OpDescriptor};
use crate::stage::FlowId;

pub mod cggmp21;
pub mod frost;
pub mod linkage;
pub mod session;
pub mod template;
pub mod umbrella;

/// The coordinator's index in a [`RoundMessage`].
pub const COORDINATOR: u16 = u16::MAX;

/// One message inside one peer-to-peer round, for the analytic accounting of key
/// generation and refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundMessage {
    /// The sender's index within the committee.
    pub from: u16,
    /// The receiver's index ([`COORDINATOR`] for the coordinator), or `None` for a
    /// broadcast that costs `n − 1` messages.
    pub to: Option<u16>,
    /// Its size.
    pub bytes: WireSize,
}

/// One peer-to-peer round: who sends what to whom, and what each participant computes.
#[derive(Debug, Clone)]
pub struct RoundPlan {
    /// Which round this is, from 1.
    pub index: u16,
    /// The messages it carries.
    pub messages: Vec<RoundMessage>,
    /// What each participant computes in it.
    pub compute: Vec<OpDescriptor>,
    /// How many attempts the scheme expects before this round succeeds. `1` for a scheme
    /// that never aborts; CELI25's threshold ML-DSA needs 4–5 [CELI25; QUORUS §5].
    pub attempts: u16,
}

impl RoundPlan {
    /// The bytes this round puts on the network, expanding broadcasts over `n`
    /// participants.
    pub fn bytes(&self, n: u16) -> u64 {
        self.messages
            .iter()
            .map(|m| {
                let copies = u64::from(if m.to.is_some() {
                    1
                } else {
                    n.saturating_sub(1)
                });
                copies * u64::from(m.bytes.bytes())
            })
            .sum::<u64>()
            .saturating_mul(u64::from(self.attempts))
    }
}

/// A protocol-level time measured as a unit, for work no primitive anchor describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasuredTime {
    /// The time per signed item, per participant.
    pub per_item: Duration,
    /// Where it was measured.
    pub source: &'static str,
}

/// One coordinator-driven round of an online operation: the coordinator sends every
/// selected participant a request, each computes and responds, and the round completes
/// when the coordinator holds `quorum` valid responses.
#[derive(Debug, Clone)]
pub struct StarRound {
    /// The round's name as the wire log records it, e.g. `frost-commit`.
    pub name: &'static str,
    /// Coordinator → each participant, payload only (the session adds the envelope).
    pub request: WireSize,
    /// Participant → coordinator, payload only.
    pub response: WireSize,
    /// What each participant computes on receiving the request.
    pub member_ops: Vec<OpDescriptor>,
    /// What each participant spends that no primitive anchor describes.
    pub member_time: Option<(Duration, &'static str)>,
    /// What the coordinator computes once the round's responses are in.
    pub coordinator_ops: Vec<OpDescriptor>,
    /// Whether the coordinator verifies each response here, so an invalid one is
    /// attributed to its sender (identifiable abort). The round where a malicious
    /// participant's lie is caught.
    pub verifies_shares: bool,
}

/// How often shares are refreshed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshPolicy {
    /// The epoch length.
    pub epoch: Duration,
    /// Whether a refresh invalidates the previous epoch's shares outright.
    pub invalidates_previous: bool,
}

impl Default for RefreshPolicy {
    /// Daily, invalidating: no source fixes a cadence, so this is a scenario parameter
    /// with a stated default.
    fn default() -> RefreshPolicy {
        RefreshPolicy {
            epoch: Duration::from_secs(86_400),
            invalidates_previous: true,
        }
    }
}

/// The committee.
#[derive(Debug, Clone)]
pub struct Committee {
    /// Committee size.
    pub n: u16,
    /// Threshold in the RFC 9591 / CGGMP21 sense: `t + 1` participants must act. A
    /// scenario's `quorum` is `t + 1`.
    pub t: u16,
    /// The nodes, in committee-index order.
    pub members: Vec<NodeId>,
    /// The coordinator, for schemes that need one (FROST does [RFC 9591 §5]).
    pub coordinator: Option<NodeId>,
}

impl Committee {
    /// How the committee appears to the role specification.
    pub const fn centrality(&self) -> Centrality {
        Centrality::Distributed {
            n: self.n,
            t: self.t,
        }
    }

    /// How many participants must act: `t + 1`.
    pub const fn quorum(&self) -> u16 {
        self.t.saturating_add(1)
    }
}

/// Which operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThresholdOp {
    /// Distributed key generation.
    Dkg,
    /// Offline presignature generation.
    Presign,
    /// Online signing.
    Sign,
    /// Share refresh.
    Refresh,
}

/// One party's view: what a protocol reveals to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Knowledge {
    /// The party, e.g. `coordinator`, `signer`, `coalition of t-1`.
    pub party: &'static str,
    /// What it learns.
    pub learns: &'static str,
}

/// How an umbrella credential relates to the pseudonyms it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UmbrellaScheme {
    /// How many leaf pseudonyms one umbrella covers.
    pub leaves_per_umbrella: u32,
    /// Whether a leaf is derived on the device (no round trip) or issued (at least one).
    pub leaf_is_derived_locally: bool,
    /// The umbrella credential's size.
    pub umbrella_bytes: WireSize,
    /// A leaf's size.
    pub leaf_bytes: WireSize,
}

/// The surface an interactive multi-party protocol implements.
///
/// The required methods are the specification's numbers; the provided ones derive the
/// totals a report needs from them. Nothing here lets a participant skip a round: the
/// [`session`] engine runs every [`StarRound`] over the network.
pub trait ThresholdProtocol: Debug + Send + Sync {
    /// The stable id a scenario names and recordings carry.
    fn id(&self) -> &'static str;

    /// A plain name for the settings window.
    fn label(&self) -> &'static str;

    /// The specification, with its clause.
    fn standard(&self) -> &'static str;

    /// The online rounds that sign `batch` messages with `signers` participants.
    fn sign_rounds(&self, signers: u16, batch: u32) -> Vec<StarRound>;

    /// Distributed key generation, peer to peer, for the analytic report.
    fn dkg_rounds(&self, committee: &Committee) -> Vec<RoundPlan>;

    /// Proactive share refresh, peer to peer, for the analytic report.
    fn refresh_rounds(&self, committee: &Committee) -> Vec<RoundPlan>;

    /// The refresh cadence.
    fn refresh_policy(&self) -> RefreshPolicy {
        RefreshPolicy::default()
    }

    /// The output's size on the wire.
    fn signature_bytes(&self) -> WireSize;

    /// The group verification key's size.
    fn verification_key_bytes(&self) -> WireSize;

    /// The `security.signature` scheme a verifier checks the output as, or a description
    /// when no registered scheme matches (a Schnorr signature is not a 1609.2 one).
    fn verifies_as(&self) -> &'static str;

    /// Whether a participant that returns an invalid share is identified and can be
    /// excluded, rather than only causing the attempt to fail.
    fn identifiable_abort(&self) -> bool;

    /// What each party learns.
    fn knowledge(&self) -> Vec<Knowledge>;

    /// Which flow each operation is recorded under when it runs on its own.
    fn flow_of(&self, op: ThresholdOp) -> FlowId {
        match op {
            ThresholdOp::Dkg => FlowId::ThresholdDkg,
            ThresholdOp::Sign | ThresholdOp::Presign => FlowId::ThresholdSign,
            ThresholdOp::Refresh => FlowId::ThresholdRefresh,
        }
    }

    /// Bytes one online signing session puts on the backend network with `signers`
    /// participants and `envelope` bytes of framing on every message, if nobody fails.
    fn sign_bytes(&self, signers: u16, batch: u32, envelope: u32) -> u64 {
        self.sign_rounds(signers, batch)
            .iter()
            .map(|r| {
                u64::from(signers)
                    * (u64::from(r.request.bytes() + envelope)
                        + u64::from(r.response.bytes() + envelope))
            })
            .sum()
    }

    /// Round trips one online signing session takes.
    fn sign_round_trips(&self) -> usize {
        self.sign_rounds(2, 1).len()
    }

    /// Total bytes key generation or refresh costs the network.
    fn setup_bytes(&self, op: ThresholdOp, committee: &Committee) -> u64 {
        let rounds = match op {
            ThresholdOp::Dkg => self.dkg_rounds(committee),
            ThresholdOp::Refresh => self.refresh_rounds(committee),
            ThresholdOp::Sign | ThresholdOp::Presign => Vec::new(),
        };
        rounds.iter().map(|r| r.bytes(committee.n)).sum()
    }

    /// How many rounds key generation or refresh takes, counting expected attempts.
    fn setup_rounds(&self, op: ThresholdOp, committee: &Committee) -> u32 {
        let rounds = match op {
            ThresholdOp::Dkg => self.dkg_rounds(committee),
            ThresholdOp::Refresh => self.refresh_rounds(committee),
            ThresholdOp::Sign | ThresholdOp::Presign => Vec::new(),
        };
        rounds.iter().map(|r| u32::from(r.attempts)).sum()
    }
}

/// What a scenario can tune about a protocol when it is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolParams {
    /// For `umbrella`: leaf pseudonyms per umbrella credential.
    pub umbrella_leaves: u32,
    /// For `umbrella`: the protocol the committee signs umbrellas with.
    pub umbrella_inner: &'static str,
    /// For protocols with an offline phase (FROST's commitments, CGGMP21's presignatures):
    /// whether it was done ahead of time, leaving only the online round.
    pub preprocessed: bool,
}

impl Default for ProtocolParams {
    fn default() -> ProtocolParams {
        ProtocolParams {
            umbrella_leaves: 20,
            umbrella_inner: frost::ID,
            preprocessed: false,
        }
    }
}

/// One registered protocol.
#[derive(Debug, Clone, Copy)]
pub struct ProtocolEntry {
    /// The id a scenario names.
    pub id: &'static str,
    /// A plain name for the settings window.
    pub label: &'static str,
    /// Builds it.
    pub build: fn(&ProtocolParams) -> Box<dyn ThresholdProtocol>,
}

/// Every protocol a scenario can name. Adding one is one line here and one module above.
pub const THRESHOLD_PROTOCOLS: &[ProtocolEntry] = &[
    frost::ENTRY,
    cggmp21::ENTRY,
    umbrella::ENTRY,
    linkage::ENTRY,
];

const N_PROTOCOLS: usize = THRESHOLD_PROTOCOLS.len();

/// The values `security.threshold.protocol` accepts, from [`THRESHOLD_PROTOCOLS`].
pub const THRESHOLD_IDS: [&str; N_PROTOCOLS] = {
    let mut out = [""; N_PROTOCOLS];
    let mut i = 0;
    while i < N_PROTOCOLS {
        out[i] = THRESHOLD_PROTOCOLS[i].id;
        i += 1;
    }
    out
};

/// Builds the protocol `id` names.
pub fn build(id: &str, params: &ProtocolParams) -> Option<Box<dyn ThresholdProtocol>> {
    THRESHOLD_PROTOCOLS
        .iter()
        .find(|e| e.id == id)
        .map(|e| (e.build)(params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_every_protocol_builds_under_its_own_id() {
        let mut seen = std::collections::BTreeSet::new();
        for (i, e) in THRESHOLD_PROTOCOLS.iter().enumerate() {
            assert!(seen.insert(e.id), "duplicate {}", e.id);
            assert_eq!(THRESHOLD_IDS[i], e.id);
            let p = (e.build)(&ProtocolParams::default());
            assert_eq!(p.id(), e.id);
            assert!(!p.sign_rounds(3, 1).is_empty(), "{}", e.id);
            assert!(!p.knowledge().is_empty(), "{}", e.id);
        }
    }
}
