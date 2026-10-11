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
//! Two treatments of a building-blocked link, chosen by `params.nlos`:
//!
//! * `sommer` (the default): the LOS law, and the obstacle stack's Sommer 2011 term
//!   charges the walls — exactly how Veins composes a distance law with
//!   `SimpleObstacleShadowing`.
//! * `manhattan`: the B1 NLOS law of WINNER+ D5.3 Table 4-1 over the two street legs
//!   `d1`, `d2` of the corner the link turns round (traced by the engine's corner tracer),
//!   and no Sommer term — the law prices the buildings itself. A blocked link no single
//!   corner connects is evaluated at `d1 = d2 = d/√2`, the two equal legs of a Manhattan
//!   path over the same straight-line distance.

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

/// Which antenna heights the B1 breakpoint and far law are evaluated with.
///
/// The published simulators disagree, and the disagreement is the whole of their range
/// difference: WINNER+ and TR 36.885 as written subtract the 1 m environment height
/// (`d'BP` ≈ 20 m), which the Veins curve of Sepulcre et al. 2021 reproduces; the
/// LTE-V2X Mode 4 curves of OpenCV2X (McCarthy 2021) and Molina-Masegosa 2017 are
/// reproduced only with the antennas' actual heights (`d_BP` ≈ 177 m), which is how
/// their implementations evaluate it. Both are offered and the card says which is which.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum B1Heights {
    /// `h' = h − 1 m`, as WINNER+ D5.3 and TR 36.885 print it. The default.
    #[default]
    Effective,
    /// The antennas' actual heights, as the LTE-V2X study simulators evaluate it.
    Actual,
}

impl B1Heights {
    /// The height subtracted from each antenna, metres.
    #[must_use]
    pub const fn environment_m(self) -> f64 {
        match self {
            B1Heights::Effective => ENVIRONMENT_HEIGHT_M,
            B1Heights::Actual => 0.0,
        }
    }
}

/// How a building-blocked link is priced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum B1Nlos {
    /// The LOS law plus the obstacle stack's Sommer 2011 wall term (Veins' composition).
    #[default]
    Sommer,
    /// The WINNER+ B1 NLOS (Manhattan two-leg) law round the traced street corner.
    Manhattan,
}

/// Half the B1 street width (w = 20 m, WINNER+ D5.3 Table 4-1): the near end of the
/// NLOS law's `d_l` range.
pub const HALF_STREET_WIDTH_M: f64 = 10.0;

/// The WINNER+ B1 NLOS path loss, dB, for a path that runs `d1` metres down the
/// transmitter's street to the corner and `d2` metres down the receiver's (WINNER+ D5.3
/// Table 4-1; the same Manhattan-grid law TR 36.885 Table A.1.4-1 cites for urban NLOS):
///
/// `PL = min(PL(d1, d2), PL(d2, d1))`,
/// `PL(dk, dl) = PL_LOS(dk) + 17.3 − 12.5·n_j + 10·n_j·log10(dl) + 3·log10(f/GHz)`,
/// `n_j = max(2.8 − 0.0024·dk, 1.84)`.
///
/// ITU-R M.2135 and TR 36.814 print the UMi variant with 20 in place of 17.3. The result
/// is never below the LOS law over the straight-line distance.
#[must_use]
pub fn b1_nlos_with(
    d1_m: f64,
    d2_m: f64,
    h_t_m: f64,
    h_r_m: f64,
    f_hz: f64,
    conv: B1Heights,
) -> f64 {
    let f_ghz = f_hz / 1e9;
    let leg = |dk: f64, dl: f64| {
        let dk = dk.max(MIN_DISTANCE_M);
        let dl = dl.max(HALF_STREET_WIDTH_M);
        let n_j = (2.8 - 0.0024 * dk).max(1.84);
        b1_los_with(dk, h_t_m, h_r_m, f_hz, conv) + 17.3 - 12.5 * n_j
            + 10.0 * n_j * math::log10(dl)
            + 3.0 * math::log10(f_ghz)
    };
    let nlos = leg(d1_m, d2_m).min(leg(d2_m, d1_m));
    let straight = (d1_m * d1_m + d2_m * d2_m).sqrt();
    nlos.max(b1_los_with(straight, h_t_m, h_r_m, f_hz, conv))
}

fn heights(h_t_m: f64, h_r_m: f64, conv: B1Heights) -> (f64, f64) {
    let e = conv.environment_m();
    (
        (h_t_m - e).max(MIN_EFFECTIVE_HEIGHT_M),
        (h_r_m - e).max(MIN_EFFECTIVE_HEIGHT_M),
    )
}

/// The effective breakpoint distance `d'BP = 4·h'_t·h'_r·f/c`, metres.
#[must_use]
pub fn breakpoint_m(h_t_m: f64, h_r_m: f64, f_hz: f64) -> f64 {
    breakpoint_with(h_t_m, h_r_m, f_hz, B1Heights::Effective)
}

/// The breakpoint distance under a height convention, metres.
#[must_use]
pub fn breakpoint_with(h_t_m: f64, h_r_m: f64, f_hz: f64, conv: B1Heights) -> f64 {
    let (ht, hr) = heights(h_t_m, h_r_m, conv);
    4.0 * ht * hr * f_hz / crate::numeric::SPEED_OF_LIGHT_M_S
}

/// The WINNER+ B1 LOS path loss, dB, with the TR 36.885 3 m floor on the distance.
#[must_use]
pub fn b1_los_db(d_m: f64, h_t_m: f64, h_r_m: f64, f_hz: f64) -> f64 {
    b1_los_with(d_m, h_t_m, h_r_m, f_hz, B1Heights::Effective)
}

/// The WINNER+ B1 LOS path loss under a height convention, dB.
#[must_use]
pub fn b1_los_with(d_m: f64, h_t_m: f64, h_r_m: f64, f_hz: f64, conv: B1Heights) -> f64 {
    let d = d_m.max(MIN_DISTANCE_M);
    let f_ghz = f_hz / 1e9;
    let d_bp = breakpoint_with(h_t_m, h_r_m, f_hz, conv);
    if d <= d_bp {
        22.7 * math::log10(d) + 27.0 + 20.0 * math::log10(f_ghz)
    } else {
        let (ht, hr) = heights(h_t_m, h_r_m, conv);
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
    heights: B1Heights,
    nlos: B1Nlos,
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
            heights: B1Heights::Effective,
            nlos: B1Nlos::Sommer,
            shadowing: ShadowingOverride::default(),
            shadows: BTreeMap::new(),
        }
    }

    /// The same model under another antenna-height convention.
    #[must_use]
    pub fn with_heights(mut self, h: B1Heights) -> Self {
        self.heights = h;
        self
    }

    /// The same model with another building-blocked-link treatment.
    #[must_use]
    pub fn with_nlos(mut self, n: B1Nlos) -> Self {
        self.nlos = n;
        self
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
        let path = if self.nlos == B1Nlos::Manhattan && los.class.has_building() {
            let (d1, d2) = los.corner.map_or(
                (
                    d * core::f64::consts::FRAC_1_SQRT_2,
                    d * core::f64::consts::FRAC_1_SQRT_2,
                ),
                |c| (c.d_t_m, c.d_r_m),
            );
            b1_nlos_with(d1, d2, tx.height_m(), rx.height_m(), f_hz, self.heights)
        } else {
            b1_los_with(d, tx.height_m(), rx.height_m(), f_hz, self.heights)
        };
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
        Equation::new(
            "breakpoint",
            "d'BP = 4·(h_t − 1)·(h_r − 1)·f/c (≈ 19.7 m at 5.9 GHz, 1.5 m antennas)",
        ),
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
        Parameter::new(
            "near_slope_db_per_decade",
            "dB",
            serde_json::json!(22.7),
            winner.clone(),
        ),
        Parameter::new(
            "far_slope_db_per_decade",
            "dB",
            serde_json::json!(40.0),
            winner.clone(),
        ),
        Parameter::new(
            "environment_height_m",
            "m",
            serde_json::json!(ENVIRONMENT_HEIGHT_M),
            winner.clone(),
        ),
        Parameter {
            name: "heights".to_string(),
            unit: "-".to_string(),
            default: serde_json::json!("effective"),
            range: Some(vec![
                serde_json::json!("effective"),
                serde_json::json!("actual"),
            ]),
            source: Source::new(
                SourceKind::Paper,
                "effective: WINNER+ D5.3 / TR 36.885 as printed, reproduces Veins (Sepulcre \
                 2021 Fig. 4a) to 5.7 PDR points; actual: reproduces OpenCV2X (McCarthy \
                 2021 Fig. 3) and Molina-Masegosa 2017 Fig. 3 to 5-6 points \
                 (docs/validation/radio-fidelity.md)",
            ),
            calibration: None,
        },
        Parameter {
            name: "nlos".to_string(),
            unit: "-".to_string(),
            default: serde_json::json!("sommer"),
            range: Some(vec![
                serde_json::json!("sommer"),
                serde_json::json!("manhattan"),
            ]),
            source: Source::new(
                SourceKind::Paper,
                "sommer: LOS law + Sommer et al. 2011 wall term (Veins); manhattan: WINNER+ \
                 D5.3 Table 4-1 B1 NLOS over the traced corner's two street legs",
            ),
            calibration: None,
        },
        Parameter::new(
            "sigma_los_db",
            "dB",
            serde_json::json!(SIGMA_LOS_DB),
            tr.clone(),
        ),
        Parameter::new(
            "sigma_nlos_db",
            "dB",
            serde_json::json!(SIGMA_NLOS_DB),
            tr.clone(),
        ),
        Parameter::new(
            "min_distance_m",
            "m",
            serde_json::json!(MIN_DISTANCE_M),
            tr.clone(),
        ),
    ];
    card.assumptions = vec![
        "The run's carrier is used in the f terms (TR 36.885 uses 6 GHz as the 5.9 GHz \
         proxy; the difference is 0.1 dB)."
            .to_string(),
        "An antenna at or below the 1 m environment height is evaluated at h' = 0.1 m.".to_string(),
    ];
    card.limitations = vec![
        "B1 NLOS (Manhattan two-leg) only with `nlos: manhattan`; by default a \
         building-blocked link keeps the LOS law and the obstacle stack's Sommer term \
         charges the walls, as Veins composes it. A blocked link with no single traced \
         corner is evaluated at d1 = d2 = d/√2."
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
            "the_nlos_law_matches_a_hand_evaluation_and_exceeds_los".to_string(),
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

    #[test]
    fn the_nlos_law_matches_a_hand_evaluation_and_exceeds_los() {
        // d1 = 100, d2 = 50, effective heights: PL_LOS(100) = 40·2 + 7.56 + 2·17.3·0.30103
        // + 2.7·log10(5.9) = 100.058; n_j = 2.56; PL(100, 50) = 100.058 + 17.3 − 32.0
        // + 25.6·1.69897 + 3·0.77085 = 131.17. PL(50, 100) uses PL_LOS(50) = 88.017,
        // n_j = 2.68: 88.017 + 17.3 − 33.5 + 53.6 + 2.313 = 127.73, the smaller.
        let pl = b1_nlos_with(100.0, 50.0, 1.5, 1.5, F, B1Heights::Effective);
        assert!((pl - 127.73).abs() < 0.05, "{pl}");
        assert_eq!(
            pl,
            b1_nlos_with(50.0, 100.0, 1.5, 1.5, F, B1Heights::Effective)
        );
        // Turning a corner costs far more than the straight path of the same length.
        assert!(pl > b1_los_db(150.0, 1.5, 1.5, F) + 15.0);
    }
}
