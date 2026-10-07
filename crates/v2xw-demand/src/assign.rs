//! `demand/assign/iterative-due` — iterative dynamic traffic assignment.
//!
//! The loop of SUMO's `duaIterate` [SUMO Dynamic User Assignment documentation; Gawron
//! 1998], with a measured convergence criterion instead of a fixed iteration count:
//!
//! 1. **Initial routes.** Every trip takes its free-flow time-dependent shortest path (or
//!    the routes of a saved assignment, to continue one).
//! 2. **Load.** A [`crate::loader::NetworkLoader`] drives the chosen routes and measures
//!    lane travel times per interval.
//! 3. **Measure the gap.** For every trip, the cost of the route it drove and of the best
//!    route under the new times, both priced lane by lane at the instant of entry. The
//!    relative gap `Σ (c_used − c_best) / Σ c_used` is the standard DTA convergence
//!    measure [Y.-C. Chiu et al., *Dynamic Traffic Assignment: A Primer*, TRB
//!    Transportation Research Circular E-C153, 2011, §4]; at a user equilibrium nobody
//!    can save time by switching and the gap is zero.
//! 4. **Stop** when the gap is below the tolerance (after `min_iterations`), or at
//!    `max_iterations`. The routes just loaded are the result.
//! 5. **Choose.** The best route joins the trip's alternatives (deduplicated by the roads
//!    it drives, at most `max_alternatives`, the least probable dropped as SUMO does); the
//!    [`crate::choice::RouteChoice`] model updates costs and probabilities; each trip
//!    draws its next route from its probabilities. Go to 2.
//!
//! # Determinism
//!
//! Every draw comes from a stream keyed by `(seed, trip id, iteration)`
//! ([`RngStream::derive`] with this model's plug-in domain), so a trip's choices do not
//! depend on how many other trips there are or the order they are processed in; searches
//! break ties by lane id; sums are id-ordered. Two runs with the same inputs write
//! byte-identical route files on any machine.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::math;
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_world::{ClassMask, World};

use crate::choice::{Alternative, ChoiceSpec, ChoiceStep};
use crate::costs::{FreeFlow, TimeCost, TravelTimes, path_cost_s};
use crate::error::{DemandError, Result};
use crate::graph::{LaneGraph, Node};
use crate::loader::{LoaderSpec, Window};
use crate::routefile::{RouteFile, RoutedTrip, SavedAlternative};
use crate::tdsp::Search;
use crate::trips::{Trip, world_hash};

/// The model id.
pub const MODEL_ID: &str = "demand/assign/iterative-due";

/// The assignment's parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AssignParams {
    /// The route-choice model.
    pub choice: ChoiceSpec,
    /// The network loader.
    pub loader: LoaderSpec,
    /// At most this many iterations (loadings).
    pub max_iterations: u32,
    /// At least this many, whatever the gap.
    pub min_iterations: u32,
    /// Stop when the relative gap falls below this.
    pub gap_tolerance: f64,
    /// The most routes a driver remembers (SUMO `--max-alternatives`, default 5).
    pub max_alternatives: usize,
    /// The travel-time aggregation interval, seconds.
    pub interval_s: f64,
    /// Trips departing within this many seconds of each other from the same lane share
    /// one shortest-path search (the route's cost is still priced at its own departure).
    pub search_quantum_s: f64,
    /// How long after the last departure the loading keeps measuring, seconds.
    pub horizon_s: f64,
    /// Weight of the new loading against the previous one (SUMO `--weight-memory`
    /// off = 1).
    pub weight_memory: f64,
    /// The master seed of the route draws.
    pub seed: u64,
}

impl Default for AssignParams {
    fn default() -> Self {
        Self {
            choice: ChoiceSpec::default(),
            loader: LoaderSpec::default(),
            max_iterations: 25,
            min_iterations: 3,
            gap_tolerance: 0.02,
            max_alternatives: 5,
            interval_s: 300.0,
            search_quantum_s: 60.0,
            horizon_s: 3600.0,
            weight_memory: 1.0,
            seed: 0x00D7_A551,
        }
    }
}

/// One iteration's record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IterationLog {
    /// The iteration, from 1.
    pub iteration: u32,
    /// The relative gap of the routes loaded in this iteration.
    pub relative_gap: f64,
    /// Mean experienced travel time of the trips that arrived, seconds.
    pub mean_travel_time_s: f64,
    /// Trips that arrived within the loading window.
    pub arrived: usize,
    /// Share of trips that drew a different route for the next iteration.
    pub switched_share: f64,
    /// Mean number of alternatives per trip.
    pub mean_alternatives: f64,
}

/// One trip's state in the assignment.
#[derive(Debug, Clone)]
struct State {
    trip: usize,
    origin: Node,
    destination: Node,
    alts: Vec<Alternative>,
    chosen: usize,
}

/// The result of an assignment.
#[derive(Debug, Clone)]
pub struct Assignment {
    /// The routed trips, with their alternatives, in trip order.
    pub routes: RouteFile,
    /// Trips that could not be routed at all (origin or destination not drivable for the
    /// class, or no path).
    pub unroutable: Vec<u64>,
    /// True if the gap fell below the tolerance.
    pub converged: bool,
}

/// The iterative assignment.
pub struct Assigner<'w> {
    world: &'w World,
    params: AssignParams,
    graph: LaneGraph,
    card: ModelCard,
}

impl<'w> Assigner<'w> {
    /// An assignment of car-class trips over `world`.
    pub fn new(world: &'w World, params: AssignParams) -> Self {
        Self::for_classes(world, params, ClassMask::CAR)
    }

    /// An assignment over the lanes `classes` may use.
    pub fn for_classes(world: &'w World, params: AssignParams, classes: ClassMask) -> Self {
        let graph = LaneGraph::new(world, classes);
        let card = card(&params);
        Self {
            world,
            params,
            graph,
            card,
        }
    }

    /// The card.
    pub fn card(&self) -> &ModelCard {
        &self.card
    }

    /// The graph.
    pub fn graph(&self) -> &LaneGraph {
        &self.graph
    }

    /// Runs the assignment on `trips`; `warm` continues a saved one (its alternatives and
    /// choices are the starting point for the trips it contains).
    ///
    /// `progress` is called after every iteration with its log line.
    ///
    /// # Errors
    /// A loader that fails.
    pub fn run(
        &self,
        trips: &[Trip],
        warm: Option<&RouteFile>,
        mut progress: impl FnMut(&IterationLog),
    ) -> Result<Assignment> {
        let p = &self.params;
        let g = &self.graph;
        let mut unroutable = Vec::new();
        let mut states: Vec<State> = Vec::new();
        for (i, t) in trips.iter().enumerate() {
            match (g.node(t.origin), g.node(t.destination)) {
                (Some(o), Some(d)) => states.push(State {
                    trip: i,
                    origin: o,
                    destination: d,
                    alts: Vec::new(),
                    chosen: 0,
                }),
                _ => unroutable.push(t.id),
            }
        }
        // Warm start.
        if let Some(w) = warm {
            let by_id: BTreeMap<u64, &RoutedTrip> = w.trips.iter().map(|r| (r.trip.id, r)).collect();
            for s in &mut states {
                let Some(r) = by_id.get(&trips[s.trip].id) else {
                    continue;
                };
                for a in &r.alternatives {
                    if let Some(path) = g.nodes_of(&a.lanes)
                        && path.first() == Some(&s.origin)
                        && path.last() == Some(&s.destination)
                    {
                        s.alts.push(Alternative {
                            signature: g.signature(&path),
                            length_m: g.path_length_m(&path),
                            path,
                            cost_s: a.cost_s,
                            probability: a.probability,
                        });
                    }
                }
                crate::choice::normalise(&mut s.alts);
                s.chosen = r.chosen.min(s.alts.len().saturating_sub(1));
            }
        }
        // Initial free-flow routes for everyone without one.
        let free = FreeFlow(g);
        let cold: Vec<usize> = (0..states.len()).filter(|i| states[*i].alts.is_empty()).collect();
        let best = self.best_routes(trips, &states, &cold, &free);
        for (k, i) in cold.iter().enumerate() {
            if let Some((path, cost)) = best[k].clone() {
                states[*i].alts.push(Alternative {
                    signature: g.signature(&path),
                    length_m: g.path_length_m(&path),
                    path,
                    cost_s: cost,
                    probability: 1.0,
                });
                states[*i].chosen = 0;
            }
        }
        let (dead, live): (Vec<State>, Vec<State>) =
            states.into_iter().partition(|s| s.alts.is_empty());
        unroutable.extend(dead.iter().map(|s| trips[s.trip].id));
        let mut states = live;
        let all: Vec<usize> = (0..states.len()).collect();

        let begin_s = trips
            .iter()
            .map(|t| t.depart_s)
            .fold(f64::INFINITY, f64::min)
            .min(0.0)
            .max(f64::MIN);
        let last = trips.iter().map(|t| t.depart_s).fold(0.0, f64::max);
        let window = Window {
            begin_s: if begin_s.is_finite() { begin_s } else { 0.0 },
            end_s: last + p.horizon_s,
            bin_s: p.interval_s,
        };
        let mut loader = p.loader.build();
        let choice = p.choice.build();
        let mut log: Vec<IterationLog> = Vec::new();
        let mut previous: Option<TravelTimes> = None;
        let mut converged = false;
        let max_it = p.max_iterations.max(1);
        for k in 1..=max_it {
            let sub_trips: Vec<Trip> = states.iter().map(|s| trips[s.trip].clone()).collect();
            let routes: Vec<Vec<Node>> = states
                .iter()
                .map(|s| s.alts[s.chosen].path.clone())
                .collect();
            let loading = loader.load(self.world, g, &sub_trips, &routes, window)?;
            let mut times = loading.times;
            if let Some(prev) = previous.as_ref() {
                times.blend(prev, p.weight_memory);
            }
            let best = self.best_routes(trips, &states, &all, &times);
            let mut used_sum: Vec<(u64, f64)> = Vec::with_capacity(states.len());
            let mut gap_sum: Vec<(u64, f64)> = Vec::with_capacity(states.len());
            let mut measured_all: Vec<Vec<f64>> = Vec::with_capacity(states.len());
            for (s, b) in states.iter().zip(&best) {
                let depart = trips[s.trip].depart_s;
                let measured: Vec<f64> = s
                    .alts
                    .iter()
                    .map(|a| path_cost_s(&times, &a.path, depart))
                    .collect();
                let used = measured[s.chosen];
                let best_cost = b.as_ref().map_or(used, |(path, _)| path_cost_s(&times, path, depart));
                let id = trips[s.trip].id;
                used_sum.push((id, used));
                gap_sum.push((id, (used - best_cost).max(0.0)));
                measured_all.push(measured);
            }
            let den = math::sum_sorted_by_key(used_sum);
            let gap = if den > 0.0 {
                math::sum_sorted_by_key(gap_sum) / den
            } else {
                0.0
            };
            let arrived_tt: Vec<(u64, f64)> = loading
                .experienced_s
                .iter()
                .zip(&states)
                .filter(|(e, _)| e.is_finite())
                .map(|(e, s)| (trips[s.trip].id, *e))
                .collect();
            let mean_tt = if arrived_tt.is_empty() {
                0.0
            } else {
                let n = arrived_tt.len() as f64;
                math::sum_sorted_by_key(arrived_tt) / n
            };
            let done = k >= p.min_iterations.max(1) && gap <= p.gap_tolerance;
            let mut switched = 0usize;
            if !done && k < max_it {
                for ((s, b), measured) in states.iter_mut().zip(best).zip(measured_all) {
                    let mut measured = measured;
                    let depart = trips[s.trip].depart_s;
                    let (best_idx, is_new) = match b {
                        Some((path, _)) => {
                            let sig = g.signature(&path);
                            match s.alts.iter().position(|a| a.signature == sig) {
                                Some(i) => (i, false),
                                None => {
                                    let cost = path_cost_s(&times, &path, depart);
                                    s.alts.push(Alternative {
                                        signature: sig,
                                        length_m: g.path_length_m(&path),
                                        path,
                                        cost_s: cost,
                                        probability: 0.0,
                                    });
                                    measured.push(cost);
                                    (s.alts.len() - 1, true)
                                }
                            }
                        }
                        None => (s.chosen, false),
                    };
                    choice.update(
                        &mut s.alts,
                        &measured,
                        ChoiceStep {
                            iteration: k + 1,
                            best: best_idx,
                            best_is_new: is_new,
                        },
                    );
                    let best_idx = prune(&mut s.alts, best_idx, p.max_alternatives.max(1));
                    let _ = best_idx;
                    let id = trips[s.trip].id;
                    let mut rng = RngStream::derive(
                        p.seed,
                        RngDomain::plugin(MODEL_ID),
                        EntityRef::custom(MODEL_ID, id ^ (u64::from(k) << 48)),
                    );
                    let weights: Vec<f64> = s.alts.iter().map(|a| a.probability).collect();
                    let before = s.alts.get(s.chosen).map(|a| a.signature.clone());
                    s.chosen = rng.choose_index(&weights).min(s.alts.len() - 1);
                    if before.as_ref() != Some(&s.alts[s.chosen].signature) {
                        switched += 1;
                    }
                }
            }
            let mean_alts = if states.is_empty() {
                0.0
            } else {
                states.iter().map(|s| s.alts.len()).sum::<usize>() as f64 / states.len() as f64
            };
            let line = IterationLog {
                iteration: k,
                relative_gap: gap,
                mean_travel_time_s: mean_tt,
                arrived: loading.arrived,
                switched_share: if states.is_empty() {
                    0.0
                } else {
                    switched as f64 / states.len() as f64
                },
                mean_alternatives: mean_alts,
            };
            progress(&line);
            log.push(line);
            previous = Some(times);
            if done {
                converged = true;
                break;
            }
        }
        let routed: Vec<RoutedTrip> = states
            .iter()
            .map(|s| RoutedTrip {
                trip: trips[s.trip].clone(),
                chosen: s.chosen,
                alternatives: s
                    .alts
                    .iter()
                    .map(|a| SavedAlternative {
                        lanes: g.lanes_of(&a.path),
                        cost_s: math::quantize(a.cost_s, 3),
                        probability: math::quantize(a.probability, 6),
                    })
                    .collect(),
            })
            .collect();
        let routes = RouteFile::new(
            world_hash(self.world),
            serde_json::json!({
                "model": MODEL_ID,
                "params": p,
                "choice": choice.card().id_at_version(),
                "loader": loader.card().id_at_version(),
            }),
            log,
            converged,
            routed,
        );
        Ok(Assignment {
            routes,
            unroutable,
            converged,
        })
    }

    /// The best route for each of `which` (indices into `states`) under `costs`.
    fn best_routes(
        &self,
        trips: &[Trip],
        states: &[State],
        which: &[usize],
        costs: &dyn TimeCost,
    ) -> Vec<Option<(Vec<Node>, f64)>> {
        let g = &self.graph;
        let q = self.params.search_quantum_s.max(1e-3);
        // Group by (origin, departure quantum); deterministic BTreeMap order.
        let mut groups: BTreeMap<(Node, i64), Vec<usize>> = BTreeMap::new();
        for (k, i) in which.iter().enumerate() {
            let s = &states[*i];
            let quantum = (trips[s.trip].depart_s / q).floor() as i64;
            groups.entry((s.origin, quantum)).or_default().push(k);
        }
        let mut out: Vec<Option<(Vec<Node>, f64)>> = vec![None; which.len()];
        let mut search = Search::new(g);
        for ((origin, quantum), members) in groups {
            let targets: Vec<Node> = members.iter().map(|k| states[which[*k]].destination).collect();
            search.run(g, costs, origin, quantum as f64 * q, &targets);
            for k in members {
                let d = states[which[k]].destination;
                out[k] = search.path_to(d).zip(search.cost_to(d));
            }
        }
        out
    }
}

/// Drops the least probable alternatives (never `keep`) until at most `max` remain,
/// renormalises, and returns `keep`'s new index.
fn prune(alts: &mut Vec<Alternative>, keep: usize, max: usize) -> usize {
    let mut keep = keep;
    while alts.len() > max {
        let victim = (0..alts.len())
            .filter(|i| *i != keep)
            .min_by(|a, b| {
                alts[*a]
                    .probability
                    .total_cmp(&alts[*b].probability)
                    .then(alts[*b].cost_s.total_cmp(&alts[*a].cost_s))
                    .then(b.cmp(a))
            })
            .expect("more than one alternative");
        alts.remove(victim);
        if victim < keep {
            keep -= 1;
        }
    }
    crate::choice::normalise(alts);
    keep
}

fn card(p: &AssignParams) -> ModelCard {
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "Iterative dynamic traffic assignment: load the network, measure time-dependent \
         travel times, update route choice, until the relative gap is below tolerance.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![Equation::new(
        "relative gap",
        "RG = sum_i (c_i(used) - c_i(best)) / sum_i c_i(used)",
    )];
    let sumo = Source::new(
        SourceKind::Code,
        "SUMO duaIterate.py and duarouter (sumo.dlr.de/docs/Demand/Dynamic_User_Assignment.html): \
         max-alternatives 5, aggregation interval, weight memory",
    );
    let primer = Source::new(
        SourceKind::Paper,
        "Y.-C. Chiu, J. Bottom, M. Mahut, A. Paz, R. Balakrishna, T. Waller, J. Hicks, \
         Dynamic Traffic Assignment: A Primer, Transportation Research Circular E-C153, TRB \
         2011 (relative gap as the DTA convergence measure)",
    );
    let mut tol = Parameter::new(
        "gap_tolerance",
        "-",
        serde_json::json!(p.gap_tolerance),
        Source::todo_calibrate("stopping tolerance"),
    );
    tol.calibration = Some(
        "the primer reports practice between 1% and 5%; tighten until the measured outputs \
         of interest stop changing"
            .to_string(),
    );
    card.parameters = vec![
        Parameter::new(
            "max_alternatives",
            "-",
            serde_json::json!(p.max_alternatives),
            sumo.clone(),
        ),
        Parameter::new(
            "interval_s",
            "s",
            serde_json::json!(p.interval_s),
            sumo.clone(),
        ),
        Parameter::new(
            "max_iterations",
            "-",
            serde_json::json!(p.max_iterations),
            sumo.clone(),
        ),
        tol,
        Parameter::new(
            "weight_memory",
            "-",
            serde_json::json!(p.weight_memory),
            sumo.clone(),
        ),
    ];
    card.assumptions = vec![
        "Route costs are priced lane by lane at the instant of entry from interval-mean \
         travel times (FIFO only approximately)."
            .to_string(),
        "Trips from one lane within one search quantum share a search; each route is still \
         priced at its own departure."
            .to_string(),
    ];
    card.sources = vec![
        Source::new(
            SourceKind::Paper,
            "C. Gawron, An iterative algorithm to determine the dynamic user equilibrium in a \
             traffic simulation model, Int. J. Mod. Phys. C 9(3) 393-407 (1998)",
        ),
        sumo,
        primer,
    ];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec![RngDomain::plugin(MODEL_ID).as_str().to_string()],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "assign::tests::the_assignment_converges_and_is_deterministic".to_string(),
        ],
    };
    card
}

/// Checks the parameters.
///
/// # Errors
/// [`DemandError::Invalid`] naming the first bad one.
pub fn validate(p: &AssignParams) -> Result<()> {
    if !(p.gap_tolerance.is_finite() && p.gap_tolerance >= 0.0) {
        return Err(DemandError::Invalid(format!(
            "assign.gap_tolerance is {}, and a gap is a non-negative number",
            p.gap_tolerance
        )));
    }
    if !(p.interval_s > 0.0) {
        return Err(DemandError::Invalid(
            "assign.interval_s must be positive".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&p.weight_memory) || p.weight_memory == 0.0 {
        return Err(DemandError::Invalid(
            "assign.weight_memory is in (0, 1]".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::od::zones::ZoneSet;
    use crate::od::{OdMatrix, od2trips};

    fn demand(w: &World, per_h: f64) -> Vec<Trip> {
        let zones = ZoneSet::grid(w, 2, 2, ClassMask::CAR).expect("zones");
        let names: Vec<String> = zones.names().map(str::to_string).collect();
        let mut m = OdMatrix::new(0.0, 900.0);
        for a in &names {
            for b in &names {
                if a != b {
                    m.add(a, b, per_h / 4.0 / 12.0);
                }
            }
        }
        od2trips(w, &zones, &m, &crate::od::Od2TripsParams::default()).expect("trips")
    }

    #[test]
    fn the_assignment_converges_and_is_deterministic() {
        let w = crate::graph::tests::grid();
        let trips = demand(&w, 1600.0);
        assert!(trips.len() > 50, "{}", trips.len());
        let params = AssignParams {
            max_iterations: 12,
            ..AssignParams::default()
        };
        let a = Assigner::new(&w, params.clone());
        let mut gaps = Vec::new();
        let r1 = a.run(&trips, None, |l| gaps.push(l.relative_gap)).expect("runs");
        assert!(r1.unroutable.len() < trips.len() / 10, "{:?}", r1.unroutable.len());
        let first = gaps[0];
        let last = *gaps.last().unwrap();
        assert!(last <= first, "gap went from {first} to {last}: {gaps:?}");
        let r2 = Assigner::new(&w, params).run(&trips, None, |_| {}).expect("runs");
        assert_eq!(
            serde_json::to_string(&r1.routes).unwrap(),
            serde_json::to_string(&r2.routes).unwrap(),
            "two runs wrote different route files"
        );
        a.card().validate().expect("card");
    }

    #[test]
    fn the_micro_loader_runs_the_routes() {
        let w = crate::graph::tests::grid();
        let trips: Vec<Trip> = demand(&w, 400.0).into_iter().take(20).collect();
        let params = AssignParams {
            loader: LoaderSpec::Micro { step_ms: 200, seed: 1 },
            max_iterations: 2,
            min_iterations: 2,
            horizon_s: 600.0,
            ..AssignParams::default()
        };
        let r = Assigner::new(&w, params).run(&trips, None, |_| {}).expect("runs");
        let arrived = r.routes.log[0].arrived;
        assert!(arrived >= trips.len() / 2, "only {arrived} of {} arrived", trips.len());
    }
}
