//! Vehicle-actuated signal control and green-wave coordination (`traffic.signals`).
//!
//! # How an actuated controller runs on a fixed-time plan
//!
//! A plan in the world is a clock: the phase showing at `t` is the phase at
//! `(t − offset) mod cycle` ([`SignalPlan::phase_at`]). Every consumer — the drivers, the
//! pedestrians, the SPaT encoder, the page's lamps — reads that clock, so a controller
//! that changes *when* a phase ends must do it by moving the clock, as the priority
//! service does (`crate::priority`). Then nothing can show a different signal from the
//! one the drivers obey: the SPaT is the signal by construction.
//!
//! The controller is the NEMA TS 2 single-ring basic actuated logic, phase by phase
//! (FHWA Traffic Signal Timing Manual, FHWA-HOP-08-024, 2008, chapter 5):
//!
//! * a green phase runs at least `min_green_s` (and never less than its own stated length
//!   when it carries a pedestrian walk, so the walk and its clearance are always served);
//! * **gap-out**: once past the minimum, if no vehicle has been on a detector of the
//!   movements this phase ends for `passage_s`, and an approach that is red now has a
//!   vehicle waiting, the phase ends at once — the clock jumps to its end, so the amber
//!   and all-red after it run in full;
//! * **extension**: at the phase's stated end, while detections keep coming within the
//!   passage time, the clock is held, up to `max_green_s` of green;
//! * **max-out**: at `max_green_s` the phase ends whatever the detectors see;
//! * **rest in green**: with no vehicle waiting on any red approach the phase holds, as
//!   a controller without recall does.
//!
//! A detector is the last `detector_m` of each approach lane (a stop-line presence loop):
//! a vehicle whose front is on it is a call. Amber and all-red phases are never moved.
//!
//! # Green wave
//!
//! `green_wave_speed_mps` sets every plan's offset to the time a vehicle at that speed
//! takes to reach the junction along `green_wave_heading_deg`, modulo the cycle: the
//! classic one-way progression (Roess, Prassas & McShane, *Traffic Engineering*, 4th ed.,
//! ch. 20, the time-space diagram). It is applied once, when the run is built.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use v2xw_core::ctx::{Record, Visibility};
use v2xw_core::ids::LaneId;
use v2xw_core::time::SimTime;
use v2xw_world::model::LaneKind;
use v2xw_world::{SignalPlan, SignalState, World};

use crate::scenario::schema::{SignalControl, SignalControlKind};

/// What one movement of a plan is, for the controller.
#[derive(Debug, Clone, Default)]
struct PlanInfo {
    /// The approach lane of each controlled movement (`None` for a crossing).
    approach: Vec<Option<LaneId>>,
    /// Which movements are pedestrian crossings.
    crossing: Vec<bool>,
    /// The phase the controller last saw, and how long it has held it.
    phase: usize,
    held_s: f64,
    /// When a vehicle was last on a detector of the movements the current phase ends.
    last_call_s: f64,
    /// Whether this phase's max-out was counted.
    maxed: bool,
}

/// What the actuated controllers did over the run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct ActuatedCounters {
    /// Greens ended early because the detectors went quiet.
    pub gap_outs: u64,
    /// Greens ended at their maximum with vehicles still arriving.
    pub max_outs: u64,
    /// Seconds of green added beyond the plans' stated lengths.
    pub extended_s: f64,
    /// Seconds of green cut from the plans' stated lengths.
    pub cut_s: f64,
}

/// What an actuated controller may still do to the plan's clock, for the SPaT
/// (`crate::infra`): its next change may come up to `cut_s` sooner (a gap-out once the
/// minimum green has run) or `extend_s` later (extension to the maximum green).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActuatedOutlook {
    /// Seconds sooner, at most.
    pub cut_s: f64,
    /// Seconds later, at most.
    pub extend_s: f64,
}

/// `traffic.control` — the traffic control's own log: actuated signal decisions and
/// gridlock resolution, cumulative (PUBLIC: what a traffic management centre logs).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrafficControlRecord {
    /// When.
    pub t: SimTime,
    /// Greens ended early by a gap.
    pub gap_outs: u64,
    /// Greens ended at their maximum.
    pub max_outs: u64,
    /// Seconds of green added.
    pub extended_s: f64,
    /// Seconds of green cut.
    pub cut_s: f64,
    /// Vehicles found standing for the whole gridlock timeout.
    pub jammed: u64,
    /// Of those, moved on along their route.
    pub teleported: u64,
    /// Of those, removed for want of room anywhere on the rest of their route.
    pub removed: u64,
}

impl Record for TrafficControlRecord {
    const CHANNEL: &'static str = "traffic.control";
    const VISIBILITY: Visibility = Visibility::Public;
}

/// Every junction's actuated controller.
#[derive(Debug, Clone, Default)]
pub struct ActuatedControllers {
    params: Option<SignalControl>,
    plans: BTreeMap<usize, PlanInfo>,
    counters: ActuatedCounters,
}

impl ActuatedControllers {
    /// Controllers for `control`, or inert ones for fixed-time control.
    pub fn new(control: &SignalControl, world: &World) -> Self {
        if control.control != SignalControlKind::Actuated {
            return Self::default();
        }
        let mut approach_of: BTreeMap<LaneId, LaneId> = BTreeMap::new();
        for c in world.roads.connections() {
            if let Some(via) = c.via {
                approach_of.entry(via).or_insert(c.from_lane);
            }
        }
        let mut plans = BTreeMap::new();
        for (i, plan) in world.signals.iter().enumerate() {
            let crossing: Vec<bool> = plan
                .controlled
                .iter()
                .map(|l| {
                    world
                        .roads
                        .try_lane(*l)
                        .is_some_and(|x| x.kind == LaneKind::Crossing)
                })
                .collect();
            let approach = plan
                .controlled
                .iter()
                .zip(&crossing)
                .map(|(l, c)| if *c { None } else { approach_of.get(l).copied() })
                .collect();
            plans.insert(
                i,
                PlanInfo {
                    approach,
                    crossing,
                    ..PlanInfo::default()
                },
            );
        }
        Self {
            params: Some(control.clone()),
            plans,
            counters: ActuatedCounters::default(),
        }
    }

    /// The window the SPaT of plan `idx` states at `now_s` ([`ActuatedOutlook`]), `None`
    /// under fixed-time control. Outside a vehicle green (an amber, an all-red) nothing
    /// moves the clock, so the window is zero both ways until the next green starts.
    pub fn outlook(&self, world: &World, idx: usize, now_s: f64) -> Option<ActuatedOutlook> {
        let p = self.params.as_ref()?;
        let st = self.plans.get(&idx)?;
        let plan = world.signals.get(idx)?;
        let (i, into) = plan.phase_at(now_s)?;
        if !vehicle_green(plan, i, &st.crossing) {
            return Some(ActuatedOutlook {
                cut_s: 0.0,
                extend_s: 0.0,
            });
        }
        let held = if st.phase == i { st.held_s } else { 0.0 };
        let elapsed = into + held;
        let duration = plan.phases[i].duration_s;
        let left = (duration - into).max(0.0);
        let walks = (0..plan.controlled.len()).any(|m| {
            st.crossing[m]
                && matches!(plan.phases[i].states.get(m), Some(SignalState::Green))
        });
        let min = if walks {
            p.min_green_s.max(duration)
        } else {
            p.min_green_s
        };
        Some(ActuatedOutlook {
            cut_s: (left - (min - elapsed).max(0.0)).clamp(0.0, left),
            extend_s: (p.max_green_s - elapsed - left).max(0.0),
        })
    }

    /// Whether any controller is actuated.
    pub fn is_active(&self) -> bool {
        self.params.is_some()
    }

    /// What the controllers have done so far.
    pub fn counters(&self) -> ActuatedCounters {
        self.counters
    }

    /// The detector calls: every approach lane with a vehicle front in its last
    /// `detector_m`, from `(lane, s)` positions.
    pub fn calls(
        &self,
        world: &World,
        positions: impl Iterator<Item = (LaneId, f64)>,
    ) -> BTreeSet<LaneId> {
        let Some(p) = &self.params else {
            return BTreeSet::new();
        };
        positions
            .filter(|(l, s)| {
                world
                    .roads
                    .try_lane(*l)
                    .is_some_and(|lane| lane.length_m - s <= p.detector_m)
            })
            .map(|(l, _)| l)
            .collect()
    }

    /// Advances every controller to `now_s` by `dt_s`, moving the plans' offsets in
    /// `world`. `calls` is [`Self::calls`] at the start of the step.
    pub fn step(&mut self, world: &mut World, calls: &BTreeSet<LaneId>, now_s: f64, dt_s: f64) {
        let Some(p) = self.params.clone() else {
            return;
        };
        for (&idx, st) in &mut self.plans {
            let Some(plan) = world.signals.get(idx) else {
                continue;
            };
            let Some((i, into)) = plan.phase_at(now_s) else {
                continue;
            };
            if i != st.phase {
                st.phase = i;
                st.held_s = 0.0;
                st.last_call_s = now_s;
                st.maxed = false;
            }
            if !vehicle_green(plan, i, &st.crossing) {
                continue;
            }
            let n = plan.phases.len();
            let next = (i + 1) % n;
            let green = |k: usize, m: usize| {
                matches!(
                    plan.phases[k].states.get(m),
                    Some(SignalState::Green | SignalState::GreenYield)
                )
            };
            // The movements this phase ends: green now, not green next.
            let ending: Vec<usize> = (0..plan.controlled.len())
                .filter(|m| !st.crossing[*m] && green(i, *m) && !green(next, *m))
                .collect();
            if ending.is_empty() {
                continue; // an overlap: the green continues into the next phase
            }
            let called = |m: &usize| st.approach[*m].is_some_and(|l| calls.contains(&l));
            if ending.iter().any(called) {
                st.last_call_s = now_s;
            }
            let waiting = (0..plan.controlled.len())
                .filter(|m| !st.crossing[*m] && !green(i, *m))
                .any(|m| called(&m));
            let duration = plan.phases[i].duration_s;
            let elapsed = into + st.held_s;
            let walks = (0..plan.controlled.len()).any(|m| st.crossing[m] && green(i, m));
            let min = if walks {
                p.min_green_s.max(duration)
            } else {
                p.min_green_s
            };
            let gapped = now_s - st.last_call_s >= p.passage_s;
            let left = duration - into;
            let cycle = plan.cycle_s;
            let mut shift = 0.0;
            if elapsed >= min && gapped && waiting && left > dt_s {
                // Gap-out: end the green now.
                shift = -left;
                self.counters.gap_outs += 1;
                self.counters.cut_s += left;
            } else if left <= dt_s + 1e-9 {
                if !waiting {
                    shift = dt_s; // rest in green
                } else if !gapped && elapsed < p.max_green_s {
                    shift = dt_s; // extension
                    self.counters.extended_s += dt_s;
                } else if !gapped && !st.maxed {
                    st.maxed = true;
                    self.counters.max_outs += 1;
                }
            }
            if shift != 0.0 {
                if shift > 0.0 {
                    st.held_s += shift;
                }
                let plan = &mut world.signals[idx];
                plan.offset_s = (plan.offset_s + shift).rem_euclid(cycle);
            }
        }
    }
}

/// Whether phase `i` gives some vehicle movement a green.
fn vehicle_green(plan: &SignalPlan, i: usize, crossing: &[bool]) -> bool {
    plan.phases[i].states.iter().enumerate().any(|(m, s)| {
        !crossing.get(m).copied().unwrap_or(false)
            && matches!(s, SignalState::Green | SignalState::GreenYield)
    })
}

/// Sets every plan's offset for a green wave at `speed_mps` along `heading_deg`
/// (clockwise from north), measured from the junction furthest upstream. A no-op without
/// a speed.
pub fn coordinate(control: &SignalControl, world: &mut World) {
    let Some(v) = control.green_wave_speed_mps.filter(|v| *v > 0.0) else {
        return;
    };
    let h = control.green_wave_heading_deg.to_radians();
    // World frame: x east, y north; a compass heading h points along (sin h, cos h).
    let (ux, uy) = (v2xw_core::math::sin(h), v2xw_core::math::cos(h));
    let along: Vec<f64> = world
        .signals
        .iter()
        .map(|p| {
            let c = world.roads.junction(p.junction).position;
            c.x * ux + c.y * uy
        })
        .collect();
    let start = along.iter().copied().fold(f64::INFINITY, f64::min);
    for (plan, d) in world.signals.iter_mut().zip(along) {
        if plan.cycle_s > 0.0 {
            plan.offset_s = ((d - start) / v).rem_euclid(plan.cycle_s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_world::{ImportOptions, procedural::GridParams};

    fn grid() -> World {
        v2xw_world::procedural::grid(
            &GridParams::legacy().with_size(3, 3).with_signals(true),
            &ImportOptions::default(),
        )
        .expect("grid")
    }

    /// The approach lanes of the movements green in `plan`'s phase 0 and of those red in it.
    fn approaches(world: &World, ctl: &ActuatedControllers) -> (Vec<LaneId>, Vec<LaneId>) {
        let plan = &world.signals[0];
        let st = &ctl.plans[&0];
        let mut green = Vec::new();
        let mut red = Vec::new();
        for (m, s) in plan.phases[0].states.iter().enumerate() {
            if let Some(l) = st.approach[m] {
                if matches!(s, SignalState::Green | SignalState::GreenYield) {
                    green.push(l);
                } else {
                    red.push(l);
                }
            }
        }
        (green, red)
    }

    fn actuated() -> SignalControl {
        SignalControl {
            control: SignalControlKind::Actuated,
            ..SignalControl::default()
        }
    }

    #[test]
    fn a_quiet_green_gaps_out_when_a_red_approach_waits() {
        let mut world = grid();
        let mut ctl = ActuatedControllers::new(&actuated(), &world);
        let (_, red) = approaches(&world, &ctl);
        let waiting: BTreeSet<LaneId> = red.into_iter().take(1).collect();
        let stated = world.signals[0].phases[0].duration_s;
        let mut t = world.signals[0].offset_s;
        let mut green_s = 0.0;
        while world.signals[0].phase_at(t).map(|(i, _)| i) == Some(0) && green_s < 200.0 {
            ctl.step(&mut world, &waiting, t, 0.1);
            t += 0.1;
            green_s += 0.1;
        }
        assert!(ctl.counters().gap_outs >= 1, "{:?}", ctl.counters());
        let min = actuated().min_green_s;
        assert!(
            green_s < stated - 1.0 && green_s >= min - 0.2,
            "a quiet green ran {green_s} s of its {stated} s (minimum {min} s)"
        );
    }

    #[test]
    fn a_busy_green_is_extended_up_to_its_maximum() {
        let mut world = grid();
        let control = actuated();
        let mut ctl = ActuatedControllers::new(&control, &world);
        let (green, red) = approaches(&world, &ctl);
        let calls: BTreeSet<LaneId> = green.into_iter().chain(red).collect();
        let stated = world.signals[0].phases[0].duration_s;
        let start = world.signals[0].offset_s;
        let mut t = start;
        let mut green_s = 0.0;
        while world.signals[0].phase_at(t).map(|(i, _)| i) == Some(0) && green_s < 200.0 {
            ctl.step(&mut world, &calls, t, 0.1);
            t += 0.1;
            green_s += 0.1;
        }
        assert!(
            green_s > stated + 1.0,
            "a called green of {stated} s was not extended ({green_s} s)"
        );
        assert!(
            green_s <= control.max_green_s + 0.5,
            "the green ran {green_s} s, past its {} s maximum",
            control.max_green_s
        );
        assert_eq!(ctl.counters().max_outs, 1);
    }

    #[test]
    fn the_spat_window_spans_the_gap_out_and_the_maximum() {
        let world = grid();
        let control = actuated();
        let ctl = ActuatedControllers::new(&control, &world);
        let plan = &world.signals[0];
        let stated = plan.phases[0].duration_s;
        let at = plan.offset_s + 1.0;
        let o = ctl.outlook(&world, 0, at).expect("actuated");
        let left = stated - 1.0;
        assert!((o.cut_s - (left - (control.min_green_s - 1.0))).abs() < 1e-9, "{o:?}");
        assert!((o.extend_s - (control.max_green_s - stated)).abs() < 1e-9, "{o:?}");
        let fixed = ActuatedControllers::new(&SignalControl::default(), &world);
        assert!(fixed.outlook(&world, 0, at).is_none());
    }

    #[test]
    fn fixed_time_control_moves_nothing() {
        let mut world = grid();
        let before: Vec<f64> = world.signals.iter().map(|p| p.offset_s).collect();
        let mut ctl = ActuatedControllers::new(&SignalControl::default(), &world);
        let calls: BTreeSet<LaneId> = world.roads.lanes().iter().map(|l| l.id).collect();
        for k in 0..600 {
            ctl.step(&mut world, &calls, f64::from(k) * 0.1, 0.1);
        }
        let after: Vec<f64> = world.signals.iter().map(|p| p.offset_s).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn a_green_wave_offsets_by_travel_time() {
        let mut world = grid();
        let control = SignalControl {
            green_wave_speed_mps: Some(10.0),
            green_wave_heading_deg: 90.0,
            ..SignalControl::default()
        };
        coordinate(&control, &mut world);
        // Two signals on one east-west row: their offset difference is the travel time
        // between them, modulo the cycle.
        let pos = |i: usize| world.roads.junction(world.signals[i].junction).position;
        let (a, b) = (0usize, 1usize);
        let dx = pos(b).x - pos(a).x;
        let cycle = world.signals[a].cycle_s;
        let want = (dx / 10.0).rem_euclid(cycle);
        let got = (world.signals[b].offset_s - world.signals[a].offset_s).rem_euclid(cycle);
        assert!(
            (want - got).abs() < 1e-6 || (cycle - (want - got).abs()).abs() < 1e-6,
            "offset difference {got} s, travel time {want} s"
        );
    }
}
