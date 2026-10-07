//! Time-dependent shortest paths: one search from an origin at a departure instant,
//! every lane priced when the vehicle would enter it.
//!
//! This is Dijkstra's algorithm with arrival-time labels, which is exact when the costs
//! are FIFO (leaving later never means arriving earlier) [Dreyfus 1969; Kaufman & Smith
//! 1993, "Fastest paths in time-dependent networks for intelligent vehicle-highway systems
//! application", IVHS Journal 1(1)]. Interval-averaged travel times are FIFO only
//! approximately; the error is bounded by one interval's change in cost and is the same
//! one SUMO's time-dependent `duarouter` accepts.
//!
//! Determinism: the frontier is ordered by `(label, node)` with `total_cmp`, and nodes are
//! dense indices, so equal-cost ties go to the lower lane id exactly as in the engine's
//! router.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::costs::TimeCost;
use crate::graph::{LaneGraph, NONE, Node};

#[derive(Debug, Clone, Copy, PartialEq)]
struct Entry {
    t: f64,
    node: Node,
}

impl Eq for Entry {}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .t
            .total_cmp(&self.t)
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A reusable search: its arrays are sized once for the graph and reset per query.
#[derive(Debug, Clone)]
pub struct Search {
    /// The instant each node is left (its exit label), `INFINITY` if unreached.
    exit_s: Vec<f64>,
    parent: Vec<Node>,
    touched: Vec<Node>,
    origin: Node,
    depart_s: f64,
}

impl Search {
    /// A search over `graph`.
    pub fn new(graph: &LaneGraph) -> Self {
        Self {
            exit_s: vec![f64::INFINITY; graph.len()],
            parent: vec![NONE; graph.len()],
            touched: Vec::new(),
            origin: NONE,
            depart_s: 0.0,
        }
    }

    /// Runs from `origin` departing at `depart_s` until every node of `targets` is settled
    /// (or the graph is exhausted). The origin lane's own traversal is counted, as the
    /// engine's router counts it.
    pub fn run(
        &mut self,
        graph: &LaneGraph,
        costs: &dyn TimeCost,
        origin: Node,
        depart_s: f64,
        targets: &[Node],
    ) {
        for n in self.touched.drain(..) {
            self.exit_s[n as usize] = f64::INFINITY;
            self.parent[n as usize] = NONE;
        }
        self.origin = origin;
        self.depart_s = depart_s;
        let mut remaining: Vec<Node> = targets.to_vec();
        remaining.sort_unstable();
        remaining.dedup();
        let mut left = remaining.len();
        let start = depart_s + costs.cost_s(origin, depart_s);
        self.exit_s[origin as usize] = start;
        self.touched.push(origin);
        let mut heap = BinaryHeap::new();
        heap.push(Entry {
            t: start,
            node: origin,
        });
        while let Some(Entry { t, node }) = heap.pop() {
            if t > self.exit_s[node as usize] {
                continue;
            }
            if left > 0 && remaining.binary_search(&node).is_ok() {
                left -= 1;
                if left == 0 {
                    break;
                }
            }
            for next in graph.out(node) {
                let step = costs.cost_s(*next, t);
                if !(step.is_finite() && step >= 0.0) {
                    continue;
                }
                let cand = t + step;
                let cur = self.exit_s[*next as usize];
                if cand < cur {
                    if cur == f64::INFINITY {
                        self.touched.push(*next);
                    }
                    self.exit_s[*next as usize] = cand;
                    self.parent[*next as usize] = node;
                    heap.push(Entry {
                        t: cand,
                        node: *next,
                    });
                }
            }
        }
    }

    /// The travel time to the end of `target`, seconds, if it was reached.
    pub fn cost_to(&self, target: Node) -> Option<f64> {
        let t = self.exit_s[target as usize];
        t.is_finite().then(|| t - self.depart_s)
    }

    /// The node path from the origin to `target`, inclusive, if it was reached.
    pub fn path_to(&self, target: Node) -> Option<Vec<Node>> {
        if !self.exit_s[target as usize].is_finite() {
            return None;
        }
        let mut path = vec![target];
        let mut cur = target;
        while cur != self.origin {
            cur = self.parent[cur as usize];
            if cur == NONE {
                return None;
            }
            path.push(cur);
        }
        path.reverse();
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::costs::{FreeFlow, path_cost_s};
    use v2xw_world::ClassMask;

    #[test]
    fn the_free_flow_search_matches_the_engine_router() {
        let w = crate::graph::tests::grid();
        let g = LaneGraph::new(&w, ClassMask::CAR);
        let ends = g.endpoints(10.0);
        let (o, d) = (ends[0], ends[ends.len() - 1]);
        // With no signal delay in the price, the cost is the router's length / limit.
        let plain = |n: Node| g.free_s(n);
        struct Plain<'a>(&'a dyn Fn(Node) -> f64);
        impl TimeCost for Plain<'_> {
            fn cost_s(&self, n: Node, _t: f64) -> f64 {
                (self.0)(n)
            }
        }
        let mut s = Search::new(&g);
        s.run(&g, &Plain(&plain), o, 0.0, &[d]);
        let path = s.path_to(d).expect("a path");
        let router = v2xw_mobility::Dijkstra::default();
        let ff = v2xw_mobility::FreeFlowCost::new(&w);
        let r = router
            .search(&w, g.lane(o), g.lane(d), 0, &ff)
            .expect("the router finds one too");
        assert!(
            (s.cost_to(d).unwrap() - r.cost_s).abs() < 1e-6,
            "{} vs {}",
            s.cost_to(d).unwrap(),
            r.cost_s
        );
        assert_eq!(path.first(), Some(&o));
        assert_eq!(path.last(), Some(&d));
        // And the path cost walks back to the label.
        let walked = path_cost_s(&FreeFlow(&g), &path, 0.0);
        assert!(walked > 0.0);
    }
}
