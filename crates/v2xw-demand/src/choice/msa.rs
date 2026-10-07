//! `demand/choice/msa` — the method of successive averages.
//!
//! At iteration `k` every driver moves `1/k` of their probability onto the route that is
//! currently best: `p ← (1 − 1/k)·p + (1/k)·[r = best]`. The step sizes satisfy
//! `Σ 1/k = ∞, Σ 1/k² < ∞`, the Robbins–Monro conditions under which the averaged flows
//! converge to the equilibrium [Y. Sheffi, *Urban Transportation Networks*, Prentice-Hall
//! 1985, ch. 12; H. Robbins, S. Monro, "A stochastic approximation method", Ann. Math.
//! Statist. 22(3):400–407, 1951]. With deterministic costs it approaches Wardrop's user
//! equilibrium [J. G. Wardrop, "Some theoretical aspects of road traffic research", Proc.
//! ICE Part II 1(3):325–378, 1952].

use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};

use super::{Alternative, ChoiceStep, RouteChoice, normalise};

/// The model id.
pub const MODEL_ID: &str = "demand/choice/msa";

/// Successive averages.
#[derive(Debug, Clone)]
pub struct Msa {
    card: ModelCard,
}

impl Default for Msa {
    fn default() -> Self {
        Self::new()
    }
}

impl Msa {
    /// The model.
    pub fn new() -> Self {
        let mut card = ModelCard::new(
            MODEL_ID,
            Family::Mobility,
            "1.0.0",
            "Method of successive averages: 1/k of each driver's choice moves to the \
             current best route at iteration k.",
        );
        card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
        card.equations = vec![Equation::new(
            "MSA",
            "p_r <- (1 - 1/k) p_r + (1/k) [r = best]",
        )];
        card.sources = vec![
            Source::new(
                SourceKind::Paper,
                "Y. Sheffi, Urban Transportation Networks, Prentice-Hall 1985, ch. 12",
            ),
            Source::new(
                SourceKind::Paper,
                "H. Robbins, S. Monro, A stochastic approximation method, Ann. Math. \
                 Statist. 22(3) 400-407 (1951)",
            ),
            Source::new(
                SourceKind::Paper,
                "J. G. Wardrop, Some theoretical aspects of road traffic research, Proc. \
                 ICE Part II 1(3) 325-378 (1952)",
            ),
        ];
        card.determinism = Determinism {
            uses_rng: false,
            rng_domains: Vec::new(),
        };
        card.validation = Validation {
            status: ValidationStatus::UnitTested,
            references: Vec::new(),
            tests: vec!["choice::msa::tests::the_best_route_gains_one_over_k".to_string()],
        };
        Self { card }
    }
}

impl RouteChoice for Msa {
    fn card(&self) -> &ModelCard {
        &self.card
    }

    fn update(&self, alts: &mut [Alternative], measured: &[f64], step: ChoiceStep) {
        let k = f64::from(step.iteration.max(1));
        for (i, a) in alts.iter_mut().enumerate() {
            a.cost_s = measured[i];
            let target = if i == step.best { 1.0 } else { 0.0 };
            a.probability = (1.0 - 1.0 / k) * a.probability + target / k;
        }
        normalise(alts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::alt;

    #[test]
    fn the_best_route_gains_one_over_k() {
        let m = Msa::new();
        let mut alts = vec![alt(100.0, 1.0), alt(90.0, 0.0)];
        m.update(
            &mut alts,
            &[100.0, 90.0],
            ChoiceStep {
                iteration: 4,
                best: 1,
                best_is_new: true,
            },
        );
        assert!((alts[1].probability - 0.25).abs() < 1e-12);
        assert!((alts[0].probability - 0.75).abs() < 1e-12);
        m.card().validate().expect("card");
    }
}
