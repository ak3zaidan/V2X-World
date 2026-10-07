//! `demand/choice/logit` — multinomial logit route choice with the C-logit commonality
//! correction.
//!
//! `p_r = exp(−θ c_r − β CF_r) / Σ_s exp(−θ c_s − β CF_s)`, where the commonality factor
//! `CF_r = ln Σ_s (L_rs / √(L_r L_s))^γ` (a utility, weighted by β) charges a route for the length `L_rs` it shares
//! with every route of the set (itself included, so a route overlapping nothing has
//! `CF = 0`). Plain multinomial logit [M. Ben-Akiva, S. Lerman, *Discrete Choice
//! Analysis*, MIT Press 1985] treats two routes that share all but one block as independent
//! and over-loads the corridor they share; C-logit [E. Cascetta, A. Nuzzolo, F. Russo,
//! A. Vitetta, "A modified logit route choice model overcoming path overlapping problems",
//! Proc. 13th ISTTT, Lyon 1996, pp. 697–711] is the correction SUMO's `--route-choice-method
//! logit` also applies.
//!
//! **The dispersion θ.** A Gumbel perception error with scale θ has standard deviation
//! `π / (√6 θ)`. With `theta_per_s` unset, θ is chosen per driver so that this standard
//! deviation is `perception_cv` times the cheapest known route's cost — perception error
//! proportional to trip length, the assumption of Daganzo & Sheffi's stochastic assignment
//! [C. Daganzo, Y. Sheffi, "On stochastic models of traffic assignment", Transp. Sci.
//! 11(3):253–274, 1977]. The default `perception_cv = 0.1` is uncalibrated and the card
//! says so.
//!
//! Each iteration's probabilities are averaged with the previous ones by the method of
//! successive averages (`p ← p + (p_logit − p)/k`), which is what turns a logit model
//! inside a simulation loop into a convergent stochastic user equilibrium [Sheffi 1985,
//! *Urban Transportation Networks*, §12.2] rather than an oscillation.

use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::math;

use super::{Alternative, ChoiceStep, RouteChoice, normalise};

/// The model id.
pub const MODEL_ID: &str = "demand/choice/logit";

/// The default perception-error coefficient of variation.
pub fn default_cv() -> f64 {
    0.1
}

/// The default commonality weight: Cascetta et al.'s β = 1.
pub fn default_cf_beta() -> f64 {
    1.0
}

/// The default commonality exponent: Cascetta et al.'s γ = 1.
pub fn default_cf_gamma() -> f64 {
    1.0
}

/// Multinomial / C-logit.
#[derive(Debug, Clone)]
pub struct Logit {
    theta_per_s: Option<f64>,
    cv: f64,
    cf_beta: f64,
    cf_gamma: f64,
    card: ModelCard,
}

impl Logit {
    /// The model.
    pub fn new(theta_per_s: Option<f64>, cv: f64, cf_beta: f64, cf_gamma: f64) -> Self {
        let cv = if cv > 0.0 { cv } else { default_cv() };
        Self {
            card: card(theta_per_s, cv, cf_beta, cf_gamma),
            theta_per_s,
            cv,
            cf_beta,
            cf_gamma,
        }
    }

    /// The logit probabilities of `alts` at their current costs.
    pub fn probabilities(&self, alts: &[Alternative]) -> Vec<f64> {
        let n = alts.len();
        if n == 0 {
            return Vec::new();
        }
        let cmin = alts
            .iter()
            .map(|a| a.cost_s)
            .fold(f64::INFINITY, f64::min)
            .max(1.0);
        let theta = self.theta_per_s.unwrap_or_else(|| {
            std::f64::consts::PI / (math::sqrt(6.0) * self.cv * cmin)
        });
        let cf: Vec<f64> = (0..n)
            .map(|r| {
                if self.cf_beta == 0.0 {
                    return 0.0;
                }
                let terms: Vec<f64> = (0..n)
                    .map(|s| {
                        let shared = shared_length(&alts[r], &alts[s]);
                        let denom = math::sqrt(alts[r].length_m * alts[s].length_m);
                        if denom > 0.0 {
                            math::pow(shared / denom, self.cf_gamma)
                        } else {
                            0.0
                        }
                    })
                    .collect();
                math::ln(math::sum_ordered(terms).max(1.0))
            })
            .collect();
        // Utilities relative to the best, so the exponentials cannot overflow.
        let u: Vec<f64> = (0..n).map(|r| -theta * alts[r].cost_s - self.cf_beta * cf[r]).collect();
        let umax = u.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let e: Vec<f64> = u.iter().map(|x| math::exp(x - umax)).collect();
        let sum = math::sum_ordered(e.iter().copied());
        e.iter().map(|x| x / sum).collect()
    }
}

/// The length two alternatives share, metres, as the overlap of their edge signatures
/// weighted by the length each shared edge has on `a`.
fn shared_length(a: &Alternative, b: &Alternative) -> f64 {
    if a.signature.is_empty() || b.signature.is_empty() {
        return if std::ptr::eq(a, b) { a.length_m } else { 0.0 };
    }
    let mut theirs = b.signature.clone();
    theirs.sort_unstable();
    let shared = a
        .signature
        .iter()
        .filter(|e| theirs.binary_search(e).is_ok())
        .count();
    a.length_m * shared as f64 / a.signature.len() as f64
}

impl RouteChoice for Logit {
    fn card(&self) -> &ModelCard {
        &self.card
    }

    fn update(&self, alts: &mut [Alternative], measured: &[f64], step: ChoiceStep) {
        for (a, m) in alts.iter_mut().zip(measured) {
            a.cost_s = *m;
        }
        if step.best_is_new {
            alts[step.best].probability = 0.0;
        }
        let target = self.probabilities(alts);
        let k = f64::from(step.iteration.max(1));
        for (a, p) in alts.iter_mut().zip(target) {
            a.probability += (p - a.probability) / k;
        }
        normalise(alts);
    }
}

fn card(theta: Option<f64>, cv: f64, cf_beta: f64, cf_gamma: f64) -> ModelCard {
    let mnl = Source::new(
        SourceKind::Paper,
        "M. Ben-Akiva, S. Lerman, Discrete Choice Analysis: Theory and Application to \
         Travel Demand, MIT Press 1985",
    );
    let clogit = Source::new(
        SourceKind::Paper,
        "E. Cascetta, A. Nuzzolo, F. Russo, A. Vitetta, A modified logit route choice model \
         overcoming path overlapping problems, Proc. 13th ISTTT, Lyon 1996, 697-711",
    );
    let ds = Source::new(
        SourceKind::Paper,
        "C. Daganzo, Y. Sheffi, On stochastic models of traffic assignment, Transportation \
         Science 11(3) 253-274 (1977)",
    );
    let msa = Source::new(
        SourceKind::Paper,
        "Y. Sheffi, Urban Transportation Networks, Prentice-Hall 1985, section 12.2 (MSA \
         for stochastic user equilibrium)",
    );
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "Multinomial logit route choice with the C-logit commonality factor, averaged over \
         iterations by successive averages into a stochastic user equilibrium.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new(
            "C-logit",
            "p_r = exp(-theta c_r - beta CF_r) / sum_s exp(-theta c_s - beta CF_s)",
        ),
        Equation::new(
            "commonality",
            "CF_r = ln sum_s (L_rs / sqrt(L_r L_s))^gamma",
        ),
        Equation::new(
            "automatic dispersion",
            "theta = pi / (sqrt(6) cv c_min)",
        ),
    ];
    let mut cv_param = Parameter::new(
        "perception_cv",
        "-",
        serde_json::json!(cv),
        Source::todo_calibrate("perception-error coefficient of variation"),
    );
    cv_param.calibration = Some(
        "fit to observed route splits between two corridors of a city with count data \
         (the counts module's GEH check against an assignment with this model)"
            .to_string(),
    );
    card.parameters = vec![
        Parameter::new(
            "theta_per_s",
            "1/s",
            serde_json::json!(theta),
            ds.clone(),
        ),
        cv_param,
        Parameter::new(
            "commonality_beta",
            "-",
            serde_json::json!(cf_beta),
            clogit.clone(),
        ),
        Parameter::new(
            "commonality_gamma",
            "-",
            serde_json::json!(cf_gamma),
            clogit.clone(),
        ),
    ];
    card.assumptions = vec![
        "Route overlap is measured on non-internal edges, each weighted by the route's \
         average edge length."
            .to_string(),
    ];
    card.sources = vec![mnl, clogit, ds, msa];
    card.determinism = Determinism {
        uses_rng: false,
        rng_domains: Vec::new(),
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec!["choice::logit::tests::overlap_is_penalised".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::alt;
    use v2xw_core::ids::EdgeId;

    #[test]
    fn cheaper_is_likelier_and_probabilities_sum_to_one() {
        let m = Logit::new(Some(0.05), 0.1, 0.0, 1.0);
        let p = m.probabilities(&[alt(100.0, 0.5), alt(120.0, 0.5)]);
        assert!((p[0] + p[1] - 1.0).abs() < 1e-12);
        // exp(-0.05*100)/(exp(-5)+exp(-6)) = 1/(1+e^-1) = 0.7311
        assert!((p[0] - 0.731_058_578_6).abs() < 1e-6, "{p:?}");
        m.card().validate().expect("card");
    }

    #[test]
    fn overlap_is_penalised() {
        // Two routes sharing 9 of 10 edges and a third disjoint one, all equally costly:
        // plain logit gives each 1/3; C-logit gives the disjoint one more.
        let e = |ids: &[u32]| ids.iter().map(|i| EdgeId::new(*i)).collect::<Vec<_>>();
        let mut a = alt(100.0, 1.0 / 3.0);
        a.signature = e(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let mut b = alt(100.0, 1.0 / 3.0);
        b.signature = e(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 11]);
        let mut c = alt(100.0, 1.0 / 3.0);
        c.signature = e(&[20, 21, 22, 23, 24, 25, 26, 27, 28, 29]);
        let plain = Logit::new(Some(0.05), 0.1, 0.0, 1.0).probabilities(&[a.clone(), b.clone(), c.clone()]);
        assert!((plain[2] - 1.0 / 3.0).abs() < 1e-12);
        let cl = Logit::new(Some(0.05), 0.1, 1.0, 1.0).probabilities(&[a, b, c]);
        assert!(cl[2] > 0.4, "{cl:?}");
        assert!((cl[0] - cl[1]).abs() < 1e-12);
    }
}
