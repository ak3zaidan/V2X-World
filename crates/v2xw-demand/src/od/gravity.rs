//! `demand/od/gravity-furness` — a doubly-constrained gravity model.
//!
//! `T_ij = a_i b_j P_i A_j f(c_ij)`, with `P_i` the trips produced in zone `i`, `A_j` those
//! attracted to `j`, `c_ij` the free-flow travel time between the zones and `f` a
//! deterrence function; the balancing factors `a_i`, `b_j` are found by Furness's
//! alternating method so every row sums to `P_i` and every column to `A_j` [K. P. Furness,
//! "Time function iteration", Traffic Engineering & Control 7(7):458–460, 1965;
//! J. de D. Ortúzar, L. G. Willumsen, *Modelling Transport*, 4th ed., Wiley 2011, ch. 5].
//!
//! The deterrence functions are the three of Ortúzar & Willumsen §5.3: exponential
//! `e^{−βc}`, power `c^{−α}` and combined `c^{−α} e^{−βc}`. β is uncalibrated and the card
//! says how to calibrate it: Hyman's method, matching the mean modelled trip time to an
//! observed one [G. M. Hyman, "The calibration of trip distribution models", Environment
//! and Planning 1(1):105–112, 1969].
//!
//! Trip ends default to the zones' weights (floor area, [`super::zones`]) scaled to the
//! requested total; a study with real productions and attractions passes them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::math;
use v2xw_world::{ClassMask, World};

use super::OdMatrix;
use super::zones::ZoneSet;
use crate::costs::FreeFlow;
use crate::error::{DemandError, Result};
use crate::graph::LaneGraph;
use crate::tdsp::Search;

/// The model id.
pub const MODEL_ID: &str = "demand/od/gravity-furness";

/// The deterrence function.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "law", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Deterrence {
    /// `e^{−βc}`.
    Exponential {
        /// β, 1/s.
        beta_per_s: f64,
    },
    /// `c^{−α}`.
    Power {
        /// α.
        alpha: f64,
    },
    /// `c^{−α} e^{−βc}`.
    Combined {
        /// α.
        alpha: f64,
        /// β, 1/s.
        beta_per_s: f64,
    },
}

impl Default for Deterrence {
    fn default() -> Self {
        Deterrence::Exponential {
            beta_per_s: 1.0 / 600.0,
        }
    }
}

impl Deterrence {
    /// `f(c)` for a cost in seconds.
    pub fn f(self, c_s: f64) -> f64 {
        let c = c_s.max(1.0);
        match self {
            Deterrence::Exponential { beta_per_s } => math::exp(-beta_per_s * c),
            Deterrence::Power { alpha } => math::pow(c, -alpha),
            Deterrence::Combined { alpha, beta_per_s } => {
                math::pow(c, -alpha) * math::exp(-beta_per_s * c)
            }
        }
    }
}

/// The gravity model's parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GravityParams {
    /// Trips over the interval, all zones together.
    pub total_trips: f64,
    /// Interval start, seconds of the day.
    pub begin_s: f64,
    /// Interval end, seconds of the day.
    pub end_s: f64,
    /// The deterrence function.
    pub deterrence: Deterrence,
    /// Trip productions by zone; `None` uses the zone weights.
    pub productions: Option<BTreeMap<String, f64>>,
    /// Trip attractions by zone; `None` uses the zone weights.
    pub attractions: Option<BTreeMap<String, f64>>,
    /// Whether a zone sends trips to itself.
    pub intrazonal: bool,
    /// Furness iterations at most.
    pub max_iterations: u32,
    /// Stop when every row and column total is within this relative error.
    pub tolerance: f64,
}

impl Default for GravityParams {
    fn default() -> Self {
        Self {
            total_trips: 1000.0,
            begin_s: 0.0,
            end_s: 3600.0,
            deterrence: Deterrence::default(),
            productions: None,
            attractions: None,
            intrazonal: false,
            max_iterations: 100,
            tolerance: 1e-6,
        }
    }
}

/// How the balancing went.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GravityReport {
    /// Furness iterations run.
    pub iterations: u32,
    /// The largest relative row or column error at the end.
    pub max_error: f64,
}

/// Synthesises a matrix over `zones` (see the module docs).
///
/// # Errors
/// [`DemandError::Invalid`] when the trip ends cannot be balanced (a zone with trips to
/// send and nowhere reachable to send them).
pub fn gravity(
    world: &World,
    zones: &ZoneSet,
    p: &GravityParams,
) -> Result<(OdMatrix, GravityReport)> {
    let names: Vec<String> = zones.names().map(str::to_string).collect();
    let n = names.len();
    let weights: Vec<f64> = zones.zones().map(|z| z.weight()).collect();
    let wsum = math::sum_ordered(weights.iter().copied()).max(1e-12);
    let ends = |given: &Option<BTreeMap<String, f64>>| -> Vec<f64> {
        match given {
            Some(m) => names
                .iter()
                .map(|z| m.get(z).copied().unwrap_or(0.0))
                .collect(),
            None => weights.iter().map(|w| p.total_trips * w / wsum).collect(),
        }
    };
    let prod = ends(&p.productions);
    let mut attr = ends(&p.attractions);
    // The totals must agree; attractions are scaled to the productions, the usual
    // convention [Ortúzar & Willumsen §5.2].
    let (ps, as_) = (
        math::sum_ordered(prod.iter().copied()),
        math::sum_ordered(attr.iter().copied()),
    );
    if as_ > 0.0 {
        for a in &mut attr {
            *a *= ps / as_;
        }
    }
    // Zone-to-zone free-flow times between each zone's heaviest lane.
    let graph = LaneGraph::new(world, ClassMask::CAR);
    let rep: Vec<Option<u32>> = zones
        .zones()
        .map(|z| {
            z.lanes
                .iter()
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
                .and_then(|(l, _)| graph.node(*l))
        })
        .collect();
    let mut f = vec![0.0; n * n];
    let mut search = Search::new(&graph);
    let free = FreeFlow(&graph);
    for i in 0..n {
        let Some(o) = rep[i] else { continue };
        let targets: Vec<u32> = rep.iter().flatten().copied().collect();
        search.run(&graph, &free, o, 0.0, &targets);
        for j in 0..n {
            if i == j && !p.intrazonal {
                continue;
            }
            if let Some(d) = rep[j]
                && let Some(c) = search.cost_to(d)
            {
                f[i * n + j] = p.deterrence.f(c);
            }
        }
    }
    let mut a = vec![1.0; n];
    let mut b = vec![1.0; n];
    let mut report = GravityReport {
        iterations: 0,
        max_error: f64::INFINITY,
    };
    for it in 1..=p.max_iterations.max(1) {
        for i in 0..n {
            let s = math::sum_ordered((0..n).map(|j| b[j] * attr[j] * f[i * n + j]));
            a[i] = if s > 0.0 { 1.0 / s } else { 0.0 };
        }
        for j in 0..n {
            let s = math::sum_ordered((0..n).map(|i| a[i] * prod[i] * f[i * n + j]));
            b[j] = if s > 0.0 { 1.0 / s } else { 0.0 };
        }
        let t = |i: usize, j: usize| a[i] * b[j] * prod[i] * attr[j] * f[i * n + j];
        let mut err: f64 = 0.0;
        for i in 0..n {
            let row = math::sum_ordered((0..n).map(|j| t(i, j)));
            if prod[i] > 0.0 {
                err = err.max((row - prod[i]).abs() / prod[i]);
            }
        }
        for j in 0..n {
            let col = math::sum_ordered((0..n).map(|i| t(i, j)));
            if attr[j] > 0.0 {
                err = err.max((col - attr[j]).abs() / attr[j]);
            }
        }
        report = GravityReport {
            iterations: it,
            max_error: err,
        };
        if err <= p.tolerance {
            break;
        }
    }
    if !report.max_error.is_finite() {
        return Err(DemandError::Invalid(
            "the gravity model could not balance: a zone has trips and no reachable zone"
                .to_string(),
        ));
    }
    let mut m = OdMatrix::new(p.begin_s, p.end_s);
    for i in 0..n {
        for j in 0..n {
            let v = a[i] * b[j] * prod[i] * attr[j] * f[i * n + j];
            if v > 0.0 {
                m.add(&names[i], &names[j], v);
            }
        }
    }
    Ok((m, report))
}

/// The model card.
pub fn card(p: &GravityParams) -> ModelCard {
    let ow = Source::new(
        SourceKind::Paper,
        "J. de D. Ortuzar, L. G. Willumsen, Modelling Transport, 4th ed., Wiley 2011, ch. 5",
    );
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "Doubly-constrained gravity trip distribution balanced by Furness's method.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![Equation::new("gravity", "T_ij = a_i b_j P_i A_j f(c_ij)")];
    let mut det = Parameter::new(
        "deterrence",
        "-",
        serde_json::to_value(p.deterrence).unwrap_or_default(),
        Source::todo_calibrate("deterrence parameter"),
    );
    det.calibration = Some(
        "Hyman (1969): adjust beta until the mean modelled trip time equals the observed \
         mean from a household travel survey for the city"
            .to_string(),
    );
    card.parameters = vec![det];
    card.sources = vec![
        Source::new(
            SourceKind::Paper,
            "K. P. Furness, Time function iteration, Traffic Engineering & Control 7(7) \
             458-460 (1965)",
        ),
        ow,
        Source::new(
            SourceKind::Paper,
            "G. M. Hyman, The calibration of trip distribution models, Environment and \
             Planning 1(1) 105-112 (1969)",
        ),
    ];
    card.determinism = Determinism {
        uses_rng: false,
        rng_domains: Vec::new(),
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec!["od::gravity::tests::rows_and_columns_balance".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_columns_balance() {
        let w = crate::graph::tests::grid();
        let zones = ZoneSet::grid(&w, 3, 3, ClassMask::CAR).expect("zones");
        let p = GravityParams::default();
        let (m, r) = gravity(&w, &zones, &p).expect("balances");
        assert!(r.max_error < 1e-4, "{r:?}");
        assert!((m.total() - 1000.0).abs() < 0.5, "{}", m.total());
        card(&p).validate().expect("card");
    }
}
