//! The cooperative-awareness feed: what each cooperative vehicle's node delivered, written
//! into the mobility tier's [`v2xw_mobility::coop::CoopLink`] for its CACC or platoon law.
//!
//! # What is offered, and what is kept
//!
//! After every node phase, for each node riding a vehicle whose class drives a cooperative
//! model, every delivered BSM or CAM whose signature the node's own security stack did not
//! refuse (`Invalid` and `Revoked` are dropped, exactly as the applications drop them) is
//! offered. It is kept only if it **associates** with the follower's radar leader or its
//! platoon head: the message's claimed position, compared with where that vehicle truly
//! was at the message's claimed generation time, must lie within the link's association
//! gate. The sender's simulation identity is never consulted — a fielded CACC fuses its
//! radar track with V2X objects by position, and so does this feed, which is what lets a
//! GNSS error, a forged position or a ghost vehicle reach the platoon.
//!
//! The leader's acceleration is read from the message's own octets with the decoder the
//! applications use (`v2xw_node::apps::decode_dynamics`) when the node kept the payload;
//! otherwise the link differentiates the claimed speeds.
//!
//! # Determinism
//!
//! Called from the merge after the node phase's map, over `results` already sorted by
//! node id, with each node's messages in delivery order: one writer, in a fixed order.

use v2xw_core::geom::Vec3;
use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::time::SimTime;
use v2xw_mobility::NeighborOptions;
use v2xw_mobility::coop::CoopRecord;
use v2xw_node::StepOutcome;
use v2xw_node::stores::VerificationState;

use super::Engine;

/// How far ahead the feed looks for a follower's radar target, metres: a long-range ACC
/// radar's reach (the 150-200 m class of automotive radar; the feed only needs the target
/// the car-following model will also find, within its own lookahead).
const RADAR_RANGE_M: f64 = 150.0;

impl Engine {
    /// What the cooperative link has been offered and kept so far, when the run's mobility
    /// tier drives a cooperative model.
    pub fn cooperative_stats(&self) -> Option<v2xw_mobility::coop::CoopStats> {
        self.mobility.cooperative().map(|l| l.stats())
    }

    /// Feeds one node phase's deliveries into the cooperative link, if the mobility tier
    /// has one. See the module documentation.
    pub(super) fn feed_cooperative(
        &self,
        results: &[(NodeId, StepOutcome, Vec<v2xw_core::ctx::OwnedRecord>, f64)],
        now: SimTime,
    ) {
        let Some(link) = self.mobility.cooperative() else {
            return;
        };
        link.prune(now);
        let gate = link.params().association_gate_m;
        for (node, outcome, _, _) in results {
            let Some(&actor) = self.node_actor.get(node) else {
                continue;
            };
            let Some(rec) = self.actors.get(&actor) else {
                continue;
            };
            if !link.subscribes(rec.class) || outcome.delivered.is_empty() {
                continue;
            }
            let Some(ego) = self.snapshot.view(actor).copied() else {
                continue;
            };
            let opts = NeighborOptions {
                lookahead_m: RADAR_RANGE_M,
                max_lane_hops: 8,
                classes: rec.class.class_mask(),
                sides: false,
                sublane: None,
            };
            let leader = self
                .snapshot
                .leader_on_route(&self.world, &ego, &[], 0, opts)
                .and_then(|l| l.vehicle.map(|v| v.actor));
            let head = link.platoon_head(actor, 32).filter(|h| Some(*h) != leader);
            let candidates: Vec<(ActorId, bool)> = leader
                .map(|l| (l, true))
                .into_iter()
                .chain(head.map(|h| (h, false)))
                .collect();
            if candidates.is_empty() {
                continue;
            }
            for m in &outcome.delivered {
                if !matches!(m.msg_type, v2xw_msg::MsgType::Bsm | v2xw_msg::MsgType::Cam)
                    || matches!(
                        m.verification,
                        VerificationState::Invalid | VerificationState::Revoked
                    )
                {
                    continue;
                }
                let Some(claimed) = m.claimed_pos else {
                    continue;
                };
                link.note_offered();
                let tg = m.claimed_generation_time;
                let hit = candidates.iter().find(|(c, _)| {
                    self.truth_at(*c, tg)
                        .is_some_and(|p| p.distance_2d(claimed) <= gate)
                });
                let Some(&(sender, predecessor)) = hit else {
                    continue;
                };
                let (accel, from_message) = match m.payload.as_deref() {
                    Some(payload) => {
                        let mut t = v2xw_node::apps::Track {
                            pos: claimed,
                            heading_rad: m.claimed_heading_rad,
                            speed_mps: m.claimed_speed_mps,
                            accel_mps2: f64::NAN,
                            yaw_rate_rad_s: 0.0,
                            length_m: 0.0,
                            width_m: 0.0,
                            hard_braking: false,
                            vru: false,
                        };
                        v2xw_node::apps::decode_dynamics(m.msg_type, payload, &mut t);
                        if t.accel_mps2.is_finite() {
                            (t.accel_mps2, true)
                        } else {
                            (0.0, false)
                        }
                    }
                    None => (0.0, false),
                };
                link.record(
                    actor,
                    sender,
                    CoopRecord {
                        generated: tg,
                        received: m.received_at,
                        speed_mps: m.claimed_speed_mps,
                        accel_mps2: accel,
                        accel_from_message: from_message,
                        claimed_pos: claimed,
                    },
                    predecessor,
                );
            }
        }
    }

    /// Where `actor` truly was at `t`: its last published state moved along its velocity,
    /// backwards as well as forwards (the published extrapolation rule applied in both
    /// directions, which for the sub-second ages of a BSM is the same straight line).
    fn truth_at(&self, actor: ActorId, t: SimTime) -> Option<Vec3> {
        let k = &self.actors.get(&actor)?.last;
        let dt = (t as f64 - k.t as f64) * 1e-9;
        Some(k.pos + k.vel.scale(dt))
    }
}
