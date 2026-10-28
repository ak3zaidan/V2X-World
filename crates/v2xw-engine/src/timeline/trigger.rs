//! WP-21: condition-triggered timeline items.
//!
//! A timeline item with a `when` clause ([`Condition`]) is armed at its `t` and fires at
//! the first mobility step at which the clause holds. This module is the clause's pure
//! half: [`Condition::eval`] reads a [`TriggerView`] of the step's ground truth and says
//! whether the clause holds, and for which vehicles ([`Binding`]). The engine builds the
//! view after the mobility update and schedules the item's control event when it holds.
//!
//! # What each clause reads
//!
//! | Clause | Holds for vehicle `f` when | Binds |
//! |---|---|---|
//! | `ttc_below {ttc_s}` | `f` closes on the vehicle `l` ahead on its lane and `gap / (v_f − v_l) < ttc_s` | `follower = f`, `leader = l` |
//! | `speed_diff_above {speed_diff_mps}` | `v_f − v_l > speed_diff_mps`, `l` ahead on the lane within [`PAIR_RANGE_M`] | `follower = f`, `leader = l` |
//! | `in_polygon {points_m}` | `f`'s reference point is inside the polygon | `actor = f` |
//! | `probability {p}` | a draw keyed by `(seed, item, step)` is below `p` | nothing |
//!
//! `all` needs every clause for the same vehicle; `any` needs one. Vehicles are visited
//! in actor-id order and the first that satisfies the clause is bound, so a replay binds
//! the same vehicles.
//!
//! The time to collision is [`v2xw_metrics::ssm::evaluate_following`]'s (gap over
//! closing speed), compared on the declared time grid as WP-11's conflict count is (build
//! decision D10: never on the raw quotient). The gap is bumper to bumper: the difference
//! of the two rear-axle reference points along the lane, less the follower's length, which
//! is exact when the two vehicles have the same rear overhang.
//!
//! # Ground truth, and determinism
//!
//! The scenario director reads ground truth by design: it is the author's hand, not a
//! detector, so the belief-only invariants I-T1 to I-T3 do not apply to it. The
//! probability draw is a keyed hash of `(seed, item, step)` on its own stream
//! ([`RNG_MODEL_ID`]), so a replay is byte-identical and no shared stream moves.

use std::collections::BTreeMap;

use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_metrics::Quantum;
use v2xw_metrics::ssm::{SsmActor, SsmParams, evaluate_following};

use crate::scenario::schema::{Clause, Condition};

/// How far ahead `speed_diff_above` looks for the vehicle a follower closes on, metres:
/// the interaction range of the car-following models in this engine (a free-road driver
/// beyond it is not following).
pub const PAIR_RANGE_M: f64 = 100.0;

/// The stream the `probability` clause draws from.
pub const RNG_MODEL_ID: &str = "scenario/trigger";

/// One vehicle as a trigger sees it: its ground truth at the step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriggerActor {
    /// The actor.
    pub actor: ActorId,
    /// Its node, when it carries one.
    pub node: Option<NodeId>,
    /// The reference point, local frame, metres.
    pub x_m: f64,
    /// The reference point, local frame, metres.
    pub y_m: f64,
    /// Ground speed, m/s.
    pub speed_mps: f64,
    /// Heading, radians, ENU.
    pub heading_rad: f64,
    /// Body length, metres.
    pub length_m: f64,
    /// Body width, metres.
    pub width_m: f64,
    /// The lane it is on and its distance along it, metres.
    pub lane: Option<(u32, f64)>,
}

/// What a trigger reads at one mobility step.
#[derive(Debug, Clone, Copy)]
pub struct TriggerView<'a> {
    /// Every vehicle, in actor-id order.
    pub actors: &'a [TriggerActor],
    /// The run's master seed.
    pub seed: u64,
    /// The item's index in `events`.
    pub item: usize,
    /// The mobility step's index.
    pub step: u64,
}

/// The vehicles a condition bound when it held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Binding {
    /// The vehicle an area clause found (`when.actor`).
    pub actor: Option<ActorId>,
    /// The vehicle ahead of a pair clause's follower (`when.leader`).
    pub leader: Option<ActorId>,
    /// A pair clause's follower (`when.follower`).
    pub follower: Option<ActorId>,
}

/// A binding an effect's `target` names: `when.actor`, `when.leader`, `when.follower`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// `when.actor`.
    Actor,
    /// `when.leader`.
    Leader,
    /// `when.follower`.
    Follower,
}

impl Role {
    /// Reads `when.actor`, `when.leader` or `when.follower`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "when.actor" => Some(Role::Actor),
            "when.leader" => Some(Role::Leader),
            "when.follower" => Some(Role::Follower),
            _ => None,
        }
    }

    /// How it is written.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Role::Actor => "when.actor",
            Role::Leader => "when.leader",
            Role::Follower => "when.follower",
        }
    }
}

impl Binding {
    /// The vehicle bound to `role`, if any.
    #[must_use]
    pub fn get(&self, role: Role) -> Option<ActorId> {
        match role {
            Role::Actor => self.actor,
            Role::Leader => self.leader,
            Role::Follower => self.follower,
        }
    }

    fn merge(&mut self, other: Binding) {
        self.actor = self.actor.or(other.actor);
        self.leader = self.leader.or(other.leader);
        self.follower = self.follower.or(other.follower);
    }
}

/// The vehicle ahead of each vehicle on its lane, as `(index into actors, gap m)`.
fn leaders(actors: &[TriggerActor]) -> Vec<Option<(usize, f64)>> {
    let mut by_lane: BTreeMap<u32, Vec<(i64, ActorId, usize)>> = BTreeMap::new();
    for (i, a) in actors.iter().enumerate() {
        if let Some((lane, s)) = a.lane {
            by_lane
                .entry(lane)
                .or_default()
                .push((Quantum::LENGTH_M.grid(s), a.actor, i));
        }
    }
    let mut out = vec![None; actors.len()];
    for v in by_lane.values_mut() {
        // A total order two platforms cannot disagree about: the grid index, then the id.
        v.sort_unstable();
        for w in v.windows(2) {
            let (f, l) = (w[0].2, w[1].2);
            let (Some((_, sf)), Some((_, sl))) = (actors[f].lane, actors[l].lane) else {
                continue;
            };
            out[f] = Some((l, sl - sf - actors[f].length_m));
        }
    }
    out
}

fn ssm(a: &TriggerActor) -> SsmActor {
    SsmActor {
        actor: a.actor,
        x_m: a.x_m,
        y_m: a.y_m,
        heading_rad: a.heading_rad,
        speed_mps: a.speed_mps,
        length_m: a.length_m,
        width_m: a.width_m,
        lane: a.lane.map(|(l, _)| l),
    }
}

/// The step's draw in `[0, 1)`, keyed by `(seed, item, step)`.
fn draw(view: &TriggerView<'_>) -> f64 {
    let mut s = RngStream::derive(
        view.seed,
        RngDomain::plugin(RNG_MODEL_ID),
        EntityRef::Global,
    );
    s.set_word_pos(((view.item as u128) << 64) | (u128::from(view.step) * 2));
    s.f64()
}

/// Whether a closed polygon (it closes itself) contains `(x, y)`: the even-odd crossing
/// rule, with a half-open edge test so a point on a shared vertex is counted once.
#[must_use]
pub fn polygon_contains(points: &[[f64; 2]], x: f64, y: f64) -> bool {
    let n = points.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let [xi, yi] = points[i];
        let [xj, yj] = points[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

impl Clause {
    /// Whether the clause reads a vehicle (every clause but `probability`).
    #[must_use]
    pub fn needs_vehicle(&self) -> bool {
        !matches!(self, Clause::Probability { .. })
    }

    /// The bindings this clause can produce.
    #[must_use]
    pub fn binds(&self) -> &'static [Role] {
        match self {
            Clause::TtcBelow { .. } | Clause::SpeedDiffAbove { .. } => {
                &[Role::Leader, Role::Follower]
            }
            Clause::InPolygon { .. } => &[Role::Actor],
            Clause::Probability { .. } => &[],
        }
    }

    /// The clause for vehicle `i`, or for no vehicle (`probability`).
    fn holds(
        &self,
        i: usize,
        view: &TriggerView<'_>,
        ahead: &[Option<(usize, f64)>],
    ) -> Option<Binding> {
        match self {
            Clause::Probability { p } => (draw(view) < *p).then(Binding::default),
            Clause::InPolygon { points_m } => {
                let a = &view.actors[i];
                polygon_contains(points_m, a.x_m, a.y_m).then_some(Binding {
                    actor: Some(a.actor),
                    ..Binding::default()
                })
            }
            Clause::TtcBelow { ttc_s } => {
                let (l, gap) = ahead[i]?;
                let (f, lead) = (&view.actors[i], &view.actors[l]);
                let ttc =
                    evaluate_following(&ssm(f), &ssm(lead), gap, &SsmParams::default())?.ttc_s?;
                (Quantum::TIME_S.grid(ttc) < Quantum::TIME_S.grid(*ttc_s)).then_some(Binding {
                    leader: Some(lead.actor),
                    follower: Some(f.actor),
                    ..Binding::default()
                })
            }
            Clause::SpeedDiffAbove { speed_diff_mps } => {
                let (l, gap) = ahead[i]?;
                let (f, lead) = (&view.actors[i], &view.actors[l]);
                (gap <= PAIR_RANGE_M
                    && Quantum::SPEED.grid(f.speed_mps - lead.speed_mps)
                        > Quantum::SPEED.grid(*speed_diff_mps))
                .then_some(Binding {
                    leader: Some(lead.actor),
                    follower: Some(f.actor),
                    ..Binding::default()
                })
            }
        }
    }
}

impl Condition {
    /// The clauses and whether every one must hold (`true`) or one (`false`); a single
    /// clause is an `all` of one.
    #[must_use]
    pub fn clauses(&self) -> (bool, Vec<Clause>) {
        match self {
            Condition::TtcBelow { ttc_s } => (true, vec![Clause::TtcBelow { ttc_s: *ttc_s }]),
            Condition::InPolygon { points_m } => (
                true,
                vec![Clause::InPolygon {
                    points_m: points_m.clone(),
                }],
            ),
            Condition::SpeedDiffAbove { speed_diff_mps } => (
                true,
                vec![Clause::SpeedDiffAbove {
                    speed_diff_mps: *speed_diff_mps,
                }],
            ),
            Condition::Probability { p } => (true, vec![Clause::Probability { p: *p }]),
            Condition::All { of } => (true, of.clone()),
            Condition::Any { of } => (false, of.clone()),
        }
    }

    /// Whether `role` is bound whenever the condition holds: by some clause of an `all`,
    /// by every clause of an `any`.
    #[must_use]
    pub fn always_binds(&self, role: Role) -> bool {
        let (all, clauses) = self.clauses();
        let by = |c: &Clause| c.binds().contains(&role);
        if all {
            clauses.iter().any(by)
        } else {
            !clauses.is_empty() && clauses.iter().all(by)
        }
    }

    /// Whether the condition holds at this step, and for which vehicles.
    #[must_use]
    pub fn eval(&self, view: &TriggerView<'_>) -> Option<Binding> {
        let (all, clauses) = self.clauses();
        let ahead = leaders(view.actors);
        let (free, per_vehicle): (Vec<&Clause>, Vec<&Clause>) =
            clauses.iter().partition(|c| !c.needs_vehicle());
        if all {
            if !free.iter().all(|c| c.holds(0, view, &ahead).is_some()) {
                return None;
            }
            if per_vehicle.is_empty() {
                return Some(Binding::default());
            }
            for i in 0..view.actors.len() {
                let mut b = Binding::default();
                let ok = per_vehicle.iter().all(|c| match c.holds(i, view, &ahead) {
                    Some(x) => {
                        b.merge(x);
                        true
                    }
                    None => false,
                });
                if ok {
                    return Some(b);
                }
            }
            None
        } else {
            if free.iter().any(|c| c.holds(0, view, &ahead).is_some()) {
                return Some(Binding::default());
            }
            for i in 0..view.actors.len() {
                if let Some(b) = per_vehicle.iter().find_map(|c| c.holds(i, view, &ahead)) {
                    return Some(b);
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(id: u32, x: f64, speed: f64, lane: Option<u32>) -> TriggerActor {
        TriggerActor {
            actor: ActorId::new(id),
            node: None,
            x_m: x,
            y_m: 0.0,
            speed_mps: speed,
            heading_rad: 0.0,
            length_m: 5.0,
            width_m: 1.8,
            lane: lane.map(|l| (l, x)),
        }
    }

    fn view(actors: &[TriggerActor], step: u64) -> TriggerView<'_> {
        TriggerView {
            actors,
            seed: 0xC0FFEE,
            item: 3,
            step,
        }
    }

    const SQUARE_A: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    const SQUARE_B: [[f64; 2]; 4] = [[9.0, -1.0], [11.0, -1.0], [11.0, 1.0], [9.0, 1.0]];

    fn poly(points: [[f64; 2]; 4]) -> Clause {
        Clause::InPolygon {
            points_m: points.to_vec(),
        }
    }

    #[test]
    fn all_any_truth_table() {
        // Actor 0 sits in square A, actor 1 in square B: each clause holds for one vehicle.
        let actors = [car(0, 0.0, 10.0, None), car(1, 10.0, 10.0, None)];
        let v = view(&actors, 0);
        let (a, b) = (poly(SQUARE_A), poly(SQUARE_B));
        let p = |p: f64| Clause::Probability { p };
        let bound = |c: Condition| c.eval(&v).map(|b| b.actor);
        let all = |of: Vec<Clause>| Condition::All { of };
        let any = |of: Vec<Clause>| Condition::Any { of };
        let a0 = Some(Some(ActorId::new(0)));
        let a1 = Some(Some(ActorId::new(1)));
        // `all` needs every clause for one vehicle; no vehicle is in both squares.
        assert_eq!(bound(all(vec![a.clone(), b.clone()])), None);
        assert_eq!(bound(all(vec![a.clone()])), a0);
        assert_eq!(bound(all(vec![b.clone()])), a1);
        // `any` needs one: the first vehicle in id order that satisfies some clause.
        assert_eq!(bound(any(vec![a.clone(), b.clone()])), a0);
        assert_eq!(bound(any(vec![b.clone(), a.clone()])), a0);
        assert_eq!(bound(any(vec![b.clone()])), a1);
        // A vehicle-free clause gates `all` and suffices for `any`.
        assert_eq!(bound(all(vec![p(1.0), b.clone()])), a1);
        assert_eq!(bound(all(vec![p(0.0), b.clone()])), None);
        assert_eq!(bound(any(vec![p(0.0), b.clone()])), a1);
        assert_eq!(bound(any(vec![p(1.0), b.clone()])), Some(None));
        assert_eq!(bound(any(vec![p(0.0)])), None);
        assert_eq!(bound(all(vec![p(1.0)])), Some(None));
        // An empty `any` never holds; an empty `all` always does (the identities).
        assert_eq!(bound(any(vec![])), None);
        assert_eq!(bound(all(vec![])), Some(None));
        // The binding promises.
        assert!(all(vec![a.clone(), p(0.5)]).always_binds(Role::Actor));
        assert!(!any(vec![a.clone(), p(0.5)]).always_binds(Role::Actor));
        assert!(any(vec![a.clone(), b]).always_binds(Role::Actor));
        assert!(!all(vec![a]).always_binds(Role::Leader));
    }

    #[test]
    fn in_polygon_binds_the_first_actor_inside() {
        let actors = [
            car(4, -50.0, 0.0, None),
            car(7, 0.5, 0.0, None),
            car(9, -0.5, 0.0, None),
        ];
        let c = Condition::InPolygon {
            points_m: SQUARE_A.to_vec(),
        };
        assert_eq!(
            c.eval(&view(&actors, 0)).and_then(|b| b.actor),
            Some(ActorId::new(7))
        );
        let outside = [car(4, -50.0, 0.0, None)];
        assert_eq!(c.eval(&view(&outside, 0)), None);
        // The crossing rule on a concave polygon (an L): the notch is outside.
        let l = [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 1.0],
            [1.0, 1.0],
            [1.0, 4.0],
            [0.0, 4.0],
        ];
        assert!(polygon_contains(&l, 0.5, 3.0));
        assert!(polygon_contains(&l, 3.0, 0.5));
        assert!(!polygon_contains(&l, 3.0, 3.0));
        assert!(!polygon_contains(&l[..2], 0.5, 0.0));
    }

    #[test]
    fn probability_zero_never_one_always() {
        let none: [TriggerActor; 0] = [];
        let c = |p: f64| Condition::Probability { p };
        let fired = |p: f64, item: usize| {
            (0..2_000u64)
                .filter(|&step| {
                    c(p).eval(&TriggerView {
                        item,
                        ..view(&none, step)
                    })
                    .is_some()
                })
                .count()
        };
        assert_eq!(fired(0.0, 3), 0);
        assert_eq!(fired(1.0, 3), 2_000);
        let third = fired(0.3, 3);
        assert!((500..=700).contains(&third), "{third} of 2000 at p = 0.3");
        // Keyed: the same (seed, item, step) gives the same answer; another item another
        // sequence.
        assert_eq!(fired(0.3, 3), third);
        let steps = |item: usize| -> Vec<u64> {
            (0..200u64)
                .filter(|&step| {
                    c(0.5)
                        .eval(&TriggerView {
                            item,
                            ..view(&none, step)
                        })
                        .is_some()
                })
                .collect()
        };
        assert_eq!(steps(3), steps(3));
        assert_ne!(steps(3), steps(4));
    }

    #[test]
    fn ttc_below_binds_the_closing_pair() {
        // Follower 2 at s = 0 doing 20 m/s, leader 5 at s = 25 doing 10 m/s: gap 25 − 5 =
        // 20 m, closing 10 m/s, TTC 2.0 s. Car 8 on another lane is ignored.
        let actors = [
            car(2, 0.0, 20.0, Some(1)),
            car(5, 25.0, 10.0, Some(1)),
            car(8, 30.0, 0.0, Some(2)),
        ];
        let v = view(&actors, 0);
        let ttc = |s: f64| Condition::TtcBelow { ttc_s: s }.eval(&v);
        assert_eq!(ttc(2.0), None, "a TTC of exactly 2 s is not below 2 s");
        let b = ttc(2.001).expect("2.0 < 2.001");
        assert_eq!(b.follower, Some(ActorId::new(2)));
        assert_eq!(b.leader, Some(ActorId::new(5)));
        assert_eq!(b.actor, None);
        assert_eq!(ttc(1.5), None);
        // An opening pair has no TTC at all.
        let opening = [car(2, 0.0, 10.0, Some(1)), car(5, 25.0, 20.0, Some(1))];
        assert_eq!(
            Condition::TtcBelow { ttc_s: 100.0 }.eval(&view(&opening, 0)),
            None
        );
        // The closing speed clause on the same pair: 10 m/s.
        let diff = |d: f64| Condition::SpeedDiffAbove { speed_diff_mps: d }.eval(&v);
        assert!(diff(9.9).is_some());
        assert_eq!(diff(10.0), None);
        // Beyond the pair range it does not hold.
        let far = [car(2, 0.0, 20.0, Some(1)), car(5, 200.0, 10.0, Some(1))];
        assert_eq!(
            Condition::SpeedDiffAbove {
                speed_diff_mps: 1.0
            }
            .eval(&view(&far, 0)),
            None
        );
    }

    /// The loader holds a `when` item to its rules: the target names a binding the clause
    /// always makes, the knobs belong to a `when` item, an area is a polygon, a composite
    /// has clauses and every number is in range.
    #[test]
    fn the_loader_holds_a_when_item_to_its_rules() {
        use crate::scenario::Scenario;
        use crate::scenario::schema::{TimelineItem, TimelineKind};
        let errors = |when: serde_json::Value, extra: serde_json::Value| -> Vec<String> {
            let mut item = serde_json::json!({"t": 1.0, "type": "safety.hard-brake"});
            if !when.is_null() {
                item["when"] = when;
            }
            for (k, v) in extra.as_object().expect("object") {
                item[k] = v.clone();
            }
            let item: TimelineItem = serde_json::from_value(item).expect("parses");
            assert_eq!(item.kind, TimelineKind::HardBrake);
            let mut s = Scenario::minimal();
            s.events = vec![item];
            crate::scenario::validate(&s)
                .iter()
                .map(ToString::to_string)
                .filter(|e| e.contains("events[0]"))
                .collect()
        };
        use serde_json::json as j;
        let ttc = j!({"kind": "ttc_below", "ttc_s": 2.0});
        let area = j!({"kind": "in_polygon", "points_m": [[0, 0], [10, 0], [10, 10]]});
        // Accepted: a pair clause and a pair binding; a composite binding through `all`.
        assert_eq!(
            errors(ttc.clone(), j!({"target": "when.leader"})),
            Vec::<String>::new()
        );
        assert_eq!(
            errors(
                j!({"kind": "all", "of": [ttc.clone(), area.clone()]}),
                j!({"target": "when.actor", "max_fires": 3, "cooldown_s": 5.0})
            ),
            Vec::<String>::new()
        );
        // Refused, each with the field named.
        let refused = |when: serde_json::Value, extra: serde_json::Value, field: &str| {
            let e = errors(when, extra);
            assert!(
                e.iter().any(|m| m.contains(field)),
                "{field} not refused: {e:?}"
            );
        };
        refused(
            ttc.clone(),
            j!({"target": "when.actor"}),
            "events[0].target",
        );
        refused(
            j!({"kind": "any", "of": [ttc.clone(), area.clone()]}),
            j!({"target": "when.actor"}),
            "events[0].target",
        );
        refused(
            serde_json::Value::Null,
            j!({"target": "when.leader"}),
            "events[0].target",
        );
        refused(
            serde_json::Value::Null,
            j!({"max_fires": 2}),
            "events[0].max_fires",
        );
        refused(ttc.clone(), j!({"max_fires": 0}), "events[0].max_fires");
        refused(
            ttc.clone(),
            j!({"cooldown_s": -1.0}),
            "events[0].cooldown_s",
        );
        refused(
            j!({"kind": "in_polygon", "points_m": [[0, 0], [10, 0]]}),
            j!({}),
            "events[0].when.points_m",
        );
        refused(
            j!({"kind": "in_polygon", "points_m": [[0, 0], [5, 5], [10, 10]]}),
            j!({}),
            "events[0].when.points_m",
        );
        refused(j!({"kind": "any", "of": []}), j!({}), "events[0].when.of");
        refused(
            j!({"kind": "ttc_below", "ttc_s": 0.0}),
            j!({}),
            "events[0].when.ttc_s",
        );
        refused(
            j!({"kind": "all", "of": [{"kind": "probability", "p": 1.5}]}),
            j!({}),
            "events[0].when.of[0].p",
        );
        refused(
            j!({"kind": "speed_diff_above", "speed_diff_mps": -1.0}),
            j!({}),
            "events[0].when.speed_diff_mps",
        );
        // A clause the schema does not know is a parse error, not a silent no-op.
        let bad = serde_json::json!({"t": 1.0, "type": "outage", "target": 1,
            "when": {"kind": "ttc_below", "s": 2.0}});
        assert!(serde_json::from_value::<TimelineItem>(bad).is_err());
    }
}
