//! `mobility/car-following/gipps` — Gipps's behavioural car-following model.
//!
//! # The equations
//!
//! Gipps, "A behavioural car-following model for computer simulation", *Transportation
//! Research Part B* 15(2):105–111, 1981, eqs. (1)–(5), with decelerations written here as
//! positive magnitudes (`B = −b_n`, `B̂ = −b̂`):
//!
//! ```text
//! v_a = v + 2.5·a·τ·(1 − v/V)·√(0.025 + v/V)                       (free acceleration)
//! v_b = −B·τ + √( B²τ² + B·[ 2·(g − s0) − v·τ + v_l²/B̂ ] )        (safe braking)
//! v(t+τ) = max(0, min(v_a, v_b))
//! ```
//!
//! `g − s0` is the paper's `x_l − s_l − x`: the net gap less the margin by which the
//! leader's *effective size* `s_l` exceeds its body. The model updates over its reaction
//! time `τ`; at a mobility step `Δt < τ` it returns the acceleration `(v(t+τ) − v)/τ`, the
//! continuous reading of the same update (the acceleration the paper's vehicle holds
//! across its reaction interval).
//!
//! # Parameters
//!
//! The means of the distributions Gipps's own simulations drew from (1981, §4): `a` ~
//! N(1.7, 0.3²) m/s², `b_n = −2·a_n`, `s_n` ~ N(6.5, 0.3²) m, `V_n` ~ N(20, 3.2²) m/s,
//! `τ` = 2/3 s, `b̂ = min(−3.0, (b_n − 3.0)/2)`. **Secondary**: as widely reproduced (for
//! example in Treiber & Kesting, *Traffic Flow Dynamics*, 2013, and in Aimsun's model
//! documentation); the 1981 paper itself was not re-read for this card, and the card says
//! so. The paper calibrated one population, so heavy classes keep their SUMO `accel` with
//! Gipps's `b = 2a` ratio — a fold recorded as a choice.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;

use super::common;
use crate::classes::VehicleClass;
use crate::traits::CarFollowing;
use crate::views::{DriverProfile, LaneView, LeaderView, VehicleView};
use crate::weather::{RoadContext, WeatherResponse};

/// The model id.
pub const MODEL_ID: &str = "mobility/car-following/gipps";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// Gipps 1981's mean maximum acceleration, m/s².
pub const GIPPS_A_MPS2: f64 = 1.7;
/// Gipps 1981's mean effective vehicle size, metres.
pub const GIPPS_EFFECTIVE_SIZE_M: f64 = 6.5;
/// Gipps 1981's mean desired speed, m/s.
pub const GIPPS_V_MPS: f64 = 20.0;
/// Gipps 1981's reaction time, seconds.
pub const GIPPS_TAU_S: f64 = 2.0 / 3.0;

/// The model-wide constants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GippsParams {
    /// The floor of the follower's estimate of the leader's braking `B̂`, m/s² (Gipps:
    /// `b̂ = min(−3.0, (b_n − 3.0)/2)`).
    pub b_hat_floor_mps2: f64,
    /// Which cited weather table applies.
    pub weather_response: WeatherResponse,
    /// Whether the FHWA freeway or arterial rows apply.
    pub road_context: RoadContext,
}

impl Default for GippsParams {
    fn default() -> Self {
        Self {
            b_hat_floor_mps2: 3.0,
            weather_response: WeatherResponse::Fhwa,
            road_context: RoadContext::BySpeedLimit,
        }
    }
}

/// The Gipps model.
#[derive(Debug, Clone)]
pub struct Gipps {
    params: GippsParams,
    card: ModelCard,
}

impl Default for Gipps {
    fn default() -> Self {
        Gipps::new(GippsParams::default())
    }
}

impl Gipps {
    /// The model with `params`.
    pub fn new(params: GippsParams) -> Self {
        Self {
            card: card(&params),
            params,
        }
    }

    /// The driver of `class`: Gipps's means for light classes, the SUMO `accel` with
    /// Gipps's `b = 2a` for heavy ones.
    pub fn class_profile(class: VehicleClass) -> DriverProfile {
        let spec = class.spec();
        let a = if spec.accel_mps2 < GIPPS_A_MPS2 {
            spec.accel_mps2
        } else {
            GIPPS_A_MPS2
        };
        DriverProfile {
            desired_speed_mps: GIPPS_V_MPS.min(spec.desired_speed_mps()),
            max_accel_mps2: a,
            comfort_decel_mps2: 2.0 * a,
            time_headway_s: GIPPS_TAU_S,
            min_gap_m: (GIPPS_EFFECTIVE_SIZE_M - spec.length_m).max(1.0),
        }
    }

    /// The speed one reaction time ahead, on bare numbers.
    pub fn next_speed(&self, v: f64, v_desired: f64, leader: Option<(f64, f64)>, d: &DriverProfile) -> f64 {
        let tau = d.time_headway_s.max(0.1);
        let a = d.max_accel_mps2;
        let vd = v_desired.max(0.1);
        let ratio = (v / vd).max(0.0);
        let v_a = if ratio >= 1.0 {
            // Above the desired speed the free term goes imaginary; Gipps's vehicle simply
            // does not accelerate there.
            v.min(vd).max(v - a * tau)
        } else {
            v + 2.5 * a * tau * (1.0 - ratio) * math::sqrt(0.025 + ratio)
        };
        let Some((gap, v_l)) = leader else {
            return v_a.max(0.0);
        };
        let b = d.comfort_decel_mps2.max(0.1);
        let b_hat = self.params.b_hat_floor_mps2.max(0.5 * (b + 3.0));
        let room = 2.0 * (gap - d.min_gap_m) - v * tau + v_l * v_l / b_hat;
        let disc = b * b * tau * tau + b * room;
        let v_b = if disc <= 0.0 {
            0.0
        } else {
            -b * tau + math::sqrt(disc)
        };
        v_a.min(v_b).max(0.0)
    }
}

impl v2xw_core::model::Model for Gipps {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl CarFollowing for Gipps {
    fn accel(
        &self,
        ego: &VehicleView,
        leader: Option<&LeaderView>,
        lane: &LaneView,
        w: &WeatherState,
    ) -> f64 {
        let sit = common::situation(
            ego,
            lane,
            w,
            self.params.weather_response,
            self.params.road_context,
        );
        let d = sit.driver;
        let next = self.next_speed(
            ego.speed_mps,
            d.desired_speed_mps,
            leader.map(|l| (l.gap_m, l.speed_mps)),
            &d,
        );
        let emergency = ego.class.spec().emergency_decel_mps2;
        ((next - ego.speed_mps) / d.time_headway_s.max(0.1))
            .clamp(-emergency, d.max_accel_mps2)
            .max(-sit.grip_decel_mps2)
    }

    fn profile(&self, class: VehicleClass) -> DriverProfile {
        Gipps::class_profile(class)
    }

    fn begin_step(&self, _now: SimTime, _dt: Duration) {}
}

/// The model card.
pub fn card(params: &GippsParams) -> ModelCard {
    let gipps = Source {
        kind: SourceKind::Paper,
        reference: "Gipps 1981, A behavioural car-following model for computer simulation, \
                    Transportation Research B 15(2):105-111, eqs. (1)-(5) and §4's parameter \
                    distributions"
            .to_string(),
        accessed: None,
        note: Some(
            "secondary: equations and parameter means as reproduced in Treiber & Kesting \
             2013 and the Aimsun model documentation; the paper was not re-read"
                .to_string(),
        ),
    };
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        MODEL_VERSION,
        "Longitudinal acceleration by Gipps's model: the lower of a free-acceleration speed \
         and the highest speed from which the vehicle can stop behind a leader that brakes \
         as hard as the follower expects. The car-following model of Aimsun.",
    );
    card.tier = vec![Tier::Medium];
    card.equations = vec![
        Equation::new("free", "v_a = v + 2.5·a·τ·(1 − v/V)·√(0.025 + v/V)"),
        Equation::new(
            "safe",
            "v_b = −B·τ + √(B²τ² + B·[2(g − s0) − v·τ + v_l²/B̂]),  B̂ = max(3, (B + 3)/2)",
        ),
        Equation::new("acceleration", "a_out = (max(0, min(v_a, v_b)) − v)/τ"),
    ];
    let pc = Gipps::class_profile(VehicleClass::Passenger);
    card.parameters = vec![
        Parameter::new("a", "m/s²", serde_json::json!(pc.max_accel_mps2), gipps.clone()),
        Parameter::new("B", "m/s²", serde_json::json!(pc.comfort_decel_mps2), gipps.clone()),
        Parameter::new("tau", "s", serde_json::json!(GIPPS_TAU_S), gipps.clone()),
        Parameter::new("V", "m/s", serde_json::json!(GIPPS_V_MPS), gipps.clone()),
        Parameter::new(
            "effective_size",
            "m",
            serde_json::json!(GIPPS_EFFECTIVE_SIZE_M),
            gipps.clone(),
        ),
        Parameter::new(
            "b_hat_floor",
            "m/s²",
            serde_json::json!(params.b_hat_floor_mps2),
            gipps.clone(),
        ),
    ];
    card.parameters.extend(crate::weather::fhwa_parameters());
    card.assumptions = vec![
        "Heavy classes keep their SUMO accel with Gipps's b = 2a: the paper calibrated one \
         population (a choice)."
            .to_string(),
        "s0 is the effective size less the class's body length, at least 1 m.".to_string(),
    ];
    card.limitations = vec![
        "Parameter means are secondary-sourced; no per-driver distribution is drawn by the \
         model itself (heterogeneity arrives through DriverProfile)."
            .to_string(),
    ];
    card.sources = vec![gipps];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec!["carfollowing::gipps::tests::stops_behind_a_standing_leader".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carfollowing::testkit::run_follow;

    #[test]
    fn stops_behind_a_standing_leader() {
        let m = Gipps::default();
        let (min_gap, v) = run_follow(&m, 15.0, 80.0, 0.0, 600);
        assert!(min_gap > 0.5, "{min_gap}");
        assert!(v < 0.05, "{v}");
    }

    #[test]
    fn free_road_reaches_the_desired_speed() {
        let m = Gipps::default();
        let d = Gipps::class_profile(VehicleClass::Passenger);
        let mut v = 0.0;
        for _ in 0..600 {
            v = (v + 0.1 * (m.next_speed(v, 20.0, None, &d) - v) / d.time_headway_s).max(0.0);
        }
        assert!((v - 20.0).abs() < 0.05, "{v}");
    }
}
