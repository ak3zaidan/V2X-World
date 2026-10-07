//! Placing a line's stops on lanes and joining them into a drivable route.
//!
//! A GTFS stop is a kerbside point; which lane it serves depends on the direction the
//! line travels, and on a one-way grid or a divided road the nearest lane is often the
//! wrong one. The stops of a line are matched **together** with a hidden Markov model,
//! as map matching places GPS points [P. Newson, J. Krumm, "Hidden Markov map matching
//! through noise and sparseness", Proc. ACM SIGSPATIAL GIS 2009, pp. 336–343]:
//!
//! * the hidden states of stop `i` are its candidate lanes — up to `candidates` lanes the
//!   line's class may drive within `radius_m`, each with the stop projected onto it;
//! * the emission score is Gaussian in the stop's distance from the lane,
//!   `−½ (d / σ)²`;
//! * the transition score between candidates of consecutive stops is exponential in how
//!   much the driven distance exceeds the straight-line distance, `−|d_route − d_line| / β`
//!   — a pair the line could only join by a long detour is improbable;
//!
//! and Viterbi finds the best joint placement. The route is the concatenation of the
//! shortest paths between consecutive placed stops. Where no candidate pair of two
//! consecutive stops is joined at all, the line is split and the longest matched stretch
//! is kept.
//!
//! SUMO places GTFS stops with `gtfs2pt.py`, which needs the OSM public-transport
//! relations (`ptLines`) to know a line's path; this works from the feed alone.

use serde::{Deserialize, Serialize};
use v2xw_core::geom::Vec3;
use v2xw_world::World;

use crate::costs::TimeCost;
use crate::graph::{LaneGraph, Node};
use crate::tdsp::Search;

/// The matcher's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MatchParams {
    /// How far from a stop a candidate lane may be, metres.
    pub radius_m: f64,
    /// Candidate lanes per stop.
    pub candidates: usize,
    /// Emission σ, metres: a stop pole stands a sidewalk's width or two from the lane it
    /// serves.
    pub sigma_m: f64,
    /// Transition β, metres.
    pub beta_m: f64,
    /// The shortest lane a stop may be placed on, metres.
    pub min_lane_m: f64,
}

impl Default for MatchParams {
    fn default() -> Self {
        Self {
            radius_m: 40.0,
            candidates: 4,
            sigma_m: 8.0,
            beta_m: 30.0,
            min_lane_m: 15.0,
        }
    }
}

/// One stop placed on a lane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// The stop's index in the pattern.
    pub stop: usize,
    /// The lane (graph node).
    pub node: Node,
    /// Where along it, metres.
    pub s_m: f64,
    /// Its index in the route.
    pub route_index: usize,
}

/// A matched line: placed stops and the route through them.
#[derive(Debug, Clone, PartialEq)]
pub struct Matched {
    /// The stops, in order (a contiguous stretch of the pattern).
    pub stops: Vec<Placed>,
    /// The route.
    pub route: Vec<Node>,
}

#[derive(Debug, Clone, Copy)]
struct Cand {
    node: Node,
    s_m: f64,
    d_m: f64,
}

struct Length<'a>(&'a LaneGraph);

impl TimeCost for Length<'_> {
    fn cost_s(&self, n: Node, _t: f64) -> f64 {
        self.0.length_m(n)
    }
}

/// The candidate lanes of a point.
fn candidates(world: &World, graph: &LaneGraph, p: Vec3, mp: &MatchParams) -> Vec<Cand> {
    let mut out: Vec<Cand> = Vec::new();
    for n in 0..graph.len() as Node {
        if graph.is_internal(n) || graph.length_m(n) < mp.min_lane_m {
            continue;
        }
        let lane = world.lane(graph.lane(n));
        // A cheap reject before projecting: the lane's end points both far away.
        let (a, b) = (lane.start(), lane.end());
        let far = |q: Vec3| ((q.x - p.x).abs() - lane.length_m).max((q.y - p.y).abs() - lane.length_m);
        if far(a) > mp.radius_m && far(b) > mp.radius_m {
            continue;
        }
        let proj = lane.project_point(p);
        if proj.distance_m <= mp.radius_m {
            out.push(Cand {
                node: n,
                s_m: proj.s_m,
                d_m: proj.distance_m,
            });
        }
    }
    out.sort_by(|a, b| a.d_m.total_cmp(&b.d_m).then(a.node.cmp(&b.node)));
    out.truncate(mp.candidates.max(1));
    out
}

/// Matches a pattern's stop points (world metres) to lanes (see the module docs).
/// `None` when no two consecutive stops could be joined.
pub fn match_pattern(world: &World, graph: &LaneGraph, points: &[Vec3], mp: &MatchParams) -> Option<Matched> {
    let cands: Vec<Vec<Cand>> = points.iter().map(|p| candidates(world, graph, *p, mp)).collect();
    let mut search = Search::new(graph);
    let len = Length(graph);
    // Viterbi with segment restarts.
    let mut best_segment: Option<Matched> = None;
    let mut i = 0usize;
    while i < points.len() {
        if cands[i].is_empty() {
            i += 1;
            continue;
        }
        // score[c], back-pointers per step, and paths per (step, from, to).
        let mut score: Vec<f64> = cands[i]
            .iter()
            .map(|c| -0.5 * (c.d_m / mp.sigma_m) * (c.d_m / mp.sigma_m))
            .collect();
        let start = i;
        let mut back: Vec<Vec<usize>> = Vec::new();
        let mut paths: Vec<Vec<Vec<Option<Vec<Node>>>>> = Vec::new();
        let mut j = i + 1;
        while j < points.len() && !cands[j].is_empty() {
            let line = {
                let (a, b) = (points[j - 1], points[j]);
                v2xw_core::math::hypot(a.x - b.x, a.y - b.y)
            };
            let from = &cands[j - 1];
            let to = &cands[j];
            let mut next = vec![f64::NEG_INFINITY; to.len()];
            let mut bp = vec![0usize; to.len()];
            let mut step_paths: Vec<Vec<Option<Vec<Node>>>> = vec![vec![None; to.len()]; from.len()];
            let targets: Vec<Node> = to.iter().map(|c| c.node).collect();
            for (a, ca) in from.iter().enumerate() {
                if score[a] == f64::NEG_INFINITY {
                    continue;
                }
                search.run(graph, &len, ca.node, 0.0, &targets);
                for (b, cb) in to.iter().enumerate() {
                    let (dist, path) = if ca.node == cb.node {
                        if cb.s_m + 1.0 >= ca.s_m {
                            (cb.s_m - ca.s_m, Some(vec![ca.node]))
                        } else {
                            (0.0, None)
                        }
                    } else {
                        match (search.cost_to(cb.node), search.path_to(cb.node)) {
                            (Some(c), Some(p)) => {
                                let between = c - graph.length_m(ca.node) - graph.length_m(cb.node);
                                ((graph.length_m(ca.node) - ca.s_m) + between + cb.s_m, Some(p))
                            }
                            _ => (0.0, None),
                        }
                    };
                    let Some(path) = path else { continue };
                    let emit = -0.5 * (cb.d_m / mp.sigma_m) * (cb.d_m / mp.sigma_m);
                    let trans = -(dist - line).abs() / mp.beta_m;
                    let s = score[a] + trans + emit;
                    if s > next[b] || (s == next[b] && a < bp[b]) {
                        next[b] = s;
                        bp[b] = a;
                    }
                    step_paths[a][b] = Some(path);
                }
            }
            if next.iter().all(|s| *s == f64::NEG_INFINITY) {
                break; // no way from stop j-1 to stop j: the segment ends here
            }
            score = next;
            back.push(bp);
            paths.push(step_paths);
            j += 1;
        }
        let end = start + back.len(); // last stop index in this segment
        if end > start {
            // Backtrack.
            let mut c = (0..score.len())
                .filter(|k| score[*k] > f64::NEG_INFINITY)
                .max_by(|a, b| score[*a].total_cmp(&score[*b]).then(b.cmp(a)))
                .expect("a reachable state");
            let mut chosen = vec![0usize; back.len() + 1];
            chosen[back.len()] = c;
            for k in (0..back.len()).rev() {
                c = back[k][c];
                chosen[k] = c;
            }
            let mut route: Vec<Node> = Vec::new();
            let mut stops: Vec<Placed> = Vec::new();
            for k in 0..=back.len() {
                let cand = cands[start + k][chosen[k]];
                if k == 0 {
                    route.push(cand.node);
                } else {
                    let p = paths[k - 1][chosen[k - 1]][chosen[k]].as_ref().expect("on the best path");
                    route.extend_from_slice(&p[1..]);
                }
                stops.push(Placed {
                    stop: start + k,
                    node: cand.node,
                    s_m: cand.s_m,
                    route_index: route.len() - 1,
                });
            }
            let better = best_segment.as_ref().is_none_or(|b| stops.len() > b.stops.len());
            if better {
                best_segment = Some(Matched { stops, route });
            }
        }
        i = end + 1;
    }
    best_segment
}
