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
//!   rest, dual entry and recall.
//!
//! The controller is not wired yet: the loader refuses `control: nema`
//! (`scenario::validate::nema_checks`) until P17.5, so nothing here runs in a scenario.

pub mod assign;

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
        }
    }
}

/// The detector calls a step sees, by NEMA phase (index 1-8; 0 unused).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Calls {
    /// A vehicle (or pedestrian) is waiting for the phase: a presence call.
    pub demand: [bool; 9],
    /// A vehicle is on the phase's extension detectors now.
    pub extend: [bool; 9],
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
    counters: RingCounters,
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
            c.demand[p] = calls.demand[p];
            c.extend[p] = calls.extend[p];
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
                s.held_s + 1e-9 >= t.min_green_s
                    && (s.maxed || s.gapped || s.gap_s + 1e-9 >= t.passage_s)
            }
            _ => false,
        }
    }

    fn end_green(&mut self, r: usize, next: u8) {
        let s = &mut self.state[r];
        if s.interval == Interval::Green {
            if s.maxed {
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
        let c = self.effective(calls);
        // Timers.
        for r in 0..2 {
            let s = self.state[r];
            let t = self.config.timing[usize::from(s.phase)];
            let mut n = s;
            n.held_s += dt_s;
            match s.interval {
                Interval::Green => {
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
                    // A gap with a conflicting call commits the phase to end: later
                    // detections do not re-extend it, and a maximum that runs out while it
                    // waits at the barrier does not make it a max-out.
                    if conflicting
                        && n.held_s + 1e-9 >= t.min_green_s
                        && n.gap_s + 1e-9 >= t.passage_s
                    {
                        n.gapped = true;
                    }
                    if !n.gapped && n.max_s + 1e-9 >= t.max_green_s {
                        n.maxed = true;
                    }
                }
                _ => {}
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
                let entry = [self.entry_phase(0, main, &c), self.entry_phase(1, main, &c)];
                for r in 0..2 {
                    self.state[r] = RingState::green(entry[r]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
        for p in 1..=8usize {
            // Throughs busier than lefts, main busier than side.
            rate[p] = match p {
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
}
