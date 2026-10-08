//! The cooperative-awareness link: what a vehicle's V2X stack has *heard* about the
//! vehicle its radar is following, as the cooperative car-following models read it.
//!
//! # Why this exists
//!
//! A cooperative controller (CACC, platooning) is only cooperative because of the
//! messages it receives. SUMO's CACC model decides it may drive cooperatively by asking
//! whether its predecessor's *car-following model id* is CACC
//! (`MSCFModel_CACC::speedGapControl`, read 2026-10-07): no message is ever sent, so packet
//! loss, latency, congestion, a revoked certificate or a forged acceleration cannot change
//! what it does. Plexe (Segata et al. 2014) does send beacons over Veins, but associates a
//! beacon with the radar target by the sender's simulation identity. Here neither shortcut
//! exists:
//!
//! * the record a controller reads is written **only** when the simulated network actually
//!   delivered a BSM or CAM to the follower's node, after the follower's own security
//!   stack accepted it (an invalid or revoked signature never reaches this table);
//! * the record is associated with the radar target **by position**, the way a fielded
//!   CACC fuses its radar track with V2X objects: the message's claimed position, moved
//!   back to where the target truly was at the message's generation time, must fall inside
//!   [`CoopParams::association_gate_m`]. A GNSS error, a pseudonym change or a position
//!   forgery therefore changes the association, exactly as it would on the road;
//! * the controller judges freshness by the message's **generation time**: a message older
//!   than [`CoopParams::max_age_s`] is not used, and the controller degrades to ACC.
//!
//! # Who writes, who reads
//!
//! The kernel (`v2xw-engine`) writes, between mobility steps, from what each node's runtime
//! delivered; the mobility step reads. The two never overlap — a mobility step and a node
//! phase are distinct events of the one event loop — so the lock below is uncontended and
//! the table a step reads is the same on every thread count. Every map is a `BTreeMap`
//! keyed by actor ids, so iteration order is id order.
//!
//! # How to add another cooperative model
//!
//! Hold an `Arc<CoopLink>`, return it from [`crate::traits::CarFollowing::cooperative`],
//! and in `accel` call [`CoopLink::fresh`] with the ego and its radar leader's actor id.
//! `None` means degrade. See `docs/models/driving-model-library.md`.

use std::collections::BTreeMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{Parameter, Source, SourceKind};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::ActorId;
use v2xw_core::time::SimTime;

use crate::classes::VehicleClass;

/// How the link associates and ages messages.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoopParams {
    /// The oldest message a controller still drives on, seconds since its generation.
    ///
    /// Three nominal BSM intervals (SAE J2945/1's 100 ms): a controller tolerates two
    /// consecutive losses and degrades on the third. **A choice**, swept by the platoon
    /// experiment (`crate::carfollowing::platoon`); see the card's calibration plan.
    pub max_age_s: f64,
    /// How far the claimed position may lie from where the radar target truly was at the
    /// message's generation time, metres.
    ///
    /// Three standard deviations of the medium-tier GNSS model's horizontal error plus a
    /// half body length. **A choice**; see the card's calibration plan.
    pub association_gate_m: f64,
    /// How long a record is kept after its generation before it is pruned, seconds.
    pub retain_s: f64,
}

impl Default for CoopParams {
    fn default() -> Self {
        Self {
            max_age_s: 0.3,
            association_gate_m: 5.0,
            retain_s: 2.0,
        }
    }
}

impl CoopParams {
    /// The card parameters these values are declared by.
    pub fn card_parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter {
                name: "coop.max_age_s".to_string(),
                unit: "s".to_string(),
                default: serde_json::json!(self.max_age_s),
                range: Some(vec![serde_json::json!(0.0), serde_json::json!(5.0)]),
                source: Source::todo_calibrate(
                    "three nominal BSM intervals (SAE J2945/1 100 ms): tolerate two losses",
                ),
                calibration: Some(
                    "sweep max_age_s in crate::carfollowing::platoon against packet loss and \
                     latency; pick the value that maximises lane throughput subject to the \
                     string-stability criterion ||e_i||_inf <= ||e_(i-1)||_inf (Ploeg, van de \
                     Wouw & Nijmeijer 2014, IEEE T-CST 22(2)); compare with Ploeg et al. \
                     2015's graceful-degradation timeout"
                        .to_string(),
                ),
            },
            Parameter {
                name: "coop.association_gate_m".to_string(),
                unit: "m".to_string(),
                default: serde_json::json!(self.association_gate_m),
                range: Some(vec![serde_json::json!(0.5), serde_json::json!(50.0)]),
                source: Source::todo_calibrate(
                    "3 sigma of the gauss-markov GNSS horizontal error plus half a body length",
                ),
                calibration: Some(
                    "measure the association hit rate and wrong-association rate against the \
                     ground-truth sender over the GNSS presets; choose the smallest gate with \
                     a hit rate above 99 % at the open-sky preset"
                        .to_string(),
                ),
            },
            Parameter::new(
                "coop.retain_s",
                "s",
                serde_json::json!(self.retain_s),
                Source::new(
                    SourceKind::Code,
                    "housekeeping: records are pruned this long after generation; any value \
                     above coop.max_age_s leaves behaviour unchanged",
                ),
            ),
        ]
    }
}

/// One heard message about one sender, as the follower's applications read it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoopRecord {
    /// The generation time the message claims.
    pub generated: SimTime,
    /// When the follower's node had it.
    pub received: SimTime,
    /// The speed it claims, m/s.
    pub speed_mps: f64,
    /// The longitudinal acceleration it claims, m/s², when the message carried one (a BSM's
    /// `accelSet.long`, a CAM's `longitudinalAcceleration`); otherwise differentiated from
    /// the previous record of the same sender.
    pub accel_mps2: f64,
    /// Whether `accel_mps2` came from the message itself rather than differentiation.
    pub accel_from_message: bool,
    /// The position it claims, world ENU metres.
    pub claimed_pos: Vec3,
}

impl CoopRecord {
    /// The record's age at `now`, seconds (never negative).
    pub fn age_s(&self, now: SimTime) -> f64 {
        now.saturating_sub(self.generated) as f64 * 1e-9
    }

    /// The sender's speed moved on to `now` at its claimed acceleration, never negative:
    /// the extrapolation a V2V application applies to a message's age
    /// (`v2xw_node::apps::Track::advanced`).
    pub fn speed_at(&self, now: SimTime) -> f64 {
        (self.speed_mps + self.accel_mps2 * self.age_s(now)).max(0.0)
    }
}

/// What the cooperative models may use about a leader this step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoopInput {
    /// The leader's speed, extrapolated to now, m/s.
    pub speed_mps: f64,
    /// The leader's acceleration, m/s².
    pub accel_mps2: f64,
    /// The message's age, seconds.
    pub age_s: f64,
}

#[derive(Debug, Default)]
struct CoopState {
    /// `(receiver, sender) → newest record`.
    heard: BTreeMap<(ActorId, ActorId), CoopRecord>,
    /// Each receiver's associated predecessor, from the newest association.
    predecessor: BTreeMap<ActorId, ActorId>,
    /// Counters for the run report.
    stats: CoopStats,
}

/// What the link has done, for the run report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoopStats {
    /// Messages offered to the link (BSM/CAM delivered to a cooperative receiver).
    pub offered: u64,
    /// Of those, the ones associated with the receiver's radar leader or platoon head.
    pub associated: u64,
}

/// The shared table. Create one with [`CoopLink::new`]; share it as `Arc<CoopLink>`.
#[derive(Debug)]
pub struct CoopLink {
    params: CoopParams,
    /// The classes driven by a cooperative model: the kernel offers messages only to these.
    subscribers: Vec<VehicleClass>,
    state: RwLock<CoopState>,
}

impl CoopLink {
    /// An empty link for vehicles of `subscribers`.
    pub fn new(params: CoopParams, subscribers: Vec<VehicleClass>) -> Self {
        let mut subscribers = subscribers;
        subscribers.sort();
        subscribers.dedup();
        Self {
            params,
            subscribers,
            state: RwLock::new(CoopState::default()),
        }
    }

    /// The parameters in force.
    pub fn params(&self) -> &CoopParams {
        &self.params
    }

    /// Whether a vehicle of `class` drives a cooperative model.
    pub fn subscribes(&self, class: VehicleClass) -> bool {
        self.subscribers.binary_search(&class).is_ok()
    }

    /// The subscribing classes, sorted.
    pub fn subscribers(&self) -> &[VehicleClass] {
        &self.subscribers
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, CoopState> {
        self.state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, CoopState> {
        self.state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Counts one message offered to the link.
    pub fn note_offered(&self) {
        self.write().stats.offered += 1;
    }

    /// Records what `receiver` heard from `sender`, keeping the newest by generation time.
    ///
    /// A record whose message carried no acceleration is given one differentiated from the
    /// previous record of the same sender, when that one is older and recent enough
    /// (within [`CoopParams::retain_s`]).
    pub fn record(
        &self,
        receiver: ActorId,
        sender: ActorId,
        mut rec: CoopRecord,
        predecessor: bool,
    ) {
        let mut st = self.write();
        st.stats.associated += 1;
        if predecessor {
            st.predecessor.insert(receiver, sender);
        }
        let key = (receiver, sender);
        if let Some(old) = st.heard.get(&key) {
            if old.generated >= rec.generated {
                return;
            }
            if !rec.accel_from_message {
                let dt = (rec.generated - old.generated) as f64 * 1e-9;
                if dt > 0.0 && dt <= self.params.retain_s {
                    rec.accel_mps2 = (rec.speed_mps - old.speed_mps) / dt;
                }
            }
        }
        st.heard.insert(key, rec);
    }

    /// The newest record `receiver` holds about `sender`, however old.
    pub fn latest(&self, receiver: ActorId, sender: ActorId) -> Option<CoopRecord> {
        self.read().heard.get(&(receiver, sender)).copied()
    }

    /// What `receiver` may use about `sender` at `now`, or `None` when it holds nothing
    /// younger than [`CoopParams::max_age_s`] — the controller's cue to degrade.
    pub fn fresh(&self, receiver: ActorId, sender: ActorId, now: SimTime) -> Option<CoopInput> {
        let rec = self.latest(receiver, sender)?;
        let age = rec.age_s(now);
        (age <= self.params.max_age_s).then(|| CoopInput {
            speed_mps: rec.speed_at(now),
            accel_mps2: rec.accel_mps2,
            age_s: age,
        })
    }

    /// `receiver`'s associated predecessor.
    pub fn predecessor(&self, receiver: ActorId) -> Option<ActorId> {
        self.read().predecessor.get(&receiver).copied()
    }

    /// The head of `receiver`'s platoon: the predecessor chain followed until it ends, at
    /// most `max_len` vehicles. `None` when `receiver` has no associated predecessor.
    pub fn platoon_head(&self, receiver: ActorId, max_len: usize) -> Option<ActorId> {
        let st = self.read();
        let mut cur = *st.predecessor.get(&receiver)?;
        for _ in 0..max_len {
            match st.predecessor.get(&cur) {
                Some(&p) if p != receiver => cur = p,
                _ => break,
            }
        }
        Some(cur)
    }

    /// Drops every record generated more than [`CoopParams::retain_s`] before `now`, and
    /// every predecessor link whose record went with it.
    pub fn prune(&self, now: SimTime) {
        let horizon = (self.params.retain_s * 1e9) as u64;
        let mut st = self.write();
        st.heard
            .retain(|_, r| now.saturating_sub(r.generated) <= horizon);
        let CoopState {
            heard, predecessor, ..
        } = &mut *st;
        predecessor.retain(|rx, tx| heard.contains_key(&(*rx, *tx)));
    }

    /// The counters so far.
    pub fn stats(&self) -> CoopStats {
        self.read().stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(t_ms: u64, v: f64, a: Option<f64>) -> CoopRecord {
        CoopRecord {
            generated: t_ms * 1_000_000,
            received: t_ms * 1_000_000 + 2_000_000,
            speed_mps: v,
            accel_mps2: a.unwrap_or(0.0),
            accel_from_message: a.is_some(),
            claimed_pos: Vec3::new(0.0, 0.0, 0.0),
        }
    }

    #[test]
    fn a_stale_message_is_not_fresh_and_the_newest_wins() {
        let link = CoopLink::new(CoopParams::default(), vec![VehicleClass::Passenger]);
        let (rx, tx) = (ActorId::new(2), ActorId::new(1));
        link.record(rx, tx, rec(100, 20.0, Some(1.0)), true);
        link.record(rx, tx, rec(50, 10.0, Some(0.0)), true); // older: ignored
        let f = link.fresh(rx, tx, 200_000_000).expect("100 ms old");
        assert!((f.speed_mps - 20.1).abs() < 1e-9, "{f:?}");
        assert!(link.fresh(rx, tx, 500_000_000).is_none(), "400 ms old");
        assert_eq!(link.predecessor(rx), Some(tx));
    }

    #[test]
    fn acceleration_is_differentiated_when_the_message_has_none() {
        let link = CoopLink::new(CoopParams::default(), vec![]);
        let (rx, tx) = (ActorId::new(2), ActorId::new(1));
        link.record(rx, tx, rec(0, 10.0, None), true);
        link.record(rx, tx, rec(100, 10.2, None), true);
        let r = link.latest(rx, tx).unwrap();
        assert!((r.accel_mps2 - 2.0).abs() < 1e-9, "{r:?}");
    }

    #[test]
    fn the_platoon_head_is_the_end_of_the_predecessor_chain() {
        let link = CoopLink::new(CoopParams::default(), vec![]);
        for i in 1..4u32 {
            link.record(
                ActorId::new(i + 1),
                ActorId::new(i),
                rec(10, 20.0, Some(0.0)),
                true,
            );
        }
        assert_eq!(
            link.platoon_head(ActorId::new(4), 16),
            Some(ActorId::new(1))
        );
        link.prune(10_000_000_000);
        assert_eq!(link.platoon_head(ActorId::new(4), 16), None);
    }
}
