//! `propagation/winner-plus-b1` — the WINNER+ B1 (urban micro-cell) line-of-sight path
//! loss with the effective antenna heights, as 3GPP TR 36.885 adopted it for V2V.
//!
//! # Why this model exists
//!
//! The published 802.11p and LTE-V2X Mode 4 curves this engine is compared against —
//! Veins in Sepulcre et al. 2021 (arXiv:2104.07923), OpenCV2X in McCarthy et al. 2021
//! (arXiv:2103.13212), Molina-Masegosa 2017 — were all produced on WINNER+ B1, and the
//! cross-validation report (`docs/validation/cross-validation.md`) found this engine's
//! 802.11p 30 PDR points above Veins at 300–400 m while running a *stand-in* for it. The
//! stand-in (`sweep::ChannelModel::WinnerB1ShapedTodoCalibrate`) put the breakpoint at
//! `4·h·h/λ` with the *real* 1.5 m antenna heights — 177 m at 5.9 GHz. WINNER's breakpoint
//! uses *effective* heights `h' = h − 1.0 m` (the environment height is 1 m), which puts it
//! at `4·0.5·0.5·f/c` ≈ 20 m, so the 40 dB/decade slope starts nine times nearer and the
//! range at which 802.11p falls below its sensitivity is about 300 m instead of beyond
//! 500 m. That one constant is the difference between the two curves.
//!
//! # The law
//!
//! WINNER+ D5.3 (2010) Table 4-1, scenario B1, LOS; 3GPP TR 36.885 V14.0.0 Table A.1.4-1
//! (V2V: "WINNER+ B1 … antenna height 1.5 m … the pathloss at 3 m is used if the distance
//! is less than 3 m", fc = 6 GHz in the TR, the run's carrier here):
//!
//! ```text
//! d'BP = 4·h'_t·h'_r·f_c/c,   h' = h − 1.0 m
//! PL = 22.7·log10(d) + 27.0 + 20.0·log10(f_GHz)                          d ≤ d'BP
//! PL = 40.0·log10(d) + 7.56 − 17.3·log10(h'_t) − 17.3·log10(h'_r) + 2.7·log10(f_GHz)   d > d'BP
//! ```
//!
//! The two segments meet at `d'BP` to within 0.05 dB (the published constants are rounded),
//! which `the_two_segments_meet_at_the_breakpoint` holds. The same coefficients appear in
//! WINNER II D1.1.2 Table 4-4 as `41.0 + 20·log10(f/5)` and `9.45 + 2.7·log10(f/5)`, which
//! are the identical lines written about 5 GHz.
//!
//! Shadowing: log-normal, σ 3 dB on a line-of-sight link and 4 dB on a link the building
//! geometry blocks (TR 36.885 Table A.1.4-1), spatially correlated with the TR 36.885
//! A.1.4 decorrelation distance through the same [`ShadowProcess`] every other law uses.
//!
//! # Buildings
//!
//! B1's NLOS formula needs the two street legs of a Manhattan grid; this model does not
//! price a blocked link itself. It reports `owns_buildings = false` to the engine, so the
//! obstacle stack's Sommer 2011 term charges the walls — exactly how Veins composes a
//! distance law with `SimpleObstacleShadowing`. The card says so.

use std::collections::BTreeMap;

use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::ctx::Ctx;
use v2xw_core::ids::{LinkKey, NodeId};
use v2xw_core::math;
use v2xw_core::model::Model;
use v2xw_core::rng::{EntityRef, RngDomain};
use v2xw_core::weather::WeatherState;
use v2xw_world::model::EnvClass;

use crate::prop::{ShadowProcess, decorrelation_distance_m};
use crate::traits::Propagation;
use crate::tuning::ShadowingOverride;
use crate::types::{LosResult, LossBreakdown, RadioEndpoint};

/// The WINNER environment height subtracted from each antenna height, metres
/// [WINNER+ D5.3 Table 4-1 note; WINNER II D1.1.2 §4.3].
pub const ENVIRONMENT_HEIGHT_M: f64 = 1.0;

/// The smallest effective height the law is evaluated with, metres. An antenna at or
/// below the 1 m environment height has no defined `h'`; 0.1 m keeps the logarithms finite
/// and is stated on the card.
pub const MIN_EFFECTIVE_HEIGHT_M: f64 = 0.1;

/// The distance below which the 3 m loss is used [TR 36.885 Table A.1.4-1].
pub const MIN_DISTANCE_M: f64 = 3.0;

/// Shadowing σ on a line-of-sight link, dB [TR 36.885 Table A.1.4-1].
pub const SIGMA_LOS_DB: f64 = 3.0;

/// Shadowing σ on a building-blocked link, dB [TR 36.885 Table A.1.4-1].
pub const SIGMA_NLOS_DB: f64 = 4.0;

/// The effective breakpoint distance `d'BP = 4·h'_t·h'_r·f/c`, metres.
#[must_use]
pub fn breakpoint_m(h_t_m: f64, h_r_m: f64, f_hz: f64) -> f64 {
    let ht = (h_t_m - ENVIRONMENT_HEIGHT_M).max(MIN_EFFECTIVE_HEIGHT_M);
    let hr = (h_r_m - ENVIRONMENT_HEIGHT_M).max(MIN_EFFECTIVE_HEIGHT_M);
    4.0 * ht * hr * f_hz / crate::numeric::SPEED_OF_LIGHT_M_S
}

/// The WINNER+ B1 LOS path loss, dB, with the TR 36.885 3 m floor on the distance.
#[must_use]
pub fn b1_los_db(d_m: f64, h_t_m: f64, h_r_m: f64, f_hz: f64) -> f64 {
    let d = d_m.max(MIN_DISTANCE_M);
    let f_ghz = f_hz / 1e9;
    let d_bp = breakpoint_m(h_t_m, h_r_m, f_hz);
    if d <= d_bp {
        22.7 * math::log10(d) + 27.0 + 20.0 * math::log10(f_ghz)
    } else {
        let ht = (h_t_m - ENVIRONMENT_HEIGHT_M).max(MIN_EFFECTIVE_HEIGHT_M);
        let hr = (h_r_m - ENVIRONMENT_HEIGHT_M).max(MIN_EFFECTIVE_HEIGHT_M);
        40.0 * math::log10(d) + 7.56 - 17.3 * math::log10(ht) - 17.3 * math::log10(hr)
            + 2.7 * math::log10(f_ghz)
    }
}

/// `propagation/winner-plus-b1`.
#[derive(Debug, Clone)]
pub struct WinnerPlusB1 {
    card: ModelCard,
    tier: Tier,
    d_corr_m: f64,
    shadowing: ShadowingOverride,
    shadows: BTreeMap<(u32, u32), ShadowProcess>,
}

impl WinnerPlusB1 {
    /// The model's id.
    pub const ID: &'static str = "propagation/winner-plus-b1";

    /// The model at `tier` for an environment class (which sets the decorrelation
    /// distance: 10 m urban, 25 m highway).
    #[must_use]
    pub fn new(tier: Tier, env: EnvClass) -> Self {
        Self {
            card: card(),
            tier,
            d_corr_m: decorrelation_distance_m(env),
            shadowing: ShadowingOverride::default(),
            shadows: BTreeMap::new(),
        }
    }

    /// The same model with the shadowing σ and decorrelation distance overridden.
    #[must_use]
    pub fn with_shadowing(mut self, o: ShadowingOverride) -> Self {
        self.shadowing = o;
        self
    }
}

impl Model for WinnerPlusB1 {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl<C: Ctx + ?Sized> Propagation<C> for WinnerPlusB1 {
    fn tier(&self) -> Tier {
        self.tier
    }

    fn forget_links(&mut self, gone: &dyn Fn(NodeId, NodeId) -> bool) -> usize {
        let before = self.shadows.len();
        self.shadows
            .retain(|&(tx, rx), _| !gone(NodeId::new(tx), NodeId::new(rx)));
        before - self.shadows.len()
    }

    fn loss_db(
        &mut self,
        ctx: &mut C,
        tx: &RadioEndpoint,
        rx: &RadioEndpoint,
        f_hz: f64,
        los: &LosResult,
        _w: &WeatherState,
    ) -> LossBreakdown {
        let d = tx.pos.distance(rx.pos);
        let path = b1_los_db(d, tx.height_m(), rx.height_m(), f_hz);
        let base_sigma = if los.class.has_building() {
            SIGMA_NLOS_DB
        } else {
            SIGMA_LOS_DB
        };
        let sigma = self.shadowing.sigma(base_sigma);
        let d_corr = self.shadowing.d_corr(self.d_corr_m);
        let link = LinkKey(tx.node, rx.node);
        let draw = if sigma > 0.0 {
            ctx.rng(RngDomain::Shadow, EntityRef::Link(link))
                .normal(0.0, 1.0)
        } else {
            0.0
        };
        let shadow = self
            .shadows
            .entry((tx.node.index(), rx.node.index()))
            .or_default()
            .update(tx.pos, rx.pos, sigma, d_corr, draw);
        LossBreakdown::new(path, shadow, 0.0, 0.0, tx.gain_dbi + rx.gain_dbi)
    }
}

fn card() -> ModelCard {
    let winner = Source {
        kind: SourceKind::Standard,
        reference: "WINNER+ D5.3 'WINNER+ Final Channel Models' (2010) Table 4-1, B1 LOS \
                    (equivalently WINNER II D1.1.2 V1.2 Table 4-4 written about 5 GHz), as \
                    adopted for V2V by 3GPP TR 36.885 V14.0.0 Table A.1.4-1"
            .to_string(),
        accessed: Some("2026-10-07".to_string()),
        note: Some(
            "Coefficients as reprinted by the TR 36.885 evaluation methodology and the \
             LTEV2Vsim / WiLabV2Xsim implementations used by the Molina-Masegosa, Bazzi \
             and Todisco studies; effective heights h' = h − 1 m."
                .to_string(),
        ),
    };
    let tr = Source::new(
        SourceKind::Standard,
        "3GPP TR 36.885 V14.0.0 Table A.1.4-1: shadowing σ 3 dB LOS, 4 dB NLOS; 3 m \
         minimum distance; decorrelation per Annex A.1.4",
    );
    let mut card = ModelCard::new(
        WinnerPlusB1::ID,
        Family::Propagation,
        "1.0.0",
        "WINNER+ B1 line-of-sight path loss with effective antenna heights — the V2V \
         channel of 3GPP TR 36.885 and of the Veins, OpenCV2X and Molina-Masegosa studies \
         — with spatially correlated log-normal shadowing.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new("breakpoint", "d'BP = 4·(h_t − 1)·(h_r − 1)·f/c (≈ 19.7 m at 5.9 GHz, 1.5 m antennas)"),
        Equation::new(
            "path loss",
            "d ≤ d'BP: 22.7·log10(d) + 27.0 + 20·log10(f_GHz); d > d'BP: 40·log10(d) + 7.56 \
             − 17.3·log10(h'_t) − 17.3·log10(h'_r) + 2.7·log10(f_GHz); d floored at 3 m",
        ),
        Equation::new(
            "shadowing",
            "AR(1) over the distance moved, σ 3 dB LOS / 4 dB building-blocked",
        ),
    ];
    card.parameters = vec![
        Parameter::new("near_slope_db_per_decade", "dB", serde_json::json!(22.7), winner.clone()),
        Parameter::new("far_slope_db_per_decade", "dB", serde_json::json!(40.0), winner.clone()),
        Parameter::new("environment_height_m", "m", serde_json::json!(ENVIRONMENT_HEIGHT_M), winner.clone()),
        Parameter::new("sigma_los_db", "dB", serde_json::json!(SIGMA_LOS_DB), tr.clone()),
        Parameter::new("sigma_nlos_db", "dB", serde_json::json!(SIGMA_NLOS_DB), tr.clone()),
        Parameter::new("min_distance_m", "m", serde_json::json!(MIN_DISTANCE_M), tr.clone()),
    ];
    card.assumptions = vec![
        "The run's carrier is used in the f terms (TR 36.885 uses 6 GHz as the 5.9 GHz \
         proxy; the difference is 0.1 dB)."
            .to_string(),
        "An antenna at or below the 1 m environment height is evaluated at h' = 0.1 m."
            .to_string(),
    ];
    card.limitations = vec![
        "No B1 NLOS (Manhattan two-leg) formula: a building-blocked link keeps the LOS law \
         and the obstacle stack's Sommer term charges the walls, as Veins composes it."
            .to_string(),
        "Below the breakpoint the law is under free space by a few dB, as published; no \
         free-space floor is applied."
            .to_string(),
    ];
    card.ignores = vec!["Weather: applied once by the composition.".to_string()];
    card.sources = vec![winner, tr];
    card.determinism = Determinism::default();
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: vec![Source::new(
            SourceKind::Paper,
            "Sepulcre et al. arXiv:2104.07923 Fig. 4a (Veins on WINNER+ B1), McCarthy et \
             al. arXiv:2103.13212 Fig. 3 (OpenCV2X on WINNER+ B1): docs/validation/radio-fidelity.md",
        )],
        tests: vec![
            "the_two_segments_meet_at_the_breakpoint".to_string(),
            "the_breakpoint_uses_effective_heights".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: f64 = 5.9e9;

    #[test]
    fn the_breakpoint_uses_effective_heights() {
        // 4 · 0.5 · 0.5 · 5.9e9 / c = 19.68 m, not the 177 m of 4·1.5·1.5/λ.
        let d = breakpoint_m(1.5, 1.5, F);
        assert!((d - 19.68).abs() < 0.01, "{d}");
    }

    #[test]
    fn the_two_segments_meet_at_the_breakpoint() {
        let d = breakpoint_m(1.5, 1.5, F);
        let below = 22.7 * math::log10(d) + 27.0 + 20.0 * math::log10(F / 1e9);
        let above = b1_los_db(d * 1.000_001, 1.5, 1.5, F);
        assert!((below - above).abs() < 0.05, "{below} vs {above}");
    }

    #[test]
    fn the_far_slope_is_forty_db_per_decade_and_the_300_m_loss_is_the_published_one() {
        let a = b1_los_db(100.0, 1.5, 1.5, F);
        let b = b1_los_db(1000.0, 1.5, 1.5, F);
        assert!((b - a - 40.0).abs() < 1e-9);
        // 40·log10(300) + 7.56 + 2·17.3·log10(2) + 2.7·log10(5.9) = 119.14 dB.
        let pl = b1_los_db(300.0, 1.5, 1.5, F);
        assert!((pl - 119.14).abs() < 0.02, "{pl}");
        // The 3 m floor.
        assert_eq!(b1_los_db(1.0, 1.5, 1.5, F), b1_los_db(3.0, 1.5, 1.5, F));
    }
}
