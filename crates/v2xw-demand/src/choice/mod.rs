//! Route choice: how a driver's probabilities over their known routes move from one
//! iteration of the assignment to the next.
//!
//! * [`gawron`] — `demand/choice/gawron`: Gawron's pairwise update with exponentially
//!   smoothed route costs [Gawron 1998], SUMO `duarouter`'s default.
//! * [`logit`] — `demand/choice/logit`: multinomial logit [Ben-Akiva & Lerman 1985], with
//!   the C-logit commonality correction for overlapping routes [Cascetta et al. 1996] on
//!   by default.
//! * [`msa`] — `demand/choice/msa`: the method of successive averages, which moves `1/k` of
//!   each driver onto the current best route at iteration `k` [Sheffi 1985, ch. 12].
//!
//! # How to add another
//!
//! Implement [`RouteChoice`] in a new file here: [`RouteChoice::update`] gets the
//! alternatives with their newly measured costs and must leave probabilities that sum to
//! one. Give it a model card with its source, add a variant to [`ChoiceSpec`] and a line in
//! [`ChoiceSpec::build`], and it is selectable from the scenario
//! (`params.assign.choice.model`) and from `v2xw-demand assign --choice`.

pub mod gawron;
pub mod logit;
pub mod msa;

use serde::{Deserialize, Serialize};
use v2xw_core::card::ModelCard;
use v2xw_core::ids::EdgeId;

use crate::graph::Node;

/// One route a driver knows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alternative {
    /// The node path.
    pub path: Vec<Node>,
    /// Its identity: the non-internal edges it drives ([`crate::graph::LaneGraph::signature`]).
    pub signature: Vec<EdgeId>,
    /// Its length, metres.
    pub length_m: f64,
    /// The cost the choice model holds for it, seconds (smoothed by Gawron's update).
    pub cost_s: f64,
    /// The probability of choosing it.
    pub probability: f64,
}

/// What [`RouteChoice::update`] is told about this iteration.
#[derive(Debug, Clone, Copy)]
pub struct ChoiceStep {
    /// The iteration, from 1.
    pub iteration: u32,
    /// The index of this iteration's best route (the one just found by the
    /// time-dependent search) in the alternatives.
    pub best: usize,
    /// True if the best route was not in the set before this iteration.
    pub best_is_new: bool,
}

/// A route-choice model.
pub trait RouteChoice: Send + Sync {
    /// Its card.
    fn card(&self) -> &ModelCard;

    /// Updates the costs and probabilities of `alts` given each one's newly measured cost
    /// `measured[i]`, seconds. On return the probabilities are non-negative and sum to 1.
    fn update(&self, alts: &mut [Alternative], measured: &[f64], step: ChoiceStep);
}

/// The route-choice model a scenario names.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ChoiceSpec {
    /// [`gawron::Gawron`].
    Gawron {
        /// The cost-smoothing weight β of an alternative that is not the current best.
        #[serde(default = "gawron::default_beta")]
        beta: f64,
        /// The sensitivity `a` of the pairwise update.
        #[serde(default = "gawron::default_a")]
        a: f64,
    },
    /// [`logit::Logit`].
    Logit {
        /// The dispersion θ, 1/s; `None` sets it per driver from the shortest route's cost
        /// ([`logit::Logit`]).
        #[serde(default)]
        theta_per_s: Option<f64>,
        /// The perception-error coefficient of variation the automatic θ assumes.
        #[serde(default = "logit::default_cv")]
        perception_cv: f64,
        /// The C-logit commonality weight β; `0` is plain multinomial logit.
        #[serde(default = "logit::default_cf_beta")]
        commonality_beta: f64,
        /// The C-logit commonality exponent γ.
        #[serde(default = "logit::default_cf_gamma")]
        commonality_gamma: f64,
    },
    /// [`msa::Msa`].
    Msa,
}

impl Default for ChoiceSpec {
    fn default() -> Self {
        ChoiceSpec::Gawron {
            beta: gawron::default_beta(),
            a: gawron::default_a(),
        }
    }
}

impl ChoiceSpec {
    /// The model.
    pub fn build(self) -> Box<dyn RouteChoice> {
        match self {
            ChoiceSpec::Gawron { beta, a } => Box::new(gawron::Gawron::new(beta, a)),
            ChoiceSpec::Logit {
                theta_per_s,
                perception_cv,
                commonality_beta,
                commonality_gamma,
            } => Box::new(logit::Logit::new(
                theta_per_s,
                perception_cv,
                commonality_beta,
                commonality_gamma,
            )),
            ChoiceSpec::Msa => Box::new(msa::Msa::new()),
        }
    }

    /// The model's label.
    pub fn label(&self) -> &'static str {
        match self {
            ChoiceSpec::Gawron { .. } => "gawron",
            ChoiceSpec::Logit { .. } => "logit",
            ChoiceSpec::Msa => "msa",
        }
    }
}

/// Rescales probabilities to sum to one; a set whose probabilities are all zero (or not
/// finite) becomes uniform.
pub fn normalise(alts: &mut [Alternative]) {
    for a in alts.iter_mut() {
        if !(a.probability.is_finite() && a.probability > 0.0) {
            a.probability = 0.0;
        }
    }
    let sum = v2xw_core::math::sum_ordered(alts.iter().map(|a| a.probability));
    if sum > 0.0 {
        for a in alts.iter_mut() {
            a.probability /= sum;
        }
    } else if !alts.is_empty() {
        let p = 1.0 / alts.len() as f64;
        for a in alts.iter_mut() {
            a.probability = p;
        }
    }
}

#[cfg(test)]
pub(crate) fn alt(cost: f64, p: f64) -> Alternative {
    Alternative {
        path: Vec::new(),
        signature: Vec::new(),
        length_m: 100.0,
        cost_s: cost,
        probability: p,
    }
}
