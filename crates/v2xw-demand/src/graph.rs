//! The assignment graph: the world's lane graph, compacted for thousands of searches.
//!
//! A node is a lane and an arc a permitted connection, exactly as
//! [`v2xw_mobility::routing::dijkstra`] has it — a turn ban is a missing arc, and a route
//! found here is one the engine will drive ([`v2xw_mobility::demand::hooks`]) — but stored
//! as a compressed-sparse-row adjacency over dense node indices rather than walked through
//! `BTreeMap`s, because an assignment runs one search per origin per departure interval per
//! iteration, and a city's worth of them must finish in seconds.
//!
//! The admission filters are [`v2xw_mobility::routing::dijkstra::Dijkstra::admits`]'s: the
//! lane must admit one of the classes, and for a motorised class its kind must be a
//! motor-traffic kind.
//!
//! Each node also carries its signal gate when it is a signal-controlled junction connector:
//! the green windows of its movement within the cycle, read from the world's
//! [`v2xw_world::SignalPlan`]. The queue loader holds vehicles to them, and the free-flow
//! cost adds Webster's zero-flow uniform delay for them.

use v2xw_core::ids::{EdgeId, LaneId};
use v2xw_world::{ClassMask, LaneKind, SignalState, TurnDirection, World};

/// A dense node index.
pub type Node = u32;

/// "Not in the graph".
pub const NONE: Node = u32::MAX;

/// The green windows of one signal-controlled movement.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalGate {
    /// Cycle length, seconds.
    pub cycle_s: f64,
    /// Offset of the cycle start from the run's `t0`, seconds.
    pub offset_s: f64,
    /// Green windows `[start, end)` within the cycle, seconds from its start, ascending.
    pub greens: Vec<(f64, f64)>,
}

impl SignalGate {
    /// The share of the cycle the movement has green, λ = g / C.
    pub fn green_ratio(&self) -> f64 {
        let g: f64 = v2xw_core::math::sum_ordered(self.greens.iter().map(|(a, b)| b - a));
        (g / self.cycle_s).clamp(0.0, 1.0)
    }

    /// The first instant at or after `t_s` at which the movement shows green.
    ///
    /// A movement that never has green returns `t_s + cycle` — held one cycle, and never
    /// forever, so a plan the importer got wrong delays a loading rather than hanging it.
    pub fn next_green(&self, t_s: f64) -> f64 {
        if self.greens.is_empty() || !(self.cycle_s > 0.0) {
            return t_s + self.cycle_s.max(0.0);
        }
        let mut into = (t_s - self.offset_s) % self.cycle_s;
        if into < 0.0 {
            into += self.cycle_s;
        }
        let cycle_start = t_s - into;
        for (a, b) in &self.greens {
            if into < *b {
                return if into >= *a { t_s } else { cycle_start + a };
            }
        }
        cycle_start + self.cycle_s + self.greens[0].0
    }

    /// Webster's uniform delay at zero flow, `C(1 − λ)² / 2` seconds: the mean wait of a
    /// vehicle arriving at a random instant [Webster 1958, eq. for d₁ with x → 0].
    pub fn zero_flow_delay_s(&self) -> f64 {
        let lambda = self.green_ratio();
        self.cycle_s * (1.0 - lambda) * (1.0 - lambda) / 2.0
    }
}

/// The assignment graph for one set of vehicle classes.
#[derive(Debug, Clone)]
pub struct LaneGraph {
    classes: ClassMask,
    allow_uturn: bool,
    node_of: Vec<Node>,
    lanes: Vec<LaneId>,
    edges: Vec<EdgeId>,
    internal: Vec<bool>,
    length_m: Vec<f64>,
    free_s: Vec<f64>,
    first_out: Vec<u32>,
    heads: Vec<Node>,
    gates: Vec<Option<SignalGate>>,
}

impl LaneGraph {
    /// The graph of every lane `classes` may use, U-turns excluded — the engine router's
    /// default.
    pub fn new(world: &World, classes: ClassMask) -> Self {
        Self::with_uturns(world, classes, false)
    }

    /// The graph, with U-turn connections included when `allow_uturn`.
    pub fn with_uturns(world: &World, classes: ClassMask, allow_uturn: bool) -> Self {
        let motorised = classes.contains_any(ClassMask::MOTOR_TRAFFIC);
        let all = world.roads.lanes();
        let mut node_of = vec![NONE; all.len()];
        let mut lanes = Vec::new();
        for l in all {
            let ok = l.admits(classes) && (!motorised || l.kind.is_motorised());
            if ok && l.speed_limit_mps > 0.0 {
                node_of[l.id.as_usize()] = lanes.len() as Node;
                lanes.push(l.id);
            }
        }
        let mut first_out = Vec::with_capacity(lanes.len() + 1);
        let mut heads = Vec::new();
        for lane in &lanes {
            first_out.push(heads.len() as u32);
            let mut outs: Vec<Node> = Vec::new();
            for c in world.successors(*lane) {
                if !c.permitted || (!allow_uturn && c.direction == TurnDirection::UTurn) {
                    continue;
                }
                let next = c.via.unwrap_or(c.to_lane);
                if next == *lane {
                    continue;
                }
                let n = node_of.get(next.as_usize()).copied().unwrap_or(NONE);
                if n != NONE {
                    outs.push(n);
                }
            }
            outs.sort_unstable();
            outs.dedup();
            heads.extend(outs);
        }
        first_out.push(heads.len() as u32);
        let mut gates: Vec<Option<SignalGate>> = vec![None; lanes.len()];
        for plan in &world.signals {
            if !(plan.cycle_s.is_finite() && plan.cycle_s > 0.0) {
                continue;
            }
            for (i, lane) in plan.controlled.iter().enumerate() {
                let n = node_of.get(lane.as_usize()).copied().unwrap_or(NONE);
                if n == NONE {
                    continue;
                }
                let mut greens: Vec<(f64, f64)> = Vec::new();
                let mut start = 0.0;
                for phase in &plan.phases {
                    let end = start + phase.duration_s;
                    let go = matches!(
                        phase.states.get(i),
                        Some(SignalState::Green | SignalState::GreenYield)
                    );
                    if go {
                        match greens.last_mut() {
                            Some(last) if last.1 == start => last.1 = end,
                            _ => greens.push((start, end)),
                        }
                    }
                    start = end;
                }
                gates[n as usize] = Some(SignalGate {
                    cycle_s: plan.cycle_s,
                    offset_s: plan.offset_s,
                    greens,
                });
            }
        }
        let length_m: Vec<f64> = lanes.iter().map(|l| world.lane(*l).length_m).collect();
        let free_s: Vec<f64> = lanes
            .iter()
            .map(|l| {
                let l = world.lane(*l);
                l.length_m / l.speed_limit_mps
            })
            .collect();
        let edges = lanes.iter().map(|l| world.lane(*l).edge).collect();
        let internal = lanes
            .iter()
            .map(|l| world.lane(*l).kind == LaneKind::Internal)
            .collect();
        Self {
            classes,
            allow_uturn,
            node_of,
            lanes,
            edges,
            internal,
            length_m,
            free_s,
            first_out,
            heads,
            gates,
        }
    }

    /// The classes the graph admits.
    pub fn classes(&self) -> ClassMask {
        self.classes
    }

    /// Whether U-turn connections are arcs.
    pub fn allows_uturn(&self) -> bool {
        self.allow_uturn
    }

    /// The number of nodes.
    pub fn len(&self) -> usize {
        self.lanes.len()
    }

    /// True if no lane admits the classes.
    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty()
    }

    /// The node of `lane`, if the lane is in the graph.
    pub fn node(&self, lane: LaneId) -> Option<Node> {
        self.node_of
            .get(lane.as_usize())
            .copied()
            .filter(|n| *n != NONE)
    }

    /// The lane of `node`.
    pub fn lane(&self, node: Node) -> LaneId {
        self.lanes[node as usize]
    }

    /// The world edge `node` lies on.
    pub fn edge(&self, node: Node) -> EdgeId {
        self.edges[node as usize]
    }

    /// True for a junction's internal connector.
    pub fn is_internal(&self, node: Node) -> bool {
        self.internal[node as usize]
    }

    /// The lane's length, metres.
    pub fn length_m(&self, node: Node) -> f64 {
        self.length_m[node as usize]
    }

    /// Free-flow traversal time at the speed limit, seconds.
    pub fn free_s(&self, node: Node) -> f64 {
        self.free_s[node as usize]
    }

    /// The free-flow cost the first iteration routes on: traversal at the limit plus, for a
    /// signal-controlled connector, Webster's zero-flow delay.
    pub fn free_cost_s(&self, node: Node) -> f64 {
        self.free_s(node)
            + self.gates[node as usize]
                .as_ref()
                .map_or(0.0, SignalGate::zero_flow_delay_s)
    }

    /// The node's signal gate, when it is a signal-controlled connector.
    pub fn gate(&self, node: Node) -> Option<&SignalGate> {
        self.gates[node as usize].as_ref()
    }

    /// The arcs out of `node`, ascending.
    pub fn out(&self, node: Node) -> &[Node] {
        let a = self.first_out[node as usize] as usize;
        let b = self.first_out[node as usize + 1] as usize;
        &self.heads[a..b]
    }

    /// Lane ids of a node path.
    pub fn lanes_of(&self, path: &[Node]) -> Vec<LaneId> {
        path.iter().map(|n| self.lane(*n)).collect()
    }

    /// The node path of a lane route, or `None` if a lane is not in the graph or two
    /// consecutive lanes are not joined by an arc.
    pub fn nodes_of(&self, lanes: &[LaneId]) -> Option<Vec<Node>> {
        let path: Vec<Node> = lanes
            .iter()
            .map(|l| self.node(*l))
            .collect::<Option<Vec<_>>>()?;
        path.windows(2)
            .all(|p| self.out(p[0]).binary_search(&p[1]).is_ok())
            .then_some(path)
    }

    /// A route's identity for the alternative set: its sequence of non-internal edges.
    ///
    /// Two lane paths that drive the same streets through the same turns but enter a
    /// junction from different lanes of one road are one route to a driver, and a choice
    /// set that counted them twice would split one route's probability.
    pub fn signature(&self, path: &[Node]) -> Vec<EdgeId> {
        let mut out: Vec<EdgeId> = Vec::with_capacity(path.len() / 2 + 1);
        for n in path {
            if self.is_internal(*n) {
                continue;
            }
            let e = self.edge(*n);
            if out.last() != Some(&e) {
                out.push(e);
            }
        }
        out
    }

    /// Total length of a path, metres (an id-ordered sum).
    pub fn path_length_m(&self, path: &[Node]) -> f64 {
        v2xw_core::math::sum_ordered(path.iter().map(|n| self.length_m(*n)))
    }

    /// Every node a trip of length `min_len_m` can start or end on: a non-internal lane at
    /// least that long.
    pub fn endpoints(&self, min_len_m: f64) -> Vec<Node> {
        (0..self.len() as Node)
            .filter(|n| !self.is_internal(*n) && self.length_m(*n) >= min_len_m)
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use v2xw_world::{ImportOptions, procedural::GridParams};

    pub(crate) fn grid() -> World {
        let params = GridParams {
            signalised: true,
            ..GridParams::legacy()
        };
        v2xw_world::procedural::grid(&params, &ImportOptions::default()).expect("grid")
    }

    #[test]
    fn the_graph_has_the_router_s_arcs() {
        let w = grid();
        let g = LaneGraph::new(&w, ClassMask::CAR);
        assert!(!g.is_empty());
        for n in 0..g.len() as Node {
            let lane = g.lane(n);
            assert_eq!(g.node(lane), Some(n));
            for m in g.out(n) {
                let ok = w.successors(lane).iter().any(|c| {
                    c.permitted
                        && c.direction != TurnDirection::UTurn
                        && c.via.unwrap_or(c.to_lane) == g.lane(*m)
                });
                assert!(ok, "arc {n}->{m} is not a permitted connection");
            }
        }
    }

    #[test]
    fn a_gate_waits_for_green() {
        let gate = SignalGate {
            cycle_s: 60.0,
            offset_s: 0.0,
            greens: vec![(10.0, 30.0)],
        };
        assert_eq!(gate.next_green(15.0), 15.0);
        assert_eq!(gate.next_green(5.0), 10.0);
        assert_eq!(gate.next_green(35.0), 70.0);
        assert!((gate.green_ratio() - 1.0 / 3.0).abs() < 1e-12);
        // C(1-λ)²/2 = 60·(2/3)²/2 = 13.33 s.
        assert!((gate.zero_flow_delay_s() - 40.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn the_signalised_grid_has_gates() {
        let w = grid();
        let g = LaneGraph::new(&w, ClassMask::CAR);
        let gated = (0..g.len() as Node)
            .filter(|n| g.gate(*n).is_some())
            .count();
        assert!(gated > 0, "a signalised grid gives some connector a gate");
    }
}
