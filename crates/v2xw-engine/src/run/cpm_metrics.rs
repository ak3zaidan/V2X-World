//! What Collective Perception buys: object awareness, redundancy, message sizes and the
//! generation rate (`messages.sets` with `cpm`; ETSI TS 103 324).
//!
//! A delivery ratio says whether a CPM arrived. A collective-perception study asks what a
//! receiver then *knows*: of the road users truly around it — equipped or not, vehicle or
//! pedestrian — how many it has heard about recently, either from their own awareness
//! messages or from someone else's CPM, and at what cost in repeated reports. Those are
//! the figures ETSI TR 103 562 V2.1.1 §6 evaluates the CPM generation rules by (object
//! awareness, redundancy of object reports, CPM size and rate), and they are measured here
//! from the frames the PHY actually delivered, with the ground truth only on the scoring
//! side.
//!
//! | Figure | Definition |
//! |---|---|
//! | `awareness_direct_permille` | over samples every [`SAMPLE_NS`] at every equipped vehicle: road users within [`AWARENESS_RADIUS_M`] it heard a CAM, BSM, VAM or PSM from within [`AWARENESS_WINDOW_NS`], over all road users within that radius |
//! | `awareness_with_cpm_permille` | the same, counting also road users a received CPM reported |
//! | `redundancy_milli` | object reports received in CPMs, over distinct (receiver, object, second) triples: how many times a receiver hears about the same object each second |
//! | `size_*` | the CPM's facilities-layer payload, octets: median, 95th percentile, and 100-octet bins |
//! | `generation_mhz` | CPMs per CPM-sending station per second, ×1000 |
//! | `cpms_verified` | CPMs (from any station) a vehicle's own receiver verified and decoded (counted when the vehicle runs applications, which is when it parses the payload) |
//! | `rsu_cpms_sent` | infrastructure CPMs: those roadside units with the `cpm` role handed to the radio |
//! | `rsu_cpms_verified` | infrastructure CPMs a vehicle's own receiver verified and decoded, recognised by the message's `originatingRsuContainer` (counted when the vehicle runs applications, which is when it parses the payload) |
//! | `rsu_objects_decoded` | perceived objects in those |
//!
//! The radius (150 m) and window (1 s, `T_GenCpmMax`) are this build's choice: the radius
//! inside the 250 m long-range radar of [`crate::perception`], the window the longest a
//! CPM may lapse (TS 103 324 Table F.1).
//!
//! # Determinism
//!
//! Every table is a `BTreeMap`; sampling walks vehicles in actor-id order.

use std::collections::BTreeMap;

use v2xw_core::ids::{ActorId, FrameSeq, NodeId};
use v2xw_core::time::SimTime;

use super::{Engine, FrameState};

/// How far around a vehicle awareness is scored, m.
pub const AWARENESS_RADIUS_M: f64 = 150.0;
/// How recent a report must be to count, ns.
pub const AWARENESS_WINDOW_NS: u64 = 1_000_000_000;
/// How far a reported object may be from the sender's track it came from, m: a track
/// moves at most about 3 m between the CPM's encoding and its hand-down (30 m/s over one
/// 100 ms step), and the CPM carries the track's own position.
pub const ASSOCIATION_GATE_M: f64 = 6.0;
/// How often awareness is sampled, ns.
pub const SAMPLE_NS: u64 = 1_000_000_000;

/// The run's collective-perception figures, for [`super::RunReport::cpm`].
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct CpmTally {
    /// CPMs handed to the radio.
    pub cpms_sent: u64,
    /// Stations that sent at least one.
    pub senders: u64,
    /// Perceived objects across those CPMs.
    pub objects_sent: u64,
    /// CPMs per sending station per second, ×1000.
    pub generation_mhz: u64,
    /// Median CPM payload, octets.
    pub size_p50_bytes: u64,
    /// 95th percentile.
    pub size_p95_bytes: u64,
    /// `(bin start octets, CPMs)` in 100-octet bins.
    pub size_bins: Vec<(u32, u64)>,
    /// Object reports receivers decoded.
    pub object_reports_received: u64,
    /// Reports per distinct (receiver, object, second), ×1000.
    pub redundancy_milli: u64,
    /// (vehicle, road user) pairs scored.
    pub awareness_pairs: u64,
    /// Share known from awareness messages alone, ‰.
    pub awareness_direct_permille: u64,
    /// Share known with CPMs as well, ‰.
    pub awareness_with_cpm_permille: u64,
    /// CPMs vehicles verified and decoded (left out of the report when none).
    #[serde(skip_serializing_if = "is_zero")]
    pub cpms_verified: u64,
    /// Infrastructure CPMs handed to the radio (left out of the report when none, so a
    /// run with no roadside CPM keeps its report).
    #[serde(skip_serializing_if = "is_zero")]
    pub rsu_cpms_sent: u64,
    /// Infrastructure CPMs vehicles verified and decoded.
    #[serde(skip_serializing_if = "is_zero")]
    pub rsu_cpms_verified: u64,
    /// Perceived objects in those.
    #[serde(skip_serializing_if = "is_zero")]
    pub rsu_objects_decoded: u64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// The run's collective-perception bookkeeping.
#[derive(Debug, Clone, Default)]
pub(super) struct CpmMeter {
    frame_objects: BTreeMap<FrameSeq, Vec<ActorId>>,
    sizes: Vec<u64>,
    by_sender: BTreeMap<NodeId, u64>,
    objects_sent: u64,
    /// receiver → road user → (last direct, last via CPM).
    known: BTreeMap<NodeId, BTreeMap<ActorId, (Option<SimTime>, Option<SimTime>)>>,
    /// (receiver, road user) → the second of the last report.
    last_second: BTreeMap<(NodeId, ActorId), u64>,
    reports: u64,
    distinct: u64,
    next_sample: SimTime,
    pairs: u64,
    direct: u64,
    with_cpm: u64,
    rsu_sent: u64,
    verified: u64,
    rsu_verified: u64,
    rsu_objects: u64,
}

impl CpmMeter {
    pub(super) fn for_scenario(s: &crate::scenario::schema::Scenario) -> Option<Self> {
        s.messages
            .sets
            .iter()
            .any(|m| m == "cpm")
            .then(CpmMeter::default)
    }
}

impl Engine {
    /// A CPM `node` hands down as `frame`: which road users it reports.
    pub(super) fn cpm_on_send(
        &mut self,
        node: NodeId,
        frame: FrameSeq,
        payload: Option<&[u8]>,
        bytes: u32,
    ) {
        if self.cpm_meter.is_none() {
            return;
        }
        let roadside = self.rsus.contains_key(&node);
        let actors: Vec<ActorId> = payload
            .and_then(|p| v2xw_msg::cpm::decode_cpm(p).ok())
            .and_then(|cpm| v2xw_msg::cpm::objects_of(&cpm, self.world.origin.into()))
            .map(|(_, objs)| {
                let perception = self.perception.as_ref();
                let mut v: Vec<ActorId> = objs
                    .iter()
                    .filter_map(|o| {
                        perception.and_then(|p| p.actor_near(node, o.1, ASSOCIATION_GATE_M))
                    })
                    .collect();
                v.sort_unstable();
                v.dedup();
                v
            })
            .unwrap_or_default();
        let Some(m) = self.cpm_meter.as_mut() else {
            return;
        };
        m.sizes.push(u64::from(bytes));
        *m.by_sender.entry(node).or_insert(0) += 1;
        if roadside {
            m.rsu_sent += 1;
        }
        m.objects_sent += actors.len() as u64;
        m.frame_objects.insert(frame, actors);
    }

    /// `rx` decoded `state` (frame `frame`) at `now`.
    pub(super) fn cpm_on_receive(
        &mut self,
        rx: NodeId,
        frame: FrameSeq,
        state: &FrameState,
        now: SimTime,
    ) {
        if self.cpm_meter.is_none() {
            return;
        }
        use v2xw_msg::MsgType as M;
        let sender_actor = self.node_actor.get(&state.tx).copied();
        let Some(m) = self.cpm_meter.as_mut() else {
            return;
        };
        match state.msg_type {
            M::Cam | M::Bsm | M::Vam | M::Psm => {
                if let Some(a) = sender_actor {
                    m.known.entry(rx).or_default().entry(a).or_default().0 = Some(now);
                }
            }
            M::Cpm => {
                let Some(objs) = m.frame_objects.get(&frame).cloned() else {
                    return;
                };
                let sec = now / 1_000_000_000;
                for a in objs {
                    m.known.entry(rx).or_default().entry(a).or_default().1 = Some(now);
                    m.reports += 1;
                    let last = m.last_second.insert((rx, a), sec);
                    if last != Some(sec) {
                        m.distinct += 1;
                    }
                }
            }
            _ => {}
        }
    }

    /// Infrastructure CPMs among what vehicle `rx`'s receiver verified: read from the
    /// payload the node itself parsed and the message's own originating container.
    pub(super) fn cpm_on_verified(&mut self, rx: NodeId, delivered: &[v2xw_node::VerifiedMessage]) {
        if self.cpm_meter.is_none() || self.rsus.contains_key(&rx) {
            return;
        }
        let origin = self.world.origin.into();
        let mut all = 0u64;
        let mut cpms = 0u64;
        let mut objects = 0u64;
        for m in delivered {
            if m.msg_type != v2xw_msg::MsgType::Cpm
                || m.verification != v2xw_node::stores::VerificationState::Verified
            {
                continue;
            }
            let Some(cpm) = m
                .payload
                .as_deref()
                .and_then(|p| v2xw_msg::cpm::decode_cpm(p).ok())
            else {
                continue;
            };
            all += 1;
            if !v2xw_msg::cpm::from_rsu(&cpm) {
                continue;
            }
            cpms += 1;
            objects += v2xw_msg::cpm::objects_of(&cpm, origin).map_or(0, |(_, o)| o.len() as u64);
        }
        if let Some(m) = self.cpm_meter.as_mut() {
            m.verified += all;
            m.rsu_verified += cpms;
            m.rsu_objects += objects;
        }
    }

    /// The frame has left every receiver.
    pub(super) fn cpm_forget(&mut self, frame: FrameSeq) {
        if let Some(m) = self.cpm_meter.as_mut() {
            m.frame_objects.remove(&frame);
        }
    }

    /// Scores awareness at every equipped vehicle, once per [`SAMPLE_NS`].
    pub(super) fn cpm_sample(&mut self, now: SimTime) {
        let Some(m) = self.cpm_meter.as_ref() else {
            return;
        };
        if now < m.next_sample {
            return;
        }
        let mut pairs = 0;
        let mut direct = 0;
        let mut with_cpm = 0;
        let fresh =
            |t: Option<SimTime>| t.is_some_and(|t| now.saturating_sub(t) <= AWARENESS_WINDOW_NS);
        for (id, a) in &self.actors {
            let Some(node) = a.node else { continue };
            if a.class.is_vru() {
                continue;
            }
            let mut near: Vec<ActorId> =
                self.snapshot.actors_within(a.last.pos, AWARENESS_RADIUS_M);
            near.sort_unstable();
            near.dedup();
            let known = m.known.get(&node);
            for other in near {
                if other == *id {
                    continue;
                }
                pairs += 1;
                let k = known
                    .and_then(|k| k.get(&other))
                    .copied()
                    .unwrap_or_default();
                if fresh(k.0) {
                    direct += 1;
                }
                if fresh(k.0) || fresh(k.1) {
                    with_cpm += 1;
                }
            }
        }
        let m = self.cpm_meter.as_mut().expect("checked above");
        m.pairs += pairs;
        m.direct += direct;
        m.with_cpm += with_cpm;
        m.next_sample = now.saturating_add(SAMPLE_NS);
        // Forget what can no longer count, so the tables stay the size of the neighbourhood.
        for k in m.known.values_mut() {
            k.retain(|_, (d, c)| fresh(*d) || fresh(*c));
        }
    }

    /// The run's figures.
    pub(super) fn cpm_finish(&mut self) {
        let duration_s = self.scenario.time.duration_s.max(1e-9);
        let Some(m) = self.cpm_meter.as_ref() else {
            return;
        };
        let mut sizes = m.sizes.clone();
        sizes.sort_unstable();
        let q = |p: f64| -> u64 {
            if sizes.is_empty() {
                0
            } else {
                sizes[(((sizes.len() - 1) as f64) * p).round() as usize]
            }
        };
        let mut bins: BTreeMap<u32, u64> = BTreeMap::new();
        for s in &sizes {
            *bins.entry((*s / 100 * 100) as u32).or_insert(0) += 1;
        }
        let senders = m.by_sender.len() as u64;
        let sent = sizes.len() as u64;
        let permille = |n: u64| if m.pairs == 0 { 0 } else { n * 1_000 / m.pairs };
        self.report.cpm = Some(CpmTally {
            cpms_sent: sent,
            senders,
            objects_sent: m.objects_sent,
            generation_mhz: if senders == 0 {
                0
            } else {
                (sent as f64 * 1_000.0 / senders as f64 / duration_s).round() as u64
            },
            size_p50_bytes: q(0.5),
            size_p95_bytes: q(0.95),
            size_bins: bins.into_iter().collect(),
            object_reports_received: m.reports,
            redundancy_milli: if m.distinct == 0 {
                0
            } else {
                m.reports * 1_000 / m.distinct
            },
            awareness_pairs: m.pairs,
            awareness_direct_permille: permille(m.direct),
            awareness_with_cpm_permille: permille(m.with_cpm),
            cpms_verified: m.verified,
            rsu_cpms_sent: m.rsu_sent,
            rsu_cpms_verified: m.rsu_verified,
            rsu_objects_decoded: m.rsu_objects,
        });
    }
}
