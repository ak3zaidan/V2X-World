//! The NEMA controller on a world (WP-17 P17.5): the rings re-projected into the plan.
//!
//! # Why the plan is rewritten
//!
//! Every consumer of a signal (the drivers, the pedestrians, the SPaT encoder, the page's
//! lamps, the priority service) reads the plan's clock through [`SignalPlan::phase_at`].
//! A dual-ring controller has no fixed sequence, so this module rewrites each junction's
//! plan into the **composite intervals** that run from now: the state vector showing now
//! (composite 0, which `phase_at(now)` lands in), then the vectors a forward run of a copy
//! of the rings predicts under the current calls, with every called phase that is not
//! green now assumed to run to its maximum. `cycle_s` is their sum and `offset_s` puts
//! the start of composite 0 where the current vector began. Nothing that reads the plan
//! changes, and the SPaT is still the signal by construction (`crate::actuated`).
//!
//! The plan is rewritten when the shown vector changes, when composite 0 is about to run
//! out while the vector still shows (a green resting or extended past the prediction), and
//! at most once a second when the calls behind the prediction change. The rewrite keeps
//! [`World::validate`]'s plan invariants (one state per movement, durations summing to the
//! cycle, offset inside it), checked in debug builds. The content hash is the load-time
//! plan's and is never recomputed.
//!
//! # What each movement shows
//!
//! * A movement whose phase is green shows green; in its yellow, amber; otherwise red.
//! * A left turn with its own phase follows `nema.left_turn_mode` and
//!   `nema.permissive_display` ([`left_rules`], P17.6): its arrow, its permissive window
//!   (green-yield, then amber), or both. The window under a flashing yellow arrow is the
//!   opposing through's green, and under a circular green its own approach's green. A
//!   protected-permissive left from a lane it shares with the through runs permissive.
//! * A crosswalk walks for `walk_s` from the start of a green that serves its walk, then
//!   shows its pedestrian change interval (amber, the phase's longest crosswalk at
//!   `ped_speed_mps`), then don't walk (P17.7, [`super::DualRing::crosswalk`]). A green
//!   serves the walk when a pedestrian standing within [`PUSH_BUTTON_REACH_M`] of the
//!   crosswalk's kerb end has called it, or when the phase is on `nema.ped_recall`; that
//!   green then holds the walk, the clearance and the MUTCD 2009 §4E.06 3 s buffer, less
//!   the vehicle yellow and red the buffer may overlap. A call made after the walk ended
//!   waits for the next green.
//!
//! # Timing
//!
//! Through phases use `traffic.signals.min_green_s`, `max_green_s` and `passage_s`; left
//! phases `nema.left_*`. Yellow is `nema.yellow_s`, else the plan's own amber, else the
//! ITE change interval STM 2nd ed. Equation 6-1 gives at the fastest approach (t = 1 s,
//! a = 3.05 m/s², level grade). Red clearance is `nema.red_clear_s`, else the plan's own
//! all-red, else none.

use std::collections::{BTreeMap, BTreeSet};

use v2xw_core::ids::LaneId;
use v2xw_world::model::LaneKind;
use v2xw_world::{SignalPhase, SignalPlan, SignalState, World};

use super::assign::{self, Assignment, MovementKind};
use super::{Calls, DualRing, Interval, PhaseTiming, RingConfig, RingCounters};
use crate::scenario::schema::{LeftTurnMode, NemaSettings, SignalControl};

/// The step of the forward run that predicts the composite intervals, seconds.
pub const PROJECTION_STEP_S: f64 = 0.5;

/// How far the forward run looks, seconds: past any cycle the bounds allow at the default
/// maxima (two sides of 45 s and 25 s greens and their clearances is under 160 s).
pub const PROJECTION_HORIZON_S: f64 = 240.0;

pub use super::PED_BUFFER_S;

/// The MUTCD 2009 §4E.06 walk and clearance speed used when neither `nema.*` nor
/// `world.signals.*` states one: 7 s and 3.5 ft/s.
pub const MUTCD_WALK_S: f64 = 7.0;
/// See [`MUTCD_WALK_S`].
pub const MUTCD_PED_SPEED_MPS: f64 = 1.067;

/// What one vehicle movement shows: green (amber) while `own` is green (yellow), else
/// green-yield (amber) while `permit` is green (yellow), else red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    /// The phase that protects it, or `None` for a permissive-only left.
    pub own: Option<u8>,
    /// The phase in whose green it turns permissively, yielding.
    pub permit: Option<u8>,
}

/// The through that opposes through `t`: 2 and 6, 4 and 8.
const fn opposing_through(t: u8) -> u8 {
    match t {
        2 => 6,
        6 => 2,
        4 => 8,
        8 => 4,
        other => other,
    }
}

/// Each movement's [`Rule`] under `n.left_turn_mode` and `n.permissive_display`, and how
/// many protected-permissive left phases were demoted for want of an exclusive lane.
///
/// * Every movement but a left with its own phase is protected by its phase.
/// * `protected`: such a left turns only on its arrow.
/// * `permissive`: it has no arrow; it turns, yielding, through the permissive window.
/// * `protected-permissive`: the arrow, then the permissive window. A left whose approach
///   lane also carries the through or the right has no arrow: a left-turn phase serves only
///   an exclusive left-turn lane in US practice (a vehicle waiting for the arrow would
///   hold the through traffic behind it), so it runs permissive and is counted. Under
///   `protected` it keeps its arrow, as asked, and blocks its lane.
/// * The permissive window is the display's: under a **flashing yellow arrow** it is the
///   *opposing* through's green (MUTCD 2009 §4D.20: the FYA "shall be displayed" during the
///   permissive movement, and the display follows the opposing traffic it yields to, which
///   is what removes the lead-lag "yellow trap"); under a **circular green** it is the
///   green of the left's own approach, whose head it shares (§4D.18), so the left sees
///   that head's yellow while the opposing through may still run. A left in a shared lane
///   has only the through's head, so its window is always its own approach's green.
pub fn left_rules(
    a: &Assignment,
    approach: &[Option<LaneId>],
    n: &NemaSettings,
) -> (Vec<Rule>, u64) {
    use crate::scenario::schema::PermissiveDisplay;
    let mut demoted = 0;
    let rules = a
        .movements
        .iter()
        .enumerate()
        .map(|(m, am)| {
            let protect = Rule {
                own: Some(am.phase),
                permit: None,
            };
            let Some(own_through) = am.permissive_with.filter(|_| am.kind == MovementKind::Left)
            else {
                return protect;
            };
            let shared = approach[m].is_some_and(|l| {
                (0..a.movements.len()).any(|k| {
                    k != m && approach[k] == Some(l) && a.movements[k].kind != MovementKind::Left
                })
            });
            // A shared lane has one head, the through's circular green: no arrow face of
            // its own to flash.
            let window = match n.permissive_display {
                PermissiveDisplay::Fya if !shared => opposing_through(own_through),
                _ => own_through,
            };
            match n.left_turn_mode {
                LeftTurnMode::Protected => protect,
                LeftTurnMode::Permissive => Rule {
                    own: None,
                    permit: Some(window),
                },
                LeftTurnMode::ProtectedPermissive if shared => {
                    demoted += 1;
                    Rule {
                        own: None,
                        permit: Some(window),
                    }
                }
                LeftTurnMode::ProtectedPermissive => Rule {
                    own: Some(am.phase),
                    permit: Some(window),
                },
            }
        })
        .collect();
    (rules, demoted)
}

/// How close to a crosswalk's kerb end a standing pedestrian must be to have pressed its
/// push button, metres: the button is at the kerb, within reach of the waiting area
/// (MUTCD 2009 §4E.08 places it within 5 ft (1.5 m) of the crosswalk's extended edge and
/// 1.5 to 6 ft (0.5 to 1.8 m) from the kerb [verify the paragraph]).
pub const PUSH_BUTTON_REACH_M: f64 = 2.5;

/// A pedestrian below this speed is standing, m/s.
pub const STANDING_MPS: f64 = 0.3;

/// The standing pedestrians of one step, bucketed on a grid so a crosswalk looks only at
/// its own neighbourhood.
#[derive(Debug, Clone, Default)]
pub struct PedestrianIndex {
    cells: BTreeMap<(i64, i64), Vec<v2xw_core::geom::Vec3>>,
}

impl PedestrianIndex {
    const CELL_M: f64 = 10.0;

    fn cell(p: v2xw_core::geom::Vec3) -> (i64, i64) {
        (
            (p.x / Self::CELL_M).floor() as i64,
            (p.y / Self::CELL_M).floor() as i64,
        )
    }

    /// The index over pedestrians at `positions` with velocities, keeping the standing.
    pub fn new(
        pedestrians: impl Iterator<Item = (v2xw_core::geom::Vec3, v2xw_core::geom::Vec3)>,
    ) -> Self {
        let mut cells: BTreeMap<(i64, i64), Vec<v2xw_core::geom::Vec3>> = BTreeMap::new();
        for (pos, vel) in pedestrians {
            if vel.x.hypot(vel.y) < STANDING_MPS {
                cells.entry(Self::cell(pos)).or_default().push(pos);
            }
        }
        PedestrianIndex { cells }
    }

    /// Whether a standing pedestrian is within push-button reach of `kerb`.
    pub fn waiting_at(&self, kerb: v2xw_core::geom::Vec3) -> bool {
        let (cx, cy) = Self::cell(kerb);
        (cx - 1..=cx + 1).any(|x| {
            (cy - 1..=cy + 1).any(|y| {
                self.cells.get(&(x, y)).is_some_and(|v| {
                    v.iter()
                        .any(|p| (p.x - kerb.x).hypot(p.y - kerb.y) <= PUSH_BUTTON_REACH_M)
                })
            })
        })
    }
}

/// One junction under the dual-ring controller.
#[derive(Debug, Clone)]
struct NemaPlan {
    assign: Assignment,
    ring: DualRing,
    /// The approach lane of each vehicle movement.
    approach: Vec<Option<LaneId>>,
    /// Where each crosswalk movement starts (its kerb end), for the push-button calls.
    ped: Vec<Option<v2xw_core::geom::Vec3>>,
    /// What each vehicle movement shows ([`left_rules`]).
    rules: Vec<Rule>,
    /// The vector showing, and when it began.
    shown: Vec<SignalState>,
    shown_since_s: f64,
    /// When composite 0 of the written plan ends.
    projected_end_s: f64,
    /// The calls the written projection assumed, and when it was written.
    projected_calls: Calls,
    projected_at_s: f64,
}

impl NemaPlan {
    /// The state vector the rings `ring` show.
    fn vector(&self, ring: &DualRing) -> Vec<SignalState> {
        let showing = |q: u8| -> Option<(Interval, f64)> {
            if q == 0 {
                return None;
            }
            let s = &ring.state[usize::from(assign::ring_of(q) - 1)];
            (s.phase == q).then_some((s.interval, s.held_s))
        };
        self.assign
            .movements
            .iter()
            .enumerate()
            .map(|(m, a)| {
                if self.ped[m].is_some() {
                    return ring.crosswalk(a.phase);
                }
                let r = self.rules[m];
                match r.own.and_then(showing) {
                    Some((Interval::Green, _)) => SignalState::Green,
                    Some((Interval::Yellow, _)) => SignalState::Amber,
                    _ => match r.permit.and_then(showing) {
                        Some((Interval::Green, _)) => SignalState::GreenYield,
                        Some((Interval::Yellow, _)) => SignalState::Amber,
                        _ => SignalState::Red,
                    },
                }
            })
            .collect()
    }

    /// The calls of this junction's phases from the detector lane sets: a movement's lane
    /// calls the phase that protects it, or, for a permissive-only left, the phase that
    /// lets it turn.
    fn calls(
        &self,
        calls: &BTreeSet<LaneId>,
        extend: &BTreeSet<LaneId>,
        peds: &PedestrianIndex,
    ) -> Calls {
        let mut c = Calls::default();
        for (m, r) in self.rules.iter().enumerate() {
            if let (Some(l), Some(q)) = (self.approach[m], r.own.or(r.permit)) {
                let q = usize::from(q);
                c.demand[q] |= calls.contains(&l);
                c.extend[q] |= extend.contains(&l);
            }
        }
        for (m, start) in self.ped.iter().enumerate() {
            if let Some(start) = start
                && peds.waiting_at(*start)
            {
                c.ped[usize::from(self.assign.movements[m].phase)] = true;
            }
        }
        c
    }

    /// Rewrites `plan` into the composite intervals from `now_s`.
    fn project(&mut self, plan: &mut SignalPlan, c: &Calls, now_s: f64) {
        let mut predicted = *c;
        let greens = self.ring.greens();
        for q in 1..=8u8 {
            if !greens.contains(&q) {
                predicted.extend[usize::from(q)] = predicted.demand[usize::from(q)];
            }
        }
        let elapsed = (now_s - self.shown_since_s).max(0.0);
        let mut segments: Vec<(Vec<SignalState>, f64)> =
            vec![(self.shown.clone(), elapsed + PROJECTION_STEP_S)];
        let mut sim = self.ring.clone();
        let crossings = sim.counters().crossings;
        let mut t = 0.0;
        while t < PROJECTION_HORIZON_S {
            sim.step(&predicted, PROJECTION_STEP_S);
            t += PROJECTION_STEP_S;
            if t <= PROJECTION_STEP_S {
                // The first step is already counted in composite 0.
                continue;
            }
            let v = self.vector(&sim);
            let last = segments.last_mut().expect("composite 0");
            if v == last.0 {
                last.1 += PROJECTION_STEP_S;
            } else if segments.len() > 1
                && v == segments[0].0
                && sim.counters().crossings >= crossings + 2
            {
                break; // a whole cycle, back to now's vector
            } else {
                segments.push((v, PROJECTION_STEP_S));
            }
        }
        let first = segments[0].1;
        plan.phases = segments
            .into_iter()
            .map(|(states, duration_s)| SignalPhase {
                duration_s,
                states,
                name: None,
            })
            .collect();
        plan.cycle_s = plan.total_phase_duration_s();
        plan.offset_s = (now_s - elapsed).rem_euclid(plan.cycle_s);
        self.projected_end_s = now_s - elapsed + first;
        self.projected_calls = *c;
        self.projected_at_s = now_s;
        debug_assert!(plan_invariants(plan).is_ok(), "{:?}", plan_invariants(plan));
    }
}

/// [`World::validate`]'s invariants for one plan: a state per movement in every phase,
/// positive durations summing to the cycle, and the offset inside the cycle.
///
/// # Errors
///
/// The first invariant broken, in words.
pub fn plan_invariants(plan: &SignalPlan) -> Result<(), String> {
    if plan.phases.is_empty() || !(plan.cycle_s.is_finite() && plan.cycle_s > 0.0) {
        return Err(format!(
            "{} phases, cycle {}",
            plan.phases.len(),
            plan.cycle_s
        ));
    }
    for (i, p) in plan.phases.iter().enumerate() {
        if p.states.len() != plan.controlled.len() {
            return Err(format!(
                "phase {i} has {} states for {} movements",
                p.states.len(),
                plan.controlled.len()
            ));
        }
        if !(p.duration_s > 0.0) {
            return Err(format!("phase {i} lasts {}", p.duration_s));
        }
    }
    let sum = plan.total_phase_duration_s();
    if (sum - plan.cycle_s).abs() > 1e-6 {
        return Err(format!("durations sum to {sum}, cycle {}", plan.cycle_s));
    }
    if !(0.0..plan.cycle_s).contains(&plan.offset_s) {
        return Err(format!(
            "offset {} outside [0, {})",
            plan.offset_s, plan.cycle_s
        ));
    }
    Ok(())
}

/// Seconds of plan cycle with some vehicle movement showing `pred`, as the longest such
/// phase: the plan's own amber or all-red.
fn plan_interval(
    plan: &SignalPlan,
    vehicle: &[bool],
    pred: impl Fn(&[SignalState]) -> bool,
) -> f64 {
    plan.phases
        .iter()
        .filter(|p| {
            let v: Vec<SignalState> = p
                .states
                .iter()
                .zip(vehicle)
                .filter(|(_, v)| **v)
                .map(|(s, _)| *s)
                .collect();
            !v.is_empty() && pred(&v)
        })
        .map(|p| p.duration_s)
        .fold(0.0, f64::max)
}

/// Every NEMA junction's controller.
#[derive(Debug, Clone, Default)]
pub struct NemaControllers {
    plans: BTreeMap<usize, NemaPlan>,
    fallback: BTreeMap<&'static str, u64>,
    shared_lane_lefts: u64,
}

impl NemaControllers {
    /// Controllers for every plan of `world` that maps to the dual ring
    /// ([`assign::assign_phases`]); the rest are counted by reason and left to the caller's
    /// single-ring controller. Under `control: nema` only.
    pub fn new(control: &SignalControl, world: &World) -> Self {
        let n = control.nema.clone().unwrap_or_default();
        let mut plans = BTreeMap::new();
        let mut fallback: BTreeMap<&'static str, u64> = BTreeMap::new();
        let mut shared_lane_lefts = 0;
        for (idx, plan) in world.signals.iter().enumerate() {
            match assign::assign_phases(world, plan) {
                Ok(a) => {
                    let (p, demoted) = Self::plan(control, &n, world, plan, a);
                    shared_lane_lefts += demoted;
                    plans.insert(idx, p);
                }
                Err(u) => *fallback.entry(u.label()).or_insert(0) += 1,
            }
        }
        NemaControllers {
            plans,
            fallback,
            shared_lane_lefts,
        }
    }

    fn plan(
        control: &SignalControl,
        n: &NemaSettings,
        world: &World,
        plan: &SignalPlan,
        a: Assignment,
    ) -> (NemaPlan, u64) {
        let junction = world.roads.junction(plan.junction);
        let mut via: BTreeMap<LaneId, LaneId> = BTreeMap::new();
        for &l in &junction.incoming {
            for c in world.roads.successors(l) {
                if let Some(v) = c.via {
                    via.entry(v).or_insert(c.from_lane);
                }
            }
        }
        let approach: Vec<Option<LaneId>> = plan
            .controlled
            .iter()
            .zip(&a.movements)
            .map(|(l, m)| {
                (m.kind != MovementKind::Crossing)
                    .then(|| via.get(l).copied())
                    .flatten()
            })
            .collect();
        let vehicle: Vec<bool> = a
            .movements
            .iter()
            .map(|m| m.kind != MovementKind::Crossing)
            .collect();
        let fastest = approach
            .iter()
            .flatten()
            .map(|l| world.roads.lane(*l).speed_limit_mps)
            .fold(0.0, f64::max);
        let plan_amber = plan_interval(plan, &vehicle, |v| v.contains(&SignalState::Amber));
        // STM 2nd ed. Equation 6-1 (ITE): Y = t + v / (2a + 2Gg), t = 1 s, a = 10 ft/s².
        let ite = 1.0 + fastest / (2.0 * 3.048);
        let yellow = n
            .yellow_s
            .unwrap_or(if plan_amber > 0.0 { plan_amber } else { ite });
        let all_red = plan_interval(plan, &vehicle, |v| {
            v.iter()
                .all(|s| matches!(s, SignalState::Red | SignalState::Off))
        });
        let red = n.red_clear_s.unwrap_or(all_red);
        let through = PhaseTiming {
            min_green_s: control.min_green_s,
            max_green_s: control.max_green_s,
            passage_s: control.passage_s,
            yellow_s: yellow,
            red_clear_s: red,
        };
        let left = PhaseTiming {
            min_green_s: n.left_min_green_s,
            max_green_s: n.left_max_green_s,
            passage_s: n.left_passage_s,
            yellow_s: yellow,
            red_clear_s: red,
        };
        let (rules, demoted) = left_rules(&a, &approach, n);
        // The phases that serve something: each movement's protecting phase, each
        // crosswalk's, and each permissive window's through.
        let mut present: Vec<u8> = Vec::new();
        for (m, am) in a.movements.iter().enumerate() {
            let r = rules[m];
            let serves = if am.kind == MovementKind::Crossing {
                [Some(am.phase), None]
            } else {
                [r.own, r.permit]
            };
            present.extend(serves.into_iter().flatten());
        }
        present.sort_unstable();
        present.dedup();
        let mut config = RingConfig::from_settings(n, &present, through, left);
        let walk = n.walk_s.unwrap_or(MUTCD_WALK_S);
        let speed = n.ped_speed_mps.unwrap_or(MUTCD_PED_SPEED_MPS);
        let mut ped = vec![None; a.movements.len()];
        for (m, am) in a.movements.iter().enumerate() {
            if am.kind != MovementKind::Crossing {
                continue;
            }
            let Some(lane) = world
                .roads
                .try_lane(plan.controlled[m])
                .filter(|l| l.kind == LaneKind::Crossing)
            else {
                continue;
            };
            ped[m] = Some(lane.start());
            // One pedestrian interval per phase, timed for its longest crosswalk.
            let q = usize::from(am.phase);
            let clear = lane.length_m / speed;
            let longest = config.ped[q].map_or(0.0, |(_, c)| c);
            config.ped[q] = Some((walk, clear.max(longest)));
        }
        let ring = DualRing::new(config);
        let mut p = NemaPlan {
            assign: a,
            ring,
            approach,
            ped,
            rules,
            shown: Vec::new(),
            shown_since_s: 0.0,
            projected_end_s: f64::NEG_INFINITY,
            projected_calls: Calls::default(),
            projected_at_s: f64::NEG_INFINITY,
        };
        p.shown = p.vector(&p.ring);
        (p, demoted)
    }

    /// Protected-permissive left phases run permissive because their lane also carries
    /// the through or the right ([`left_rules`]), over every junction.
    pub fn shared_lane_lefts(&self) -> u64 {
        self.shared_lane_lefts
    }

    /// Whether plan `idx` runs under the dual ring.
    pub fn controls(&self, idx: usize) -> bool {
        self.plans.contains_key(&idx)
    }

    /// The plans under the dual ring.
    pub fn plan_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.plans.keys().copied()
    }

    /// Plans left to the single-ring controller, by reason.
    pub fn fallback_counts(&self) -> &BTreeMap<&'static str, u64> {
        &self.fallback
    }

    /// What the rings have done, over every junction.
    pub fn counters(&self) -> RingCounters {
        let mut c = RingCounters::default();
        for p in self.plans.values() {
            let k = p.ring.counters();
            c.gap_outs += k.gap_outs;
            c.max_outs += k.max_outs;
            c.crossings += k.crossings;
        }
        c
    }

    /// The window the SPaT of plan `idx` may state at `now_s`: until P17.9 gives each
    /// group its own, the next change may come at once or as late as the longest maximum
    /// green after the projected one (a bound that always holds, not a tight one).
    pub fn outlook(&self, idx: usize, now_s: f64) -> Option<crate::actuated::ActuatedOutlook> {
        let p = self.plans.get(&idx)?;
        let longest = p
            .ring
            .config()
            .timing
            .iter()
            .map(|t| t.max_green_s + t.yellow_s + t.red_clear_s)
            .fold(0.0, f64::max);
        Some(crate::actuated::ActuatedOutlook {
            cut_s: (p.projected_end_s - now_s).max(0.0),
            extend_s: longest,
        })
    }

    /// Advances every dual-ring junction by `dt_s` at `now_s` under the detector calls,
    /// rewriting its plan in `world` where the shown signal or its prediction moved.
    pub fn step(
        &mut self,
        world: &mut World,
        extend: &BTreeSet<LaneId>,
        calls: &BTreeSet<LaneId>,
        peds: &PedestrianIndex,
        now_s: f64,
        dt_s: f64,
    ) {
        for (&idx, p) in &mut self.plans {
            let c = p.calls(calls, extend, peds);
            p.ring.step(&c, dt_s);
            let v = p.vector(&p.ring);
            let mut rewrite = false;
            if v != p.shown {
                p.shown = v;
                p.shown_since_s = now_s;
                rewrite = true;
            }
            if p.projected_end_s < now_s + 2.0 * dt_s {
                rewrite = true;
            }
            if c != p.projected_calls && now_s - p.projected_at_s >= 1.0 {
                rewrite = true;
            }
            if rewrite && let Some(plan) = world.signals.get_mut(idx) {
                p.project(plan, &c, now_s);
            }
        }
    }

    /// The vector plan `idx`'s rings show now, for tests.
    #[cfg(test)]
    fn showing(&self, idx: usize) -> Option<Vec<SignalState>> {
        self.plans.get(&idx).map(|p| p.vector(&p.ring))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_world::model::RoadClass;

    /// Accept (WP-17 P17.5): the plan shows what the rings show. Ten minutes of random
    /// calls on a generated crossroads and tee: after every step the rewritten plan keeps
    /// `World::validate`'s invariants, `states_at(now)` (and one step on) is the rings'
    /// own vector, and no composite of the plan gives two foes protected greens (opposing
    /// lefts excepted) or a permissive left without its own through green.
    #[test]
    fn the_plan_shows_what_the_rings_show() {
        let (e, w, n, s) = ((1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0));
        let worlds = [
            assign::test_junction(&[
                (e, RoadClass::Primary, 2),
                (n, RoadClass::Residential, 1),
                (w, RoadClass::Primary, 2),
                (s, RoadClass::Residential, 1),
            ]),
            assign::test_junction(&[
                (e, RoadClass::Secondary, 1),
                (w, RoadClass::Secondary, 1),
                (s, RoadClass::Residential, 1),
            ]),
        ];
        let control = SignalControl {
            control: crate::scenario::schema::SignalControlKind::Nema,
            ..SignalControl::default()
        };
        for (k, base) in worlds.iter().enumerate() {
            let mut world = base.clone();
            let mut ctl = NemaControllers::new(&control, &world);
            assert!(ctl.controls(0), "{:?}", ctl.fallback_counts());
            let plan0 = world.signals[0].clone();
            let lanes: Vec<LaneId> = world.roads.junction(plan0.junction).incoming.clone();
            let junction = world.roads.junction(plan0.junction).clone();
            let a = ctl.plans[&0].assign.clone();
            let rules = ctl.plans[&0].rules.clone();
            let row: Vec<Option<usize>> = plan0
                .controlled
                .iter()
                .map(|l| junction.internal.iter().position(|x| x == l))
                .collect();
            let dt = 0.1;
            let mut rewrites = 0;
            let mut last_phases = world.signals[0].phases.clone();
            for step in 0..6000u64 {
                let now = step as f64 * dt;
                // A keyed draw per lane and 5 s window: a queue appears and stays a while.
                let calls: BTreeSet<LaneId> = lanes
                    .iter()
                    .copied()
                    .filter(|l| {
                        let h = (u64::from(l.index()) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                            ^ (step / 50 + 17 * k as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                        (h >> 59) < 9
                    })
                    .collect();
                ctl.step(
                    &mut world,
                    &calls,
                    &calls,
                    &PedestrianIndex::default(),
                    now,
                    dt,
                );
                let plan = &world.signals[0];
                plan_invariants(plan).unwrap_or_else(|e| panic!("world {k} t {now}: {e}"));
                let want = ctl.showing(0).expect("controlled");
                for t in [now, now + dt * 0.99] {
                    assert_eq!(
                        plan.states_at(t).expect("states"),
                        want.as_slice(),
                        "world {k}: the plan at {t} is not the rings' vector at {now}"
                    );
                }
                // The time into composite 0 is the time the vector has shown, so the
                // SPaT's time to change is counted from the real start of the interval.
                let (i, into) = plan.phase_at(now).expect("a phase");
                let since = now - ctl.plans[&0].shown_since_s;
                assert!(
                    i == 0 && (into - since).abs() < 1e-6,
                    "world {k} t {now}: composite {i} at {into} s, shown for {since} s"
                );
                if plan.phases != last_phases {
                    rewrites += 1;
                    last_phases = plan.phases.clone();
                    for ph in &plan.phases {
                        for x in 0..ph.states.len() {
                            for y in x + 1..ph.states.len() {
                                let (mx, my) = (a.movements[x], a.movements[y]);
                                if ph.states[x] != SignalState::Green
                                    || ph.states[y] != SignalState::Green
                                    || mx.kind == MovementKind::Crossing
                                    || my.kind == MovementKind::Crossing
                                {
                                    continue;
                                }
                                let opposing = mx.phase != my.phase
                                    && mx.permissive_with.is_some()
                                    && my.permissive_with.is_some();
                                if let (Some(rx), Some(ry)) = (row[x], row[y]) {
                                    assert!(
                                        opposing || !junction.conflicts.is_foe(rx, ry),
                                        "world {k} t {now}: {x} and {y} green together"
                                    );
                                }
                            }
                        }
                        for (m, st) in ph.states.iter().enumerate() {
                            if *st == SignalState::GreenYield {
                                // Its window's through is green beside it (P17.6: the
                                // opposing through under FYA, its own under green ball).
                                let window = rules[m].permit.expect("a permissive left");
                                let open = ph.states.iter().zip(&a.movements).any(|(s, mm)| {
                                    mm.phase == window
                                        && mm.kind == MovementKind::Through
                                        && *s == SignalState::Green
                                });
                                assert!(open, "world {k}: a permissive left with its window shut");
                            }
                        }
                    }
                }
            }
            assert!(rewrites > 50, "world {k}: {rewrites} rewrites");
            assert!(
                ctl.counters().crossings > 5,
                "world {k}: {:?}",
                ctl.counters()
            );
        }
    }

    use super::super::AssignedMovement;
    use crate::scenario::schema::{NemaLeftSequence, PermissiveDisplay};

    /// An east-west street: EB through (2), EB left (5), WB through (6), WB left (1); each
    /// left on its own lane, or (`shared`) on the through's lane.
    fn street(shared: bool) -> (Assignment, Vec<Option<LaneId>>) {
        let m = |phase, kind, permissive_with| AssignedMovement {
            phase,
            kind,
            permissive_with,
        };
        let a = Assignment {
            movements: vec![
                m(2, MovementKind::Through, None),
                m(5, MovementKind::Left, Some(2)),
                m(6, MovementKind::Through, None),
                m(1, MovementKind::Left, Some(6)),
            ],
        };
        let lane = LaneId::new;
        let approach = if shared {
            vec![Some(lane(0)), Some(lane(0)), Some(lane(2)), Some(lane(2))]
        } else {
            vec![Some(lane(0)), Some(lane(1)), Some(lane(2)), Some(lane(3))]
        };
        (a, approach)
    }

    fn rule(own: Option<u8>, permit: Option<u8>) -> Rule {
        Rule { own, permit }
    }

    /// Accept (WP-17 P17.6): the left-turn modes. With exclusive lanes, protected-permissive
    /// under FYA opens the EB left's window with the opposing (WB) through 6, under a
    /// circular green with its own through 2; protected keeps only the arrow; permissive
    /// only the window. A protected-permissive left sharing the through's lane has no arrow
    /// and no FYA face, so it runs in its own approach's green and is counted.
    #[test]
    fn left_modes_follow_the_display_and_the_lane() {
        let n = |left_turn_mode, permissive_display| NemaSettings {
            left_turn_mode,
            permissive_display,
            ..NemaSettings::default()
        };
        let (a, own_lanes) = street(false);
        let pp = LeftTurnMode::ProtectedPermissive;
        let (r, d) = left_rules(&a, &own_lanes, &n(pp, PermissiveDisplay::Fya));
        assert_eq!(
            r,
            [
                rule(Some(2), None),
                rule(Some(5), Some(6)),
                rule(Some(6), None),
                rule(Some(1), Some(2))
            ]
        );
        assert_eq!(d, 0);
        let (r, _) = left_rules(&a, &own_lanes, &n(pp, PermissiveDisplay::GreenBall));
        assert_eq!(r[1], rule(Some(5), Some(2)));
        let (r, _) = left_rules(
            &a,
            &own_lanes,
            &n(LeftTurnMode::Protected, PermissiveDisplay::Fya),
        );
        assert_eq!(r[1], rule(Some(5), None));
        let (r, _) = left_rules(
            &a,
            &own_lanes,
            &n(LeftTurnMode::Permissive, PermissiveDisplay::Fya),
        );
        assert_eq!(r[1], rule(None, Some(6)));
        let (a, shared) = street(true);
        let (r, d) = left_rules(&a, &shared, &n(pp, PermissiveDisplay::Fya));
        assert_eq!(r[1], rule(None, Some(2)));
        assert_eq!(r[3], rule(None, Some(6)));
        assert_eq!(d, 2);
    }

    /// The lead-lag "yellow trap", and why the flashing yellow arrow ends it. Ring 1 leads
    /// (1 then 2), ring 2 lags (6 then 5). When 6 ends while 2 runs on to serve the lagging
    /// EB left 5, the WB left (phase 1, behind 6) under a circular green sees its own head
    /// turn yellow while the opposing EB through still runs (the trap). Under FYA it keeps
    /// turning, yielding, because its window is the opposing through's green.
    #[test]
    fn fya_ends_the_lead_lag_yellow_trap() {
        let (a, lanes) = street(false);
        let shows = |display| {
            let n = NemaSettings {
                lefts: NemaLeftSequence::LeadLag,
                permissive_display: display,
                soft_recall: Vec::new(),
                ..NemaSettings::default()
            };
            let (rules, _) = left_rules(&a, &lanes, &n);
            let t = |min: f64, max: f64| PhaseTiming {
                min_green_s: min,
                max_green_s: max,
                passage_s: 2.5,
                yellow_s: 3.6,
                red_clear_s: 1.0,
            };
            let ring = DualRing::new(RingConfig::from_settings(
                &n,
                &[1, 2, 5, 6],
                t(7.0, 45.0),
                t(4.0, 25.0),
            ));
            let mut p = NemaPlan {
                assign: a.clone(),
                ring,
                approach: lanes.clone(),
                ped: vec![None; 4],
                rules,
                shown: Vec::new(),
                shown_since_s: 0.0,
                projected_end_s: 0.0,
                projected_calls: Calls::default(),
                projected_at_s: 0.0,
            };
            let mut c = Calls::default();
            c.demand[2] = true;
            c.extend[2] = true;
            c.demand[5] = true;
            let mut t_s = 0.0;
            while p.ring.state[1].interval != Interval::Yellow {
                c.extend[6] = t_s < 10.0;
                p.ring.step(&c, 0.1);
                t_s += 0.1;
                assert!(t_s < 60.0, "6 never ended");
            }
            assert_eq!(p.ring.greens()[0], 2, "the EB through runs on");
            p.vector(&p.ring)
        };
        let ball = shows(PermissiveDisplay::GreenBall);
        let fya = shows(PermissiveDisplay::Fya);
        // [EB through, EB left, WB through, WB left]
        assert_eq!(ball[3], SignalState::Amber, "the trap: {ball:?}");
        assert_eq!(fya[3], SignalState::GreenYield, "FYA: {fya:?}");
        assert_eq!(fya[2], SignalState::Amber);
        assert_eq!(fya[0], SignalState::Green);
    }

    /// Accept (WP-17 P17.7): a pedestrian standing at a crosswalk's kerb end calls its walk.
    /// On the TR 36.885 urban grid (sidewalks and signalised crosswalks), with no vehicle
    /// and no pedestrian recall, a crosswalk of the first plan never walks in five minutes;
    /// with one pedestrian standing at its kerb end it walks, and the walk is in the plan
    /// the pedestrians read. A pedestrian walking past does not press the button.
    #[test]
    fn a_pedestrian_at_the_kerb_calls_the_walk() {
        use v2xw_core::geom::Vec3;
        use v2xw_world::ImportOptions;
        use v2xw_world::procedural::GridParams;
        let base =
            v2xw_world::procedural::grid(&GridParams::tr36885_urban(), &ImportOptions::default())
                .expect("grid");
        let control = SignalControl {
            control: crate::scenario::schema::SignalControlKind::Nema,
            ..SignalControl::default()
        };
        let probe = NemaControllers::new(&control, &base);
        let crossings = base
            .signals
            .iter()
            .map(|s| {
                s.controlled
                    .iter()
                    .filter(|l| base.roads.lane(**l).kind == LaneKind::Crossing)
                    .count()
            })
            .sum::<usize>();
        let (idx, m) = probe
            .plan_indices()
            .find_map(|i| {
                let p = &probe.plans[&i];
                (0..p.ped.len())
                    .find(|m| p.ped[*m].is_some())
                    .map(|m| (i, m))
            })
            .unwrap_or_else(|| {
                panic!(
                    "no dual-ring plan has a crosswalk: {} plans, {crossings} controlled \
                     crosswalks, fallbacks {:?}",
                    base.signals.len(),
                    probe.fallback_counts()
                )
            });
        let kerb = probe.plans[&idx].ped[m].expect("kerb");
        let walks = |peds: Vec<(Vec3, Vec3)>| {
            let mut world = base.clone();
            let mut ctl = NemaControllers::new(&control, &world);
            let index = PedestrianIndex::new(peds.into_iter());
            let none = BTreeSet::new();
            let mut walked = 0.0;
            for k in 0..3000u32 {
                let now = f64::from(k) * 0.1;
                ctl.step(&mut world, &none, &none, &index, now, 0.1);
                if world.signals[idx].states_at(now).expect("states")[m] == SignalState::Green {
                    walked += 0.1;
                }
            }
            walked
        };
        let still = Vec3::new(0.0, 0.0, 0.0);
        assert_eq!(walks(Vec::new()), 0.0, "a walk with nobody waiting");
        let passing = Vec3::new(1.4, 0.0, 0.0);
        assert_eq!(
            walks(vec![(kerb, passing)]),
            0.0,
            "a passer-by pressed the button"
        );
        let w = walks(vec![(Vec3::new(kerb.x + 1.0, kerb.y, kerb.z), still)]);
        assert!(w >= 7.0 - 0.1, "the waiting pedestrian got {w} s of walk");
    }
}
