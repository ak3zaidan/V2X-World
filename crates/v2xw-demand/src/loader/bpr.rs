//! `demand/loader/bpr` — static link performance functions.
//!
//! `t(n) = t₀(n) · (1 + α (v(n)/c(n))^β)` with `v` the lane's hourly flow over the window
//! and `c` its capacity: the saturation flow, times the green ratio `g/C` for a
//! signal-controlled connector. α = 0.15 and β = 4 are the Bureau of Public Roads' values
//! [U.S. Bureau of Public Roads, *Traffic Assignment Manual*, 1964]. Static: one interval,
//! the same cost all window long. A trip's experienced time is the sum of its lanes' costs.

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
pub const MODEL_ID: &str = "demand/loader/bpr";

/// BPR α.
pub fn default_alpha() -> f64 {
    0.15
}

/// BPR β.
pub fn default_beta() -> f64 {
    4.0
}

/// The BPR loader.
#[derive(Debug, Clone)]
pub struct Bpr {
    saturation: f64,
    alpha: f64,
    beta: f64,
    card: ModelCard,
}

impl Bpr {
    /// The loader.
    pub fn new(saturation: f64, alpha: f64, beta: f64) -> Self {
        let saturation = if saturation > 0.0 { saturation } else { 1900.0 };
        Self {
            saturation,
            alpha,
            beta,
            card: card(saturation, alpha, beta),
        }
    }

    /// The BPR cost of `node` at hourly flow `v`.
    pub fn cost_s(&self, graph: &LaneGraph, node: Node, v_per_h: f64) -> f64 {
        let lambda = graph.gate(node).map_or(1.0, |g| g.green_ratio().max(0.05));
        let c = self.saturation * lambda;
        graph.free_cost_s(node) * (1.0 + self.alpha * v2xw_core::math::pow(v_per_h / c, self.beta))
    }
}

impl NetworkLoader for Bpr {
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
        let hours = ((window.end_s - window.begin_s) / 3600.0).max(1e-9);
        let mut volume = vec![0u32; graph.len()];
        for r in routes {
            for n in r {
                volume[*n as usize] += 1;
            }
        }
        // One interval spanning the window: static costs.
        let span = (window.end_s - window.begin_s).max(1.0);
        let mut times = TravelTimes::new(graph, window.begin_s, window.end_s, span);
        let cost: Vec<f64> = (0..graph.len() as Node)
            .map(|n| self.cost_s(graph, n, f64::from(volume[n as usize]) / hours))
            .collect();
        for n in 0..graph.len() as Node {
            if volume[n as usize] > 0 {
                times.record(n, window.begin_s, cost[n as usize]);
            }
        }
        let experienced: Vec<f64> = routes
            .iter()
            .map(|r| v2xw_core::math::sum_ordered(r.iter().map(|n| cost[*n as usize])))
            .collect();
        let _ = trips;
        Ok(Loading {
            times,
            arrived: experienced.len(),
            experienced_s: experienced,
        })
    }
}

fn card(saturation: f64, alpha: f64, beta: f64) -> ModelCard {
    let bpr = Source::new(
        SourceKind::Standard,
        "U.S. Bureau of Public Roads, Traffic Assignment Manual, U.S. Dept. of Commerce 1964 \
         (link performance function, alpha 0.15, beta 4)",
    );
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "Static BPR link performance: t = t0 (1 + alpha (v/c)^beta), capacity the \
         saturation flow times the green ratio.",
    );
    card.tier = vec![Tier::Abstract];
    card.equations = vec![Equation::new("BPR", "t = t0 (1 + alpha (v/c)^beta)")];
    card.parameters = vec![
        Parameter::new(
            "saturation_veh_per_h",
            "veh/h/lane",
            serde_json::json!(saturation),
            Source::new(
                SourceKind::Standard,
                "Highway Capacity Manual, 6th ed., TRB 2016, ch. 19",
            ),
        ),
        Parameter::new("alpha", "-", serde_json::json!(alpha), bpr.clone()),
        Parameter::new("beta", "-", serde_json::json!(beta), bpr.clone()),
    ];
    card.limitations = vec!["Static: no time of day, no queues, no spill-back.".to_string()];
    card.sources = vec![bpr];
    card.determinism = Determinism {
        uses_rng: false,
        rng_domains: Vec::new(),
    };
    card.validation = Validation::new(ValidationStatus::UnitTested);
    card
}
