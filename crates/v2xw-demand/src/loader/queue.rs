//! `demand/loader/queue` — an event-driven point-queue network with signal gating.
//!
//! Every lane is a free-flow delay followed by a FIFO server: a vehicle entering lane `n`
//! at `t` reaches its end at `t + t₀(n)` and leaves at
//!
//! ```text
//! exit = max(t + t₀(n), last_exit(n) + h)     h = 3600 / s   (s: saturation flow, veh/h)
//! ```
//!
//! and, when the next lane on its route is a signal-controlled junction connector, not
//! before that movement's next green ([`crate::graph::SignalGate::next_green`]). This is
//! Vickrey's point queue [W. S. Vickrey, "Congestion theory and transport investment",
//! Am. Econ. Rev. 59(2):251–260, 1969] — the queue model of Gawron's original dynamic
//! user-equilibrium study [Gawron 1998] without its storage constraint — with the HCM's
//! saturation flow as the discharge rate [*Highway Capacity Manual*, 6th ed.,
//! TRB 2016, ch. 19: base saturation flow 1,900 pc/h/ln].
//!
//! Events are processed in `(time, trip)` order from one heap, so each lane sees its
//! entries in time order and the FIFO discharge is exact; nothing here depends on hash
//! order or thread count.
//!
//! **What it leaves out.** Queues have no length, so a queue never spills back into the
//! junction upstream; gridlock cannot happen, and a corridor loaded far past capacity shows
//! long delays on its bottleneck lane rather than on the lanes behind it. Lane changes, gap
//! acceptance at unsignalised junctions and turning conflicts are not modelled; the micro
//! loader has all of them.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_world::World;

use super::{Loading, NetworkLoader, Window};
use crate::costs::TravelTimes;
use crate::error::Result;
use crate::graph::{LaneGraph, Node};
use crate::trips::Trip;

/// The model id.
pub const MODEL_ID: &str = "demand/loader/queue";

/// HCM 6th ed. base saturation flow, pc/h/ln.
pub fn default_saturation() -> f64 {
    1900.0
}

/// The point-queue loader.
#[derive(Debug, Clone)]
pub struct PointQueue {
    headway_s: f64,
    card: ModelCard,
}

impl PointQueue {
    /// The loader with saturation flow `saturation_veh_per_h` per lane.
    pub fn new(saturation_veh_per_h: f64) -> Self {
        let s = if saturation_veh_per_h > 0.0 {
            saturation_veh_per_h
        } else {
            default_saturation()
        };
        Self {
            headway_s: 3600.0 / s,
            card: card(s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Event {
    t: f64,
    trip: u32,
    pos: u32,
}

impl Eq for Event {}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .t
            .total_cmp(&self.t)
            .then_with(|| other.trip.cmp(&self.trip))
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl NetworkLoader for PointQueue {
    fn card(&self) -> &ModelCard {
        &self.card
    }

    fn load(
        &mut self,
        _world: &World,
        graph: &LaneGraph,
        trips: &[Trip],
        routes: &[Vec<Node>],
        window: Window,
    ) -> Result<Loading> {
        let mut times = TravelTimes::new(graph, window.begin_s, window.end_s, window.bin_s);
        let mut last_exit = vec![f64::NEG_INFINITY; graph.len()];
        let mut experienced = vec![f64::NAN; trips.len()];
        let mut arrived = 0usize;
        let mut heap = BinaryHeap::with_capacity(trips.len());
        for (i, (trip, route)) in trips.iter().zip(routes).enumerate() {
            if !route.is_empty() {
                heap.push(Event {
                    t: trip.depart_s,
                    trip: i as u32,
                    pos: 0,
                });
            }
        }
        while let Some(Event { t, trip, pos }) = heap.pop() {
            if t > window.end_s {
                break;
            }
            let route = &routes[trip as usize];
            let node = route[pos as usize];
            let ready = t + graph.free_s(node);
            let mut exit = ready.max(last_exit[node as usize] + self.headway_s);
            let next = route.get(pos as usize + 1).copied();
            if let Some(n) = next
                && let Some(gate) = graph.gate(n)
            {
                exit = gate.next_green(exit);
            }
            last_exit[node as usize] = exit;
            times.record(node, t, exit - t);
            match next {
                Some(_) => heap.push(Event {
                    t: exit,
                    trip,
                    pos: pos + 1,
                }),
                None => {
                    if exit <= window.end_s {
                        experienced[trip as usize] = exit - trips[trip as usize].depart_s;
                        arrived += 1;
                    }
                }
            }
        }
        Ok(Loading {
            times,
            experienced_s: experienced,
            arrived,
        })
    }
}

fn card(saturation: f64) -> ModelCard {
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "Event-driven point-queue network loading: free-flow traversal, FIFO discharge at \
         the saturation flow, signal-controlled movements held to their green windows.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium];
    card.equations = vec![Equation::new(
        "lane exit",
        "exit = max(t_in + L/v_limit, last_exit + 3600/s), then the next green of the \
         downstream signal-controlled movement",
    )];
    card.parameters = vec![Parameter::new(
        "saturation_veh_per_h",
        "veh/h/lane",
        serde_json::json!(saturation),
        Source::new(
            SourceKind::Standard,
            "Highway Capacity Manual, 6th ed., TRB 2016, ch. 19: base saturation flow rate \
             1,900 pc/h/ln",
        ),
    )];
    card.assumptions = vec![
        "Queues have no physical length (a point queue): no spill-back, no gridlock."
            .to_string(),
        "A trip's lanes are the ones its route names; no lane changes.".to_string(),
    ];
    card.limitations = vec![
        "Unsignalised junction priority and turning conflicts are not modelled.".to_string(),
    ];
    card.sources = vec![
        Source::new(
            SourceKind::Paper,
            "W. S. Vickrey, Congestion theory and transport investment, American Economic \
             Review 59(2) 251-260 (1969)",
        ),
        Source::new(
            SourceKind::Paper,
            "C. Gawron, Simulation-based traffic assignment, PhD thesis, University of \
             Cologne 1998 (the queue model of the iterative DUE)",
        ),
        Source::new(
            SourceKind::Standard,
            "Highway Capacity Manual, 6th ed., Transportation Research Board 2016",
        ),
    ];
    card.determinism = Determinism {
        uses_rng: false,
        rng_domains: Vec::new(),
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec!["loader::queue::tests::a_bottleneck_discharges_at_saturation".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::ids::LaneId;
    use v2xw_world::ClassMask;

    #[test]
    fn a_bottleneck_discharges_at_saturation() {
        let w = crate::graph::tests::grid();
        let g = LaneGraph::new(&w, ClassMask::CAR);
        let n = g.endpoints(50.0)[0];
        let lane: LaneId = g.lane(n);
        // 100 vehicles all entering one lane at t = 0: the last leaves 99 headways after
        // the first.
        let trips: Vec<Trip> = (0..100)
            .map(|i| Trip {
                id: i,
                depart_s: 0.0,
                origin: lane,
                destination: lane,
                class: Default::default(),
                purpose: None,
                tag: None,
            })
            .collect();
        let routes = vec![vec![n]; 100];
        let mut q = PointQueue::new(1800.0);
        let l = q
            .load(
                &w,
                &g,
                &trips,
                &routes,
                Window {
                    begin_s: 0.0,
                    end_s: 3600.0,
                    bin_s: 900.0,
                },
            )
            .expect("loads");
        assert_eq!(l.arrived, 100);
        let first = l.experienced_s[0];
        let last = l.experienced_s[99];
        assert!((first - g.free_s(n)).abs() < 1e-9);
        assert!((last - first - 99.0 * 2.0).abs() < 1e-6, "{first} {last}");
        q.card().validate().expect("card");
    }
}
