//! Time-dependent lane travel times: what a network loading measures and what the next
//! iteration routes on.
//!
//! Times are kept per node per aggregation interval, as SUMO's `duaIterate` keeps edge
//! weights per `--aggregation` interval (default 900 s) from the simulation's edge dumps
//! [SUMO duaIterate documentation]. A cell nobody entered falls back to the free-flow cost
//! ([`crate::graph::LaneGraph::free_cost_s`]), so an empty interval is never "free" nor
//! "closed".

use crate::graph::{LaneGraph, Node};

/// The cost a time-dependent search reads: seconds to traverse `node` entering at `t_s`.
pub trait TimeCost {
    /// Seconds to traverse `node` when entering it at `t_s` (seconds from the run start).
    fn cost_s(&self, node: Node, t_s: f64) -> f64;
}

/// Free-flow costs, time-independent.
#[derive(Debug, Clone, Copy)]
pub struct FreeFlow<'a>(pub &'a LaneGraph);

impl TimeCost for FreeFlow<'_> {
    fn cost_s(&self, node: Node, _t_s: f64) -> f64 {
        self.0.free_cost_s(node)
    }
}

/// Measured travel times per node per interval.
#[derive(Debug, Clone, PartialEq)]
pub struct TravelTimes {
    begin_s: f64,
    bin_s: f64,
    bins: usize,
    nodes: usize,
    sum_s: Vec<f64>,
    count: Vec<u32>,
    free: Vec<f64>,
}

impl TravelTimes {
    /// An empty table for `graph` covering `[begin_s, end_s)` in intervals of `bin_s`.
    pub fn new(graph: &LaneGraph, begin_s: f64, end_s: f64, bin_s: f64) -> Self {
        let bin_s = if bin_s > 0.0 { bin_s } else { 900.0 };
        let bins = (((end_s - begin_s) / bin_s).ceil().max(1.0)) as usize;
        let nodes = graph.len();
        Self {
            begin_s,
            bin_s,
            bins,
            nodes,
            sum_s: vec![0.0; nodes * bins],
            count: vec![0; nodes * bins],
            free: (0..nodes as Node).map(|n| graph.free_cost_s(n)).collect(),
        }
    }

    /// The interval index of `t_s`, clamped into the table.
    pub fn bin(&self, t_s: f64) -> usize {
        let i = ((t_s - self.begin_s) / self.bin_s).floor();
        if i <= 0.0 {
            0
        } else {
            (i as usize).min(self.bins - 1)
        }
    }

    /// The interval length, seconds.
    pub fn bin_s(&self) -> f64 {
        self.bin_s
    }

    /// The number of intervals.
    pub fn bins(&self) -> usize {
        self.bins
    }

    /// Records one traversal of `node` that entered at `enter_s` and took `tt_s`.
    pub fn record(&mut self, node: Node, enter_s: f64, tt_s: f64) {
        if !(tt_s.is_finite() && tt_s >= 0.0) || node as usize >= self.nodes {
            return;
        }
        let i = node as usize * self.bins + self.bin(enter_s);
        self.sum_s[i] += tt_s;
        self.count[i] += 1;
    }

    /// The number of traversals recorded for `node` in interval `bin`.
    pub fn count(&self, node: Node, bin: usize) -> u32 {
        self.count[node as usize * self.bins + bin]
    }

    /// Exponential smoothing with the previous iteration's table:
    /// `t ← α·t + (1 − α)·previous`, cell by cell, an unobserved cell counting as its
    /// free-flow value. SUMO's `--weight-memory`; `α = 1` keeps only the new loading.
    pub fn blend(&mut self, previous: &TravelTimes, alpha: f64) {
        if alpha >= 1.0 || previous.sum_s.len() != self.sum_s.len() {
            return;
        }
        for node in 0..self.nodes {
            for b in 0..self.bins {
                let i = node * self.bins + b;
                let now = self.mean(node, b);
                let before = previous.mean(node, b);
                self.sum_s[i] = alpha * now + (1.0 - alpha) * before;
                self.count[i] = 1;
            }
        }
    }

    fn mean(&self, node: usize, bin: usize) -> f64 {
        let i = node * self.bins + bin;
        if self.count[i] == 0 {
            self.free[node]
        } else {
            self.sum_s[i] / f64::from(self.count[i])
        }
    }
}

impl TimeCost for TravelTimes {
    fn cost_s(&self, node: Node, t_s: f64) -> f64 {
        self.mean(node as usize, self.bin(t_s))
    }
}

/// The cost of driving `path` departing at `depart_s`, each lane priced at the instant it
/// is entered: the time-dependent route cost of the alternative set.
pub fn path_cost_s(costs: &dyn TimeCost, path: &[Node], depart_s: f64) -> f64 {
    let mut t = depart_s;
    for n in path {
        t += costs.cost_s(*n, t);
    }
    t - depart_s
}
