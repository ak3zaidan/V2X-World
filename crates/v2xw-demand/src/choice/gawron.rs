//! `demand/choice/gawron` — Gawron's route-choice update [Gawron 1998].
//!
//! Each driver keeps a cost and a probability per known route. After a loading:
//!
//! 1. **New route.** If this iteration's best route is new it joins the set with
//!    probability `1/n` and every other probability is scaled by `(n − 1)/n`.
//! 2. **Costs.** The best route's cost becomes its measured cost; every other route's cost
//!    is smoothed, `c ← β·measured + (1 − β)·c`.
//! 3. **Probabilities.** For every pair `(r, s)`, with `δ = (c_s − c_r)/(c_s + c_r)`,
//!    `p_r' = p_r (p_r + p_s) g / (p_r g + p_s)` where `g = exp(a δ / (1 − δ²))`, and
//!    `p_s' = p_r + p_s − p_r'` — probability flows towards the cheaper route of each pair
//!    and the pair's total is conserved.
//!
//! This is the update of C. Gawron, "An iterative algorithm to determine the dynamic user
//! equilibrium in a traffic simulation model", Int. J. Mod. Phys. C 9(3):393–407 (1998),
//! doi:10.1142/S0129183198000303, in the form SUMO's `duarouter` implements it
//! (`ROGawronCalculator`, `RORouteDef::addAlternative`), whose defaults β = 0.3 and
//! a = 0.05 are kept.

use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::math;

use super::{Alternative, ChoiceStep, RouteChoice, normalise};

/// The model id.
pub const MODEL_ID: &str = "demand/choice/gawron";

/// SUMO's `--gawron.beta` default.
pub fn default_beta() -> f64 {
    0.3
}

/// SUMO's `--gawron.a` default.
pub fn default_a() -> f64 {
    0.05
}

/// Gawron's update.
#[derive(Debug, Clone)]
pub struct Gawron {
    beta: f64,
    a: f64,
    card: ModelCard,
}

impl Gawron {
    /// The model with smoothing weight `beta` and sensitivity `a`.
    pub fn new(beta: f64, a: f64) -> Self {
        let beta = beta.clamp(0.0, 1.0);
        Self {
            beta,
            a,
            card: card(beta, a),
        }
    }
}

fn g(a: f64, x: f64) -> f64 {
    let d = 1.0 - x * x;
    if d <= 0.0 {
        return f64::MAX;
    }
    math::exp(a * x / d)
}

fn f(pr: f64, ps: f64, x: f64, a: f64) -> f64 {
    let gx = g(a, x);
    let den = pr * gx + ps;
    if den == 0.0 || !den.is_finite() {
        // One of the pair has all the probability, or the exponential overflowed: the
        // cheaper route takes the pair's whole mass.
        return if x > 0.0 { pr + ps } else { pr };
    }
    pr * (pr + ps) * gx / den
}

impl RouteChoice for Gawron {
    fn card(&self) -> &ModelCard {
        &self.card
    }

    fn update(&self, alts: &mut [Alternative], measured: &[f64], step: ChoiceStep) {
        let n = alts.len();
        if n == 0 {
            return;
        }
        if step.best_is_new && n > 1 {
            let scale = (n - 1) as f64 / n as f64;
            for (i, a) in alts.iter_mut().enumerate() {
                a.probability = if i == step.best {
                    1.0 / n as f64
                } else {
                    a.probability * scale
                };
            }
        }
        for (i, a) in alts.iter_mut().enumerate() {
            let m = measured[i];
            a.cost_s = if i == step.best || !a.cost_s.is_finite() {
                m
            } else {
                self.beta * m + (1.0 - self.beta) * a.cost_s
            };
        }
        for r in 0..n {
            for s in (r + 1)..n {
                let (cr, cs) = (alts[r].cost_s, alts[s].cost_s);
                let sum = cr + cs;
                let delta = if sum > 0.0 { (cs - cr) / sum } else { 0.0 };
                let (pr, ps) = (alts[r].probability, alts[s].probability);
                let new_pr = f(pr, ps, delta, self.a).clamp(0.0, pr + ps);
                alts[r].probability = new_pr;
                alts[s].probability = pr + ps - new_pr;
            }
        }
        normalise(alts);
    }
}

fn card(beta: f64, a: f64) -> ModelCard {
    let paper = Source {
        kind: SourceKind::Paper,
        reference: "C. Gawron, An iterative algorithm to determine the dynamic user \
                    equilibrium in a traffic simulation model, Int. J. Mod. Phys. C 9(3) \
                    393-407 (1998), doi:10.1142/S0129183198000303"
            .to_string(),
        accessed: Some("2026-10-07".to_string()),
        note: None,
    };
    let sumo = Source {
        kind: SourceKind::Code,
        reference: "SUMO duarouter: ROGawronCalculator and RORouteDef::addAlternative; \
                    options --gawron.beta (default 0.3) and --gawron.a (default 0.05) \
                    (sumo.dlr.de/docs/duarouter.html)"
            .to_string(),
        accessed: Some("2026-10-07".to_string()),
        note: None,
    };
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "Gawron's route-choice update for iterative dynamic user equilibrium: smoothed \
         route costs and a pairwise probability transfer towards the cheaper route.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new("cost smoothing", "c_r <- beta*T_r + (1-beta)*c_r (r not the best)"),
        Equation::new(
            "pairwise update",
            "p_r' = p_r (p_r+p_s) g / (p_r g + p_s), g = exp(a d/(1-d^2)), d = (c_s-c_r)/(c_s+c_r)",
        ),
    ];
    card.parameters = vec![
        Parameter::new("beta", "-", serde_json::json!(beta), sumo.clone()),
        Parameter::new("a", "-", serde_json::json!(a), sumo.clone()),
    ];
    card.assumptions = vec![
        "Drivers know only the routes they have found in earlier iterations (at most \
         max_alternatives of them)."
            .to_string(),
    ];
    card.limitations = vec![
        "Probabilities move by a fixed sensitivity a; convergence is to a stochastic \
         equilibrium, not Wardrop's deterministic one."
            .to_string(),
    ];
    card.sources = vec![paper, sumo];
    card.determinism = Determinism {
        uses_rng: false,
        rng_domains: Vec::new(),
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "choice::gawron::tests::probability_moves_to_the_cheaper_route".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::alt;

    #[test]
    fn probability_moves_to_the_cheaper_route() {
        let m = Gawron::new(0.3, 0.05);
        let mut alts = vec![alt(100.0, 0.5), alt(200.0, 0.5)];
        let step = ChoiceStep {
            iteration: 2,
            best: 0,
            best_is_new: false,
        };
        let mut last = 0.5;
        for _ in 0..50 {
            m.update(&mut alts, &[100.0, 200.0], step);
            let p: f64 = alts.iter().map(|a| a.probability).sum();
            assert!((p - 1.0).abs() < 1e-12);
            assert!(alts[0].probability >= last, "monotone towards the cheaper route");
            last = alts[0].probability;
        }
        assert!(alts[0].probability > 0.9, "{}", alts[0].probability);
        m.card().validate().expect("card");
    }

    #[test]
    fn equal_costs_leave_probabilities_alone() {
        let m = Gawron::new(0.3, 0.05);
        let mut alts = vec![alt(100.0, 0.3), alt(100.0, 0.7)];
        m.update(
            &mut alts,
            &[100.0, 100.0],
            ChoiceStep {
                iteration: 2,
                best: 0,
                best_is_new: false,
            },
        );
        assert!((alts[0].probability - 0.3).abs() < 1e-12);
    }

    #[test]
    fn a_new_route_starts_at_one_over_n() {
        let m = Gawron::new(0.3, 0.0); // a = 0: no transfer, so the scaling is visible
        let mut alts = vec![alt(100.0, 1.0), alt(90.0, 0.0)];
        m.update(
            &mut alts,
            &[100.0, 90.0],
            ChoiceStep {
                iteration: 2,
                best: 1,
                best_is_new: true,
            },
        );
        assert!((alts[1].probability - 0.5).abs() < 1e-12);
    }
}
