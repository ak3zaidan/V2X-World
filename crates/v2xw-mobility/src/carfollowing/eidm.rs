//! `mobility/car-following/eidm` — the Extended Intelligent Driver Model.
//!
//! # What it is
//!
//! Salles, Kaufmann & Reuss, "Extending the Intelligent Driver Model in SUMO and Verifying
//! the Drive Off Trajectories with Aerial Measurements", SUMO User Conference 2020
//! (EPiC Series in Computing 72:1–14). The EIDM is three published pieces on top of the
//! IDM, and this implementation carries the three that need no per-vehicle memory beyond
//! the previous acceleration the engine already hands every model:
//!
//! 1. **The improved IDM** (Treiber & Kesting, *Traffic Flow Dynamics*, Springer 2013,
//!    §11.3.7), which removes the base model's "bunching" below the desired speed and its
//!    over-braking above it:
//!
//!    ```text
//!    z = s*(v,Δv)/s
//!    v ≤ v0:  a_free = a·(1 − (v/v0)^δ);  a = a·(1 − z²)                    if z ≥ 1
//!                                       a = a_free·(1 − z^(2a/a_free))     otherwise
//!    v > v0:  a_free = −b·(1 − (v0/v)^(a·δ/b)); a = a_free + a·(1 − z²)     if z ≥ 1
//!                                              a = a_free                   otherwise
//!    ```
//! 2. **The constant-acceleration heuristic blend** of Kesting, Treiber & Helbing 2010
//!    (Phil. Trans. R. Soc. A 368:4585), coolness `c` = 0.99 — the same expression
//!    `crate::carfollowing::idm`'s optional enhancement uses, here always on because the
//!    EIDM is defined with it.
//! 3. **The jerk bound** of Salles et al. 2020: a rising acceleration grows by at most
//!    `j_max·Δt` per step (`2·j_max·Δt` while the vehicle is still decelerating), above
//!    walking pace (`2 × 0.5 m/s`, SUMO's `ClutchEngageSpeed`), with `j_max` = 3 m/s³.
//!
//! **Left out**, and stated on the card: the EIDM's reaction-time action points, its
//! Wiener-process estimation errors (`sigmaleader`, `sigmagap`, `sigmaerror`) and its
//! tanh drive-off profile (`taccmax`, `Mflatness`, `Mbegin`), each of which needs a
//! per-vehicle state this trait does not carry. The SUMO constants were read from
//! `MSCFModel_EIDM.cpp` (EPL-2.0, read only, never vendored; 2026-10-07).

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;

use super::common::{self, StepClock};
use super::idm::IdmPreset;
use crate::classes::VehicleClass;
use crate::traits::CarFollowing;
use crate::views::{DriverProfile, LaneView, LeaderView, VehicleView};
use crate::weather::{RoadContext, WeatherResponse};

/// The model id.
pub const MODEL_ID: &str = "mobility/car-following/eidm";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// The model-wide constants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EidmParams {
    /// The driver parameter set (the IDM presets: the EIDM keeps the IDM's parameters).
    pub preset: IdmPreset,
    /// The free-acceleration exponent `δ`.
    pub delta: f64,
    /// The coolness factor `c` of the CAH blend.
    pub coolness: f64,
    /// The jerk bound on a rising acceleration, m/s³.
    pub jerk_max_mps3: f64,
    /// Which cited weather table applies.
    pub weather_response: WeatherResponse,
    /// Whether the FHWA freeway or arterial rows apply.
    pub road_context: RoadContext,
}

impl Default for EidmParams {
    fn default() -> Self {
        Self {
            // The native engine's drivers, so that swapping IDM for EIDM changes the
            // equation and nothing else.
            preset: IdmPreset::UrbanHcm,
            delta: 4.0,
            coolness: 0.99,
            jerk_max_mps3: 3.0,
            weather_response: WeatherResponse::Fhwa,
            road_context: RoadContext::BySpeedLimit,
        }
    }
}

/// SUMO's `ClutchEngageSpeed`: below twice this, the jerk bound is not applied, m/s.
pub const CLUTCH_ENGAGE_SPEED_MPS: f64 = 0.5;

/// The EIDM.
#[derive(Debug, Clone)]
pub struct Eidm {
    params: EidmParams,
    clock: StepClock,
    card: ModelCard,
}

impl Default for Eidm {
    fn default() -> Self {
        Eidm::new(EidmParams::default())
    }
}

impl Eidm {
    /// The model with `params`.
    pub fn new(params: EidmParams) -> Self {
        Self {
            card: card(&params),
            params,
            clock: StepClock::default(),
        }
    }

    /// The improved IDM with the CAH blend, on bare numbers (no jerk bound).
    pub fn accel_of(&self, v: f64, v0: f64, leader: Option<(f64, f64, f64)>, d: &DriverProfile) -> f64 {
        let p = &self.params;
        let a = d.max_accel_mps2;
        let b = d.comfort_decel_mps2.max(0.1);
        let v0 = v0.max(0.1);
        let a_free = if v <= v0 {
            a * (1.0 - math::pow(v / v0, p.delta))
        } else {
            -b * (1.0 - math::pow(v0 / v, a * p.delta / b))
        };
        let Some((gap, v_l, a_l)) = leader else {
            return a_free;
        };
        let s = gap.max(0.1);
        let dv = v - v_l;
        let s_star = d.min_gap_m
            + (d.time_headway_s * v + v * dv / (2.0 * math::sqrt(a * b))).max(0.0);
        let z = s_star / s;
        let iidm = if v <= v0 {
            if z >= 1.0 {
                a * (1.0 - z * z)
            } else if a_free > 1e-9 {
                a_free * (1.0 - math::pow(z, 2.0 * a / a_free))
            } else {
                0.0
            }
        } else if z >= 1.0 {
            a_free + a * (1.0 - z * z)
        } else {
            a_free
        };
        // The constant-acceleration heuristic (Kesting et al. 2010), as in `idm.rs`.
        let a_tilde = a_l.min(a);
        let cah = if v_l * dv < -2.0 * s * a_tilde {
            let denom = v_l * v_l - 2.0 * s * a_tilde;
            if denom.abs() < f64::EPSILON {
                iidm
            } else {
                v * v * a_tilde / denom
            }
        } else {
            let step = if dv > 0.0 { 1.0 } else { 0.0 };
            a_tilde - dv * dv * step / (2.0 * s)
        };
        if iidm >= cah {
            iidm
        } else {
            let c = p.coolness;
            (1.0 - c) * iidm + c * (cah + b * math::tanh((iidm - cah) / b))
        }
    }
}

impl v2xw_core::model::Model for Eidm {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl CarFollowing for Eidm {
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
        let mut a = self.accel_of(
            ego.speed_mps,
            d.desired_speed_mps,
            leader.map(|l| (l.gap_m, l.speed_mps, l.accel_mps2)),
            &d,
        );
        let dt = self.clock.dt_s();
        let prev = ego.accel_mps2;
        if a > prev && ego.speed_mps > 2.0 * CLUTCH_ENGAGE_SPEED_MPS {
            let j = self.params.jerk_max_mps3;
            let rise = if prev + j * dt < 0.0 { 2.0 * j } else { j };
            a = a.min(prev + rise * dt);
        }
        let emergency = ego.class.spec().emergency_decel_mps2;
        a.clamp(-emergency, d.max_accel_mps2)
            .max(-sit.grip_decel_mps2)
    }

    fn profile(&self, class: VehicleClass) -> DriverProfile {
        self.params.preset.profile(class)
    }

    fn begin_step(&self, now: SimTime, dt: Duration) {
        self.clock.begin(now, dt);
    }
}

/// The model card.
pub fn card(params: &EidmParams) -> ModelCard {
    let salles = Source::new(
        SourceKind::Paper,
        "Salles, Kaufmann & Reuss 2020, Extending the Intelligent Driver Model in SUMO and \
         Verifying the Drive Off Trajectories with Aerial Measurements, SUMO User Conference \
         2020, EPiC Series in Computing 72",
    );
    let iidm = Source {
        kind: SourceKind::Paper,
        reference: "Treiber & Kesting 2013, Traffic Flow Dynamics, Springer, §11.3.7 (the \
                    improved IDM)"
            .to_string(),
        accessed: None,
        note: Some("secondary: the book as recalled and quoted, not re-read".to_string()),
    };
    let sumo = Source {
        kind: SourceKind::Code,
        reference: "SUMO src/microsim/cfmodels/MSCFModel_EIDM.cpp (EPL-2.0, read only): \
                    jerkmax = 3 m/s³, ClutchEngageSpeed = 0.5 m/s, c_coolness = 0.99"
            .to_string(),
        accessed: Some("2026-10-07".to_string()),
        note: None,
    };
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        MODEL_VERSION,
        "Longitudinal acceleration by the Extended IDM: the improved IDM, blended with the \
         constant-acceleration heuristic, with a jerk-bounded rise in acceleration.",
    );
    card.tier = vec![Tier::Medium];
    card.equations = vec![
        Equation::new(
            "improved IDM",
            "z = s*/s; v≤v0: a(1−z²) if z≥1 else a_free(1 − z^(2a/a_free)); \
             v>v0: a_free + a(1−z²) if z≥1 else a_free",
        ),
        Equation::new(
            "CAH blend",
            "a = (1−c)·a_IIDM + c·[a_CAH + b·tanh((a_IIDM − a_CAH)/b)] when a_IIDM < a_CAH",
        ),
        Equation::new("jerk bound", "a ≤ a_prev + j_max·Δt (2·j_max while a_prev + j_max·Δt < 0)"),
    ];
    card.parameters = vec![
        Parameter::new(
            "preset",
            "-",
            serde_json::json!(params.preset.label()),
            params.preset.source(),
        ),
        Parameter::new("delta", "1", serde_json::json!(params.delta), iidm.clone()),
        Parameter::new(
            "coolness",
            "1",
            serde_json::json!(params.coolness),
            Source::new(
                SourceKind::Paper,
                "Kesting, Treiber & Helbing 2010: c = 0.99 [R10 §B2]",
            ),
        ),
        Parameter::new(
            "jerk_max",
            "m/s³",
            serde_json::json!(params.jerk_max_mps3),
            sumo.clone(),
        ),
    ];
    card.parameters.extend(crate::weather::fhwa_parameters());
    card.limitations = vec![
        "Leaves out the EIDM's reaction-time action points, estimation errors and tanh \
         drive-off profile: each needs per-vehicle memory the CarFollowing trait does not \
         carry."
            .to_string(),
    ];
    card.sources = vec![salles, iidm, sumo];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "carfollowing::eidm::tests::no_bunching_below_v0".to_string(),
            "carfollowing::eidm::tests::stops_behind_a_standing_leader".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carfollowing::testkit::run_follow;

    #[test]
    fn stops_behind_a_standing_leader() {
        let m = Eidm::default();
        let (min_gap, v) = run_follow(&m, 15.0, 80.0, 0.0, 600);
        assert!(min_gap > 1.0, "{min_gap}");
        assert!(v < 0.05, "{v}");
    }

    #[test]
    fn no_bunching_below_v0() {
        // The improved IDM's point: at the equilibrium gap s* with v < v0 the base IDM
        // brakes (a_free − a < 0) while the IIDM holds speed.
        let m = Eidm::default();
        let d = IdmPreset::UrbanHcm.profile(VehicleClass::Passenger);
        let v = 10.0;
        let s_eq = d.min_gap_m + d.time_headway_s * v;
        let a = m.accel_of(v, 15.0, Some((s_eq, v, 0.0)), &d);
        assert!(a.abs() < 1e-9, "{a}");
    }
}
