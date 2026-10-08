//! `mobility/demand/counts` — routes and trips that reproduce observed counts.
//!
//! Two steps, after SUMO's `routeSampler` [J. Erdmann, SUMO `routeSampler.py`,
//! sumo.dlr.de/docs/Tools/Turns.html#routesamplerpy]:
//!
//! 1. **Candidate routes.** SUMO's tool needs the user to supply them (from `randomTrips`
//!    and `duarouter`). Here they are generated: shortest paths between random endpoint
//!    pairs under randomly perturbed lane costs (each lane's free-flow cost times a factor
//!    uniform in `[1, 1 + ε]`, keyed by sample and lane — `duarouter --weights.random-factor`'s
//!    idea), kept when they pass at least one count; then, for any count no candidate
//!    passes, routes forced through it (origin → counted lane → destination). Every count
//!    is covered by construction. A route file can be supplied instead.
//! 2. **Route flows.** Per count interval:
//!    * `greedy` — `routeSampler`'s default: repeatedly draw one route among those all of
//!      whose counted places still have a vehicle to spare, and use it once.
//!    * `max-entropy` (default) — the route flows of maximum entropy consistent with the
//!      counts, found by iterative proportional fitting over the counts (each sweep scales
//!      the routes through a count so they sum to it) [H. J. van Zuylen, L. G. Willumsen,
//!      "The most likely trip matrix estimated from traffic counts", Transp. Res. B
//!      14(3):281–293, 1980]; then rounded down, and the remainder filled greedily with
//!      the fractional parts as weights. Unlike the greedy draw it spreads vehicles over
//!      all consistent routes rather than over whichever were drawn first, so a corridor
//!      with several parallel routes is not loaded onto one.
//!
//!    `routeSampler --optimize` solves a linear program instead; this crate has no LP
//!    solver and the entropy solution is the classical estimator for the same problem.
//!
//! Each sampled vehicle departs uniformly in the interval **shifted back by its free-flow
//! time to the first counted place on its route**, so it passes the counter inside the
//! interval rather than departing in it. (`routeSampler` departs vehicles in the interval
//! itself, which on a large network shifts a vehicle's count into the next interval.)
//!
//! The result is checked against the counts with [`super::check`].

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::math;
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_mobility::VehicleClass;

use super::PlacedCount;
use crate::costs::{FreeFlow, TimeCost};
use crate::graph::{LaneGraph, Node};
use crate::od::zones::MIN_ENDPOINT_LANE_M;
use crate::tdsp::Search;
use crate::trips::Trip;

/// The model id (also the scenario kind).
pub const MODEL_ID: &str = "mobility/demand/counts";

/// How route flows are found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SamplerMethod {
    /// `routeSampler`'s greedy draw.
    Greedy,
    /// Maximum-entropy route flows by iterative proportional fitting.
    #[default]
    MaxEntropy,
}

/// The sampler's parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SamplerParams {
    /// How route flows are found.
    pub method: SamplerMethod,
    /// Random endpoint pairs tried for candidate routes.
    pub samples: usize,
    /// The cost perturbation ε.
    pub perturbation: f64,
    /// Forced routes generated per uncovered count.
    pub forced_per_count: usize,
    /// IPF sweeps at most.
    pub max_sweeps: u32,
    /// IPF stops when every count is matched within this relative error.
    pub tolerance: f64,
    /// The class of the trips.
    pub class: VehicleClass,
    /// The seed.
    pub seed: u64,
}

impl Default for SamplerParams {
    fn default() -> Self {
        Self {
            method: SamplerMethod::MaxEntropy,
            samples: 2000,
            perturbation: 0.5,
            forced_per_count: 8,
            max_sweeps: 500,
            tolerance: 1e-4,
            class: VehicleClass::Passenger,
            seed: 0xC0_0475,
        }
    }
}

/// A splitmix64 step: the per-(sample, lane) perturbation is a hash, not a stream, so
/// building it costs nothing per lane until the search touches the lane.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

struct Perturbed<'a> {
    graph: &'a LaneGraph,
    salt: u64,
    eps: f64,
}

impl TimeCost for Perturbed<'_> {
    fn cost_s(&self, node: Node, _t: f64) -> f64 {
        let u = (mix(self.salt ^ u64::from(node)) >> 11) as f64 / (1u64 << 53) as f64;
        self.graph.free_cost_s(node) * (1.0 + self.eps * u)
    }
}

/// Candidate routes passing at least one count, deduplicated by the roads they drive.
pub fn candidates(graph: &LaneGraph, counts: &[PlacedCount], p: &SamplerParams) -> Vec<Vec<Node>> {
    let ends = graph.endpoints(MIN_ENDPOINT_LANE_M);
    if ends.len() < 2 {
        return Vec::new();
    }
    let observed: BTreeSet<Node> = counts
        .iter()
        .flat_map(|c| c.nodes.iter().copied())
        .collect();
    let mut seen: BTreeSet<Vec<v2xw_core::ids::EdgeId>> = BTreeSet::new();
    let mut out: Vec<Vec<Node>> = Vec::new();
    let mut search = Search::new(graph);
    let mut keep = |path: Vec<Node>, out: &mut Vec<Vec<Node>>| {
        if path.iter().any(|n| observed.contains(n)) && seen.insert(graph.signature(&path)) {
            out.push(path);
        }
    };
    for s in 0..p.samples {
        let mut rng = RngStream::derive(
            p.seed,
            RngDomain::plugin(MODEL_ID),
            EntityRef::custom(MODEL_ID, s as u64),
        );
        let o = ends[rng.below(ends.len() as u64) as usize];
        let d = ends[rng.below(ends.len() as u64) as usize];
        if o == d {
            continue;
        }
        let cost = Perturbed {
            graph,
            salt: mix(p.seed ^ (s as u64)),
            eps: p.perturbation,
        };
        search.run(graph, &cost, o, 0.0, &[d]);
        if let Some(path) = search.path_to(d) {
            keep(path, &mut out);
        }
    }
    // Force coverage of every count no candidate passes.
    for (ci, c) in counts.iter().enumerate() {
        if out.iter().any(|r| r.iter().any(|n| c.sees(*n))) {
            continue;
        }
        for k in 0..p.forced_per_count.max(1) {
            let mut rng = RngStream::derive(
                p.seed,
                RngDomain::plugin(MODEL_ID),
                EntityRef::custom(MODEL_ID, (1 << 40) | ((ci as u64) << 8) | k as u64),
            );
            let via = c.nodes[rng.below(c.nodes.len() as u64) as usize];
            let o = ends[rng.below(ends.len() as u64) as usize];
            let d = ends[rng.below(ends.len() as u64) as usize];
            let cost = Perturbed {
                graph,
                salt: mix(p.seed ^ (1 << 41) ^ ((ci as u64) << 8) ^ k as u64),
                eps: p.perturbation,
            };
            let head = if o == via {
                Some(vec![via])
            } else {
                search.run(graph, &cost, o, 0.0, &[via]);
                search.path_to(via)
            };
            let tail = if d == via {
                Some(vec![via])
            } else {
                search.run(graph, &cost, via, 0.0, &[d]);
                search.path_to(d)
            };
            if let (Some(mut h), Some(t)) = (head, tail) {
                h.extend_from_slice(&t[1..]);
                // A forced route must not loop through a lane twice.
                let distinct: BTreeSet<Node> = h.iter().copied().collect();
                if distinct.len() == h.len() {
                    keep(h, &mut out);
                }
            }
        }
    }
    out
}

/// What the sampler made.
#[derive(Debug, Clone)]
pub struct Sampled {
    /// The trips, ids from 0 in departure order.
    pub trips: Vec<Trip>,
    /// Each trip's route.
    pub routes: Vec<Vec<Node>>,
    /// Counts that no candidate route passes (and so cannot be matched).
    pub uncovered: Vec<String>,
}

/// Samples trips on `candidates` reproducing `counts` (see the module docs).
pub fn sample(
    graph: &LaneGraph,
    counts: &[PlacedCount],
    candidates: &[Vec<Node>],
    p: &SamplerParams,
) -> Sampled {
    let free = FreeFlow(graph);
    let mut groups: BTreeMap<(u64, u64), Vec<usize>> = BTreeMap::new();
    for (i, c) in counts.iter().enumerate() {
        groups
            .entry((c.count.begin_s.to_bits(), c.count.end_s.to_bits()))
            .or_default()
            .push(i);
    }
    let mut uncovered = Vec::new();
    let mut raw: Vec<(f64, usize, u64)> = Vec::new();
    let mut gi = 0u64;
    for ((b, e), members) in groups {
        gi += 1;
        let (begin_s, end_s) = (f64::from_bits(b), f64::from_bits(e));
        // Incidence: which members each route passes, and its free-flow offset to the first.
        let mut inc: Vec<Vec<usize>> = vec![Vec::new(); candidates.len()];
        let mut offset = vec![0.0; candidates.len()];
        for (r, path) in candidates.iter().enumerate() {
            let mut t = 0.0;
            let mut first: Option<f64> = None;
            for n in path {
                for (k, ci) in members.iter().enumerate() {
                    if counts[*ci].sees(*n) && !inc[r].contains(&k) {
                        inc[r].push(k);
                        first.get_or_insert(t);
                    }
                }
                t += free.cost_s(*n, t);
            }
            offset[r] = first.unwrap_or(0.0);
        }
        let target: Vec<f64> = members
            .iter()
            .map(|ci| counts[*ci].count.count.max(0.0).round())
            .collect();
        for (k, ci) in members.iter().enumerate() {
            if !inc.iter().any(|x| x.contains(&k)) && target[k] > 0.0 {
                uncovered.push(counts[*ci].count.id.clone());
            }
        }
        let used: Vec<usize> = (0..candidates.len())
            .filter(|r| !inc[*r].is_empty())
            .collect();
        let mut n_r = vec![0u64; candidates.len()];
        let mut residual = target.clone();
        let mut weights = vec![1.0; candidates.len()];
        if p.method == SamplerMethod::MaxEntropy && !used.is_empty() {
            let mut x = vec![0.0; candidates.len()];
            for r in &used {
                x[*r] = 1.0;
            }
            for _ in 0..p.max_sweeps.max(1) {
                for k in 0..members.len() {
                    let through: Vec<usize> = used
                        .iter()
                        .copied()
                        .filter(|r| inc[*r].contains(&k))
                        .collect();
                    let v = math::sum_ordered(through.iter().map(|r| x[*r]));
                    if v > 0.0 {
                        let f = target[k] / v;
                        for r in through {
                            x[r] *= f;
                        }
                    }
                }
                let err = (0..members.len())
                    .map(|k| {
                        let v = math::sum_ordered(
                            used.iter().filter(|r| inc[**r].contains(&k)).map(|r| x[*r]),
                        );
                        if inc.iter().any(|i| i.contains(&k)) {
                            (v - target[k]).abs() / target[k].max(1.0)
                        } else {
                            0.0
                        }
                    })
                    .fold(0.0, f64::max);
                if err <= p.tolerance {
                    break;
                }
            }
            for r in &used {
                n_r[*r] = x[*r].max(0.0).floor() as u64;
                weights[*r] = x[*r] - x[*r].floor();
                for k in &inc[*r] {
                    residual[*k] -= n_r[*r] as f64;
                }
            }
        }
        // Greedy fill (the whole of the greedy method; the remainder of max-entropy).
        let mut rng = RngStream::derive(
            p.seed,
            RngDomain::plugin(MODEL_ID),
            EntityRef::custom(MODEL_ID, (1 << 50) | gi),
        );
        loop {
            let open: Vec<usize> = used
                .iter()
                .copied()
                .filter(|r| inc[*r].iter().all(|k| residual[*k] >= 1.0))
                .collect();
            if open.is_empty() {
                break;
            }
            let w: Vec<f64> = open.iter().map(|r| weights[*r]).collect();
            let pick = if w.iter().any(|x| *x > 0.0) {
                open[rng.choose_index(&w)]
            } else {
                open[rng.below(open.len() as u64) as usize]
            };
            n_r[pick] += 1;
            weights[pick] = 0.0;
            if p.method == SamplerMethod::Greedy {
                weights[pick] = 1.0;
            }
            for k in &inc[pick] {
                residual[*k] -= 1.0;
            }
        }
        for r in used {
            for v in 0..n_r[r] {
                let mut d = RngStream::derive(
                    p.seed,
                    RngDomain::plugin(MODEL_ID),
                    EntityRef::custom(MODEL_ID, (gi << 44) ^ ((r as u64) << 20) ^ v),
                );
                let pass = d.uniform(begin_s, end_s);
                raw.push(((pass - offset[r]).max(0.0), r, v));
            }
        }
    }
    raw.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut trips = Vec::with_capacity(raw.len());
    let mut routes = Vec::with_capacity(raw.len());
    for (id, (t, r, _)) in raw.into_iter().enumerate() {
        let path = &candidates[r];
        trips.push(Trip {
            id: id as u64,
            depart_s: math::quantize(t, 3),
            origin: graph.lane(path[0]),
            destination: graph.lane(*path.last().expect("non-empty")),
            class: p.class,
            purpose: None,
            tag: Some(format!("route {r}")),
        });
        routes.push(path.clone());
    }
    uncovered.sort();
    uncovered.dedup();
    Sampled {
        trips,
        routes,
        uncovered,
    }
}

/// The model card.
pub fn card(p: &SamplerParams) -> ModelCard {
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "Trips and routes that reproduce observed link counts: generated candidate routes, \
         maximum-entropy (or greedy) route flows, departures shifted so vehicles pass the \
         counters inside the counted interval.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![Equation::new(
        "IPF sweep",
        "for each count l: x_r <- x_r * c_l / sum_{s through l} x_s (r through l)",
    )];
    card.parameters = vec![
        Parameter::new(
            "method",
            "-",
            serde_json::to_value(p.method).unwrap_or_default(),
            Source::new(
                SourceKind::Paper,
                "H. J. van Zuylen, L. G. Willumsen, The most likely trip matrix estimated \
                 from traffic counts, Transportation Research B 14(3) 281-293 (1980)",
            ),
        ),
        Parameter::new(
            "perturbation",
            "-",
            serde_json::json!(p.perturbation),
            Source::new(
                SourceKind::Code,
                "SUMO duarouter --weights.random-factor (randomised route alternatives)",
            ),
        ),
    ];
    card.sources = vec![
        Source::new(
            SourceKind::Code,
            "SUMO routeSampler.py (J. Erdmann, DLR), sumo.dlr.de/docs/Tools/Turns.html",
        ),
        Source::new(
            SourceKind::Standard,
            "UK DfT TAG Unit M3.1 Highway Assignment Modelling (GEH < 5 on 85% of counts)",
        ),
    ];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec![RngDomain::plugin(MODEL_ID).as_str().to_string()],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec!["counts::sampler::tests::sampled_routes_reproduce_the_counts".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::counts::{Count, Location, check, place};
    use v2xw_world::ClassMask;

    fn some_counts(g: &LaneGraph, w: &v2xw_world::World) -> Vec<Count> {
        // Eight counts on distinct roads in the middle of the grid, 40-120 vehicles.
        let ends = g.endpoints(60.0);
        let mut edges: Vec<u32> = ends.iter().map(|n| g.edge(*n).0).collect();
        edges.sort_unstable();
        edges.dedup();
        let step = (edges.len() / 9).max(1);
        let _ = w;
        (1..=8)
            .map(|i| Count {
                id: format!("c{i}"),
                location: Location::Edge {
                    edge: edges[(i * step) % edges.len()],
                },
                begin_s: 0.0,
                end_s: 3600.0,
                count: (30 + 11 * i) as f64,
            })
            .collect()
    }

    #[test]
    fn sampled_routes_reproduce_the_counts() {
        let w = crate::graph::tests::grid();
        let g = LaneGraph::new(&w, ClassMask::CAR);
        let counts = place(&w, &g, &some_counts(&g, &w)).expect("placed");
        for method in [SamplerMethod::MaxEntropy, SamplerMethod::Greedy] {
            let p = SamplerParams {
                method,
                samples: 600,
                ..SamplerParams::default()
            };
            let cands = candidates(&g, &counts, &p);
            assert!(!cands.is_empty());
            let s = sample(&g, &counts, &cands, &p);
            assert!(s.uncovered.is_empty(), "{:?}", s.uncovered);
            // Checked the way a run would see them: free-flow passing times.
            let c = check(&counts, &s.trips, &s.routes, &FreeFlow(&g));
            assert!(
                c.passes_tag_m3_1,
                "{method:?}: {:.0}% under GEH 5\n{:#?}",
                100.0 * c.share_geh_below_5,
                c.rows
            );
        }
        card(&SamplerParams::default()).validate().expect("card");
    }

    #[test]
    fn a_wrong_demand_fails_the_check() {
        // The check must be able to fail: halve every trip set and it does.
        let w = crate::graph::tests::grid();
        let g = LaneGraph::new(&w, ClassMask::CAR);
        let counts = place(&w, &g, &some_counts(&g, &w)).expect("placed");
        let p = SamplerParams {
            samples: 600,
            ..SamplerParams::default()
        };
        let cands = candidates(&g, &counts, &p);
        let s = sample(&g, &counts, &cands, &p);
        let half: Vec<usize> = (0..s.trips.len()).step_by(2).collect();
        let trips: Vec<Trip> = half.iter().map(|i| s.trips[*i].clone()).collect();
        let routes: Vec<Vec<Node>> = half.iter().map(|i| s.routes[*i].clone()).collect();
        let c = check(&counts, &trips, &routes, &FreeFlow(&g));
        assert!(!c.passes_tag_m3_1, "{:?}", c.share_geh_below_5);
    }
}
