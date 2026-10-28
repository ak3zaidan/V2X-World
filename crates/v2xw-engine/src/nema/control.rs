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
//! * A left turn with its own phase is protected-permissive (the only mode this build
//!   runs; `nema.left_turn_mode` is checked by the loader): outside its own green it turns
//!   permissively (green-yield) while the through of its own approach is green, and shows
//!   amber in that through's yellow.
//! * A crosswalk walks for `walk_s` from the start of its phase's green, then shows its
//!   pedestrian change interval (amber, `crossing length / ped_speed_mps`), then don't
//!   walk. Until pedestrian push-button calls exist (P17.7) every crosswalk phase is on
//!   pedestrian recall, as is usual downtown, and its minimum green is raised to hold the
//!   walk, the clearance and the MUTCD 2009 §4E.06 3 s buffer, less the vehicle yellow and
//!   red that the buffer may overlap.
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

/// MUTCD 2009 §4E.06 ¶15: at least 3 s of steady don't walk before a conflicting vehicle
/// is released.
pub const PED_BUFFER_S: f64 = 3.0;

/// The MUTCD 2009 §4E.06 walk and clearance speed used when neither `nema.*` nor
/// `world.signals.*` states one: 7 s and 3.5 ft/s.
pub const MUTCD_WALK_S: f64 = 7.0;
/// See [`MUTCD_WALK_S`].
pub const MUTCD_PED_SPEED_MPS: f64 = 1.067;

/// One junction under the dual-ring controller.
#[derive(Debug, Clone)]
struct NemaPlan {
    assign: Assignment,
    ring: DualRing,
    /// The approach lane of each vehicle movement.
    approach: Vec<Option<LaneId>>,
    /// `(walk_s, clearance_s)` of each crosswalk movement.
    ped: Vec<Option<(f64, f64)>>,
    /// Lefts with their own phase also turn permissively.
    permissive: bool,
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
                if let Some((walk, clear)) = self.ped[m] {
                    return match showing(a.phase) {
                        Some((Interval::Green, h)) if h + 1e-9 < walk => SignalState::Green,
                        Some((Interval::Green, h)) if h + 1e-9 < walk + clear => SignalState::Amber,
                        _ => SignalState::Red,
                    };
                }
                match showing(a.phase) {
                    Some((Interval::Green, _)) => SignalState::Green,
                    Some((Interval::Yellow, _)) => SignalState::Amber,
                    _ => match a
                        .permissive_with
                        .filter(|_| self.permissive)
                        .and_then(showing)
                    {
                        Some((Interval::Green, _)) => SignalState::GreenYield,
                        Some((Interval::Yellow, _)) => SignalState::Amber,
                        _ => SignalState::Red,
                    },
                }
            })
            .collect()
    }

    /// The calls of this junction's phases from the detector lane sets.
    fn calls(&self, calls: &BTreeSet<LaneId>, extend: &BTreeSet<LaneId>) -> Calls {
        let mut c = Calls::default();
        for (m, a) in self.assign.movements.iter().enumerate() {
            if let Some(l) = self.approach[m] {
                let q = usize::from(a.phase);
                c.demand[q] |= calls.contains(&l);
                c.extend[q] |= extend.contains(&l);
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
}

impl NemaControllers {
    /// Controllers for every plan of `world` that maps to the dual ring
    /// ([`assign::assign_phases`]); the rest are counted by reason and left to the caller's
    /// single-ring controller. Under `control: nema` only.
    pub fn new(control: &SignalControl, world: &World) -> Self {
        let n = control.nema.clone().unwrap_or_default();
        let mut plans = BTreeMap::new();
        let mut fallback: BTreeMap<&'static str, u64> = BTreeMap::new();
        for (idx, plan) in world.signals.iter().enumerate() {
            match assign::assign_phases(world, plan) {
                Ok(a) => {
                    plans.insert(idx, Self::plan(control, &n, world, plan, a));
                }
                Err(u) => *fallback.entry(u.label()).or_insert(0) += 1,
            }
        }
        NemaControllers { plans, fallback }
    }

    fn plan(
        control: &SignalControl,
        n: &NemaSettings,
        world: &World,
        plan: &SignalPlan,
        a: Assignment,
    ) -> NemaPlan {
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
        let mut config = RingConfig::from_settings(n, &a.phases(), through, left);
        let walk = n.walk_s.unwrap_or(MUTCD_WALK_S);
        let speed = n.ped_speed_mps.unwrap_or(MUTCD_PED_SPEED_MPS);
        let mut ped = vec![None; a.movements.len()];
        for (m, am) in a.movements.iter().enumerate() {
            if am.kind != MovementKind::Crossing {
                continue;
            }
            let length = world
                .roads
                .try_lane(plan.controlled[m])
                .filter(|l| l.kind == LaneKind::Crossing)
                .map_or(0.0, |l| l.length_m);
            let clear = length / speed;
            ped[m] = Some((walk, clear));
            let q = usize::from(am.phase);
            let t = &mut config.timing[q];
            let need = walk + clear + (PED_BUFFER_S - t.yellow_s - t.red_clear_s).max(0.0);
            t.min_green_s = t.min_green_s.max(need);
            t.max_green_s = t.max_green_s.max(t.min_green_s);
            if !config.min_recall.contains(&am.phase) {
                config.min_recall.push(am.phase);
            }
        }
        let ring = DualRing::new(config);
        let mut p = NemaPlan {
            assign: a,
            ring,
            approach,
            ped,
            permissive: n.left_turn_mode == LeftTurnMode::ProtectedPermissive,
            shown: Vec::new(),
            shown_since_s: 0.0,
            projected_end_s: f64::NEG_INFINITY,
            projected_calls: Calls::default(),
            projected_at_s: f64::NEG_INFINITY,
        };
        p.shown = p.vector(&p.ring);
        p
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
        now_s: f64,
        dt_s: f64,
    ) {
        for (&idx, p) in &mut self.plans {
            let c = p.calls(calls, extend);
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
                ctl.step(&mut world, &calls, &calls, now, dt);
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
                                let through = a.movements[m].permissive_with.expect("a left");
                                let own =
                                    ph.states.iter().zip(&a.movements).any(|(s, mm)| {
                                        mm.phase == through && *s == SignalState::Green
                                    });
                                assert!(own, "world {k}: a permissive left without its through");
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
}
