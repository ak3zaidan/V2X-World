//! The NEMA dual-ring eight-phase controller (`traffic.signals.control: nema`, WP-17).
//!
//! Re-implemented from the FHWA Signal Timing Manual 2nd ed. (NCHRP Report 812, 2015) and
//! the MUTCD 2009; NEMA TS 2-2016 is paywalled, so the STM is the working text. SUMO's
//! NEMA controller was read for its documented parameter names and behaviour only, never
//! for code. The build plan is `docs/design/frameworks/wp16-24-build-plans.md` §WP-17.
//!
//! # Pieces
//!
//! * [`assign`] — NEMA phase numbers for a junction's movements, from its geometry
//!   (P17.3), with the junctions that do not map counted rather than guessed.
//! * [`DualRing`] — the ring state machine as a pure unit with no world (P17.4): minimum,
//!   extension, gap-out and max-out, yellow and red clearance, barrier synchronisation,
//!   rest, dual entry and recall; pedestrian intervals, calls and recycle (P17.7);
//!   coordination with yield points, permissive windows and force-offs (P17.8); priority
//!   extension and early green (P17.9).
//! * [`control`] — the controller on a world (P17.5): the rings re-projected into each
//!   junction's [`v2xw_world::SignalPlan`] as the composite intervals from now, driven by
//!   the single-ring controller's detectors through `crate::actuated`; left-turn modes and
//!   the flashing yellow arrow (P17.6); pedestrian push buttons (P17.7); the SPaT window
//!   (P17.9).
//!
//! `control: nema` is off by default. WP-17 P17.10 (the G-TV safety check, expectation
//! rows, the city run and the model card) is still to come; until then the controller is
//! not proposed as a default.

pub mod assign;
pub mod control;

pub use assign::{AssignedMovement, Assignment, MovementKind, Unmapped, assign_phases};

use crate::scenario::schema::{NemaLeftSequence, NemaSettings};

/// One NEMA phase's timing (STM 2nd ed. ch. 6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseTiming {
    /// The shortest green, seconds (Exhibit 6-4).
    pub min_green_s: f64,
    /// The longest green once a conflicting phase has a call, seconds (Exhibit 6-9).
    pub max_green_s: f64,
    /// The passage time: the gap in detections that ends the green, seconds (§6.1.5).
    pub passage_s: f64,
    /// The yellow change interval, seconds (Equation 6-1).
    pub yellow_s: f64,
    /// The red clearance interval, seconds (Exhibit 6-3).
    pub red_clear_s: f64,
}

/// What a ring is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    /// The phase's green.
    Green,
    /// Its yellow change interval.
    Yellow,
    /// Its red clearance (and, at a barrier, the red dwell until the other ring's
    /// clearance ends too).
    RedClear,
    /// No phase of this ring is served on this side of the barrier (every phase of the
    /// ring on this side is absent or omitted): the ring shows red.
    Idle,
}

/// One ring's state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingState {
    /// The phase timing, 0 when [`Interval::Idle`].
    pub phase: u8,
    /// What it is showing.
    pub interval: Interval,
    /// Seconds into the interval.
    pub held_s: f64,
    /// Seconds since the phase's extension detectors last saw a vehicle (the gap timer).
    pub gap_s: f64,
    /// Seconds the maximum-green timer has run: it starts on the first call on a
    /// conflicting phase and does not reset (STM 2nd ed. ch. 6, maximum green).
    pub max_s: f64,
    /// Whether the maximum-green timer has started.
    pub max_running: bool,
    /// Whether the green has maxed out.
    pub maxed: bool,
    /// Whether the green has gapped out with a conflicting call waiting (latched).
    pub gapped: bool,
    /// The phase the clearance leads to, 0 when it leads across the barrier.
    pub next: u8,
    /// Whether this green serves its pedestrian walk (decided when the green starts, or
    /// recycled while it rests).
    pub walk: bool,
    /// Seconds into the green at which the walk began.
    pub walk_from_s: f64,
    /// Whether [`RingState::walk`] has been decided for this green.
    walk_decided: bool,
}

impl RingState {
    fn green(phase: u8) -> Self {
        RingState {
            phase,
            interval: if phase == 0 {
                Interval::Idle
            } else {
                Interval::Green
            },
            held_s: 0.0,
            gap_s: 0.0,
            max_s: 0.0,
            max_running: false,
            maxed: false,
            gapped: false,
            next: 0,
            walk: false,
            walk_from_s: 0.0,
            walk_decided: false,
        }
    }
}

/// The detector calls a step sees, by NEMA phase (index 1-8; 0 unused).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Calls {
    /// A vehicle is waiting for the phase: a presence call.
    pub demand: [bool; 9],
    /// A vehicle is on the phase's extension detectors now.
    pub extend: [bool; 9],
    /// A pedestrian is waiting to cross on the phase's walk (a push-button call).
    pub ped: [bool; 9],
}

/// The fixed part of a dual-ring controller: which phases exist, their order and timing,
/// and the recalls.
#[derive(Debug, Clone, PartialEq)]
pub struct RingConfig {
    /// Each ring's phases in service order: main-street side first, then side street.
    pub order: [Vec<u8>; 2],
    /// Each phase's timing, by NEMA number (index 0 unused).
    pub timing: [PhaseTiming; 9],
    /// Phases called every cycle (minimum recall).
    pub min_recall: Vec<u8>,
    /// Phases called when no other phase has a call (soft recall).
    pub soft_recall: Vec<u8>,
    /// Phases called and extended every cycle, so they time to their maximum.
    pub max_recall: Vec<u8>,
    /// The phase each ring serves on each side when only the other ring has a call there
    /// (dual entry, the usual setting for the through phases; STM 2nd ed. ch. 5
    /// [verify the clause]): `[ring][0 main, 1 side]`, 0 for none.
    pub dual_entry: [[u8; 2]; 2],
    /// Each phase's pedestrian interval `(walk_s, clearance_s)`, for the phases that carry
    /// a crosswalk (P17.7). The clearance is the longest crosswalk's at the clearance
    /// walking speed (MUTCD 2009 §4E.06).
    pub ped: [Option<(f64, f64)>; 9],
    /// Phases whose walk is served every cycle without a call (pedestrian recall).
    pub ped_recall: Vec<u8>,
    /// Coordinated operation (P17.8), or `None` for free (isolated) operation.
    pub coord: Option<Coordination>,
}

/// Coordinated operation (STM 2nd ed. ch. 7): a common cycle, an offset, a split per
/// phase, the coordinated phases' yield point and permissive window, and force-offs.
#[derive(Debug, Clone, PartialEq)]
pub struct Coordination {
    /// The common cycle, seconds.
    pub cycle_s: f64,
    /// The offset: when, on the shared clock, this junction's cycle reference falls.
    pub offset_s: f64,
    /// The coordinated phases (2 and 6 by the STM's convention).
    pub phases: Vec<u8>,
    /// Each phase's split: its green, yellow and red clearance, seconds.
    pub split_s: [f64; 9],
    /// Fixed or floating force-offs.
    pub force_off: crate::scenario::schema::ForceOffMode,
    /// What the offset references: the start of the coordinated green (TS 2) or of its
    /// yellow (Type 170), SUMO's `controllerType` documentation.
    pub reference: crate::scenario::schema::NemaControllerType,
}

impl Coordination {
    /// The yield point, seconds into the local cycle (whose zero is the offset's
    /// reference): the end of the coordinated green.
    pub fn yield_s(&self, t: &[PhaseTiming; 9]) -> f64 {
        use crate::scenario::schema::NemaControllerType;
        let c = usize::from(self.phases.first().copied().unwrap_or(2));
        match self.reference {
            NemaControllerType::Ts2 => {
                (self.split_s[c] - t[c].yellow_s - t[c].red_clear_s).rem_euclid(self.cycle_s)
            }
            NemaControllerType::Type170 => 0.0,
        }
    }

    /// Seconds since the last yield point at shared time `clock_s`.
    pub fn since_yield_s(&self, clock_s: f64, t: &[PhaseTiming; 9]) -> f64 {
        (clock_s - self.offset_s - self.yield_s(t)).rem_euclid(self.cycle_s)
    }

    /// Phase `p`'s fixed force-off, seconds after the yield point: the end of its split's
    /// green when ring `order`'s splits are laid end to end from the coordinated phase's
    /// clearance (STM 2nd ed. §7.3.4, Exhibit 7-14). `None` for a coordinated phase.
    pub fn force_off_s(&self, order: &[u8], p: u8, t: &[PhaseTiming; 9]) -> Option<f64> {
        let c = order.iter().position(|q| self.phases.contains(q))?;
        if self.phases.contains(&p) {
            return None;
        }
        let cc = usize::from(order[c]);
        let mut at = t[cc].yellow_s + t[cc].red_clear_s;
        for k in 1..order.len() {
            let q = order[(c + k) % order.len()];
            let i = usize::from(q);
            if q == p {
                return Some(at + self.split_s[i] - t[i].yellow_s - t[i].red_clear_s);
            }
            at += self.split_s[i];
        }
        None
    }
}

/// MUTCD 2009 §4E.06 ¶15: at least 3 s of steady don't walk before a conflicting vehicle
/// is released. The buffer may overlap the vehicle yellow and red.
pub const PED_BUFFER_S: f64 = 3.0;

impl RingConfig {
    /// The shortest green of phase `p` when it serves its walk: the walk, the pedestrian
    /// change interval and whatever of the buffer the yellow and red do not cover.
    pub fn walk_min_green_s(&self, p: u8) -> Option<f64> {
        let (walk, clear) = self.ped[usize::from(p)]?;
        let t = &self.timing[usize::from(p)];
        Some(walk + clear + (PED_BUFFER_S - t.yellow_s - t.red_clear_s).max(0.0))
    }
}

impl RingConfig {
    /// The configuration for the phases in `present`, ordered by `lefts`, with the
    /// through and left-turn timing of the two phase classes.
    pub fn from_settings(
        n: &NemaSettings,
        present: &[u8],
        through: PhaseTiming,
        left: PhaseTiming,
    ) -> Self {
        let has = |p: &u8| present.contains(p);
        let ring = |phases: &[u8], lead: bool| -> Vec<u8> {
            let mut v: Vec<u8> = Vec::new();
            for side in [true, false] {
                let mut group: Vec<u8> = phases
                    .iter()
                    .copied()
                    .filter(|p| has(p) && assign::main_side(*p) == side)
                    .collect();
                group.sort_unstable();
                // Ascending is left (odd) then through (even): leading.
                if !lead {
                    group.reverse();
                }
                v.extend(group);
            }
            v
        };
        let (lead1, lead2) = match n.lefts {
            NemaLeftSequence::LeadLead => (true, true),
            NemaLeftSequence::LagLag => (false, false),
            NemaLeftSequence::LeadLag => (true, false),
        };
        let mut timing = [through; 9];
        for p in [1usize, 3, 5, 7] {
            timing[p] = left;
        }
        let keep = |v: &[u8]| v.iter().copied().filter(|p| has(p)).collect::<Vec<u8>>();
        let entry = |pair: &[u8], r: usize| pair.get(r).copied().filter(|p| has(p)).unwrap_or(0);
        RingConfig {
            order: [ring(&n.ring1, lead1), ring(&n.ring2, lead2)],
            timing,
            min_recall: keep(&n.min_recall),
            soft_recall: keep(&n.soft_recall),
            max_recall: keep(&n.max_recall),
            dual_entry: [
                [entry(&n.barrier2_phases, 0), entry(&n.barrier_phases, 0)],
                [entry(&n.barrier2_phases, 1), entry(&n.barrier_phases, 1)],
            ],
            ped: [None; 9],
            ped_recall: keep(&n.ped_recall),
            coord: None,
        }
    }
}

/// What a dual-ring controller has done.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct RingCounters {
    /// Greens ended by a gap.
    pub gap_outs: u64,
    /// Greens ended at their maximum.
    pub max_outs: u64,
    /// Barrier crossings.
    pub crossings: u64,
    /// Non-coordinated greens ended at their force-off (coordination).
    pub force_offs: u64,
}

/// The NEMA dual-ring state machine, with no world: two rings of phases timing
/// concurrently, each phase's green running from its minimum through extensions to a gap
/// or its maximum, then its yellow and red clearance; the rings cross each barrier
/// together (STM 2nd ed. §5.1.2, Exhibit 5-3).
///
/// # Rules (STM 2nd ed. ch. 5-6; NEMA TS 2 behaviour as the STM describes it)
///
/// * A green runs at least `min_green_s`. Past it, it is **ready to end** once its
///   extension detectors have been clear for `passage_s` (a gap) or its maximum timer,
///   started by the first call on a conflicting phase, reaches `max_green_s`. A gap
///   while a conflicting call waits commits the phase to end (it is not re-extended), so
///   a phase held at the barrier by its partner ends as a gap-out, not a max-out.
/// * A ready green ends only for a call it conflicts with. With none it **rests** in green.
/// * Within its side of the barrier, a ring moves to its next called phase on its own:
///   the other ring is not consulted (concurrent timing).
/// * A ring with no called phase left on its side waits at the barrier, holding its green.
///   When both rings are ready at the barrier and a call needs the other side (or a phase
///   this side already passed), both begin their clearances on the same step, and both
///   start their next greens on the step the later clearance ends: the earlier ring
///   dwells in red.
/// * Entering a side, each ring serves its first called phase there; a ring with no call
///   there serves its dual-entry phase (the through), or shows red when it has none.
/// * Recall: a minimum-recall phase is always called; a maximum-recall phase is always
///   called and extended, so it times to its maximum; a soft-recall phase is called when
///   no other phase is.
#[derive(Debug, Clone, PartialEq)]
pub struct DualRing {
    config: RingConfig,
    present: [bool; 9],
    /// The two rings.
    pub state: [RingState; 2],
    /// The side of the barrier being served: true is the main street (1, 2, 5, 6).
    pub main: bool,
    /// A crossing is under way: both rings are clearing towards the other side.
    pub crossing: bool,
    /// Pedestrian calls waiting for a phase's next walk (a push button latches).
    pub ped_waiting: [bool; 9],
    /// The shared clock, seconds: coordination reads its local cycle from it.
    pub clock_s: f64,
    /// The priority request served through the rings, if one stands (P17.9).
    pub priority: Option<PriorityCall>,
    counters: RingCounters,
}

/// A priority request as the rings serve it (NTCIP 1211's strategies, with the limits of
/// `crate::priority::PriorityParams`): a **green extension** holds the requested phase's
/// green until the vehicle has arrived and cleared, at most `max_extension_s` past its
/// maximum; an **early green** ends every conflicting green once it has run
/// `min_green_s` (and any walk it serves), so the rings reach the requested phase sooner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriorityCall {
    /// The phase the vehicle needs.
    pub phase: u8,
    /// Hold its green until this clock time: the arrival plus the clearance margin.
    pub until_s: f64,
    /// The longest its green is held past its maximum, seconds.
    pub max_extension_s: f64,
    /// A conflicting green is never ended before this, seconds.
    pub min_green_s: f64,
}

impl DualRing {
    /// A controller starting in the main street's through phases (2 and 6, the usual
    /// start-up phases), or its first main-street phases when those are absent.
    pub fn new(config: RingConfig) -> Self {
        let mut present = [false; 9];
        for r in &config.order {
            for &p in r {
                if (1..=8).contains(&p) {
                    present[usize::from(p)] = true;
                }
            }
        }
        let start = |r: usize| {
            let d = config.dual_entry[r][0];
            if d != 0 {
                d
            } else {
                config.order[r]
                    .iter()
                    .copied()
                    .find(|p| assign::main_side(*p))
                    .unwrap_or(0)
            }
        };
        let state = [RingState::green(start(0)), RingState::green(start(1))];
        DualRing {
            config,
            present,
            state,
            main: true,
            crossing: false,
            ped_waiting: [false; 9],
            clock_s: 0.0,
            priority: None,
            counters: RingCounters::default(),
        }
    }

    /// The configuration.
    pub fn config(&self) -> &RingConfig {
        &self.config
    }

    /// What the controller has done.
    pub fn counters(&self) -> RingCounters {
        self.counters
    }

    /// The phases showing green now, by ring (0 for none).
    pub fn greens(&self) -> [u8; 2] {
        let g = |s: &RingState| {
            if s.interval == Interval::Green {
                s.phase
            } else {
                0
            }
        };
        [g(&self.state[0]), g(&self.state[1])]
    }

    /// The calls with the recalls applied.
    fn effective(&self, calls: &Calls) -> Calls {
        let mut c = Calls::default();
        for p in 1..=8usize {
            if !self.present[p] {
                continue;
            }
            c.demand[p] = calls.demand[p] || self.ped_waiting[p];
            c.extend[p] = calls.extend[p];
        }
        // A priority request calls its phase.
        if let Some(pc) = self.priority
            && self.present[usize::from(pc.phase)]
        {
            c.demand[usize::from(pc.phase)] = true;
        }
        // The coordinated phases are always called: the cycle returns to them.
        if let Some(k) = &self.config.coord {
            for &p in &k.phases {
                if self.present[usize::from(p)] {
                    c.demand[usize::from(p)] = true;
                }
            }
        }
        for &p in &self.config.ped_recall {
            if self.config.ped[usize::from(p)].is_some() {
                c.demand[usize::from(p)] = true;
            }
        }
        for &p in &self.config.min_recall {
            c.demand[usize::from(p)] = true;
        }
        for &p in &self.config.max_recall {
            c.demand[usize::from(p)] = true;
            c.extend[usize::from(p)] = true;
        }
        let soft = &self.config.soft_recall;
        let other = (1..=8u8).any(|p| c.demand[usize::from(p)] && !soft.contains(&p));
        if !other {
            for &p in soft {
                c.demand[usize::from(p)] = true;
            }
        }
        c
    }

    /// The next phase after `p` in ring `r` on the current side that has a call.
    fn next_called(&self, r: usize, p: u8, c: &Calls) -> Option<u8> {
        let order = &self.config.order[r];
        let at = order.iter().position(|x| *x == p)?;
        order[at + 1..]
            .iter()
            .copied()
            .take_while(|q| assign::main_side(*q) == self.main)
            .find(|q| c.demand[usize::from(*q)])
    }

    /// Whether some call can be served only by crossing the barrier: one on the other
    /// side, or one this side that a ring has already passed.
    fn wants_crossing(&self, c: &Calls) -> bool {
        (1..=8u8).any(|p| {
            if !c.demand[usize::from(p)] {
                return false;
            }
            if assign::main_side(p) != self.main {
                return true;
            }
            let r = usize::from(assign::ring_of(p) - 1);
            let s = &self.state[r];
            if s.phase == 0 || s.phase == p {
                return false;
            }
            let order = &self.config.order[r];
            let (Some(at), Some(q)) = (
                order.iter().position(|x| *x == s.phase),
                order.iter().position(|x| *x == p),
            ) else {
                return false;
            };
            q < at
        })
    }

    /// The phase ring `r` serves on entering the side `main`.
    fn entry_phase(&self, r: usize, main: bool, c: &Calls) -> u8 {
        let called = self.config.order[r]
            .iter()
            .copied()
            .find(|p| assign::main_side(*p) == main && c.demand[usize::from(*p)]);
        called.unwrap_or(self.config.dual_entry[r][usize::from(!main)])
    }

    /// Whether ring `r`'s green is past its minimum and gapped or maxed.
    fn ready(&self, r: usize) -> bool {
        let s = &self.state[r];
        match s.interval {
            Interval::Idle => true,
            Interval::Green => {
                let t = &self.config.timing[usize::from(s.phase)];
                let past_min = s.held_s + 1e-9 >= self.min_green(s);
                if let Some(pc) = self.priority {
                    if s.phase == pc.phase {
                        // Green extension: held until the vehicle has cleared, capped.
                        if self.clock_s < pc.until_s
                            && s.held_s < t.max_green_s + pc.max_extension_s
                        {
                            return false;
                        }
                    } else if !assign::concurrent(s.phase, pc.phase)
                        && !self.greens().contains(&pc.phase)
                    {
                        // Early green: a conflicting green ends once past its minimum.
                        return past_min && s.held_s + 1e-9 >= pc.min_green_s;
                    }
                }
                if let Some(k) = &self.config.coord {
                    if k.phases.contains(&s.phase) {
                        // A coordinated green holds to its yield point and may end only in
                        // the permissive window after it.
                        return past_min && self.in_permissive_window(k, r);
                    }
                    if self.forced_off(k, r) {
                        return past_min;
                    }
                }
                past_min && (s.maxed || s.gapped || s.gap_s + 1e-9 >= t.passage_s)
            }
            _ => false,
        }
    }

    /// Whether the clock is in the coordinated phases' permissive window: from the yield
    /// point until the phase that follows the coordinated phase in ring `r`'s sequence
    /// could no longer time its minimum green before its force-off (STM 2nd ed. ch. 7,
    /// permissive period). A call that misses it waits for the next cycle's yield point.
    fn in_permissive_window(&self, k: &Coordination, r: usize) -> bool {
        let t = &self.config.timing;
        let since = k.since_yield_s(self.clock_s, t);
        let order = &self.config.order[r];
        let next = order
            .iter()
            .position(|q| k.phases.contains(q))
            .and_then(|c| {
                (1..order.len())
                    .map(|j| order[(c + j) % order.len()])
                    .next()
            });
        // A ring with nothing but its coordinated phase may end whenever after the yield.
        let window = next
            .and_then(|q| Some(k.force_off_s(order, q, t)? - t[usize::from(q)].min_green_s))
            .unwrap_or(k.cycle_s);
        // At least a second, so a split too short for its minimum still opens the window.
        since + 1e-9 < window.max(1.0)
    }

    /// Whether ring `r`'s non-coordinated green has reached its force-off: fixed, at its
    /// point in the cycle; floating, when it has run its split's green (STM 2nd ed. §7.3.4,
    /// Exhibit 7-14).
    fn forced_off(&self, k: &Coordination, r: usize) -> bool {
        use crate::scenario::schema::ForceOffMode;
        let s = &self.state[r];
        let t = &self.config.timing;
        let i = usize::from(s.phase);
        match k.force_off {
            ForceOffMode::Floating => {
                s.held_s + 1e-9 >= k.split_s[i] - t[i].yellow_s - t[i].red_clear_s
            }
            ForceOffMode::Fixed => k
                .force_off_s(&self.config.order[r], s.phase, t)
                .is_some_and(|fo| k.since_yield_s(self.clock_s, t) + 1e-9 >= fo),
        }
    }

    /// The minimum of ring state `s`'s green: the vehicle minimum, raised to hold the walk,
    /// its clearance and the buffer when this green serves the walk.
    fn min_green(&self, s: &RingState) -> f64 {
        let t = &self.config.timing[usize::from(s.phase)];
        match self.config.walk_min_green_s(s.phase) {
            Some(w) if s.walk => t.min_green_s.max(s.walk_from_s + w),
            _ => t.min_green_s,
        }
    }

    fn end_green(&mut self, r: usize, next: u8) {
        let forced = self
            .config
            .coord
            .as_ref()
            .is_some_and(|k| !k.phases.contains(&self.state[r].phase) && self.forced_off(k, r));
        let s = &mut self.state[r];
        if s.interval == Interval::Green {
            if forced {
                self.counters.force_offs += 1;
            } else if s.maxed {
                self.counters.max_outs += 1;
            } else {
                self.counters.gap_outs += 1;
            }
            *s = RingState {
                interval: Interval::Yellow,
                held_s: 0.0,
                next,
                ..*s
            };
        }
    }

    /// Advances the controller by `dt_s` seconds under `calls`.
    pub fn step(&mut self, calls: &Calls, dt_s: f64) {
        self.clock_s += dt_s;
        // A pedestrian's push-button call waits for the phase's next walk, unless the walk
        // showing now already lets them start.
        for p in 1..=8u8 {
            let i = usize::from(p);
            if !calls.ped[i] || self.config.ped[i].is_none() {
                continue;
            }
            let s = &self.state[usize::from(assign::ring_of(p) - 1)];
            let walking = s.phase == p
                && s.interval == Interval::Green
                && s.walk
                && self.config.ped[i].is_some_and(|(w, _)| s.held_s - s.walk_from_s + 1e-9 < w);
            if !walking {
                self.ped_waiting[i] = true;
            }
        }
        let c = self.effective(calls);
        // Timers.
        for r in 0..2 {
            let s = self.state[r];
            let t = self.config.timing[usize::from(s.phase)];
            let min_g = self.min_green(&s);
            let mut n = s;
            n.held_s += dt_s;
            if s.interval == Interval::Green {
                let p = s.phase;
                n.gap_s = if c.extend[usize::from(p)] {
                    0.0
                } else {
                    s.gap_s + dt_s
                };
                let conflicting =
                    (1..=8u8).any(|q| c.demand[usize::from(q)] && !assign::concurrent(p, q));
                n.max_running = s.max_running || conflicting;
                if n.max_running {
                    n.max_s += dt_s;
                }
                // A gap with a conflicting call commits the phase to end: later detections
                // do not re-extend it, and a maximum that runs out while it waits at the
                // barrier does not make it a max-out.
                if conflicting && n.held_s + 1e-9 >= min_g && n.gap_s + 1e-9 >= t.passage_s {
                    n.gapped = true;
                }
                // A served walk holds the green past a shorter maximum.
                if !n.gapped && n.max_s + 1e-9 >= t.max_green_s.max(min_g) {
                    n.maxed = true;
                }
            }
            if n.interval == Interval::Yellow && n.held_s + 1e-9 >= t.yellow_s {
                n.interval = Interval::RedClear;
                n.held_s = (n.held_s - t.yellow_s).max(0.0);
            }
            if n.interval == Interval::RedClear && n.next != 0 && n.held_s + 1e-9 >= t.red_clear_s {
                n = RingState {
                    held_s: (n.held_s - t.red_clear_s).max(0.0),
                    ..RingState::green(n.next)
                };
            }
            self.state[r] = n;
        }
        // An idle ring takes up a call on its own side.
        if !self.crossing {
            for r in 0..2 {
                if self.state[r].interval == Interval::Idle {
                    let p = self.entry_phase(r, self.main, &c);
                    if p != 0 && c.demand[usize::from(p)] {
                        self.state[r] = RingState::green(p);
                    }
                }
            }
        }
        // Within a side, each ring moves on by itself.
        for r in 0..2 {
            if self.crossing || self.state[r].interval != Interval::Green || !self.ready(r) {
                continue;
            }
            let p = self.state[r].phase;
            if let Some(n) = self.next_called(r, p, &c) {
                self.end_green(r, n);
            }
        }
        // The barrier: both rings ready with nothing left this side.
        if !self.crossing {
            let at_barrier = |r: usize| {
                let s = &self.state[r];
                s.interval == Interval::Idle
                    || (s.interval == Interval::Green
                        && self.ready(r)
                        && self.next_called(r, s.phase, &c).is_none())
            };
            if at_barrier(0) && at_barrier(1) && self.wants_crossing(&c) {
                self.crossing = true;
                for r in 0..2 {
                    self.end_green(r, 0);
                }
            }
        }
        if self.crossing {
            let cleared = |s: &RingState| {
                s.interval == Interval::Idle
                    || (s.interval == Interval::RedClear
                        && s.held_s + 1e-9 >= self.config.timing[usize::from(s.phase)].red_clear_s)
            };
            if cleared(&self.state[0]) && cleared(&self.state[1]) {
                self.crossing = false;
                self.counters.crossings += 1;
                let other = !self.main;
                let any_there =
                    (1..=8u8).any(|p| c.demand[usize::from(p)] && assign::main_side(p) == other);
                // With nothing called over there, the rings pass straight back through
                // (every phase there skipped) to the calls this side already passed.
                if any_there {
                    self.main = other;
                } else {
                    self.counters.crossings += 1;
                }
                let main = self.main;
                self.state = [
                    RingState::green(self.entry_phase(0, main, &c)),
                    RingState::green(self.entry_phase(1, main, &c)),
                ];
            }
        }
        // A green that has just started serves its walk if a pedestrian has called it or
        // it is on pedestrian recall; the call is then answered.
        for r in 0..2 {
            let s = self.state[r];
            if s.interval == Interval::Green && !s.walk_decided {
                let i = usize::from(s.phase);
                let walk = self.config.ped[i].is_some()
                    && (self.ped_waiting[i] || self.config.ped_recall.contains(&s.phase));
                if walk {
                    self.ped_waiting[i] = false;
                }
                self.state[r].walk = walk;
                self.state[r].walk_decided = true;
            } else if s.interval == Interval::Green && self.ped_waiting[usize::from(s.phase)] {
                // Pedestrian recycle: a green resting with no conflicting call serves a
                // new walk at once rather than leave the pedestrian waiting for a call
                // elsewhere to cycle the controller.
                let i = usize::from(s.phase);
                let p = s.phase;
                let resting = !(1..=8u8)
                    .any(|q| c.demand[usize::from(q)] && q != p && !assign::concurrent(p, q));
                let done = self.config.ped[i]
                    .is_some_and(|(w, cl)| !s.walk || s.held_s - s.walk_from_s + 1e-9 >= w + cl);
                if resting && done {
                    self.ped_waiting[i] = false;
                    self.state[r].walk = true;
                    self.state[r].walk_from_s = s.held_s;
                    self.state[r].gapped = false;
                }
            }
        }
    }

    /// The soonest any indication can change, seconds from now (for SPaT's
    /// `minEndTime`): a green not before its minimum (or the end of its walk or
    /// pedestrian clearance), a yellow or red clearance at its end, and an idle ring at once
    /// (a call may start it).
    pub fn earliest_change_s(&self) -> f64 {
        let mut soonest = f64::INFINITY;
        for s in &self.state {
            let t = &self.config.timing[usize::from(s.phase)];
            let left = match s.interval {
                Interval::Idle => 0.0,
                Interval::Yellow => t.yellow_s - s.held_s,
                Interval::RedClear => t.red_clear_s - s.held_s,
                Interval::Green => {
                    let mut g = self.min_green(s) - s.held_s;
                    if s.walk
                        && let Some((walk, clear)) = self.config.ped[usize::from(s.phase)]
                    {
                        let h = s.held_s - s.walk_from_s;
                        for edge in [walk, walk + clear] {
                            if h < edge {
                                g = g.min(edge - h);
                            }
                        }
                    }
                    g
                }
            };
            soonest = soonest.min(left.max(0.0));
        }
        soonest
    }

    /// What a crosswalk of phase `p` shows: walk (green) for the walk interval of a green
    /// that serves it, then the pedestrian change interval (amber), else don't walk (red).
    pub fn crosswalk(&self, p: u8) -> v2xw_world::SignalState {
        use v2xw_world::SignalState;
        let Some((walk, clear)) = self.config.ped.get(usize::from(p)).copied().flatten() else {
            return SignalState::Red;
        };
        if p == 0 {
            return SignalState::Red;
        }
        let s = &self.state[usize::from(assign::ring_of(p) - 1)];
        let h = s.held_s - s.walk_from_s;
        if s.phase != p || s.interval != Interval::Green || !s.walk {
            SignalState::Red
        } else if h + 1e-9 < walk {
            SignalState::Green
        } else if h + 1e-9 < walk + clear {
            SignalState::Amber
        } else {
            SignalState::Red
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::scenario::schema::{NemaLeftSequence, NemaSettings};

    /// STM 2nd ed. values: through min/max/passage inside Exhibits 6-4/6-9 and p. 6-12
    /// (the engine's 7 / 45 / 2.5), yellow 3.6 s at 35 mph (Exhibit 6-2), red 1.0 s;
    /// left 4 / 25 / 2.5 (P17.1), yellow 3.0 s at 25 mph.
    const THROUGH: PhaseTiming = PhaseTiming {
        min_green_s: 7.0,
        max_green_s: 45.0,
        passage_s: 2.5,
        yellow_s: 3.6,
        red_clear_s: 1.0,
    };
    const LEFT: PhaseTiming = PhaseTiming {
        min_green_s: 4.0,
        max_green_s: 25.0,
        passage_s: 2.5,
        yellow_s: 3.0,
        red_clear_s: 0.5,
    };
    const DT: f64 = 0.1;
    const ALL: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

    fn controller(lefts: NemaLeftSequence, present: &[u8]) -> DualRing {
        let n = NemaSettings {
            lefts,
            soft_recall: Vec::new(),
            ..NemaSettings::default()
        };
        DualRing::new(RingConfig::from_settings(&n, present, THROUGH, LEFT))
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() <= 2.0 * DT + 1e-9
    }

    /// The minimum, extension, maximum and rest of one green: a quiet 2 against a call
    /// on 4 ends at its minimum; a busy 2 runs to its maximum, holding 6 (gapped out at
    /// its minimum) at the barrier; with no conflicting call a green rests.
    #[test]
    fn a_green_runs_its_minimum_extends_to_its_maximum_and_rests() {
        let mut quiet = controller(NemaLeftSequence::LeadLead, &ALL);
        let mut calls = Calls::default();
        calls.demand[4] = true;
        let mut t = 0.0;
        while quiet.greens() == [2, 6] && t < 100.0 {
            quiet.step(&calls, DT);
            t += DT;
        }
        assert!(near(t, THROUGH.min_green_s), "quiet greens ended at {t}");
        let mut busy = controller(NemaLeftSequence::LeadLead, &ALL);
        calls.extend[2] = true;
        let (mut t, mut six_ready_at) = (0.0, None);
        while busy.state[0].interval == Interval::Green && t < 100.0 {
            busy.step(&calls, DT);
            t += DT;
            if six_ready_at.is_none() && busy.ready(1) {
                six_ready_at = Some(t);
            }
            assert_eq!(
                busy.greens()[1] == 6,
                busy.greens()[0] == 2,
                "6 waits for 2 at {t}"
            );
        }
        assert!(near(t, THROUGH.max_green_s), "2 maxed out at {t}");
        assert!(near(six_ready_at.expect("6 gapped"), THROUGH.min_green_s));
        assert_eq!(busy.counters().max_outs, 1);
        assert_eq!(busy.counters().gap_outs, 1);
        // Through both clearances to 4 and its dual-entry partner 8.
        for _ in 0..((THROUGH.yellow_s + THROUGH.red_clear_s) / DT).round() as usize + 1 {
            busy.step(&calls, DT);
        }
        assert_eq!(busy.greens(), [4, 8]);
        // No conflicting call: 2 and 6 rest in green.
        let mut rest = controller(NemaLeftSequence::LeadLead, &ALL);
        for _ in 0..3000 {
            rest.step(&Calls::default(), DT);
        }
        assert_eq!(rest.greens(), [2, 6]);
    }

    /// Accept (WP-17): two concurrent throughs gap out independently. Lead-lag puts 5
    /// after 6 in ring 2. Calls continue on 2 and stop on 6 at 10 s; 5 is called. 6 gaps
    /// out at 10 + passage, 5 is served after 6's clearance, and 2 runs on throughout.
    #[test]
    fn concurrent_throughs_gap_out_independently() {
        let mut ctl = controller(NemaLeftSequence::LeadLag, &ALL);
        assert_eq!(ctl.config().order[1], [6, 5, 8, 7]);
        let (mut six_end, mut five_start) = (None, None);
        let mut t = 0.0;
        while t < 60.0 {
            let mut calls = Calls::default();
            calls.demand[2] = true;
            calls.extend[2] = true;
            calls.extend[6] = t < 10.0;
            calls.demand[5] = true;
            ctl.step(&calls, DT);
            t += DT;
            let g = ctl.greens();
            assert_eq!(g[0], 2, "2 stays green at {t}: {:?}", ctl.state);
            if six_end.is_none() && g[1] != 6 {
                six_end = Some(t);
            }
            if five_start.is_none() && g[1] == 5 {
                five_start = Some(t);
            }
        }
        let six_end = six_end.expect("6 ended");
        assert!(
            near(six_end, 10.0 + THROUGH.passage_s),
            "6 ended at {six_end}"
        );
        let five = five_start.expect("5 served while 2 continues");
        assert!(
            near(five, six_end + THROUGH.yellow_s + THROUGH.red_clear_s),
            "5 began at {five}"
        );
        assert_eq!(ctl.counters().gap_outs, 1);
    }

    /// A deterministic arrival stream: a SplitMix64 counter keyed by seed, phase and step,
    /// in the spirit of the engine's keyed hashes (no shared stream position).
    fn uniform(seed: u64, phase: u8, step: u64) -> f64 {
        let mut z = seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(u64::from(phase) << 40)
            .wrapping_add(step);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Vehicles arriving at random on each phase and leaving one per 2 s of green: the
    /// demand is a waiting queue, the extension a vehicle crossing the detector.
    struct Traffic {
        queue: [u32; 9],
        discharge_s: [f64; 9],
        seed: u64,
        step: u64,
    }

    impl Traffic {
        fn new(seed: u64) -> Self {
            Traffic {
                queue: [0; 9],
                discharge_s: [0.0; 9],
                seed,
                step: 0,
            }
        }

        fn calls(&mut self, greens: [u8; 2], rate_per_s: &[f64; 9]) -> Calls {
            self.step += 1;
            let mut c = Calls::default();
            for p in 1..=8u8 {
                let i = usize::from(p);
                if uniform(self.seed, p, self.step) < rate_per_s[i] * DT {
                    self.queue[i] += 1;
                }
                if greens.contains(&p) && self.queue[i] > 0 {
                    self.discharge_s[i] += DT;
                    c.extend[i] = true;
                    if self.discharge_s[i] >= 2.0 {
                        self.discharge_s[i] = 0.0;
                        self.queue[i] -= 1;
                    }
                }
                c.demand[i] = self.queue[i] > 0;
            }
            c
        }
    }

    /// Runs `ctl` for `secs` under random traffic, checking at every step that the rings
    /// keep to one side of the barrier, that every green, yellow and red clearance ran in
    /// full, and that both rings start their greens on the same step at each crossing.
    /// Returns the phases served.
    fn run_checked(
        ctl: &mut DualRing,
        seed: u64,
        secs: f64,
        mut on_step: impl FnMut(&DualRing),
    ) -> [bool; 9] {
        let mut rate = [0.0; 9];
        for (p, r) in rate.iter_mut().enumerate().skip(1) {
            // Throughs busier than lefts, main busier than side.
            *r = match p {
                2 | 6 => 0.15,
                4 | 8 => 0.08,
                _ => 0.03,
            };
        }
        let mut traffic = Traffic::new(seed);
        let mut served = [false; 9];
        let mut prev = ctl.state;
        let mut crossings = ctl.counters().crossings;
        let steps = (secs / DT).round() as u64;
        for k in 0..steps {
            let calls = traffic.calls(ctl.greens(), &rate);
            ctl.step(&calls, DT);
            on_step(ctl);
            let s = ctl.state;
            let sides: Vec<bool> = s
                .iter()
                .filter(|x| x.phase != 0)
                .map(|x| assign::main_side(x.phase))
                .collect();
            assert!(
                sides.windows(2).all(|w| w[0] == w[1]),
                "seed {seed} step {k}: rings on both sides of the barrier: {s:?}"
            );
            for r in 0..2 {
                let (a, b) = (prev[r], s[r]);
                let t = ctl.config().timing[usize::from(a.phase)];
                let changed = a.interval != b.interval || a.phase != b.phase;
                if changed {
                    let ran = a.held_s + DT;
                    let need = match a.interval {
                        Interval::Green => t.min_green_s,
                        Interval::Yellow => t.yellow_s,
                        Interval::RedClear => t.red_clear_s,
                        Interval::Idle => 0.0,
                    };
                    assert!(
                        ran + 1e-6 >= need,
                        "seed {seed} step {k} ring {r}: {:?} of phase {} ran {ran} s < {need} s",
                        a.interval,
                        a.phase
                    );
                }
                if b.interval == Interval::Green {
                    served[usize::from(b.phase)] = true;
                }
            }
            if ctl.counters().crossings != crossings {
                for r in 0..2 {
                    assert!(
                        s[r].interval == Interval::Idle
                            || (s[r].interval == Interval::Green && s[r].held_s == 0.0),
                        "seed {seed} step {k}: ring {r} did not start with the other: {s:?}"
                    );
                }
                crossings = ctl.counters().crossings;
            }
            prev = s;
        }
        served
    }

    /// Accept (WP-17): the ring pointers meet at each barrier. One hour of random calls
    /// under five seeds (lead-lead, lag-lag and lead-lag): no instant has the rings on
    /// opposite sides, both clearances end together at every crossing, every interval
    /// runs in full, and every phase is served.
    #[test]
    fn rings_cross_barriers_together() {
        let lefts = [
            NemaLeftSequence::LeadLead,
            NemaLeftSequence::LagLag,
            NemaLeftSequence::LeadLag,
            NemaLeftSequence::LeadLead,
            NemaLeftSequence::LeadLag,
        ];
        for (seed, l) in lefts.into_iter().enumerate() {
            let mut ctl = controller(l, &ALL);
            let served = run_checked(&mut ctl, seed as u64 + 1, 3600.0, |_| {});
            assert!(served[1..].iter().all(|s| *s), "seed {seed}: {served:?}");
            assert!(
                ctl.counters().crossings >= 40,
                "seed {seed}: {:?}",
                ctl.counters()
            );
        }
    }

    /// Accept (WP-17): no conflicting greens. Random calls on generated crossroads and
    /// tees, phases from `assign`; at every step the movements of the green phases hold
    /// no pair the junction's conflict matrix calls foes, the opposing lefts that Exhibit
    /// 5-3 runs together excepted.
    #[test]
    fn no_conflicting_greens() {
        use v2xw_world::model::RoadClass;
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
        for (k, world) in worlds.iter().enumerate() {
            let plan = &world.signals[0];
            let junction = world.roads.junction(plan.junction);
            let a = assign::assign_phases(world, plan).expect("maps");
            let row: Vec<Option<usize>> = plan
                .controlled
                .iter()
                .map(|l| junction.internal.iter().position(|x| x == l))
                .collect();
            for l in [NemaLeftSequence::LeadLead, NemaLeftSequence::LeadLag] {
                let mut ctl = controller(l, &a.phases());
                let mut steps = 0;
                run_checked(&mut ctl, 7 + k as u64, 1800.0, |c| {
                    steps += 1;
                    let g = c.greens();
                    let on: Vec<usize> = (0..a.movements.len())
                        .filter(|&m| {
                            a.movements[m].kind != MovementKind::Crossing
                                && g.contains(&a.movements[m].phase)
                        })
                        .collect();
                    for (i, &x) in on.iter().enumerate() {
                        for &y in &on[i + 1..] {
                            let (mx, my) = (a.movements[x], a.movements[y]);
                            let opposing_lefts = mx.phase != my.phase
                                && mx.permissive_with.is_some()
                                && my.permissive_with.is_some();
                            if let (Some(rx), Some(ry)) = (row[x], row[y]) {
                                assert!(
                                    opposing_lefts || !junction.conflicts.is_foe(rx, ry),
                                    "world {k} greens {g:?}: movements {x} ({mx:?}) and {y} ({my:?}) are foes"
                                );
                            }
                        }
                    }
                });
                assert!(steps > 0);
            }
        }
    }

    /// Accept (WP-17): an eight-phase example's timing reproduces under maximum recall.
    /// The STM 2nd ed. gives no numeric worked example (P17.1), so this one is built from
    /// its tables, as the plan says: a 35 mph main street and a 30 mph side street, yellow
    /// from Exhibit 6-2 (3.6 s and 3.2 s; lefts at 25 mph, 3.0 s), red clearance 1.5 s and
    /// 1.0 s (Exhibit 6-3's range), maximum greens inside Exhibit 6-9 (main through 50 s,
    /// main left 20 s, side through 30 s, side left 15 s). With every phase on maximum
    /// recall, every split is its maximum plus its yellow and red, lead-lead, so the cycle
    /// is (20+3.0+1.0)+(50+3.6+1.5) on the main street and (15+3.0+1.0)+(30+3.2+1.0) on
    /// the side street: 132.3 s.
    #[test]
    fn stm_example_timing() {
        let t = |max: f64, y: f64, r: f64| PhaseTiming {
            min_green_s: 4.0,
            max_green_s: max,
            passage_s: 2.5,
            yellow_s: y,
            red_clear_s: r,
        };
        let (ml, mt, sl, st) = (
            t(20.0, 3.0, 1.0),
            t(50.0, 3.6, 1.5),
            t(15.0, 3.0, 1.0),
            t(30.0, 3.2, 1.0),
        );
        let n = NemaSettings {
            soft_recall: Vec::new(),
            max_recall: ALL.to_vec(),
            ..NemaSettings::default()
        };
        let mut config = RingConfig::from_settings(&n, &ALL, mt, ml);
        for p in [3usize, 7] {
            config.timing[p] = sl;
        }
        for p in [4usize, 8] {
            config.timing[p] = st;
        }
        let mut ctl = DualRing::new(config);
        // Green start times of every phase, from a step-by-step run.
        let mut starts: BTreeMap<u8, Vec<f64>> = BTreeMap::new();
        let mut prev = ctl.greens();
        let mut time = 0.0;
        for _ in 0..(600.0 / DT) as usize {
            ctl.step(&Calls::default(), DT);
            time += DT;
            let g = ctl.greens();
            for r in 0..2 {
                if g[r] != 0 && g[r] != prev[r] {
                    starts.entry(g[r]).or_default().push(time);
                }
            }
            prev = g;
        }
        let want_cycle =
            (20.0 + 3.0 + 1.0) + (50.0 + 3.6 + 1.5) + (15.0 + 3.0 + 1.0) + (30.0 + 3.2 + 1.0);
        assert!((want_cycle - 132.3_f64).abs() < 1e-9);
        let split = |a: u8, b: u8| {
            let (sa, sb) = (&starts[&a], &starts[&b]);
            // The second cycle, past start-up in 2 and 6.
            let s = sa[1];
            let next = sb.iter().copied().find(|x| *x > s).expect("next phase");
            next - s
        };
        let near = |x: f64, y: f64| (x - y).abs() <= DT + 1e-6;
        for (p, q) in [(1u8, 2u8), (5, 6)] {
            assert!(near(split(p, q), 24.0), "phase {p} split {}", split(p, q));
        }
        for (p, q) in [(2u8, 3u8), (6, 7)] {
            assert!(near(split(p, q), 55.1), "phase {p} split {}", split(p, q));
        }
        for (p, q) in [(3u8, 4u8), (7, 8)] {
            assert!(near(split(p, q), 19.0), "phase {p} split {}", split(p, q));
        }
        for p in [1u8, 5] {
            let s = &starts[&p];
            let cycle = s[2] - s[1];
            assert!(near(cycle, want_cycle), "phase {p} cycle {cycle}");
        }
    }

    /// Accept (WP-17 P17.7): pedestrians are served. Phase 2 is on pedestrian recall with
    /// a 12 m crosswalk: every one of its greens walks for 7 s (MUTCD 2009 §4E.06 ¶11),
    /// then clears for 12 m / 1.067 m/s, and lasts at least walk + clearance + what of the
    /// 3 s buffer the yellow and red leave. Phase 6 has a crosswalk but no recall and no
    /// call, so it never walks. One press of phase 4's button, a single step, is latched
    /// and served at 4's next green.
    #[test]
    fn ped_recall_serves_walk() {
        let n = NemaSettings {
            soft_recall: Vec::new(),
            ped_recall: vec![2],
            ..NemaSettings::default()
        };
        let mut config = RingConfig::from_settings(&n, &ALL, THROUGH, LEFT);
        let clear = 12.0 / 1.067;
        for p in [2usize, 6, 4, 8] {
            config.ped[p] = Some((7.0, clear));
        }
        let mut ctl = DualRing::new(config);
        let walk_min =
            7.0 + clear + (PED_BUFFER_S - THROUGH.yellow_s - THROUGH.red_clear_s).max(0.0);
        let mut calls = Calls::default();
        calls.demand[4] = true;
        let mut t = 0.0;
        let (mut greens2, mut walks2, mut greens6) = (0, 0, 0);
        let (mut walk_s, mut clear_s, mut green2_s, mut green6_s) = (0.0, 0.0, 0.0, 0.0);
        let mut prev = ctl.greens();
        let mut four_walked = false;
        let mut pressed = false;
        while t < 900.0 {
            // Press 4's button once, during a main-street green.
            let press = !pressed && t > 300.0 && ctl.greens()[0] == 2;
            calls.ped[4] = press;
            pressed |= press;
            ctl.step(&calls, DT);
            t += DT;
            let g = ctl.greens();
            if g[0] == 2 {
                green2_s += DT;
                match ctl.crosswalk(2) {
                    v2xw_world::SignalState::Green => walk_s += DT,
                    v2xw_world::SignalState::Amber => clear_s += DT,
                    _ => {}
                }
            }
            if g[1] == 6 {
                green6_s += DT;
                assert_eq!(
                    ctl.crosswalk(6),
                    v2xw_world::SignalState::Red,
                    "6 walked uncalled"
                );
            }
            if pressed && g[0] == 4 && ctl.crosswalk(4) == v2xw_world::SignalState::Green {
                four_walked = true;
            }
            if prev[0] == 2 && g[0] != 2 {
                greens2 += 1;
                walks2 += usize::from(walk_s > 0.0);
                assert!(
                    (walk_s - 7.0).abs() <= DT + 1e-6 && (clear_s - clear).abs() <= DT + 1e-6,
                    "green {greens2} of 2: walk {walk_s} s, clearance {clear_s} s"
                );
                // Counted per step after it; the start-up green began before the first.
                assert!(
                    green2_s + DT + 1e-6 >= walk_min,
                    "2's green {green2_s} < {walk_min}"
                );
                walk_s = 0.0;
                clear_s = 0.0;
                green2_s = 0.0;
            }
            if prev[1] == 6 && g[1] != 6 {
                greens6 += 1;
                green6_s = 0.0;
            }
            prev = g;
        }
        assert!(
            greens2 >= 10 && walks2 == greens2,
            "{walks2} walks in {greens2} greens of 2"
        );
        assert!(greens6 >= 10, "{greens6}");
        assert!(four_walked, "4's single press was not served");
        let _ = green6_s;
    }

    /// A coordinated controller: a 100 s cycle at offset 13 s, splits 15/45/12/28 s on
    /// each ring (main street 60 s, side street 40 s), every phase called and extended
    /// (saturated traffic), coordinated phases 2 and 6.
    fn coordinated(force_off: crate::scenario::schema::ForceOffMode) -> DualRing {
        let n = NemaSettings {
            soft_recall: Vec::new(),
            coordination: true,
            force_off,
            ..NemaSettings::default()
        };
        let mut config = RingConfig::from_settings(&n, &ALL, THROUGH, LEFT);
        let mut split = [0.0; 9];
        for (p, s) in [(1, 15.0), (2, 45.0), (3, 12.0), (4, 28.0)] {
            split[p] = s;
            split[p + 4] = s;
        }
        config.coord = Some(Coordination {
            cycle_s: 100.0,
            offset_s: 13.0,
            phases: vec![2, 6],
            split_s: split,
            force_off,
            reference: crate::scenario::schema::NemaControllerType::Ts2,
        });
        DualRing::new(config)
    }

    /// Accept (WP-17 P17.8): coordinated phases hold their offset. Under saturated calls
    /// every non-coordinated phase would run to its maximum (45 s throughs, 25 s lefts);
    /// with coordination the coordinated green ends at the yield point (the offset plus
    /// its split less its yellow and red: 13 + 45 − 3.6 − 1.0 s mod 100 s) within one step,
    /// in every cycle, under both force-off modes. Floating force-offs hold each
    /// non-coordinated green to its split's green; fixed ones to its point in the cycle.
    #[test]
    fn coordinated_phases_hold_their_offset() {
        use crate::scenario::schema::ForceOffMode;
        for mode in [ForceOffMode::Floating, ForceOffMode::Fixed] {
            let mut ctl = coordinated(mode);
            let mut c = Calls::default();
            for p in 1..=8 {
                c.demand[p] = true;
                c.extend[p] = true;
            }
            let yield_local = 45.0 - THROUGH.yellow_s - THROUGH.red_clear_s;
            let mut prev = ctl.greens();
            let mut held4 = 0.0;
            let mut yields = 0;
            let mut t = 0.0;
            while t < 1000.0 {
                ctl.step(&c, DT);
                t += DT;
                let g = ctl.greens();
                if g[0] == 4 {
                    held4 += DT;
                }
                if prev[0] == 4 && g[0] != 4 {
                    assert!(
                        held4 <= 28.0 - THROUGH.yellow_s - THROUGH.red_clear_s + DT + 1e-6,
                        "{mode:?}: 4 ran {held4} s past its split"
                    );
                    held4 = 0.0;
                }
                if prev[0] == 2 && g[0] != 2 && t > 100.0 {
                    let local = (t - 13.0).rem_euclid(100.0);
                    assert!(
                        (local - yield_local).abs() <= DT + 1e-6,
                        "{mode:?}: 2 yielded at {local} s into the cycle, not {yield_local}"
                    );
                    yields += 1;
                }
                prev = g;
            }
            // One yield per cycle after the first: none skipped.
            assert!(yields >= 8, "{mode:?}: {yields} yields in 9 cycles");
            assert!(
                ctl.counters().force_offs > 0,
                "{mode:?}: {:?}",
                ctl.counters()
            );
        }
    }

    /// Accept (WP-17 P17.9): priority through the rings. A quiet 2 against a call on 4
    /// would end at its 7 s minimum; a request for 2 arriving at 15 s holds it to 15 s
    /// (green extension), and a request that never arrives holds it only to its maximum
    /// plus the extension cap (45 + 10 s). Busy 2 and 6 against a call on 4 would run to
    /// their 45 s maximum; a request for 4 ends them at the 7 s priority minimum (early
    /// green).
    #[test]
    fn priority_extends_and_truncates_through_the_rings() {
        let ends_at = |pc: Option<PriorityCall>, busy: bool| {
            let mut ctl = controller(NemaLeftSequence::LeadLead, &ALL);
            ctl.priority = pc;
            let mut c = Calls::default();
            c.demand[4] = true;
            c.extend[2] = busy;
            c.extend[6] = busy;
            let mut t = 0.0;
            while ctl.greens()[0] == 2 && t < 200.0 {
                ctl.step(&c, DT);
                t += DT;
            }
            t
        };
        let call = |phase, until_s| {
            Some(PriorityCall {
                phase,
                until_s,
                max_extension_s: 10.0,
                min_green_s: 7.0,
            })
        };
        assert!(near(ends_at(None, false), 7.0));
        let held = ends_at(call(2, 15.0), false);
        assert!(near(held, 15.0), "extension ended 2 at {held}");
        let capped = ends_at(call(2, 1.0e9), false);
        assert!(near(capped, 55.0), "the cap ended 2 at {capped}");
        assert!(near(ends_at(None, true), THROUGH.max_green_s));
        let cut = ends_at(call(4, 1.0e9), true);
        assert!(near(cut, 7.0), "early green ended 2 at {cut}");
    }
}
