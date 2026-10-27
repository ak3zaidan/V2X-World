//! `mobility/car-following/krauss` — Krauß's collision-free stochastic model, SUMO's
//! default car-following model.
//!
//! # The equations
//!
//! Krauß, *Microscopic Modeling of Traffic Flow: Investigation of Collision Free Vehicle
//! Dynamics*, PhD thesis, Universität zu Köln, 1998 (DLR-Forschungsbericht 98-08):
//!
//! ```text
//! v_safe = v_l + (g − v_l·τ) / ( v̄/b + τ ),   v̄ = (v + v_l)/2,  g = net gap − s0
//! v_des  = min( v + a·Δt, v0, v_safe )
//! v'     = max( 0, v_des − σ·a·Δt·η ),          η ~ U[0, 1)
//! ```
//!
//! The dawdling term follows SUMO's `MSCFModel_Krauss::dawdle2` (read only, EPL-2.0,
//! never vendored; 2026-10-07) in one respect the thesis does not have: below a speed
//! numerically equal to `a`, the dawdle is `σ·v_des·Δt·η` rather than `σ·a·Δt·η`, "so a
//! starting vehicle definitely starts". `g` subtracts the standstill gap `s0` from the net
//! gap because SUMO's leader gap does (its `minGap` is part of the vehicle's reserved
//! space), and the parameter set is SUMO's.
//!
//! The model returns the acceleration `(v' − v)/Δt`, which the engine integrates, so the
//! speed it reaches at the end of the step is exactly the model's `v'`.
//!
//! # Parameters
//!
//! Per class, the SUMO vType defaults this crate already carries
//! ([`crate::classes::VehicleClass::spec`], R10 §B4): `accel`, `decel`, `minGap` and the
//! desired speed; `τ` = 1 s and `σ` = 0.5 are SUMO's vType defaults for the Krauss model
//! (SUMO documentation, "Vehicle Type Parameter Defaults", `tau` and `sigma`).
//!
//! # Determinism
//!
//! `η` is [`super::common::keyed_uniform`]: the model's plug-in stream, keyed by the actor
//! and positioned at the step index — a pure function of `(seed, actor, step)`.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;

use super::common::{self, StepClock};
use crate::classes::VehicleClass;
use crate::traits::CarFollowing;
use crate::views::{DriverProfile, LaneView, LeaderView, VehicleView};
use crate::weather::{RoadContext, WeatherResponse};

/// The model id.
pub const MODEL_ID: &str = "mobility/car-following/krauss";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// The model-wide constants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KraussParams {
    /// The driver imperfection `σ`, `0..=1`. 0 makes the model deterministic.
    pub sigma: f64,
    /// Which cited weather table applies (04-models.md §2.6).
    pub weather_response: WeatherResponse,
    /// Whether the FHWA freeway or arterial rows apply.
    pub road_context: RoadContext,
}

impl Default for KraussParams {
    fn default() -> Self {
        Self {
            sigma: 0.5,
            weather_response: WeatherResponse::Fhwa,
            road_context: RoadContext::BySpeedLimit,
        }
    }
}

/// The SUMO default reaction time `τ`, seconds.
pub const SUMO_TAU_S: f64 = 1.0;

/// The Krauss model.
#[derive(Debug, Clone)]
pub struct Krauss {
    params: KraussParams,
    seed: u64,
    clock: StepClock,
    card: ModelCard,
}

impl Krauss {
    /// The model with `params`, drawing its dawdling from the run's master `seed`.
    pub fn new(params: KraussParams, seed: u64) -> Self {
        Self {
            card: card(&params),
            params,
            seed,
            clock: StepClock::default(),
        }
    }

    /// The parameters in force.
    pub fn params(&self) -> &KraussParams {
        &self.params
    }

    /// The SUMO-default driver of `class`.
    pub fn class_profile(class: VehicleClass) -> DriverProfile {
        let s = class.spec();
        DriverProfile {
            desired_speed_mps: s.desired_speed_mps(),
            max_accel_mps2: s.accel_mps2,
            comfort_decel_mps2: s.decel_mps2,
            time_headway_s: SUMO_TAU_S,
            min_gap_m: s.min_gap_m,
            sigma: None,
        }
    }

    /// The next speed on bare numbers, given the dawdle draw `eta` in `[0, 1)`.
    #[allow(clippy::too_many_arguments)]
    pub fn next_speed(
        &self,
        v: f64,
        v0: f64,
        leader: Option<(f64, f64)>,
        driver: &DriverProfile,
        dt: f64,
        eta: f64,
    ) -> f64 {
        let a = driver.max_accel_mps2;
        let mut v_des = (v + a * dt).min(v0);
        if let Some((gap, v_l)) = leader {
            let g = gap - driver.min_gap_m;
            v_des = v_des.min(common::krauss_safe_speed(
                v,
                v_l,
                g,
                driver.comfort_decel_mps2,
                driver.time_headway_s,
            ));
        }
        // A vehicle type's own `sigma` (an imported `.rou.xml` vType) wins over the
        // model-wide one, as in SUMO where `sigma` is a vType attribute.
        let sigma = driver.sigma.unwrap_or(self.params.sigma);
        let dawdle = if v_des < a {
            sigma * v_des * dt * eta
        } else {
            sigma * a * dt * eta
        };
        (v_des - dawdle).max(0.0)
    }
}

impl v2xw_core::model::Model for Krauss {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl CarFollowing for Krauss {
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
        let dt = self.clock.dt_s();
        let eta = if ego.driver.sigma.unwrap_or(self.params.sigma) > 0.0 {
            common::keyed_uniform(self.seed, MODEL_ID, ego.actor, self.clock.step_index(), 0)
        } else {
            0.0
        };
        let v = ego.speed_mps;
        let next = self.next_speed(
            v,
            sit.driver.desired_speed_mps,
            leader.map(|l| (l.gap_m, l.speed_mps)),
            &sit.driver,
            dt,
            eta,
        );
        let emergency = ego.class.spec().emergency_decel_mps2;
        ((next - v) / dt)
            .clamp(-emergency, sit.driver.max_accel_mps2)
            .max(-sit.grip_decel_mps2)
    }

    fn profile(&self, class: VehicleClass) -> DriverProfile {
        Krauss::class_profile(class)
    }

    fn begin_step(&self, now: SimTime, dt: Duration) {
        self.clock.begin(now, dt);
    }
}

/// The model card.
pub fn card(params: &KraussParams) -> ModelCard {
    let krauss = Source {
        kind: SourceKind::Paper,
        reference: "Krauß 1998, Microscopic Modeling of Traffic Flow: Investigation of \
                    Collision Free Vehicle Dynamics, PhD thesis, Univ. Köln \
                    (DLR-Forschungsbericht 98-08)"
            .to_string(),
        accessed: None,
        note: Some(
            "secondary: the safe-velocity expression as SUMO's MSCFModel_KraussOrig1 and \
             Krajzewicz et al. 2012 state it; the thesis itself was not re-read"
                .to_string(),
        ),
    };
    let sumo = Source {
        kind: SourceKind::Code,
        reference: "SUMO src/microsim/cfmodels/MSCFModel_Krauss.cpp (EPL-2.0, read only, \
                    never vendored): dawdle2 and the vType defaults tau = 1 s, sigma = 0.5"
            .to_string(),
        accessed: Some("2026-10-07".to_string()),
        note: None,
    };
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        MODEL_VERSION,
        "Longitudinal acceleration by Krauß's stochastic, collision-free model: drive at the \
         highest speed from which the vehicle can still stop behind a braking leader, less a \
         random dawdle. SUMO's default car-following model.",
    );
    card.tier = vec![Tier::Medium];
    card.equations = vec![
        Equation::new(
            "safe speed",
            "v_safe = v_l + (g − v_l·τ) / ((v + v_l)/(2b) + τ),  g = net gap − s0",
        ),
        Equation::new("desired speed", "v_des = min(v + a·Δt, v0, v_safe)"),
        Equation::new(
            "dawdle",
            "v' = max(0, v_des − σ·Δt·η·(v_des if v_des < a else a)),  η ~ U[0,1)",
        ),
        Equation::new(
            "acceleration",
            "a_out = (v' − v)/Δt, clamped to [−b_emergency, a]",
        ),
    ];
    let vtype = Source::new(
        SourceKind::Standard,
        "SUMO vType defaults per vClass (R10 §B4), crate::classes::VehicleClass::spec",
    );
    let pc = Krauss::class_profile(VehicleClass::Passenger);
    let tr = Krauss::class_profile(VehicleClass::Truck);
    card.parameters = vec![
        Parameter {
            name: "sigma".to_string(),
            unit: "1".to_string(),
            default: serde_json::json!(params.sigma),
            range: Some(vec![serde_json::json!(0.0), serde_json::json!(1.0)]),
            source: sumo.clone(),
            calibration: None,
        },
        Parameter::new("tau", "s", serde_json::json!(SUMO_TAU_S), sumo.clone()),
        Parameter::new(
            "a",
            "m/s²",
            serde_json::json!({"car": pc.max_accel_mps2, "truck": tr.max_accel_mps2}),
            vtype.clone(),
        ),
        Parameter::new(
            "b",
            "m/s²",
            serde_json::json!({"car": pc.comfort_decel_mps2, "truck": tr.comfort_decel_mps2}),
            vtype.clone(),
        ),
        Parameter::new(
            "s0",
            "m",
            serde_json::json!({"car": pc.min_gap_m, "truck": tr.min_gap_m}),
            vtype,
        ),
    ];
    card.parameters.extend(crate::weather::fhwa_parameters());
    card.assumptions = vec![
        "The leader brakes no harder than the follower's own b (Krauß's symmetric \
         assumption)."
            .to_string(),
        "τ is the driver's time_headway_s, so weather and per-driver heterogeneity reach it."
            .to_string(),
    ];
    card.limitations = vec![
        "Dawdling makes single-lane capacity lower and stop-and-go waves stronger than the \
         IDM's; set sigma = 0 for a deterministic variant."
            .to_string(),
        "The thesis was not re-read: the safe speed is as SUMO documents it.".to_string(),
    ];
    card.sources = vec![krauss, sumo];
    card.determinism = v2xw_core::card::Determinism {
        uses_rng: params.sigma > 0.0,
        rng_domains: if params.sigma > 0.0 {
            vec!["plugin".to_string()]
        } else {
            Vec::new()
        },
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "carfollowing::krauss::tests::stops_behind_a_standing_leader".to_string(),
            "carfollowing::krauss::tests::sigma_zero_reaches_v0".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carfollowing::testkit::{lane, run_follow, vehicle};

    #[test]
    fn sigma_zero_reaches_v0() {
        let m = Krauss::new(
            KraussParams {
                sigma: 0.0,
                ..KraussParams::default()
            },
            1,
        );
        let mut ego = vehicle(0, 0.0, m.profile(VehicleClass::Passenger));
        for _ in 0..400 {
            let a = m.accel(&ego, None, &lane(13.9), &WeatherState::default());
            ego.speed_mps = (ego.speed_mps + a * 0.1).max(0.0);
        }
        assert!((ego.speed_mps - 13.9).abs() < 1e-6, "{}", ego.speed_mps);
    }

    #[test]
    fn stops_behind_a_standing_leader() {
        let m = Krauss::new(KraussParams::default(), 7);
        let (min_gap, final_speed) = run_follow(&m, 15.0, 80.0, 0.0, 600);
        assert!(min_gap > 0.0, "collided: {min_gap}");
        assert!(final_speed < 0.05, "{final_speed}");
    }
}
