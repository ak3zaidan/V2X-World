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
//! * Two movements the rings can show green together that the junction's conflict matrix
//!   calls foes are never both protected ([`concurrent_foe_yields`]): while its foe shows
//!   any green, the one that gives way shows green-yield instead of green. The phase
//!   numbers keep foes of different approaches apart, but not two lanes of one approach
//!   whose paths cross inside the junction (a turn across the cycle or bus lane beside
//!   it), nor foes the geometry of a skewed junction makes. A signal head cannot give one
//!   lane of a road green and its neighbour red on the same green, so the fixed-time
//!   import makes the yielding movement of such a pair permissive (SUMO's `g` beside
//!   `G`), and the dual ring does the same at every instant.
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
use v2xw_world::{
    SignalPhase, SignalPlan, SignalProgramme, SignalProgrammeKind, SignalState, World,
};

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

/// The pairs `(gives way, has right of way)` of `plan`'s vehicle movements that the rules
/// `rules` can show green together and that `plan`'s junction's conflict matrix calls foes:
/// the movement that gives way shows green-yield, not green, while the other shows any
/// green ([`NemaPlan::vector`]).
///
/// * Two movements from one approach lane share one head and are never split; two lanes
///   of one road merging into one exit lane are a lane drop, which the drivers zip. Both
///   are left out, as `v2xw_mobility::audit::audit_world` leaves them out of a fixed plan.
/// * Two lefts on their own phases from opposite approaches (1+5, 3+7) that the matrix
///   leaves level are the dual leading lefts of STM 2nd ed. Exhibit 5-3; the generated
///   matrices record them as a level conflict because no highway code settles it, and
///   their turning paths pass each other on a crossroads. They keep their arrows.
/// * Who gives way: the matrix's response, where it names one (both, where it names
///   both). Where it leaves them level, the movement that crosses opposing traffic (a left
///   or U-turn gives way, UVC §11-402), else the later movement, as the fixed-time import
///   chooses (`v2xw_world::osm`, the permissive pair rule).
pub fn concurrent_foe_yields(
    world: &World,
    plan: &SignalPlan,
    a: &Assignment,
    rules: &[Rule],
) -> Vec<(usize, usize)> {
    let Some(junction) = world.roads.try_junction(plan.junction) else {
        return Vec::new();
    };
    let mut via: BTreeMap<LaneId, (LaneId, LaneId, v2xw_world::model::TurnDirection)> =
        BTreeMap::new();
    for &l in &junction.incoming {
        for c in world.roads.successors(l) {
            if let Some(v) = c.via {
                via.entry(v)
                    .or_insert((c.from_lane, c.to_lane, c.direction));
            }
        }
    }
    let row = |l: LaneId| junction.internal.iter().position(|x| *x == l);
    let greens = |m: usize| [rules[m].own, rules[m].permit];
    let mut out = Vec::new();
    for x in 0..a.movements.len() {
        for y in x + 1..a.movements.len() {
            let (mx, my) = (a.movements[x], a.movements[y]);
            if mx.kind == MovementKind::Crossing || my.kind == MovementKind::Crossing {
                continue;
            }
            let (lx, ly) = (plan.controlled[x], plan.controlled[y]);
            let (Some(&(fx, tx, dx)), Some(&(fy, ty, dy))) = (via.get(&lx), via.get(&ly)) else {
                continue;
            };
            let (Some(rx), Some(ry)) = (row(lx), row(ly)) else {
                continue;
            };
            if fx == fy || !junction.conflicts.is_foe(rx, ry) {
                continue;
            }
            if tx == ty && world.roads.lane(fx).edge == world.roads.lane(fy).edge {
                continue;
            }
            let together = greens(x).into_iter().flatten().any(|p| {
                greens(y)
                    .into_iter()
                    .flatten()
                    .any(|q| assign::concurrent(p, q))
            });
            if !together {
                continue;
            }
            let (yx, yy) = (
                junction.conflicts.must_yield(rx, ry),
                junction.conflicts.must_yield(ry, rx),
            );
            let dual_lead = !yx
                && !yy
                && mx.kind == MovementKind::Left
                && my.kind == MovementKind::Left
                && mx.phase != my.phase
                && mx.permissive_with.is_some()
                && my.permissive_with.is_some();
            if dual_lead {
                continue;
            }
            match (yx, yy) {
                (true, false) => out.push((x, y)),
                (false, true) => out.push((y, x)),
                (true, true) => {
                    out.push((x, y));
                    out.push((y, x));
                }
                (false, false) => {
                    let (cx, cy) = (dx.crosses_opposing_traffic(), dy.crosses_opposing_traffic());
                    if cx && !cy {
                        out.push((x, y));
                    } else {
                        out.push((y, x));
                    }
                }
            }
        }
    }
    out
}

/// The NEMA number a programme phase's `name` states, 1 to 8.
fn programme_phase(ph: &v2xw_world::ProgrammePhase) -> Option<u8> {
    ph.name
        .as_deref()
        .and_then(|n| n.trim().parse::<u8>().ok())
        .filter(|q| (1..=8).contains(q))
}

/// The assignment and rules a SUMO NEMA programme states (WP-23): a movement is protected
/// by the phase whose state shows it `G` (the first, if several do), and turns
/// permissively in the phase whose state shows it `g`. The file's own numbering stands,
/// so the geometry's ([`assign::assign_phases`]) is not used.
///
/// # Errors
///
/// A label, counted as a fallback: a phase whose `name` is not a NEMA number 1-8, or a
/// movement no phase serves.
pub fn programme_rules(
    world: &World,
    plan: &SignalPlan,
    prog: &SignalProgramme,
) -> Result<(Assignment, Vec<Rule>), &'static str> {
    let numbers: Vec<u8> = prog
        .phases
        .iter()
        .map(|ph| programme_phase(ph).ok_or("programme-phase-name"))
        .collect::<Result<_, _>>()?;
    let mut dir: BTreeMap<LaneId, v2xw_world::model::TurnDirection> = BTreeMap::new();
    for c in world.roads.connections() {
        if let Some(v) = c.via {
            dir.entry(v).or_insert(c.direction);
        }
    }
    let mut movements = Vec::with_capacity(plan.controlled.len());
    let mut rules = Vec::with_capacity(plan.controlled.len());
    for (m, l) in plan.controlled.iter().enumerate() {
        let shown = |want: SignalState| {
            plan.phases
                .iter()
                .zip(&numbers)
                .find(|(ph, _)| ph.states.get(m) == Some(&want))
                .map(|(_, q)| *q)
        };
        let (own, permit) = (shown(SignalState::Green), shown(SignalState::GreenYield));
        let phase = own.or(permit).ok_or("programme-unserved-movement")?;
        let crossing = world
            .roads
            .try_lane(*l)
            .is_some_and(|x| x.kind == LaneKind::Crossing);
        let kind = if crossing {
            MovementKind::Crossing
        } else {
            match dir.get(l) {
                Some(d) if d.crosses_opposing_traffic() => MovementKind::Left,
                Some(v2xw_world::model::TurnDirection::Right) => MovementKind::Right,
                _ => MovementKind::Through,
            }
        };
        movements.push(assign::AssignedMovement {
            phase,
            kind,
            permissive_with: None,
        });
        rules.push(Rule {
            own,
            permit: if own.is_some() && permit == own {
                None
            } else {
                permit
            },
        });
    }
    Ok((Assignment { movements }, rules))
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
    /// `(gives way, has right of way)`: the foes the rings can show green together
    /// ([`concurrent_foe_yields`]).
    yields: Vec<(usize, usize)>,
    /// The phase a priority request for each signal group asks for (P17.9).
    group_phase: BTreeMap<u16, u8>,
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
        let shown: Vec<SignalState> = self
            .assign
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
            .collect();
        // A movement that gives way to a foe showing a green of its own turns permissive.
        let mut out = shown.clone();
        for &(loser, winner) in &self.yields {
            if shown[loser] == SignalState::Green
                && matches!(shown[winner], SignalState::Green | SignalState::GreenYield)
            {
                out[loser] = SignalState::GreenYield;
            }
        }
        out
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
        if p.duration_s.is_nan() || p.duration_s <= 0.0 {
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

/// The coordination of one junction (P17.8) from its load-time plan: the plan's cycle and
/// offset (a green wave, `traffic.signals.green_wave_speed_mps`, has already set the
/// offset), and splits that keep the plan's division of the cycle between the two
/// streets. The main street's group is every load-time phase in which one of its vehicle
/// movements is not red (held to 20-80 % of the cycle); within a group each left phase's
/// split is its maximum green and clearance, at most a third of the group, and the through
/// takes the rest. The splits are this build's choice: the STM computes them from volumes
/// (ch. 7), which a plan does not carry.
fn coordination(
    n: &NemaSettings,
    plan: &SignalPlan,
    a: &Assignment,
    config: &RingConfig,
) -> super::Coordination {
    let cycle = plan.cycle_s;
    let main: f64 = plan
        .phases
        .iter()
        .filter(|ph| {
            ph.states.iter().zip(&a.movements).any(|(s, m)| {
                m.kind != MovementKind::Crossing
                    && assign::main_side(m.phase)
                    && !matches!(s, SignalState::Red | SignalState::Off)
            })
        })
        .map(|ph| ph.duration_s)
        .sum();
    let main = main.clamp(0.2 * cycle, 0.8 * cycle);
    let mut split = [0.0; 9];
    for order in &config.order {
        for (side, total) in [(true, main), (false, cycle - main)] {
            let group: Vec<u8> = order
                .iter()
                .copied()
                .filter(|p| assign::main_side(*p) == side)
                .collect();
            let throughs = group.iter().filter(|p| *p % 2 == 0).count();
            let mut lefts_s = 0.0;
            for &l in group.iter().filter(|p| *p % 2 == 1) {
                let t = &config.timing[usize::from(l)];
                let s = if throughs == 0 {
                    total
                } else {
                    (t.max_green_s + t.yellow_s + t.red_clear_s).min(total / 3.0)
                };
                split[usize::from(l)] = s;
                lefts_s += s;
            }
            for &th in group.iter().filter(|p| *p % 2 == 0) {
                split[usize::from(th)] = (total - lefts_s) / throughs as f64;
            }
        }
    }
    let present: Vec<u8> = config.order.iter().flatten().copied().collect();
    super::Coordination {
        cycle_s: cycle,
        offset_s: plan.offset_s,
        phases: n
            .coordinate_phases
            .iter()
            .copied()
            .filter(|p| present.contains(p))
            .collect(),
        split_s: split,
        force_off: n.force_off,
        reference: n.controller_type,
    }
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
    ///
    /// A plan whose source file gave it a NEMA programme (`World::signal_programmes`,
    /// WP-23) runs the dual ring the file states, whatever `control.control` is: its
    /// phases' movements, per-phase timing and SUMO ring parameters
    /// ([`programme_rules`], `NemaSettings::from_sumo_params`). A plan with an actuated
    /// programme is the single ring's.
    pub fn new(control: &SignalControl, world: &World) -> Self {
        let n = control.nema.clone().unwrap_or_default();
        let global = control.control == crate::scenario::schema::SignalControlKind::Nema;
        let mut plans = BTreeMap::new();
        let mut fallback: BTreeMap<&'static str, u64> = BTreeMap::new();
        let mut shared_lane_lefts = 0;
        for (idx, plan) in world.signals.iter().enumerate() {
            let programme = world.signal_programmes.iter().find(|p| p.plan == plan.id);
            match programme {
                Some(prog) if prog.kind == SignalProgrammeKind::Nema => {
                    let built = programme_rules(world, plan, prog).and_then(|(a, rules)| {
                        let mut nf = NemaSettings::from_sumo_params(&prog.params)
                            .map_err(|_| "programme-params")?;
                        nf.walk_s = nf.walk_s.or(n.walk_s);
                        nf.ped_speed_mps = nf.ped_speed_mps.or(n.ped_speed_mps);
                        Ok((nf, a, rules))
                    });
                    match built {
                        Ok((nf, a, rules)) => {
                            let (p, _) =
                                Self::plan(control, &nf, world, plan, a, Some((&rules, prog)));
                            plans.insert(idx, p);
                        }
                        Err(label) => *fallback.entry(label).or_insert(0) += 1,
                    }
                }
                Some(_) => {}
                None if global => match assign::assign_phases(world, plan) {
                    Ok(a) => {
                        let (p, demoted) = Self::plan(control, &n, world, plan, a, None);
                        shared_lane_lefts += demoted;
                        plans.insert(idx, p);
                    }
                    Err(u) => *fallback.entry(u.label()).or_insert(0) += 1,
                },
                None => {}
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
        file: Option<(&[Rule], &SignalProgramme)>,
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
        let (rules, demoted) = match file {
            Some((r, _)) => (r.to_vec(), 0),
            None => left_rules(&a, &approach, n),
        };
        let yields = concurrent_foe_yields(world, plan, &a, &rules);
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
        // A programme's own timing, phase by phase, where its file states it (SUMO NEMA
        // `minDur`, `maxDur`, `vehext`, `yellow`, `red`).
        if let Some((_, prog)) = file {
            for ph in &prog.phases {
                let Some(q) = programme_phase(ph) else {
                    continue;
                };
                let t = &mut config.timing[usize::from(q)];
                t.min_green_s = ph.min_s.unwrap_or(t.min_green_s);
                t.max_green_s = ph.max_s.unwrap_or(t.max_green_s).max(t.min_green_s);
                t.passage_s = ph.vehext_s.unwrap_or(t.passage_s);
                t.yellow_s = ph.yellow_s.unwrap_or(t.yellow_s);
                t.red_clear_s = ph.red_s.unwrap_or(t.red_clear_s);
            }
        }
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
        if n.coordination {
            config.coord = Some(coordination(n, plan, &a, &config));
        }
        let ring = DualRing::new(config);
        // The phase a priority request for a head's signal group asks for: the phase that
        // protects the through movement of the lane the head faces.
        let mut group_phase: BTreeMap<u16, u8> = BTreeMap::new();
        for h in &plan.heads {
            let through = (0..a.movements.len()).find_map(|m| {
                (approach[m] == Some(h.lane) && a.movements[m].kind == MovementKind::Through)
                    .then_some(rules[m].own)
                    .flatten()
            });
            if let Some(q) = through {
                group_phase.entry(h.group).or_insert(q);
            }
        }
        let mut p = NemaPlan {
            assign: a,
            ring,
            approach,
            ped,
            rules,
            yields,
            group_phase,
            shown: Vec::new(),
            shown_since_s: 0.0,
            projected_end_s: f64::NEG_INFINITY,
            projected_calls: Calls::default(),
            projected_at_s: f64::NEG_INFINITY,
        };
        p.shown = p.vector(&p.ring);
        (p, demoted)
    }

    /// Serves plan `idx`'s standing priority requests, `(signal group, arrival)` earliest
    /// first, through its rings: the earliest request whose group maps to a phase
    /// ([`super::PriorityCall`], with `params`' limits). No request clears it.
    pub fn set_priority(
        &mut self,
        idx: usize,
        requests: &[(u16, f64)],
        params: &crate::priority::PriorityParams,
    ) {
        let Some(p) = self.plans.get_mut(&idx) else {
            return;
        };
        p.ring.priority = requests.iter().find_map(|(group, eta)| {
            let phase = *p.group_phase.get(group)?;
            Some(super::PriorityCall {
                phase,
                until_s: eta + params.clearance_margin_s,
                max_extension_s: params.max_extension_s,
                min_green_s: params.min_green_s,
            })
        });
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
            c.force_offs += k.force_offs;
        }
        c
    }

    /// The window the SPaT of plan `idx` states at `now_s` (P17.9): the next change comes
    /// no sooner than the rings' earliest possible change ([`super::DualRing::
    /// earliest_change_s`]) and no later than the longest green and clearance after the
    /// projected one (the whole cycle under coordination); `likelyTime` is the projection.
    /// One window serves every group of the junction, so the bound holds for each.
    pub fn outlook(&self, idx: usize, now_s: f64) -> Option<crate::actuated::ActuatedOutlook> {
        let p = self.plans.get(&idx)?;
        let longest = p
            .ring
            .config()
            .timing
            .iter()
            .map(|t| t.max_green_s + t.yellow_s + t.red_clear_s)
            .fold(0.0, f64::max);
        // A coordinated green may rest a whole cycle.
        let longest = p
            .ring
            .config()
            .coord
            .as_ref()
            .map_or(longest, |k| longest.max(k.cycle_s));
        let remaining = (p.projected_end_s - now_s).max(0.0);
        let earliest = p.ring.earliest_change_s();
        Some(crate::actuated::ActuatedOutlook {
            cut_s: (remaining - earliest).max(0.0),
            extend_s: longest,
            earliest_s: Some(earliest),
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
            // The shared clock: coordination keeps every junction's cycle on it.
            p.ring.clock_s = now_s;
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
                yields: Vec::new(),
                group_phase: BTreeMap::new(),
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

    /// `world` with plan 0's junction matrix rewritten by `edit`.
    fn with_matrix(world: &World, edit: impl Fn(&mut v2xw_world::model::ConflictMatrix)) -> World {
        let j = world.signals[0].junction;
        let mut junctions = world.roads.junctions().to_vec();
        edit(&mut junctions[j.as_usize()].conflicts);
        let mut w = world.clone();
        w.roads = v2xw_world::model::RoadNetwork::new(
            world.roads.lanes().to_vec(),
            world.roads.edges().to_vec(),
            junctions,
            world.roads.connections().to_vec(),
            world.roads.crossings().to_vec(),
        )
        .expect("the same network");
        w
    }

    /// The NEMA concurrency defect (`invariants/nema-safety` on Manhattan): foes the rings
    /// show green together are never both protected. On a generated crossroads and tee,
    /// every pair of vehicle movements from different lanes that may be green together is
    /// made a foe, a third with the first giving way, a third the second, a third level.
    /// Ten minutes of random calls: no two foes show green together (the dual leading
    /// lefts the matrix leaves level excepted, Exhibit 5-3), the one that gives way is the
    /// one shown green-yield, and protected greens and demotions both occur.
    #[test]
    fn concurrent_foes_are_never_both_protected() {
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
            let probe = NemaControllers::new(&control, base);
            let (a, rules) = (
                probe.plans[&0].assign.clone(),
                probe.plans[&0].rules.clone(),
            );
            let plan0 = base.signals[0].clone();
            let junction = base.roads.junction(plan0.junction).clone();
            let mut via: BTreeMap<LaneId, (LaneId, LaneId)> = BTreeMap::new();
            for c in base.roads.connections() {
                if let Some(v) = c.via {
                    via.entry(v).or_insert((c.from_lane, c.to_lane));
                }
            }
            let row = |m: usize| {
                junction
                    .internal
                    .iter()
                    .position(|x| *x == plan0.controlled[m])
            };
            let greens = |m: usize| [rules[m].own, rules[m].permit];
            let mut made: Vec<(usize, usize, u8)> = Vec::new();
            for x in 0..a.movements.len() {
                for y in x + 1..a.movements.len() {
                    let (mx, my) = (a.movements[x], a.movements[y]);
                    if mx.kind == MovementKind::Crossing || my.kind == MovementKind::Crossing {
                        continue;
                    }
                    let (Some(&(fx, tx)), Some(&(fy, ty))) =
                        (via.get(&plan0.controlled[x]), via.get(&plan0.controlled[y]))
                    else {
                        continue;
                    };
                    let (Some(rx), Some(ry)) = (row(x), row(y)) else {
                        continue;
                    };
                    let together = greens(x).into_iter().flatten().any(|p| {
                        greens(y)
                            .into_iter()
                            .flatten()
                            .any(|q| assign::concurrent(p, q))
                    });
                    let drop = tx == ty && base.lane(fx).edge == base.lane(fy).edge;
                    if fx != fy && !drop && together && !junction.conflicts.is_foe(rx, ry) {
                        made.push((rx, ry, (made.len() % 3) as u8));
                    }
                }
            }
            assert!(made.len() >= 2, "world {k}: {} pairs", made.len());
            let mut world = with_matrix(base, |c| {
                for &(rx, ry, how) in &made {
                    c.set_foe(rx, ry, true);
                    match how {
                        0 => c.set_response(rx, ry, true),
                        1 => c.set_response(ry, rx, true),
                        _ => {}
                    }
                }
            });
            let j = world.roads.junction(plan0.junction).clone();
            let mut ctl = NemaControllers::new(&control, &world);
            assert!(ctl.controls(0));
            let lanes = j.incoming.clone();
            let (mut demoted, mut protected) = (0u64, 0u64);
            let dt = 0.1;
            for step in 0..6000u64 {
                let now = step as f64 * dt;
                let calls: BTreeSet<LaneId> = lanes
                    .iter()
                    .copied()
                    .filter(|l| {
                        let h = (u64::from(l.index()) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                            ^ (step / 50 + 31 * k as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
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
                let st = world.signals[0].states_at(now).expect("states").to_vec();
                let go = |x: usize| matches!(st[x], SignalState::Green | SignalState::GreenYield);
                let ring = &ctl.plans[&0].ring;
                for x in 0..st.len() {
                    if st[x] == SignalState::Green {
                        protected += 1;
                    }
                    let own_green = rules[x].own.is_some_and(|q| {
                        let r = &ring.state[usize::from(assign::ring_of(q) - 1)];
                        r.phase == q && r.interval == Interval::Green
                    });
                    if own_green && st[x] == SignalState::GreenYield {
                        demoted += 1;
                    }
                    for y in 0..st.len() {
                        let (Some(rx), Some(ry)) = (row(x), row(y)) else {
                            continue;
                        };
                        if x == y || !j.conflicts.is_foe(rx, ry) {
                            continue;
                        }
                        let (mx, my) = (a.movements[x], a.movements[y]);
                        let dual_lead = !j.conflicts.must_yield(rx, ry)
                            && !j.conflicts.must_yield(ry, rx)
                            && mx.kind == MovementKind::Left
                            && my.kind == MovementKind::Left
                            && mx.phase != my.phase
                            && mx.permissive_with.is_some()
                            && my.permissive_with.is_some();
                        let same_lane = via.get(&plan0.controlled[x]).map(|v| v.0)
                            == via.get(&plan0.controlled[y]).map(|v| v.0);
                        if dual_lead || same_lane {
                            continue;
                        }
                        assert!(
                            !(st[x] == SignalState::Green && st[y] == SignalState::Green),
                            "world {k} t {now}: foes {x} and {y} both protected"
                        );
                        if j.conflicts.must_yield(rx, ry) && !j.conflicts.must_yield(ry, rx) {
                            assert!(
                                !(st[x] == SignalState::Green && go(y)),
                                "world {k} t {now}: {x} gives way to {y} but keeps its green"
                            );
                        }
                    }
                }
            }
            assert!(demoted > 100, "world {k}: {demoted} demoted movement-steps");
            assert!(
                protected > 200,
                "world {k}: {protected} protected movement-steps"
            );
        }
    }
}
