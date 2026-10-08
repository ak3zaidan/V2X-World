//! Signing overhead: what the chosen signature scheme costs, on the air, in the devices and
//! in the backend — one coherent group, so that a researcher who switches
//! `security.signature` from ECDSA P-256 to ML-DSA-44, Falcon-512 or a hybrid reads the
//! difference in one place.
//!
//! Opt-in: a scenario asks for it with `metrics: [all, signing]` (or any metric name below),
//! and `v2xw compare-signatures` asks for it in every run it makes. It is not part of a bare
//! `all`, so the digests of every run recorded before the group existed are unchanged.
//!
//! | Metric | Formula | Unit |
//! |---|---|---|
//! | `sig_signature_bytes` | Σ signature octets / signed messages | B/msg |
//! | `sig_signature_rate` | Σ signature octets on the air / window | B/s |
//! | `sig_signer_id_bytes` | Σ signer-identifier octets (certificate, or 8-octet digest) / signed messages | B/msg |
//! | `sig_cert_bytes` | Σ attached-certificate octets / signed messages | B/msg |
//! | `sig_cert_airtime_share` | Σ certificate octets / Σ octets on the air | ratio |
//! | `sig_signer_id_airtime_share` | Σ signer-identifier octets / Σ octets on the air | ratio |
//! | `sig_security_airtime_share` | Σ envelope octets / Σ octets on the air | ratio |
//! | `sig_security_airtime_share_nocert` | Σ (envelope − certificate) octets / Σ (octets on the air − certificate) | ratio |
//! | `sig_sign_latency` | t_signed − t_sign_start, per scheme@engine | ms |
//! | `sig_sign_wait` | t_sign_start − t_generated | ms |
//! | `sig_verify_latency` | t_done − t_start of a verification, per scheme@engine | ms |
//! | `sig_signer_util`, `sig_signer_util_max` | Σ signing service time of a node / window, mean and max over signing nodes | ratio |
//! | `sig_verifier_util`, `sig_verifier_util_max` | Σ verification service time of a node / window, mean and max | ratio |
//! | `sig_verify_capacity` | 1 / mean verification service time | 1/s |
//! | `sig_neighbour_capacity` | `sig_verify_capacity` / messages per second per sender | count |
//! | `sig_verify_headroom` | `sig_verify_capacity` / verification arrivals per second per verifying node | × |
//! | `sig_verify_dropped` | verifications dropped / verification tasks | ratio |
//! | `sig_verify_late` | verifications whose enqueue-to-done exceeded 100 ms / completed | ratio |
//! | `sig_fragments_per_msg` | frames on the air / messages sent | frames/msg |
//! | `sig_fragmented_share` | messages sent in more than one frame / messages | ratio |
//! | `sig_reassembly_loss` | fragmented SDUs failed or expired at a receiver / fragmented SDUs resolved | ratio |
//! | `sig_e2e_added` | (t_signed − t_generated) + (t_verify_done − t_rx_done) of a delivered message | ms |
//! | `sig_e2e_added_share` | Σ `sig_e2e_added` / Σ end-to-end latency | ratio |
//! | `sig_cbr_extra` | mean CBR × `sig_security_airtime_share` | ratio |
//! | `sig_backend_bytes` | credential-system octets to and from devices / window | B/s |
//! | `sig_ca_sign_rate` | signatures made by the certificate authorities / window | 1/s |
//! | `sig_ca_busy` | certificate-authority service time / window | ms/s |
//!
//! # What the scheme facts are for
//!
//! The run's records say how long a signature took and how big an envelope was; they do not
//! say how many of those octets were the signature. [`SchemeFacts`] carries what the scheme
//! registry (`v2xw_sec::scheme`) and the node's hardware profile say — the signature's
//! encoded length, and which engine signs and verifies — so the signature octets are the
//! registry's figure per signed message, and the latency dimension names the engine
//! (`ml-dsa-44@cpu`, `ecdsa-p256@hsm`).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use v2xw_core::card::ModelCard;
use v2xw_core::ctx::{ChannelName, EventRecord, Visibility};
use v2xw_core::ids::NodeId;
use v2xw_core::model::Model;
use v2xw_core::time::{Duration, SimTime};

use crate::cards;
use crate::channels::{
    ChannelView, FragOutcome, MacCbrView, NetFragView, NodeRxView, NodeTxView, RxFate, SignerId,
    VerifyOutcome, decode,
};
use crate::def::{Agg, DEFAULT_LEVEL, Dim, DimValue, Dims, MetricDef, MetricSample, SampleValue};
use crate::health::{Basis, Better, HealthBand};
use crate::provider::{Decoded, MetricProvider};
use crate::quant::Quantum;
use crate::stats::{ConfidenceLevel, Distribution, Estimate, Proportion, ratio_of_sums};

/// The token a scenario's `metrics` list names to switch the group on beside `all`.
pub const GROUP: &str = "signing";

/// The octets of a HashedId8 signer identifier (IEEE 1609.2 §6.3.4), the digest a message
/// carries when it does not attach its certificate.
pub const DIGEST_OCTETS: u64 = 8;

/// The latency past which a verification is late: the 100 ms basic-safety budget
/// (3GPP TS 22.185 §5.1), spent on verification alone.
pub const LATE_MS: f64 = crate::health::SAFETY_LATENCY_BUDGET_MS;

/// What the scheme registry and the hardware profile say about the run's scheme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemeFacts {
    /// The scheme id (`security.signature`).
    pub scheme: String,
    /// `classical`, `hybrid` or `post-quantum`.
    pub mode: String,
    /// The encoded signature's octets in the envelope.
    pub sig_octets: u64,
    /// Where the default OBU signs: `hsm`, `accelerator`, `cpu` or `unknown`.
    pub sign_engine: String,
    /// Where it verifies.
    pub verify_engine: String,
}

impl Default for SchemeFacts {
    fn default() -> Self {
        SchemeFacts {
            scheme: "ecdsa-p256".to_string(),
            mode: "classical".to_string(),
            sig_octets: 66,
            sign_engine: "unknown".to_string(),
            verify_engine: "unknown".to_string(),
        }
    }
}

/// The names of every metric in the group, in definition order.
#[must_use]
pub fn metric_names() -> Vec<String> {
    SigningProvider::new(0, SchemeFacts::default())
        .definitions()
        .into_iter()
        .map(|d| d.name)
        .collect()
}

/// The drop reason a full verification queue records (`v2xw_node::queue::DropCause`).
pub const VERIFY_OVERFLOW: &str = "verify_overflow";

/// `node.verify` as this provider reads it: the shared view plus the drop reason, which
/// tells a full queue from a policy choice.
#[derive(Debug, Clone, Deserialize)]
struct VerifyRec {
    t_enqueue: SimTime,
    #[serde(default)]
    t_start: Option<SimTime>,
    #[serde(default)]
    t_done: Option<SimTime>,
    node: NodeId,
    #[serde(default)]
    cost_us: Option<u64>,
    outcome: VerifyOutcome,
    #[serde(default)]
    reason: Option<String>,
}

impl ChannelView for VerifyRec {
    const CHANNEL: &'static str = "node.verify";
}

/// The subset of `backend.state` this provider reads.
#[derive(Debug, Clone, Deserialize)]
struct BackendLite {
    #[serde(default)]
    entities: Vec<EntityLite>,
    #[serde(default)]
    edges: Vec<EdgeLite>,
}

#[derive(Debug, Clone, Deserialize)]
struct EntityLite {
    #[serde(default)]
    tier: String,
    #[serde(default)]
    queue: Option<QueueLite>,
    #[serde(default)]
    ops: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct QueueLite {
    #[serde(default)]
    busy_ns: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct EdgeLite {
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    bytes: u64,
}

impl ChannelView for BackendLite {
    const CHANNEL: &'static str = "backend.state";
}

/// Cumulative backend counters at one snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct BackendTotals {
    device_bytes: u64,
    ca_signs: u64,
    ca_busy_ns: u64,
}

/// Per-window sums.
#[derive(Debug, Default)]
struct Window {
    air: u64,
    envelope: u64,
    cert: u64,
    signer_id: u64,
    signed_msgs: u64,
    frames: u64,
    messages: BTreeMap<(NodeId, u64), u32>,
    sign_ms: Distribution,
    sign_wait_ms: Distribution,
    sign_busy: BTreeMap<NodeId, u64>,
    verify_ms: Distribution,
    verify_busy: BTreeMap<NodeId, u64>,
    verify_arrivals: BTreeMap<NodeId, u64>,
    dropped: Proportion,
    late: Proportion,
    reassembly: Proportion,
    added_ms: Distribution,
    added_sum_ns: u64,
    e2e_sum_ns: u64,
    added_n: u64,
    cbr_sum: f64,
    cbr_n: u64,
}

/// The signing-overhead provider.
pub struct SigningProvider {
    card: ModelCard,
    facts: SchemeFacts,
    level: ConfidenceLevel,
    min_samples: u64,
    window_start: SimTime,
    w: Window,
    backend_now: Option<BackendTotals>,
    backend_prev: BackendTotals,
    rejected: u64,
}

impl SigningProvider {
    /// A provider whose first window starts at `t0`, for a run under `facts`' scheme.
    #[must_use]
    pub fn new(t0: SimTime, facts: SchemeFacts) -> Self {
        Self {
            card: Self::build_card(),
            facts,
            level: DEFAULT_LEVEL,
            min_samples: 1,
            window_start: t0,
            w: Window::default(),
            backend_now: None,
            backend_prev: BackendTotals::default(),
            rejected: 0,
        }
    }

    /// The scheme facts the provider was built with.
    #[must_use]
    pub const fn facts(&self) -> &SchemeFacts {
        &self.facts
    }

    fn build_card() -> ModelCard {
        let mut card = cards::provider_card(
            "metric/security/signing-overhead",
            "1.0.0",
            "What the signature scheme costs: signature, signer-identifier and certificate \
             octets and their share of the air; sign and verify latency and where they run; \
             signer and verifier utilisation and headroom; verifications dropped or late; \
             fragmentation and the delivery lost to it; latency added by security; the \
             extra channel load; and the credential system's provisioning bytes and CA \
             signing time.",
        );
        card.equations = vec![
            v2xw_core::card::Equation::new(
                "sig_security_airtime_share",
                "Σ envelope octets / Σ octets on the air",
            ),
            v2xw_core::card::Equation::new(
                "sig_verify_headroom",
                "(1 / mean verification service time) / (verification arrivals per second \
                 per verifying node)",
            ),
            v2xw_core::card::Equation::new(
                "sig_cbr_extra",
                "mean measured CBR × Σ envelope octets / Σ octets on the air",
            ),
        ];
        card.parameters = cards::statistics_params();
        card.sources = vec![
            cards::design("08-measurement-and-data.md §2.2 (security processing)"),
            cards::design("extending-crypto.md (the scheme registry the facts come from)"),
            cards::standard("IEEE 1609.2-2022 §6.3.4 (SignerIdentifier: certificate or HashedId8)"),
            cards::standard("3GPP TS 22.185 §5.1 (100 ms basic-safety latency)"),
        ];
        card.limitations = vec![
            "Signature octets are the registry's encoded length per signed message, not a \
             parse of each envelope."
                .to_string(),
            "sig_cbr_extra is an estimate from the octet share; the measured difference is \
             the comparison's CBR column against the ECDSA P-256 run."
                .to_string(),
            "Utilisation assumes one signing and one verifying engine per node.".to_string(),
        ];
        card.ignores = vec!["Encryption to the PCA, which is the KEM's cost.".to_string()];
        card.validation.tests =
            vec!["signing::tests::a_hand_computed_window_reproduces_its_numbers".to_string()];
        card
    }

    #[allow(clippy::too_many_lines)]
    fn definitions(&self) -> Vec<MetricDef> {
        let src = cards::design("08-measurement-and-data.md §2.2");
        let eng = |caution: f64,
                   unhealthy: f64,
                   better: Better,
                   judged: &str,
                   notes: [&str; 3],
                   why: &str| {
            HealthBand {
                better,
                judged: judged.to_string(),
                caution,
                unhealthy,
                healthy_note: notes[0].to_string(),
                caution_note: notes[1].to_string(),
                unhealthy_note: notes[2].to_string(),
                source: why.to_string(),
                basis: Basis::Engineering,
            }
        };
        let m = |name: &str, unit: &str, agg: Agg, q: Quantum, def: &str, omit: &str| {
            MetricDef::new(name, unit, agg, Visibility::Node, q, def.to_string())
                .with_dims([Dim::T])
                .with_source(src.clone())
                .with_min_samples(1)
                .not_accounting_for(omit.to_string())
        };
        let share = |name: &str, num: &str, den: &str, def: &str, omit: &str| {
            m(
                name,
                "ratio",
                Agg::ratio(num, den),
                Quantum::RATIO,
                def,
                omit,
            )
            .with_range(0.0, 1.0)
        };
        let budget = "engineering choice against the 100 ms safety budget of 3GPP TS 22.185 §5.1";
        vec![
            m("sig_signature_bytes", "B/msg", Agg::ratio("Σ signature octets", "signed messages"),
              Quantum::BYTES,
              "Signature octets per signed message: the scheme's encoded signature, both halves \
               for a hybrid (the scheme registry's figure).",
              "messages signed without an envelope split on node.tx"),
            m("sig_signature_rate", "B/s", Agg::Rate, Quantum::BYTES,
              "Signature octets put on the air per second, by every sender together.",
              "retransmissions below the network layer"),
            m("sig_signer_id_bytes", "B/msg", Agg::ratio("Σ signer-identifier octets", "signed messages"),
              Quantum::BYTES,
              "Signer-identifier octets per signed message: the attached certificate when one \
               is sent, the 8-octet HashedId8 digest otherwise.",
              "the identifier's choice tag"),
            m("sig_cert_bytes", "B/msg", Agg::ratio("Σ attached-certificate octets", "signed messages"),
              Quantum::BYTES,
              "Attached-certificate octets per signed message, averaged over messages with and \
               without one.",
              "peer-to-peer certificate responses"),
            share("sig_cert_airtime_share", "Σ certificate octets", "Σ octets on the air",
              "The share of the octets on the air that are attached certificates.",
              "the PHY preamble, which is air time but not octets")
              .with_health(eng(0.15, 0.30, Better::Lower, "", [
                  "certificates are under 15 % of what goes on the air",
                  "certificates are over 15 % of the air: attach them less often",
                  "certificates are over 30 % of the air"],
                  "engineering choice: one certificate a second with ECDSA P-256 is about 10 %")),
            share("sig_signer_id_airtime_share", "Σ signer-identifier octets", "Σ octets on the air",
              "The share of the octets on the air that are signer identifiers, certificate or \
               digest.",
              "the PHY preamble"),
            share("sig_security_airtime_share", "Σ envelope octets", "Σ octets on the air",
              "The share of the octets on the air that are the security envelope — headers, \
               signer identifier and signature — with certificates.",
              "the PHY preamble")
              .with_health(eng(0.35, 0.60, Better::Lower, "", [
                  "security is under 35 % of the air (ECDSA P-256 BSMs are about 30 %)",
                  "security is over 35 % of the air: the scheme costs more than the payload",
                  "security is over 60 % of the air: most of the channel carries signatures"],
                  "engineering choice anchored on the ECDSA P-256 BSM of SAE J2945/1")),
            share("sig_security_airtime_share_nocert", "Σ envelope octets less certificates",
              "Σ octets on the air less certificates",
              "The security envelope's share of the air had no certificate been attached: \
               what signing alone costs.",
              "the PHY preamble"),
            m("sig_sign_latency", "ms", Agg::Distribution, Quantum::TIME_MS,
              "Time to make one signature (signer start to signature done), labelled \
               scheme@engine with the engine the device's hardware profile signs on.",
              "the wait for the signer, which is sig_sign_wait")
              .with_dims([Dim::T, Dim::Primitive])
              .with_health(eng(10.0, 20.0, Better::Lower, ".p95", [
                  "p95 signing within 10 ms",
                  "p95 signing above 10 ms: a tenth of the 100 ms budget",
                  "p95 signing above 20 ms: a fifth of the 100 ms budget"], budget)),
            m("sig_sign_wait", "ms", Agg::Distribution, Quantum::TIME_MS,
              "Time a message waited for the signer, generation to signer start.",
              "time spent after the signature, in the hand-off and the MAC"),
            m("sig_verify_latency", "ms", Agg::Distribution, Quantum::TIME_MS,
              "Time to check one signature (verification start to done), labelled \
               scheme@engine with the engine the device verifies on.",
              "the queue wait before it, which is verify_wait")
              .with_dims([Dim::T, Dim::Primitive])
              .with_health(eng(5.0, 10.0, Better::Lower, ".p95", [
                  "p95 verification within 5 ms",
                  "p95 verification above 5 ms: a busy neighbourhood will queue",
                  "p95 verification above 10 ms: under 100 verifications a second"], budget)),
            share("sig_signer_util", "Σ signing service time per node", "window",
              "Mean over signing nodes of the fraction of the window the signer was busy.",
              "other work sharing the signing engine")
              .with_health(eng(0.5, 0.8, Better::Lower, "", [
                  "signers busy under half the time",
                  "signers busy over half the time: queueing grows quickly from here",
                  "signers busy over 80 %: messages wait for signatures"],
                  "engineering choice: an M/D/1 queue's wait doubles between 50 % and 67 % load")),
            share("sig_signer_util_max", "max Σ signing service time per node", "window",
              "The busiest signer's fraction of the window spent signing.",
              "other work sharing the signing engine"),
            share("sig_verifier_util", "Σ verification service time per node", "window",
              "Mean over verifying nodes of the fraction of the window spent verifying.",
              "verifications dropped before they ran"),
            share("sig_verifier_util_max", "max Σ verification service time per node", "window",
              "The busiest verifier's fraction of the window spent verifying.",
              "verifications dropped before they ran")
              .with_health(eng(0.7, 0.9, Better::Lower, "", [
                  "the busiest verifier has spare capacity",
                  "the busiest verifier is over 70 % busy",
                  "the busiest verifier is over 90 % busy: its queue is growing"],
                  "engineering choice: queue wait grows without bound as load approaches 1")),
            m("sig_verify_capacity", "1/s", Agg::Mean, Quantum::COUNT,
              "Verifications one engine can complete per second: one over the mean \
               verification service time.",
              "parallel verification engines, which the profile does not model"),
            m("sig_neighbour_capacity", "count", Agg::Mean, Quantum::COUNT,
              "How many neighbours a device can verify every message from before its queue \
               grows: verification capacity over each sender's message rate.",
              "a verification policy that skips messages")
              .with_health(eng(150.0, 50.0, Better::Higher, "", [
                  "a device keeps up with more than 150 neighbours",
                  "a device keeps up with fewer than 150 neighbours: a dense junction overloads it",
                  "a device keeps up with fewer than 50 neighbours"],
                  "engineering choice: dense urban V2X evaluations reach 100-200 neighbours (3GPP TR 37.885 urban grid)")),
            m("sig_verify_headroom", "×", Agg::Mean, Quantum::RATIO,
              "Verification capacity over the verifications a node is actually asked for per \
               second: how many times the load could grow before the queue does.",
              "bursts inside the window")
              .with_health(eng(2.0, 1.0, Better::Higher, "", [
                  "verification could take twice the load",
                  "less than twice the load fits: one busy junction fills the queue",
                  "below 1: verifications arrive faster than they can be checked"],
                  "engineering choice: capacity / arrival rate below 1 is an unstable queue")),
            m("sig_verify_dropped", "ratio", Agg::ratio("verifications dropped", "verification tasks"),
              Quantum::RATIO,
              "Verification tasks dropped by queue overflow or policy before they ran.",
              "messages lost before reaching the verifier")
              .with_range(0.0, 1.0)
              .with_health(eng(0.01, 0.05, Better::Lower, "", [
                  "under 1 % of verifications dropped",
                  "over 1 % dropped: the verifier cannot keep up",
                  "over 5 % dropped"],
                  "engineering choice matching the run analyst's 5 % verification bottleneck")),
            m("sig_verify_late", "ratio", Agg::ratio("verifications over 100 ms", "verifications completed"),
              Quantum::RATIO,
              "Completed verifications whose enqueue-to-done time exceeded the 100 ms safety \
               budget on verification alone.",
              "the rest of the end-to-end path")
              .with_range(0.0, 1.0)
              .with_health(eng(0.01, 0.05, Better::Lower, "", [
                  "under 1 % of verifications late",
                  "over 1 % of verifications finish after the 100 ms budget",
                  "over 5 % late"], budget)),
            m("sig_fragments_per_msg", "frames/msg", Agg::ratio("frames on the air", "messages sent"),
              Quantum::RATIO,
              "Frames on the air per message: 1 when every signed message fits the MTU.",
              "fragments the MAC refused before the air"),
            share("sig_fragmented_share", "messages in more than one frame", "messages sent",
              "The share of messages that had to be fragmented, which large signatures cause.",
              "certificate-only fragments sent on a cycle")
              .with_health(eng(0.0, 0.5, Better::Lower, "", [
                  "no message is fragmented",
                  "some messages are fragmented: each piece must arrive",
                  "most messages are fragmented"],
                  "engineering choice: any fragmentation multiplies loss")),
            m("sig_reassembly_loss", "ratio", Agg::ratio("fragmented SDUs failed or expired", "fragmented SDUs resolved"),
              Quantum::RATIO,
              "Fragmented messages a receiver could not reassemble: the delivery lost to \
               fragmentation.",
              "receivers that heard no fragment at all")
              .with_range(0.0, 1.0)
              .with_health(eng(0.05, 0.10, Better::Lower, "", [
                  "under 5 % of fragmented messages lost in reassembly",
                  "over 5 % lost in reassembly",
                  "over 10 % lost in reassembly"],
                  "engineering choice matching the 0.9 reception level of 3GPP TR 37.885")),
            m("sig_e2e_added", "ms", Agg::Distribution, Quantum::TIME_MS,
              "End-to-end latency added by security on a delivered message: the wait for and \
               time of signing, plus the wait for and time of verification.",
              "the extra air time of the envelope, which is a fraction of a millisecond")
              .with_health(eng(20.0, 50.0, Better::Lower, ".p95", [
                  "security adds under 20 ms at p95",
                  "security adds over 20 ms at p95",
                  "security adds over half the 100 ms budget at p95"], budget)),
            share("sig_e2e_added_share", "Σ latency added by security", "Σ end-to-end latency",
              "The share of end-to-end latency that is signing and verification.",
              "messages delivered without timestamps"),
            share("sig_cbr_extra", "mean CBR × security octet share", "1",
              "Estimated channel busy ratio the security envelope adds: the measured CBR \
               times the envelope's share of the octets on the air.",
              "the preamble share of each frame, so this slightly overstates")
              .with_health(eng(0.05, 0.15, Better::Lower, "", [
                  "security adds under 0.05 CBR",
                  "security adds over 0.05 CBR",
                  "security adds over 0.15 CBR: a quarter of the ETSI 0.68 target"],
                  "engineering choice against the ETSI TS 102 687 CBR target 0.68")),
            m("sig_backend_bytes", "B/s", Agg::Rate, Quantum::BYTES,
              "Credential-system octets to and from devices per second: certificate \
               provisioning, downloads and the requests for them.",
              "traffic between authorities")
              .with_visibility_public(),
            m("sig_ca_sign_rate", "1/s", Agg::Rate, Quantum::COUNT,
              "Signatures the certificate authorities made per second.",
              "signatures made at setup by offline authorities")
              .with_visibility_public(),
            m("sig_ca_busy", "ms/s", Agg::Rate, Quantum::TIME_MS,
              "Certificate-authority service time per second of simulated time: what the \
               scheme costs the backend.",
              "service time spent on work other than signing")
              .with_visibility_public(),
        ]
    }

    fn def(&self, name: &str) -> MetricDef {
        self.definitions()
            .into_iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("metric {name} is not one of this provider's definitions"))
    }

    fn on_tx(&mut self, v: &NodeTxView) {
        let w = &mut self.w;
        w.air += v.bytes_on_wire;
        w.frames += 1;
        // The fragments of one message are separate frames with separate frame ids; what
        // they share is the sender and the generation instant.
        let key = (v.node, v.t_generated.unwrap_or(u64::MAX - w.frames));
        // A fragment's `envelope_bytes` is the share of the envelope that fragment carries,
        // so it is summed over every frame; `cert_bytes` is the whole certificate repeated on
        // every fragment, so it counts once per message, with the signer identifier.
        if let Some(env) = v.envelope_bytes {
            w.envelope += env;
        }
        let n = w.messages.entry(key).or_insert(0);
        *n += 1;
        if *n > 1 {
            return;
        }
        if v.envelope_bytes.is_some() {
            let cert = v.cert_bytes.unwrap_or(0);
            w.cert += cert;
            w.signed_msgs += 1;
            w.signer_id += match v.signer {
                Some(SignerId::Certificate) => cert,
                Some(SignerId::Digest) => DIGEST_OCTETS,
                _ => 0,
            };
        }
        if let (Some(s), Some(d)) = (v.t_sign_start, v.t_signed)
            && d >= s
        {
            w.sign_ms.observe((d - s) as f64 / 1e6);
            *w.sign_busy.entry(v.node).or_insert(0) += d - s;
            if let Some(g) = v.t_generated
                && s >= g
            {
                w.sign_wait_ms.observe((s - g) as f64 / 1e6);
            }
        }
    }

    fn on_verify(&mut self, v: &VerifyRec) {
        let w = &mut self.w;
        *w.verify_arrivals.entry(v.node).or_insert(0) += 1;
        match v.outcome {
            VerifyOutcome::Valid | VerifyOutcome::Invalid => {
                w.dropped.observe(false);
                let service = match (v.t_start, v.t_done) {
                    (Some(s), Some(d)) if d >= s => Some(d - s),
                    _ => v.cost_us.map(|us| us * 1000),
                };
                if let Some(ns) = service {
                    w.verify_ms.observe(ns as f64 / 1e6);
                    *w.verify_busy.entry(v.node).or_insert(0) += ns;
                }
                if let Some(d) = v.t_done
                    && d >= v.t_enqueue
                {
                    w.late.observe((d - v.t_enqueue) as f64 / 1e6 > LATE_MS);
                }
            }
            // Only a full queue is the verifier failing to keep up; a policy drop is a choice.
            VerifyOutcome::Dropped => {
                if v.reason.as_deref() == Some(VERIFY_OVERFLOW) {
                    w.dropped.observe(true);
                }
            }
            VerifyOutcome::Skipped => {}
        }
    }

    fn on_rx(&mut self, v: &NodeRxView) {
        if v.outcome != RxFate::Delivered {
            return;
        }
        let (Some(g), Some(signed), Some(e2e)) = (v.t_generated, v.t_signed, v.e2e_ns()) else {
            return;
        };
        let sign = signed.saturating_sub(g);
        let verify = match (v.t_rx_done, v.t_verify_done) {
            (Some(a), Some(b)) => b.saturating_sub(a),
            _ => 0,
        };
        let added = sign + verify;
        let w = &mut self.w;
        w.added_ms.observe(added as f64 / 1e6);
        w.added_sum_ns += added;
        w.e2e_sum_ns += e2e;
        w.added_n += 1;
    }

    fn on_frag(&mut self, v: &NetFragView) {
        match v.outcome {
            FragOutcome::Complete => self.w.reassembly.observe(false),
            FragOutcome::Failed | FragOutcome::Expired => self.w.reassembly.observe(true),
            FragOutcome::Pending | FragOutcome::Duplicate => {}
        }
    }

    fn on_backend(&mut self, v: &BackendLite) {
        let mut t = BackendTotals::default();
        for e in &v.edges {
            if e.from == "ee" || e.to == "ee" {
                t.device_bytes += e.bytes;
            }
        }
        for e in &v.entities {
            if e.tier == "ca" {
                t.ca_signs += e
                    .ops
                    .iter()
                    .filter(|(k, _)| k.ends_with(" sign"))
                    .map(|(_, n)| *n)
                    .sum::<u64>();
                t.ca_busy_ns += e.queue.as_ref().map_or(0, |q| q.busy_ns);
            }
        }
        self.backend_now = Some(t);
    }

    fn scalar(&self, name: &str, at: SimTime, dims: Dims, v: Option<(f64, u64)>) -> MetricSample {
        let value = match v {
            Some((point, n)) if point.is_finite() => Estimate::Value { point, n },
            Some((_, n)) => Estimate::Insufficient { n, required: 1 },
            None => Estimate::Insufficient { n: 0, required: 1 },
        };
        MetricSample::new(&self.def(name), at, dims, SampleValue::Scalar(value))
    }

    fn ratio(&self, name: &str, at: SimTime, num: f64, den: f64, n: u64) -> MetricSample {
        MetricSample::new(
            &self.def(name),
            at,
            Dims::new(),
            SampleValue::Ratio(ratio_of_sums(num, den, n, 1)),
        )
    }

    fn prop(&self, name: &str, at: SimTime, p: Proportion) -> MetricSample {
        MetricSample::new(
            &self.def(name),
            at,
            Dims::new(),
            SampleValue::Ratio(p.estimate(self.min_samples, self.level)),
        )
    }

    fn dist(&self, name: &str, at: SimTime, dims: Dims, d: &Distribution) -> MetricSample {
        MetricSample::new(
            &self.def(name),
            at,
            dims,
            SampleValue::Distribution(d.summary(self.min_samples)),
        )
    }
}

trait PublicVisibility {
    fn with_visibility_public(self) -> Self;
}

impl PublicVisibility for MetricDef {
    fn with_visibility_public(mut self) -> Self {
        self.visibility = Visibility::Public;
        self
    }
}

/// Mean and max over a per-node map of busy nanoseconds, as fractions of `window_ns`.
fn util(busy: &BTreeMap<NodeId, u64>, window_ns: u64) -> Option<(f64, f64, u64)> {
    if busy.is_empty() || window_ns == 0 {
        return None;
    }
    let total: u64 = busy.values().sum();
    let max = busy.values().copied().max().unwrap_or(0);
    let n = busy.len() as u64;
    Some((
        total as f64 / n as f64 / window_ns as f64,
        max as f64 / window_ns as f64,
        n,
    ))
}

impl Model for SigningProvider {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl MetricProvider for SigningProvider {
    fn defs(&self) -> Vec<MetricDef> {
        self.definitions()
    }

    fn subscribe(&self) -> Vec<ChannelName> {
        vec![
            NodeTxView::channel_name(),
            VerifyRec::channel_name(),
            NodeRxView::channel_name(),
            NetFragView::channel_name(),
            MacCbrView::channel_name(),
            BackendLite::channel_name(),
        ]
    }

    fn on_event(&mut self, ev: &EventRecord) {
        self.on_decoded(&Decoded::new(ev));
    }

    fn on_decoded(&mut self, d: &Decoded<'_>) {
        let ev = d.record();
        match ev.channel {
            NodeTxView::CHANNEL => d.with(|v: Option<&NodeTxView>| match v {
                Some(v) => self.on_tx(v),
                None => self.rejected += 1,
            }),
            VerifyRec::CHANNEL => match decode::<VerifyRec>(ev) {
                Ok(v) => self.on_verify(&v),
                Err(_) => self.rejected += 1,
            },
            NodeRxView::CHANNEL => d.with(|v: Option<&NodeRxView>| match v {
                Some(v) => self.on_rx(v),
                None => self.rejected += 1,
            }),
            NetFragView::CHANNEL => match decode::<NetFragView>(ev) {
                Ok(v) => self.on_frag(&v),
                Err(_) => self.rejected += 1,
            },
            MacCbrView::CHANNEL => match decode::<MacCbrView>(ev) {
                Ok(v) if v.cbr.is_finite() => {
                    self.w.cbr_sum += v.cbr;
                    self.w.cbr_n += 1;
                }
                Ok(_) => {}
                Err(_) => self.rejected += 1,
            },
            BackendLite::CHANNEL => match decode::<BackendLite>(ev) {
                Ok(v) => self.on_backend(&v),
                Err(_) => self.rejected += 1,
            },
            _ => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    fn flush(&mut self, at: SimTime) -> Vec<MetricSample> {
        let w = core::mem::take(&mut self.w);
        let window_ns = at.saturating_sub(self.window_start);
        let secs = (window_ns > 0).then(|| Duration::from_nanos(window_ns).as_secs_f64());
        let mut out = Vec::new();
        let sig = w.signed_msgs * self.facts.sig_octets;
        let msgs = w.messages.len() as u64;

        out.push(self.ratio(
            "sig_signature_bytes",
            at,
            sig as f64,
            w.signed_msgs as f64,
            w.signed_msgs,
        ));
        out.push(self.scalar(
            "sig_signature_rate",
            at,
            Dims::new(),
            secs.map(|s| (sig as f64 / s, w.signed_msgs)),
        ));
        out.push(self.ratio(
            "sig_signer_id_bytes",
            at,
            w.signer_id as f64,
            w.signed_msgs as f64,
            w.signed_msgs,
        ));
        out.push(self.ratio(
            "sig_cert_bytes",
            at,
            w.cert as f64,
            w.signed_msgs as f64,
            w.signed_msgs,
        ));
        out.push(self.ratio(
            "sig_cert_airtime_share",
            at,
            w.cert as f64,
            w.air as f64,
            w.frames,
        ));
        out.push(self.ratio(
            "sig_signer_id_airtime_share",
            at,
            w.signer_id as f64,
            w.air as f64,
            w.frames,
        ));
        out.push(self.ratio(
            "sig_security_airtime_share",
            at,
            w.envelope as f64,
            w.air as f64,
            w.frames,
        ));
        out.push(self.ratio(
            "sig_security_airtime_share_nocert",
            at,
            w.envelope.saturating_sub(w.cert) as f64,
            w.air.saturating_sub(w.cert) as f64,
            w.frames,
        ));

        let label = |engine: &str| {
            let mut d = Dims::new();
            d.insert(
                Dim::Primitive,
                DimValue::label(format!("{}@{engine}", self.facts.scheme)),
            );
            d
        };
        out.push(self.dist(
            "sig_sign_latency",
            at,
            label(&self.facts.sign_engine),
            &w.sign_ms,
        ));
        out.push(self.dist("sig_sign_wait", at, Dims::new(), &w.sign_wait_ms));
        out.push(self.dist(
            "sig_verify_latency",
            at,
            label(&self.facts.verify_engine),
            &w.verify_ms,
        ));

        let su = util(&w.sign_busy, window_ns);
        out.push(self.scalar(
            "sig_signer_util",
            at,
            Dims::new(),
            su.map(|(m, _, n)| (m, n)),
        ));
        out.push(self.scalar(
            "sig_signer_util_max",
            at,
            Dims::new(),
            su.map(|(_, x, n)| (x, n)),
        ));
        let vu = util(&w.verify_busy, window_ns);
        out.push(self.scalar(
            "sig_verifier_util",
            at,
            Dims::new(),
            vu.map(|(m, _, n)| (m, n)),
        ));
        out.push(self.scalar(
            "sig_verifier_util_max",
            at,
            Dims::new(),
            vu.map(|(_, x, n)| (x, n)),
        ));

        // Capacity: one over the mean service time of the window's verifications.
        let vn = w.verify_ms.len() as u64;
        let mean_ms = w.verify_busy.values().sum::<u64>() as f64 / 1e6 / vn.max(1) as f64;
        let capacity = (vn > 0 && mean_ms > 0.0).then(|| 1000.0 / mean_ms);
        out.push(self.scalar(
            "sig_verify_capacity",
            at,
            Dims::new(),
            capacity.map(|c| (c, vn)),
        ));
        // Messages per second per sender, from the window's distinct messages.
        let senders: BTreeSet<NodeId> = w.messages.keys().map(|(n, _)| *n).collect();
        let per_sender = match secs {
            Some(s) if !senders.is_empty() => Some(msgs as f64 / senders.len() as f64 / s),
            _ => None,
        };
        out.push(
            self.scalar(
                "sig_neighbour_capacity",
                at,
                Dims::new(),
                capacity
                    .zip(per_sender)
                    .filter(|(_, r)| *r > 0.0)
                    .map(|(c, r)| (c / r, vn)),
            ),
        );
        let arrivals = match secs {
            Some(s) if !w.verify_arrivals.is_empty() => Some(
                w.verify_arrivals.values().sum::<u64>() as f64 / w.verify_arrivals.len() as f64 / s,
            ),
            _ => None,
        };
        out.push(
            self.scalar(
                "sig_verify_headroom",
                at,
                Dims::new(),
                capacity
                    .zip(arrivals)
                    .filter(|(_, a)| *a > 0.0)
                    .map(|(c, a)| (c / a, vn)),
            ),
        );
        out.push(self.prop("sig_verify_dropped", at, w.dropped));
        out.push(self.prop("sig_verify_late", at, w.late));

        out.push(self.ratio(
            "sig_fragments_per_msg",
            at,
            w.frames as f64,
            msgs as f64,
            msgs,
        ));
        let mut frag = Proportion::new();
        for n in w.messages.values() {
            frag.observe(*n > 1);
        }
        out.push(self.prop("sig_fragmented_share", at, frag));
        out.push(self.prop("sig_reassembly_loss", at, w.reassembly));

        out.push(self.dist("sig_e2e_added", at, Dims::new(), &w.added_ms));
        out.push(self.ratio(
            "sig_e2e_added_share",
            at,
            w.added_sum_ns as f64,
            w.e2e_sum_ns as f64,
            w.added_n,
        ));
        let cbr_extra = (w.cbr_n > 0 && w.air > 0)
            .then(|| (w.cbr_sum / w.cbr_n as f64) * (w.envelope as f64 / w.air as f64));
        out.push(self.ratio(
            "sig_cbr_extra",
            at,
            cbr_extra.unwrap_or(0.0),
            1.0,
            if cbr_extra.is_some() { w.cbr_n } else { 0 },
        ));

        if let (Some(now), Some(s)) = (self.backend_now, secs) {
            let prev = self.backend_prev;
            let d = |a: u64, b: u64| a.saturating_sub(b) as f64 / s;
            out.push(self.scalar(
                "sig_backend_bytes",
                at,
                Dims::new(),
                Some((d(now.device_bytes, prev.device_bytes), 1)),
            ));
            out.push(self.scalar(
                "sig_ca_sign_rate",
                at,
                Dims::new(),
                Some((d(now.ca_signs, prev.ca_signs), 1)),
            ));
            out.push(self.scalar(
                "sig_ca_busy",
                at,
                Dims::new(),
                Some((d(now.ca_busy_ns, prev.ca_busy_ns) / 1e6, 1)),
            ));
            self.backend_prev = now;
        }

        self.window_start = at;
        out
    }

    fn rejected(&self) -> u64 {
        self.rejected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use v2xw_core::ctx::OwnedRecord;

    fn rec(channel: &'static str, json: serde_json::Value) -> OwnedRecord {
        OwnedRecord {
            channel,
            visibility: Visibility::Node,
            json: serde_json::to_vec(&json).unwrap(),
        }
    }

    fn point(samples: &[MetricSample], key: &str) -> f64 {
        samples
            .iter()
            .find(|s| s.key() == key)
            .and_then(|s| s.value.point())
            .unwrap_or_else(|| panic!("no value for {key}"))
    }

    #[test]
    fn every_definition_validates_and_the_card_is_accepted() {
        let p = SigningProvider::new(0, SchemeFacts::default());
        p.validate_defs().unwrap();
        p.card().validate().unwrap();
    }

    /// Two 1,000-octet frames of one fragmented message and one 400-octet message, a
    /// 2 ms signature each, two 1 ms verifications at one node, over one second.
    #[test]
    fn a_hand_computed_window_reproduces_its_numbers() {
        let facts = SchemeFacts {
            scheme: "ml-dsa-44".into(),
            mode: "post-quantum".into(),
            sig_octets: 2420,
            sign_engine: "cpu".into(),
            verify_engine: "cpu".into(),
        };
        let mut p = SigningProvider::new(0, facts);
        let tx = |msg: u64, bytes: u64, env: u64, cert: u64, signer: &str| {
            rec(
                "node.tx",
                json!({"t": 10_000_000, "node": 1, "msg": msg, "bytes_on_wire": bytes,
                "payload_bytes": 100, "envelope_bytes": env, "cert_bytes": cert, "signer": signer,
                "t_generated": msg, "t_sign_start": 1_000_000, "t_signed": 3_000_000}),
            )
        };
        for r in [
            tx(7, 1000, 850, 0, "digest"),
            tx(7, 1000, 650, 0, "digest"),
            tx(8, 400, 200, 100, "certificate"),
        ] {
            p.on_event(&r);
        }
        for i in 0..2u64 {
            p.on_event(&rec(
                "node.verify",
                json!({"t_enqueue": 0, "t_start": i * 1_000_000,
                "t_done": (i + 1) * 1_000_000, "node": 2, "outcome": "valid"}),
            ));
        }
        let s = p.flush(1_000_000_000);
        assert_eq!(point(&s, "sig_signature_bytes"), 2420.0);
        assert_eq!(point(&s, "sig_signer_id_bytes"), (8.0 + 100.0) / 2.0);
        assert!((point(&s, "sig_security_airtime_share") - 1700.0 / 2400.0).abs() < 1e-3);
        assert_eq!(point(&s, "sig_fragments_per_msg"), 1.5);
        assert_eq!(point(&s, "sig_fragmented_share"), 0.5);
        assert!((point(&s, "sig_signer_util") - 0.004).abs() < 1e-4);
        assert_eq!(point(&s, "sig_verify_capacity"), 1000.0);
        // Two messages a second from one sender: 1000 / 2 neighbours.
        assert_eq!(point(&s, "sig_neighbour_capacity"), 500.0);
        assert_eq!(point(&s, "sig_verify_headroom"), 500.0);
    }
}
