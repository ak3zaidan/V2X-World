//! Vehicle antenna patterns: `antenna/vehicle/tr37885-option1`.
//!
//! 3GPP TR 37.885 V15.3.0 §6.1.4, Tables 6.1.4-8 and 6.1.4-9, Option 1 at 6 GHz (the
//! frequency the TR evaluates 5.9 GHz at). Where the antenna sits depends on the vehicle
//! type:
//!
//! * **Type 2** (a passenger vehicle with its antenna on the roof, 1.6 m): one rooftop
//!   panel, omnidirectional in azimuth, `A_E,H(φ) = 0`.
//! * **Types 1 and 3** (a passenger vehicle with a low antenna, 0.75 m, and a truck or bus,
//!   3 m): two panels, at the front (bearing 0°) and the rear (180°), each with
//!   `A_E,H(φ) = −min{12·(φ/φ3dB)², A_m}`, `φ3dB` = 120°, `A_m` = 20 dB.
//!
//! Every element has the vertical pattern `A_E,V(θ) = −min{12·((θ − 90°)/θ3dB)², SLA_V}`,
//! `θ3dB` = 90°, `SLA_V` = 20 dB (θ the zenith angle, 90° the horizon), the two combine
//! as `A(θ, φ) = −min{−(A_E,V + A_E,H), A_m}`, and the element's maximum directional gain
//! is 3 dBi — the scalar gain the link budget already carries, so this module returns the
//! pattern *relative* to it, 0 dB at best.
//!
//! How the two panels of a Type 1 or 3 vehicle are combined is "up to proponents
//! decision" (Table 6.1.4-9, TXRU mapping). This model takes the better panel toward the
//! other end of the link — selection, which is what a receiver with one chain per panel
//! does — and says so on the card. A truck heard from its side is therefore about 7 dB
//! down on the same truck heard from ahead or behind.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};

use crate::types::ActorClass;

/// Where a vehicle's V2X antenna is mounted, which decides its pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AntennaMount {
    /// One rooftop panel, omnidirectional in azimuth (TR 37.885 vehicle Type 2).
    Rooftop,
    /// Two panels, front and rear (TR 37.885 vehicle Types 1 and 3).
    FrontRear,
    /// No pattern: the scalar gain in every direction (a handheld, a mast).
    Isotropic,
}

impl AntennaMount {
    /// The mount TR 37.885 gives a class: a car or van is Type 2 (rooftop), a truck or bus
    /// Type 3 (front and rear), a motorcycle a rooftop-like single antenna; a pedestrian,
    /// a bicycle and a roadside unit carry no vehicle pattern.
    #[must_use]
    pub const fn for_class(class: ActorClass) -> Self {
        match class {
            ActorClass::Car | ActorClass::Van | ActorClass::Motorcycle => AntennaMount::Rooftop,
            ActorClass::Truck => AntennaMount::FrontRear,
            ActorClass::Bicycle
            | ActorClass::Pedestrian
            | ActorClass::Rsu
            | ActorClass::BaseStation => AntennaMount::Isotropic,
        }
    }

    /// The scenario's name for it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            AntennaMount::Rooftop => "rooftop",
            AntennaMount::FrontRear => "front-rear",
            AntennaMount::Isotropic => "isotropic",
        }
    }

    /// The mount a scenario names.
    #[must_use]
    pub fn from_label(s: &str) -> Option<Self> {
        [
            AntennaMount::Rooftop,
            AntennaMount::FrontRear,
            AntennaMount::Isotropic,
        ]
        .into_iter()
        .find(|m| m.label() == s)
    }

    /// The pattern toward a direction, dB relative to the element's maximum gain.
    ///
    /// `azimuth_rad` is the direction's bearing relative to the vehicle's heading (0
    /// ahead, ±π behind); `elevation_rad` its angle above the horizon.
    #[must_use]
    pub fn relative_gain_db(self, azimuth_rad: f64, elevation_rad: f64) -> f64 {
        if self == AntennaMount::Isotropic {
            return 0.0;
        }
        let theta_deg = 90.0 - elevation_rad * DEG_PER_RAD;
        let a_v = -(12.0 * sq((theta_deg - 90.0) / THETA_3DB_DEG)).min(SLA_V_DB);
        let a_h = match self {
            AntennaMount::Rooftop | AntennaMount::Isotropic => 0.0,
            AntennaMount::FrontRear => {
                let phi = wrap_deg(azimuth_rad * DEG_PER_RAD);
                let panel = |bearing: f64| {
                    let off = wrap_deg(phi - bearing);
                    -(12.0 * sq(off / PHI_3DB_DEG)).min(A_M_DB)
                };
                panel(0.0).max(panel(180.0))
            }
        };
        -(-(a_v + a_h)).min(A_M_DB)
    }
}

/// `θ3dB`, the vertical half-power beamwidth at 6 GHz, degrees.
pub const THETA_3DB_DEG: f64 = 90.0;
/// `SLA_V`, the vertical side-lobe attenuation at 6 GHz, dB.
pub const SLA_V_DB: f64 = 20.0;
/// `φ3dB`, the horizontal half-power beamwidth of a Type 1 or 3 panel at 6 GHz, degrees.
pub const PHI_3DB_DEG: f64 = 120.0;
/// `A_m`, the maximum attenuation at 6 GHz, dB.
pub const A_M_DB: f64 = 20.0;

const DEG_PER_RAD: f64 = 180.0 / core::f64::consts::PI;

fn sq(x: f64) -> f64 {
    x * x
}

/// An angle in degrees folded into (−180, 180].
fn wrap_deg(a: f64) -> f64 {
    let mut x = a % 360.0;
    if x > 180.0 {
        x -= 360.0;
    } else if x <= -180.0 {
        x += 360.0;
    }
    x
}

/// The model's id.
pub const ANTENNA_ID: &str = "antenna/vehicle/tr37885-option1";

/// The card of the vehicle antenna patterns.
#[must_use]
pub fn card() -> ModelCard {
    let tr = Source {
        kind: SourceKind::Standard,
        reference: "3GPP TR 37.885 V15.3.0 (2019-06) §6.1.4, Tables 6.1.4-8 and 6.1.4-9 \
                    (Option 1, 6 GHz), via ATIS.3GPP.37.885.V1530"
            .to_string(),
        accessed: Some("2026-09-29".to_string()),
        note: None,
    };
    let mut card = ModelCard::new(
        ANTENNA_ID,
        Family::Propagation,
        "1.0.0",
        "Vehicle antenna patterns from TR 37.885 Option 1: a rooftop panel omnidirectional \
         in azimuth on cars and vans, front and rear panels on trucks and buses, and the \
         vertical pattern of every element, relative to the element's 3 dBi maximum.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new(
            "vertical",
            "A_E,V(θ) = −min{12·((θ − 90°)/θ3dB)², SLA_V}, θ3dB = 90°, SLA_V = 20 dB",
        ),
        Equation::new(
            "horizontal",
            "rooftop: A_E,H = 0; front-rear: A_E,H(φ) = −min{12·(φ/φ3dB)², A_m} per panel at \
             0° and 180°, φ3dB = 120°, A_m = 20 dB, the better panel taken",
        ),
        Equation::new("combined", "A(θ, φ) = −min{−(A_E,V + A_E,H), A_m}"),
    ];
    card.parameters = vec![
        Parameter::new(
            "theta_3db_deg",
            "deg",
            serde_json::json!(THETA_3DB_DEG),
            tr.clone(),
        ),
        Parameter::new("sla_v_db", "dB", serde_json::json!(SLA_V_DB), tr.clone()),
        Parameter::new(
            "phi_3db_deg",
            "deg",
            serde_json::json!(PHI_3DB_DEG),
            tr.clone(),
        ),
        Parameter::new("a_m_db", "dB", serde_json::json!(A_M_DB), tr.clone()),
    ];
    card.assumptions = vec![
        "A front-rear vehicle uses the better of its two panels toward the other end \
         (selection); TR 37.885 leaves the TXRU mapping to the proponent."
            .to_string(),
        "The class-to-type mapping: car, van and motorcycle are Type 2 (rooftop), truck \
         and bus Type 3 (front and rear)."
            .to_string(),
    ];
    card.limitations = vec![
        "No measured installed pattern: the roof's own shaping (ripple of a few dB, the \
         5GAA P-190033 Figure 10 patterns) is not modelled."
            .to_string(),
    ];
    card.sources = vec![tr.clone()];
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: vec![tr],
        tests: vec!["the_patterns_are_tr37885s".to_string()],
    };
    card.determinism = Determinism::default();
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_patterns_are_tr37885s() {
        let deg = |d: f64| d / DEG_PER_RAD;
        // A rooftop antenna is the same in every horizontal direction.
        for a in [0.0, 45.0, 90.0, 180.0, -135.0] {
            assert_eq!(AntennaMount::Rooftop.relative_gain_db(deg(a), 0.0), 0.0);
        }
        // Its vertical pattern: 12·(θ−90)²/90², so 3 dB down at 45° elevation ... and
        // SLA_V = 20 dB at most.
        let v45 = AntennaMount::Rooftop.relative_gain_db(0.0, deg(45.0));
        assert!((v45 + 3.0).abs() < 1e-9, "{v45}");
        let v90 = AntennaMount::Rooftop.relative_gain_db(0.0, deg(90.0));
        assert!((v90 + 12.0).abs() < 1e-9, "{v90}");
        // A truck: full gain ahead and behind, 12·(90/120)² = 6.75 dB down to the side,
        // 3 dB down at the half-power angle of 60° off either panel's bearing.
        let t = |a: f64| AntennaMount::FrontRear.relative_gain_db(deg(a), 0.0);
        assert_eq!(t(0.0), 0.0);
        assert_eq!(t(180.0), 0.0);
        assert!((t(90.0) + 6.75).abs() < 1e-9, "{}", t(90.0));
        assert!((t(-90.0) + 6.75).abs() < 1e-9);
        assert!((t(60.0) + 3.0).abs() < 1e-9, "{}", t(60.0));
        assert!((t(120.0) + 3.0).abs() < 1e-9, "{}", t(120.0));
        // Symmetric, and never above the maximum.
        for a in (-180..=180).step_by(15) {
            let g = t(f64::from(a));
            assert!((g - t(-f64::from(a))).abs() < 1e-9);
            assert!(g <= 0.0 && g >= -A_M_DB);
        }
        // The class mapping.
        assert_eq!(
            AntennaMount::for_class(ActorClass::Car),
            AntennaMount::Rooftop
        );
        assert_eq!(
            AntennaMount::for_class(ActorClass::Truck),
            AntennaMount::FrontRear
        );
        assert_eq!(
            AntennaMount::for_class(ActorClass::Rsu),
            AntennaMount::Isotropic
        );
        assert_eq!(AntennaMount::Isotropic.relative_gain_db(1.0, 1.0), 0.0);
        card().validate().expect("the card validates");
    }
}
