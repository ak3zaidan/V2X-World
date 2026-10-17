//! Peer-to-peer certificate distribution: the SSME's state machine (IEEE 1609.2 clause 8).
//!
//! WP-C3. Replaces the counter that stood in for P2PCD: a node now *asks* for a CA
//! certificate it lacks, a peer that holds it *answers*, and every node that hears the
//! answer *learns* it. The wire formats are `v2xw_sec::p2pcd` and the envelope's
//! `HeaderInfo` fields; this module decides when each is used.
//!
//! # The rules, clause by clause (IEEE 1609.2a-2017, the amendment that wrote clause 8 in
//! its present form; 1609.2-2022 keeps it)
//!
//! | Role | Flavour | Rule | Here |
//! |---|---|---|---|
//! | Requester | out-of-band | §8.2.4.1.1 a): a received SPDU with an attached certificate whose issuer is unknown starts a request for that issuer, "unless at least one of the following exception conditions holds" | [`P2pcd::on_unknown_issuer`] |
//! | | | §8.2.3 `p2pcd_requestActiveTimeout`: no second request for the same certificate sooner | `requests_suppressed_active` |
//! | | | §8.2.3 `p2pcd_observedRequestTimeout`: no request for a certificate a peer asked for this recently | `requests_suppressed_observed` |
//! | | | §6.3.9: the request is the `HashedId3` in `p2pcdLearningRequest` of the requester's next signed SPDU (one per SPDU) | [`P2pcd::take_header`] |
//! | Requester | inline | §8.2.4.1.2 b)-d): an unknown signer digest, or an unknown issuer, adds a `HashedId3` to `p2pcd_inline_potentiallyRequestedCerts`; the next signed SPDU carries a selection in `inlineP2pcdRequest` and the array is emptied | [`P2pcd::on_unknown_signer_digest`], [`P2pcd::take_header`] |
//! | Responder | out-of-band | §8.2.4.2.2 a)1): only for a CA certificate in the chain of a certificate the node signed with within `p2pcd_currentlyUsedTriggerCertificateTime` | [`P2pcd::note_signed_with`] |
//! | | | exceptions: `p2pcd_responseActiveTimeout` since the last trigger for it; more active responses than the implementation supports | `responses_suppressed_timeout`, `responses_suppressed_capacity` |
//! | | | §8.2.1 and Figure 14 c): a random back-off up to `p2pcd_maxResponseBackoff`, then a response only if fewer than `p2pcd_responseCountThreshold` responses were observed | [`P2pcd::due_responses`], `responses_suppressed_threshold` |
//! | Responder | inline | §8.2.4.2.3 c)-f): requests heard go into `p2pcd_inline_requestedCerts`; a `requestedCertificate` heard removes its entry; at the next signing, the node's own certificate is attached if asked for, else one requested CA certificate it knows goes in `requestedCertificate`; the array is emptied | [`P2pcd::take_header`] |
//! | Everyone | both | §8.2.1: the P2PCD Entity monitors responses "even if its WAVE device has not recently requested" one, and learns from them | the caller, `NodeSecurity::p2pcd_learn` |
//!
//! The base text of 1609.2-2016 §8.2.4.1.1 lists the requester's exception conditions;
//! the amendment reprints only the parameters, and this module applies the two that the
//! parameters define. The observed-request rule's text says "for at least time
//! p2pcd_requestActiveTimeout"; this module reads the parameter it is named for
//! (`p2pcd_observedRequestTimeout`). Both are 0.25 s in Annex C.2.1.3.1, so the reading
//! does not move a number at the recommended values.
//!
//! # What the standard leaves open, and what this module chose
//!
//! * Which pending request goes first when several are queued: the oldest.
//! * How many entries an inline request carries, and how many response processes may run
//!   at once: [`P2pcdParams::max_inline_entries`], [`P2pcdParams::max_active_responses`]
//!   (both ASSUMPTIONS, 8).
//! * Whether a node validates what it learns: yes, before it uses it
//!   (`NodeSecurity::p2pcd_learn`, the CA path to an installed root). The standard
//!   matches a response to a request by its `HashedId3`, three octets, so a forged
//!   certificate with a matching `HashedId3` costs about 2^24 hashes. Such a response is
//!   counted as observed by every responder (it is indistinguishable on the air), so it
//!   can push the response count past the threshold and silence the honest responders:
//!   that is the protocol as written, and the counters make it measurable.
//! * A pending out-of-band request is withdrawn when a peer's request for the same
//!   certificate is heard first, as the observed-request rule intends.
//! * A root certificate is never served or learned: roots come from the CTL.

use std::collections::{BTreeMap, BTreeSet};

use v2xw_core::time::SimTime;
use v2xw_msg::sec_types::HashedId3;

pub use v2xw_sec::p2pcd::{P2pcdFlavor, P2pcdParams};

/// A `HashedId3` as bytes.
pub type Id3 = [u8; 3];

/// The bytes of a `HashedId3`.
pub fn id3_bytes(h: &HashedId3) -> Id3 {
    let mut out = [0u8; 3];
    out.copy_from_slice(&h.0[..]);
    out
}

/// A `HashedId3` from bytes.
pub fn id3_of(b: Id3) -> HashedId3 {
    HashedId3(b.into())
}

/// What P2PCD did at one node. Every counter is a protocol event, so a run can be
/// benchmarked (frames per learned certificate, time to learn) and attacked (forged
/// responses, suppression) from these alone.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct P2pcdStats {
    /// Unknown certificates that started a request process (out-of-band) or were added to
    /// the inline request array.
    pub triggers: u64,
    /// Request processes not started: the same certificate was requested within
    /// `p2pcd_requestActiveTimeout`.
    pub requests_suppressed_active: u64,
    /// Request processes not started, or withdrawn: a peer requested the certificate
    /// within `p2pcd_observedRequestTimeout`.
    pub requests_suppressed_observed: u64,
    /// Signed SPDUs this node sent carrying `p2pcdLearningRequest`.
    pub requests_sent: u64,
    /// Signed SPDUs this node sent carrying `inlineP2pcdRequest`.
    pub inline_requests_sent: u64,
    /// `HashedId3` entries in those inline requests.
    pub inline_entries_sent: u64,
    /// Requests heard from peers (either flavour).
    pub requests_observed: u64,
    /// Out-of-band response processes started (a back-off timer was set).
    pub responses_triggered: u64,
    /// Not started: `p2pcd_responseActiveTimeout` since the last one for this certificate.
    pub responses_suppressed_timeout: u64,
    /// Not started: [`P2pcdParams::max_active_responses`] already running.
    pub responses_suppressed_capacity: u64,
    /// Back-off expired with `p2pcd_responseCountThreshold` responses already observed.
    pub responses_suppressed_threshold: u64,
    /// Out-of-band learning responses this node broadcast.
    pub responses_sent: u64,
    /// Signed SPDUs that attached the node's own certificate because a peer asked for it
    /// inline (§8.2.4.2.3 e)1)).
    pub inline_cert_forced: u64,
    /// Signed SPDUs that carried a `requestedCertificate` (§8.2.4.2.3 e)2)).
    pub inline_ca_sent: u64,
    /// Responses heard (learning-response PDUs, or `requestedCertificate` fields).
    pub responses_observed: u64,
    /// CA certificates learned from responses, after validation.
    pub learned: u64,
    /// Certificates in responses that were already held.
    pub learn_known: u64,
    /// Certificates in responses refused by validation (bad chain, a root, an end entity).
    pub learn_refused: u64,
    /// Certificates in responses whose issuer is still unknown (kept out of the cache).
    pub learn_unanchored: u64,
    /// Frames heard on the P2PCD PSID that did not decode as a learning response (a
    /// garbled or hostile PDU): refused before anything was learned or counted as
    /// observed.
    pub responses_malformed: u64,
    /// Σ, over the responses sent, of how long after its back-off expired each one left
    /// (ns): zero when the host wakes the entity at the back-off instant, the step's
    /// quantisation when it does not. A response that leaves late lets more peers'
    /// responses be observed first and moves the threshold suppression.
    pub response_lag_ns: u64,
}

impl P2pcdStats {
    /// What happened since `prev` (counter by counter), or `None` when nothing did.
    pub fn since(&self, prev: &P2pcdStats) -> Option<P2pcdStats> {
        let d = P2pcdStats {
            triggers: self.triggers.saturating_sub(prev.triggers),
            requests_suppressed_active: self
                .requests_suppressed_active
                .saturating_sub(prev.requests_suppressed_active),
            requests_suppressed_observed: self
                .requests_suppressed_observed
                .saturating_sub(prev.requests_suppressed_observed),
            requests_sent: self.requests_sent.saturating_sub(prev.requests_sent),
            inline_requests_sent: self
                .inline_requests_sent
                .saturating_sub(prev.inline_requests_sent),
            inline_entries_sent: self
                .inline_entries_sent
                .saturating_sub(prev.inline_entries_sent),
            requests_observed: self
                .requests_observed
                .saturating_sub(prev.requests_observed),
            responses_triggered: self
                .responses_triggered
                .saturating_sub(prev.responses_triggered),
            responses_suppressed_timeout: self
                .responses_suppressed_timeout
                .saturating_sub(prev.responses_suppressed_timeout),
            responses_suppressed_capacity: self
                .responses_suppressed_capacity
                .saturating_sub(prev.responses_suppressed_capacity),
            responses_suppressed_threshold: self
                .responses_suppressed_threshold
                .saturating_sub(prev.responses_suppressed_threshold),
            responses_sent: self.responses_sent.saturating_sub(prev.responses_sent),
            inline_cert_forced: self
                .inline_cert_forced
                .saturating_sub(prev.inline_cert_forced),
            inline_ca_sent: self.inline_ca_sent.saturating_sub(prev.inline_ca_sent),
            responses_observed: self
                .responses_observed
                .saturating_sub(prev.responses_observed),
            learned: self.learned.saturating_sub(prev.learned),
            learn_known: self.learn_known.saturating_sub(prev.learn_known),
            learn_refused: self.learn_refused.saturating_sub(prev.learn_refused),
            learn_unanchored: self.learn_unanchored.saturating_sub(prev.learn_unanchored),
            responses_malformed: self
                .responses_malformed
                .saturating_sub(prev.responses_malformed),
            response_lag_ns: self.response_lag_ns.saturating_sub(prev.response_lag_ns),
        };
        (d != P2pcdStats::default()).then_some(d)
    }
}

/// One node's P2PCD activity over one step, on `node.p2pcd` (only steps in which
/// something happened). The counters are [`P2pcdStats`] deltas; `t` is the node's clock.
#[derive(Debug, Clone, serde::Serialize)]
pub struct P2pcdRecord {
    /// When the step ended, node clock.
    pub t: SimTime,
    /// The node.
    pub node: v2xw_core::ids::NodeId,
    /// `out-of-band`, `inline` or `none`.
    pub flavor: &'static str,
    /// What happened in the step.
    #[serde(flatten)]
    pub delta: P2pcdStats,
}

impl v2xw_core::ctx::Record for P2pcdRecord {
    const CHANNEL: &'static str = "node.p2pcd";
    const VISIBILITY: v2xw_core::ctx::Visibility = v2xw_core::ctx::Visibility::Node;
}

/// One out-of-band response process (§8.2.4.2.2): the back-off and the responses observed
/// since the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Active {
    fire_at: SimTime,
    observed: u32,
}

/// What [`P2pcd::take_header`] asks the next signed SPDU to carry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderAdditions {
    /// `p2pcdLearningRequest` (out-of-band).
    pub learning_request: Option<HashedId3>,
    /// `inlineP2pcdRequest`.
    pub inline_request: Vec<HashedId3>,
    /// Attach the signer's own certificate (inline response, §8.2.4.2.3 e)1)).
    pub force_certificate: bool,
    /// The `HashedId3` of the CA certificate to put in `requestedCertificate`
    /// (§8.2.4.2.3 e)2)); the caller resolves it.
    pub requested_ca: Option<Id3>,
}

/// The P2PCD state of one node's safety SDEE.
#[derive(Debug, Clone)]
pub struct P2pcd {
    params: P2pcdParams,
    pending: Vec<Id3>,
    last_requested: BTreeMap<Id3, SimTime>,
    last_observed_request: BTreeMap<Id3, SimTime>,
    potentially_requested: Vec<Id3>,
    chain_used: BTreeMap<Id3, SimTime>,
    last_trigger: BTreeMap<Id3, SimTime>,
    active: BTreeMap<Id3, Active>,
    inline_requested: BTreeSet<Id3>,
    /// What the protocol did.
    pub stats: P2pcdStats,
}

fn within(now: SimTime, then: Option<&SimTime>, window: v2xw_core::time::Duration) -> bool {
    then.is_some_and(|t| now.saturating_sub(*t) < window.as_nanos())
}

impl P2pcd {
    /// A machine running `params`.
    pub fn new(params: P2pcdParams) -> P2pcd {
        P2pcd {
            params,
            pending: Vec::new(),
            last_requested: BTreeMap::new(),
            last_observed_request: BTreeMap::new(),
            potentially_requested: Vec::new(),
            chain_used: BTreeMap::new(),
            last_trigger: BTreeMap::new(),
            active: BTreeMap::new(),
            inline_requested: BTreeSet::new(),
            stats: P2pcdStats::default(),
        }
    }

    /// The parameters.
    pub fn params(&self) -> &P2pcdParams {
        &self.params
    }

    /// The flavour.
    pub fn flavor(&self) -> P2pcdFlavor {
        self.params.flavor
    }

    /// Out-of-band requests waiting for the next signed SPDU.
    pub fn pending_requests(&self) -> &[Id3] {
        &self.pending
    }

    /// A received SPDU attached a certificate whose issuer (`issuer`, the `HashedId3` of
    /// the issuer field) this node does not hold (§8.2.4.1.1 a), §8.2.4.1.2 b)2)).
    pub fn on_unknown_issuer(&mut self, issuer: Id3, now: SimTime) {
        match self.params.flavor {
            P2pcdFlavor::None => {}
            P2pcdFlavor::Inline => self.add_inline(issuer),
            P2pcdFlavor::OutOfBand => {
                if self.pending.contains(&issuer) {
                    return;
                }
                if within(
                    now,
                    self.last_requested.get(&issuer),
                    self.params.request_active_timeout,
                ) {
                    self.stats.requests_suppressed_active += 1;
                    return;
                }
                if within(
                    now,
                    self.last_observed_request.get(&issuer),
                    self.params.observed_request_timeout,
                ) {
                    self.stats.requests_suppressed_observed += 1;
                    return;
                }
                self.stats.triggers += 1;
                self.pending.push(issuer);
            }
        }
    }

    /// A received SPDU named its signer by a digest this node cannot resolve
    /// (§8.2.4.1.2 b)1)). Inline only: out-of-band P2PCD "only supports requesting CA
    /// certificates" (§8.2.1).
    pub fn on_unknown_signer_digest(&mut self, digest: Id3) {
        if self.params.flavor == P2pcdFlavor::Inline {
            self.add_inline(digest);
        }
    }

    fn add_inline(&mut self, id: Id3) {
        if !self.potentially_requested.contains(&id) {
            self.stats.triggers += 1;
            self.potentially_requested.push(id);
        }
    }

    /// The node signed at `now` with a certificate whose CA chain (issuer first, the root
    /// excluded) is `chain` (§8.2.4.2.2 a)1)).
    pub fn note_signed_with(&mut self, chain: &[Id3], now: SimTime) {
        for id in chain {
            self.chain_used.insert(*id, now);
        }
    }

    /// A peer's signed SPDU carried P2PCD requests. `draw` is a uniform draw on `[0, 1)`
    /// for the response back-off, taken only when a response process starts.
    pub fn on_requests(
        &mut self,
        learning: Option<Id3>,
        inline: &[Id3],
        now: SimTime,
        draw: &mut dyn FnMut() -> f64,
    ) {
        if let Some(x) = learning {
            self.stats.requests_observed += 1;
            self.last_observed_request.insert(x, now);
            if let Some(i) = self.pending.iter().position(|p| *p == x) {
                self.pending.remove(i);
                self.stats.requests_suppressed_observed += 1;
            }
            if self.params.flavor == P2pcdFlavor::OutOfBand {
                self.trigger_response(x, now, draw);
            }
        }
        if !inline.is_empty() {
            self.stats.requests_observed += 1;
            if self.params.flavor == P2pcdFlavor::Inline {
                self.inline_requested.extend(inline.iter().copied());
            }
        }
    }

    fn trigger_response(&mut self, x: Id3, now: SimTime, draw: &mut dyn FnMut() -> f64) {
        let used = within(
            now,
            self.chain_used.get(&x),
            self.params.currently_used_trigger_certificate_time,
        );
        if !used || self.active.contains_key(&x) {
            return;
        }
        if within(
            now,
            self.last_trigger.get(&x),
            self.params.response_active_timeout,
        ) {
            self.stats.responses_suppressed_timeout += 1;
            return;
        }
        if self.active.len() >= self.params.max_active_responses {
            self.stats.responses_suppressed_capacity += 1;
            return;
        }
        let u = draw().clamp(0.0, 1.0);
        let backoff = (u * self.params.max_response_backoff.as_nanos() as f64) as u64;
        self.active.insert(
            x,
            Active {
                fire_at: now.saturating_add(backoff),
                observed: 0,
            },
        );
        self.last_trigger.insert(x, now);
        self.stats.responses_triggered += 1;
    }

    /// A learning-response PDU was heard whose first certificate is `first`.
    pub fn on_response_observed(&mut self, first: Id3) {
        self.stats.responses_observed += 1;
        if let Some(a) = self.active.get_mut(&first) {
            a.observed = a.observed.saturating_add(1);
        }
    }

    /// A peer's signed SPDU carried `requestedCertificate` with `HashedId3` `id`
    /// (§8.2.4.2.3 d)).
    pub fn on_requested_certificate(&mut self, id: Id3) {
        self.stats.responses_observed += 1;
        self.inline_requested.remove(&id);
    }

    /// The certificate `id` is now held: no request for it is needed.
    pub fn on_learned(&mut self, id: Id3) {
        self.pending.retain(|p| *p != id);
        self.potentially_requested.retain(|p| *p != id);
    }

    /// What the next signed safety SPDU carries. `own` is the `HashedId3` of the signing
    /// certificate; `known_ca` says whether a requested `HashedId3` names a CA
    /// certificate this node holds (with `certIssuePermissions`, and not a root).
    pub fn take_header(
        &mut self,
        now: SimTime,
        own: Option<Id3>,
        known_ca: &dyn Fn(Id3) -> bool,
    ) -> HeaderAdditions {
        let mut h = HeaderAdditions::default();
        match self.params.flavor {
            P2pcdFlavor::None => {}
            P2pcdFlavor::OutOfBand => {
                while !self.pending.is_empty() {
                    let x = self.pending.remove(0);
                    if within(
                        now,
                        self.last_observed_request.get(&x),
                        self.params.observed_request_timeout,
                    ) {
                        self.stats.requests_suppressed_observed += 1;
                        continue;
                    }
                    self.last_requested.insert(x, now);
                    self.stats.requests_sent += 1;
                    h.learning_request = Some(id3_of(x));
                    break;
                }
            }
            P2pcdFlavor::Inline => {
                let take: Vec<Id3> = self
                    .potentially_requested
                    .iter()
                    .take(self.params.max_inline_entries)
                    .copied()
                    .collect();
                self.potentially_requested.clear();
                if !take.is_empty() {
                    self.stats.inline_requests_sent += 1;
                    self.stats.inline_entries_sent += take.len() as u64;
                    h.inline_request = take.into_iter().map(id3_of).collect();
                }
                if own.is_some_and(|o| self.inline_requested.contains(&o)) {
                    self.stats.inline_cert_forced += 1;
                    h.force_certificate = true;
                } else if let Some(ca) =
                    self.inline_requested.iter().copied().find(|i| known_ca(*i))
                {
                    self.stats.inline_ca_sent += 1;
                    h.requested_ca = Some(ca);
                }
                self.inline_requested.clear();
            }
        }
        h
    }

    /// The out-of-band responses whose back-off has expired by `now` and which are still
    /// under the response-count threshold, in back-off order. Each one returned is a
    /// learning response the node broadcasts now.
    pub fn due_responses(&mut self, now: SimTime) -> Vec<Id3> {
        let mut due: Vec<(SimTime, Id3)> = self
            .active
            .iter()
            .filter(|(_, a)| a.fire_at <= now)
            .map(|(k, a)| (a.fire_at, *k))
            .collect();
        due.sort();
        let mut out = Vec::new();
        for (_, k) in due {
            let a = self.active.remove(&k).expect("listed");
            if a.observed < self.params.response_count_threshold {
                self.stats.responses_sent += 1;
                self.stats.response_lag_ns += now.saturating_sub(a.fire_at);
                out.push(k);
            } else {
                self.stats.responses_suppressed_threshold += 1;
            }
        }
        out
    }

    /// When the earliest running back-off expires.
    pub fn next_due(&self) -> Option<SimTime> {
        self.active.values().map(|a| a.fire_at).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::time::NS_PER_MS;

    const X: Id3 = [0xAA, 0xBB, 0xCC];

    fn oob() -> P2pcd {
        P2pcd::new(P2pcdParams::RECOMMENDED)
    }

    #[test]
    fn a_request_is_sent_once_per_request_active_timeout() {
        let mut p = oob();
        p.on_unknown_issuer(X, 0);
        p.on_unknown_issuer(X, 10 * NS_PER_MS); // already pending
        let h = p.take_header(20 * NS_PER_MS, None, &|_| false);
        assert_eq!(h.learning_request.map(|h| id3_bytes(&h)), Some(X));
        p.on_unknown_issuer(X, 200 * NS_PER_MS);
        assert_eq!(p.stats.requests_suppressed_active, 1, "180 ms < 250 ms");
        p.on_unknown_issuer(X, 270 * NS_PER_MS);
        assert_eq!(p.pending_requests(), &[X], "250 ms later it may ask again");
        assert_eq!(p.stats.requests_sent, 1);
    }

    #[test]
    fn a_peers_request_suppresses_and_withdraws_ours() {
        let mut p = oob();
        p.on_unknown_issuer(X, 0);
        p.on_requests(Some(X), &[], 5 * NS_PER_MS, &mut || 0.5);
        assert!(p.pending_requests().is_empty(), "withdrawn");
        p.on_unknown_issuer(X, 100 * NS_PER_MS);
        assert!(p.pending_requests().is_empty(), "observed 95 ms ago");
        assert_eq!(p.stats.requests_suppressed_observed, 2);
        p.on_unknown_issuer(X, 300 * NS_PER_MS);
        assert_eq!(p.pending_requests(), &[X]);
    }

    #[test]
    fn a_responder_backs_off_and_stops_at_the_threshold() {
        let mut p = oob();
        p.note_signed_with(&[X], 0);
        p.on_requests(Some(X), &[], 10 * NS_PER_MS, &mut || 0.4);
        assert_eq!(p.next_due(), Some(10 * NS_PER_MS + 100 * NS_PER_MS));
        assert!(p.due_responses(109 * NS_PER_MS).is_empty());
        assert_eq!(p.due_responses(110 * NS_PER_MS), vec![X]);
        // A second request inside the response-active timeout starts nothing.
        p.on_requests(Some(X), &[], 200 * NS_PER_MS, &mut || 0.0);
        assert_eq!(p.stats.responses_suppressed_timeout, 1);
        // After it, a new process; three responses heard first silence it.
        p.on_requests(Some(X), &[], 300 * NS_PER_MS, &mut || 0.9);
        for _ in 0..3 {
            p.on_response_observed(X);
        }
        assert!(p.due_responses(600 * NS_PER_MS).is_empty());
        assert_eq!(p.stats.responses_suppressed_threshold, 1);
        assert_eq!(p.stats.responses_sent, 1);
    }

    #[test]
    fn only_a_recently_used_chain_is_served() {
        let mut p = oob();
        p.on_requests(Some(X), &[], 0, &mut || 0.0);
        assert_eq!(p.stats.responses_triggered, 0, "never signed under X");
        p.note_signed_with(&[X], 0);
        p.on_requests(Some(X), &[], 61_000 * NS_PER_MS, &mut || 0.0);
        assert_eq!(
            p.stats.responses_triggered, 0,
            "used more than a minute ago"
        );
    }

    #[test]
    fn inline_answers_with_the_own_certificate_first_then_a_ca() {
        let mut p = P2pcd::new(P2pcdParams::recommended(P2pcdFlavor::Inline));
        let own = [1, 2, 3];
        p.on_requests(None, &[X, own], 0, &mut || 0.0);
        let h = p.take_header(1, Some(own), &|i| i == X);
        assert!(h.force_certificate && h.requested_ca.is_none());
        p.on_requests(None, &[X], 2, &mut || 0.0);
        let h = p.take_header(3, Some(own), &|i| i == X);
        assert_eq!(h.requested_ca, Some(X));
        p.on_requests(None, &[X], 4, &mut || 0.0);
        p.on_requested_certificate(X);
        let h = p.take_header(5, Some(own), &|i| i == X);
        assert_eq!(h.requested_ca, None, "someone else answered");
        p.on_unknown_signer_digest([7, 7, 7]);
        p.on_unknown_issuer(X, 6);
        let h = p.take_header(7, Some(own), &|_| false);
        assert_eq!(h.inline_request.len(), 2);
        assert!(h.learning_request.is_none());
        assert!(p.take_header(8, None, &|_| false).inline_request.is_empty());
    }
}
