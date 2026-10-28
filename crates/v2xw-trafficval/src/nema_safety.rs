//! `invariants/nema-safety` (WP-17 P17.10): Manhattan under the NEMA dual ring keeps its
//! defects visible.
//!
//! The study harness steps mobility only; signal control lives in the kernel
//! (`v2xw_engine::actuated::ActuatedControllers`, which runs the dual ring of
//! `v2xw_engine::nema::control`). The check builds those controllers from the scenario's
//! `traffic.signals` and steps them through [`crate::study::run_controlled`]'s hook, in the
//! kernel's order: the detectors and push buttons are read from the vehicles and
//! pedestrians as the previous step left them, the plans are rewritten, then the drivers
//! move (`v2xw_engine::run`, the "Actuated control reads its detectors" block).
//!
//! What is held, every step, over the rewritten plans:
//! * no two conflicting movements both showing a protected green (the junction's
//!   conflict matrix, as `v2xw_mobility::audit::audit_world` judges a fixed plan, but at
//!   every instant of a plan that changes);
//! * no auditor `red-entry` that is not a tagged violation (violations are off, so every
//!   red entry is a defect), and every other safety class as the city checks hold them.
//!
//! The fault skips the barrier: after each controller step, every movement of a NEMA
//! junction that shows red while a movement of the other barrier group shows a protected
//! green is turned green too (FHWA STM 2nd ed. §5.1.2: the rings cross a barrier only
//! together, which is what keeps the two groups' movements apart).
//!
//! Re-implemented from FHWA STM 2nd ed. and NEMA TS 2-2016; SUMO was read for behaviour
//! only.

use std::collections::{BTreeMap, BTreeSet};

use v2xw_core::ids::LaneId;
use v2xw_engine::actuated::ActuatedControllers;
use v2xw_engine::scenario::schema::{SignalControlKind, Traffic};
use v2xw_mobility::audit::{AuditParams, Check as Class};
use v2xw_mobility::rules::TrafficRules;
use v2xw_world::{SignalState, World};

use crate::invariants::{Overrides, audit_rows, load, world_of};
use crate::study;
use crate::{Check, Group, Lab, Outcome, Row, Variant};

/// The check.
pub fn checks() -> Vec<Check> {
    vec![Check {
        id: "invariants/nema-safety",
        group: Group::Invariants,
        title: "Under the NEMA dual ring no two conflicting movements are green together and no driver enters on red",
        procedure: "`scenarios/manhattan-5min.yaml` at 1,500 veh/h with 150 pedestrians and \
            `traffic.signals.control: nema`, 600 s (120 s in quick mode). The kernel's \
            signal controllers (`ActuatedControllers`, the dual ring at every junction \
            that maps to it) are stepped before each mobility step from the previous \
            step's detector calls and push-button pedestrians. Every step, every \
            NEMA-run plan's current states are checked pairwise against the junction's \
            conflict matrix (two protected greens on foe movements from different \
            approaches); the auditor counts red entries and the other safety classes.",
        faults: &[
            "barrier skipped: after each controller step every red movement of a NEMA junction is shown green while a movement of the other group is protected green",
        ],
        run: nema_safety,
    }]
}

/// The pairs `(row a, row b)` of each plan's movements that must never be protected
/// green together: foes in the junction's matrix, from different approaches, and not
/// two lanes of one road merging into one (a lane drop, which the engine zips) — the
/// rule of `v2xw_mobility::audit::audit_world`.
fn foe_pairs(world: &World) -> BTreeMap<usize, Vec<(usize, usize)>> {
    let mut approach_of: BTreeMap<LaneId, LaneId> = BTreeMap::new();
    let mut exit_of: BTreeMap<LaneId, LaneId> = BTreeMap::new();
    for c in world.roads.connections() {
        if let Some(via) = c.via {
            approach_of.entry(via).or_insert(c.from_lane);
            exit_of.entry(via).or_insert(c.to_lane);
        }
    }
    let mut out = BTreeMap::new();
    for (pi, plan) in world.signals.iter().enumerate() {
        let Some(j) = world.roads.try_junction(plan.junction) else {
            continue;
        };
        let row = |l: LaneId| j.internal.iter().position(|x| *x == l);
        let mut pairs = Vec::new();
        for a in 0..plan.controlled.len() {
            for b in a + 1..plan.controlled.len() {
                let (la, lb) = (plan.controlled[a], plan.controlled[b]);
                if approach_of.get(&la) == approach_of.get(&lb) {
                    continue;
                }
                let same_road = approach_of
                    .get(&la)
                    .zip(approach_of.get(&lb))
                    .is_some_and(|(x, y)| world.lane(*x).edge == world.lane(*y).edge);
                if same_road && exit_of.get(&la) == exit_of.get(&lb) {
                    continue;
                }
                if let (Some(ra), Some(rb)) = (row(la), row(lb))
                    && j.conflicts.is_foe(ra, rb)
                {
                    pairs.push((a, b));
                }
            }
        }
        out.insert(pi, pairs);
    }
    out
}

/// The fault: every red movement of plan `pi`'s current phase that is a foe of a
/// protected green turns green (the rings no longer wait for each other at the barrier).
fn skip_barrier(world: &mut World, pi: usize, pairs: &[(usize, usize)], now_s: f64) {
    let Some(plan) = world.signals.get_mut(pi) else {
        return;
    };
    let Some((i, _)) = plan.phase_at(now_s) else {
        return;
    };
    let states = &mut plan.phases[i].states;
    let mut turn: BTreeSet<usize> = BTreeSet::new();
    for &(a, b) in pairs {
        match (states.get(a), states.get(b)) {
            (Some(SignalState::Green), Some(SignalState::Red)) => {
                turn.insert(b);
            }
            (Some(SignalState::Red), Some(SignalState::Green)) => {
                turn.insert(a);
            }
            _ => {}
        }
    }
    for m in turn {
        states[m] = SignalState::Green;
    }
}

fn nema_safety(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let seconds = lab.cfg.secs(600, 120) as f64;
    let mut s = load(
        &lab.cfg.root.join("scenarios/manhattan-5min.yaml"),
        &Overrides {
            rate_veh_per_h: Some(1_500.0),
            seconds: Some(seconds),
            pedestrians: Some(150),
        },
    )?;
    let mut traffic = s.traffic.clone().unwrap_or_else(Traffic::default);
    traffic.signals.control = SignalControlKind::Nema;
    s.traffic = Some(traffic.clone());
    // The plans are rewritten as the run goes, so the study drives its own copy.
    let mut world: World = (*world_of(lab, &s)?).clone();
    let mut control = ActuatedControllers::new(&traffic.signals, &world);
    let nema: Vec<usize> = control.nema_plans().collect();
    let fallbacks = control.nema_fallbacks();
    let pairs = foe_pairs(&world);
    // The control: the same rule over every phase of the fixed-time plans the city was
    // imported with, which `audit_world` already holds at zero (cities/manhattan).
    let fixed_conflicts: usize = nema
        .iter()
        .filter_map(|pi| Some((world.signals.get(*pi)?, pairs.get(pi)?)))
        .map(|(plan, pp)| {
            pp.iter()
                .filter(|(a, b)| {
                    plan.phases.iter().any(|ph| {
                        ph.states.get(*a) == Some(&SignalState::Green)
                            && ph.states.get(*b) == Some(&SignalState::Green)
                    })
                })
                .count()
        })
        .sum();
    let rules = TrafficRules::of_highway_preset(s.world.highway_preset.map(|p| p.label()));
    let mobility = v2xw_engine::wiring::native_mobility(&s);
    let demand = v2xw_engine::wiring::build_demand(&s, &world).map_err(|e| e.to_string())?;
    let fault = variant == Variant::Fault(0);
    let mut conflicting_steps = 0u64;
    let mut conflicting_pairs: BTreeSet<(usize, usize, usize)> = BTreeSet::new();
    let mut examples: Vec<String> = Vec::new();
    let mut steps = 0u64;
    let mut hook = |w: &mut World,
                    now_s: f64,
                    dt_s: f64,
                    actors: &[v2xw_mobility::audit::AuditActor],
                    people: &[v2xw_mobility::audit::AuditPedestrian]| {
        let positions = || {
            actors
                .iter()
                .filter(|a| !a.class.is_vru())
                .map(|a| (a.lane, a.s_m))
        };
        let calls = control.calls(w, positions());
        let extend = control.extension_calls(w, positions());
        let peds =
            v2xw_engine::nema::control::PedestrianIndex::new(people.iter().map(|p| (p.pos, p.vel)));
        control.step_with_pedestrians(w, &extend, &calls, &peds, now_s, dt_s);
        steps += 1;
        let mut here = false;
        for &pi in &nema {
            let Some(pp) = pairs.get(&pi) else { continue };
            if fault {
                skip_barrier(w, pi, pp, now_s);
            }
            let Some(states) = w.signals.get(pi).and_then(|p| p.states_at(now_s)) else {
                continue;
            };
            for &(a, b) in pp {
                if states.get(a) == Some(&SignalState::Green)
                    && states.get(b) == Some(&SignalState::Green)
                {
                    here = true;
                    if conflicting_pairs.insert((pi, a, b)) && examples.len() < 6 {
                        let what = |k: usize| {
                            let via = w.signals[pi].controlled[k];
                            w.roads
                                .connections()
                                .iter()
                                .find(|c| c.via == Some(via))
                                .map_or(format!("{}", via.index()), |c| {
                                    format!(
                                        "{} ({:?}, lane {} edge {} -> lane {} edge {})",
                                        via.index(),
                                        c.direction,
                                        c.from_lane.index(),
                                        w.lane(c.from_lane).edge.index(),
                                        c.to_lane.index(),
                                        w.lane(c.to_lane).edge.index()
                                    )
                                })
                        };
                        examples.push(format!(
                            "t={now_s:.1}s plan {pi} (junction {}): {} and {} are foes and both protected green",
                            w.signals[pi].junction.index(),
                            what(a),
                            what(b)
                        ));
                    }
                }
            }
        }
        if here {
            conflicting_steps += 1;
        }
    };
    let r = study::run_controlled(
        &mut world,
        mobility,
        demand,
        s.seed,
        s.time.mobility_step(),
        s.time.duration_s,
        AuditParams {
            right_turn_on_red: rules.right_turn_on_red,
            ..AuditParams::default()
        },
        v2xw_metrics::ssm::SsmParams::default(),
        None,
        |_, _| {},
        &mut hook,
    )?;
    let counters = control.counters();
    let a = &r.audit;
    let (mut rows, mut notes) = audit_rows("manhattan under nema", a, seconds);
    rows.insert(
        0,
        Row::zero(
            "steps with two foe movements both protected green at a NEMA junction",
            conflicting_steps,
            "FHWA STM 2nd ed. §5.1.2 (Exhibit 5-3): the rings cross a barrier together, so no two conflicting phases time at once; NEMA TS 2-2016 conflict monitor",
        )
        .n(steps as usize),
    );
    rows.insert(
        1,
        Row::zero(
            "junction entries on red (untagged: violations are off)",
            a.count(Class::RedEntry),
            "UVC §11-202(c) / MUTCD §4D.04: steady red, do not enter",
        ),
    );
    rows.push(Row::held(
        "junctions run by the dual ring",
        nema.len() as f64,
        (1.0, f64::INFINITY),
        "the check must exercise the NEMA controller: a city left wholly to the single ring proves nothing about it",
    ));
    rows.push(Row::held(
        "signalised junction entries",
        a.stats.signalised_entries as f64,
        (100.0, f64::INFINITY),
        "the run must exercise the signals",
    ));
    rows.push(Row::reported(
        "control: foe pairs protected green together in a phase of the imported fixed-time plans",
        fixed_conflicts as f64,
        "the same rule on the plans before the dual ring rewrites them",
    ));
    rows.push(Row::reported(
        "dual-ring gap-outs",
        counters.gap_outs as f64,
        "—",
    ));
    rows.push(Row::reported(
        "dual-ring max-outs",
        counters.max_outs as f64,
        "—",
    ));
    rows.push(Row::reported(
        "network mean speed, m/s",
        a.stats.mean_speed_mps,
        "—",
    ));
    notes.push(format!(
        "{} plans under the dual ring; left to the single ring: {fallbacks:?}; {} distinct conflicting pairs",
        nema.len(),
        conflicting_pairs.len()
    ));
    notes.extend(examples);
    Ok(Outcome { rows, notes })
}
