//! `mobility/car-following/acc` — adaptive cruise control, the PATH multi-mode controller.
//!
//! # The controller
//!
//! Milanés & Shladover, "Modeling cooperative and autonomous adaptive cruise control
//! dynamic responses using experimental data", *Transportation Research Part C*
//! 48:285–300, 2014, identified the gap-regulation law of a production ACC from field
//! data: `a = k1·e + k2·Δv`, `e = g − t_hw·v`, with **k1 = 0.23 s⁻² and k2 = 0.07 s⁻¹**.
//! Xiao, Wang & van Arem, "Realistic car-following models for microscopic simulation of
//! adaptive and cooperative adaptive cruise control vehicles", *Transportation Research
//! Record* 2623:1–9, 2017, made it a microsimulation model with three modes and a speed
//! controller, and Xiao, Wang, Schakel & van Arem 2018 (*Transportation Research Part C*
//! 96) added a dynamic gap margin `d0`. With `Δv = v_l − v`, `g` the net gap less `s0`:
//!
//! ```text
//! speed control  (g > 120 m):  a = −k_sc·(v − v0),             k_sc = 0.4 s⁻¹
//! gap control    (g < 100 m):  e = g − t_hw·v − d0,  d0 = max(0, min(75/v − 5, 2))
//!     gap        |e| < 0.2 m and |v − v0| < 0.1 m/s:  a = 0.07·Δv + 0.23·e
//!     collision  e < 0:                               a = 0.23·Δv + 0.8·e
//!     closing    otherwise:                           a = 0.8·Δv + 0.04·e
//! between:       the lower of the two (SUMO keeps the previous mode; see below)
//! ```
//!
//! The gains and thresholds are SUMO's `MSCFModel_ACC` defaults (EPL-2.0, read only,
//! never vendored; 2026-10-07), which cite these three papers. Two deliberate differences
//! from SUMO, both recorded on the card:
//!
//! * **Between 100 m and 120 m** SUMO holds the previously applied law, which needs
//!   per-vehicle memory; this takes the lower of the two, which is the same law whenever
//!   the gap is being closed from far away (speed control is the lower) and never brakes
//!   harder than gap control would.
//! * **A virtual leader** (a stop line, a red signal, a yield) is handed to the IDM: a
//!   production ACC does not stop for signals, its driver does, and the linear law above
//!   was identified on vehicle targets only.
//!
//! A safety envelope caps the controller the way SUMO caps its ACC/CACC: never faster
//! than Krauß's safe speed (emergency deceleration, one step of reaction) plus an override
//! of at most 2 m/s (`DEFAULT_EMERGENCY_OVERRIDE_THRESHOLD`).

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;

use super::common::{self, StepClock};
use super::idm::{Idm, IdmPreset};
use crate::classes::VehicleClass;
use crate::traits::CarFollowing;
use crate::views::{DriverProfile, LaneView, LeaderView, VehicleView};
use crate::weather::{RoadContext, WeatherResponse};

/// The model id.
pub const MODEL_ID: &str = "mobility/car-following/acc";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// The ACC controller's constants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccParams {
    /// The time gap the ACC regulates to, seconds. Milanés & Shladover 2014's shortest
    /// production-ACC setting (1.1 s; they tested 1.1–2.2 s).
    pub time_gap_s: f64,
    /// Speed-control gain `k_sc`, s⁻¹ (applied as `−k_sc·(v − v0)`).
    pub speed_gain: f64,
    /// Gap mode `(k_Δv, k_e)`: Milanés & Shladover 2014's `(k2, k1)`.
    pub gap_gains: (f64, f64),
    /// Collision-avoidance mode `(k_Δv, k_e)`.
    pub collision_gains: (f64, f64),
    /// Gap-closing mode `(k_Δv, k_e)`.
    pub closing_gains: (f64, f64),
    /// Above this gap the speed controller applies, metres.
    pub speed_control_gap_m: f64,
    /// Below this gap the gap controller applies, metres.
    pub gap_control_gap_m: f64,
    /// The emergency override above the safe speed, m/s.
    pub emergency_override_mps: f64,
    /// The driver parameter set (`a_max`, `b`, `s0`, desired speed).
    pub preset: IdmPreset,
    /// Which cited weather table applies.
    pub weather_response: WeatherResponse,
    /// Whether the FHWA freeway or arterial rows apply.
    pub road_context: RoadContext,
}

impl Default for AccParams {
    fn default() -> Self {
        Self {
            time_gap_s: 1.1,
            speed_gain: 0.4,
            gap_gains: (0.07, 0.23),
            collision_gains: (0.23, 0.8),
            closing_gains: (0.8, 0.04),
            speed_control_gap_m: 120.0,
            gap_control_gap_m: 100.0,
            emergency_override_mps: 2.0,
            preset: IdmPreset::UrbanHcm,
            weather_response: WeatherResponse::Fhwa,
            road_context: RoadContext::BySpeedLimit,
        }
    }
}

/// The ACC model.
#[derive(Debug, Clone)]
pub struct Acc {
    params: AccParams,
    fallback: Idm,
    clock: StepClock,
    card: ModelCard,
}

impl Default for Acc {
    fn default() -> Self {
        Acc::new(AccParams::default())
    }
}

impl Acc {
    /// The model with `params`.
    pub fn new(params: AccParams) -> Self {
        Self {
            card: card(&params),
            fallback: Idm::new(params.preset),
            params,
            clock: StepClock::default(),
        }
    }

    /// The parameters in force.
    pub fn params(&self) -> &AccParams {
        &self.params
    }

    /// The speed controller, m/s².
    pub fn speed_control(&self, v: f64, v0: f64) -> f64 {
        -self.params.speed_gain * (v - v0)
    }

    /// The gap controller (Xiao et al. 2017/2018 modes), m/s². `g` is the net gap less
    /// `s0`.
    pub fn gap_control(&self, v: f64, v0: f64, g: f64, v_l: f64, t_gap: f64) -> f64 {
        let p = &self.params;
        let dv = v_l - v;
        let d0 = if v > 1e-6 {
            (75.0 / v - 5.0).clamp(0.0, 2.0)
        } else {
            2.0
        };
        let e = g - t_gap * v - d0;
        let (kv, ke) = if e.abs() < 0.2 && (v - v0).abs() < 0.1 {
            p.gap_gains
        } else if e < 0.0 {
            p.collision_gains
        } else {
            p.closing_gains
        };
        kv * dv + ke * e
    }

    /// The controller's mode choice by gap, m/s².
    pub fn controller(&self, v: f64, v0: f64, g: f64, v_l: f64, t_gap: f64) -> f64 {
        let p = &self.params;
        if g > p.speed_control_gap_m {
            self.speed_control(v, v0)
        } else if g < p.gap_control_gap_m {
            self.gap_control(v, v0, g, v_l, t_gap)
        } else {
            self.speed_control(v, v0)
                .min(self.gap_control(v, v0, g, v_l, t_gap))
        }
    }

    /// The safety envelope: the highest acceleration that keeps the vehicle at or below
    /// Krauß's safe speed (emergency deceleration, one step of reaction) plus the override.
    pub fn safety_cap(&self, v: f64, g: f64, v_l: f64, emergency: f64, dt: f64) -> f64 {
        let v_safe = common::krauss_safe_speed(v, v_l, g, emergency, dt);
        (v_safe + self.params.emergency_override_mps.min(g.max(0.0)) - v) / dt
    }

    /// The whole ACC acceleration with a time gap of the caller's choosing — what a CACC
    /// that has lost its messages degrades to.
    pub fn accel_with_time_gap(
        &self,
        ego: &VehicleView,
        leader: Option<&LeaderView>,
        lane: &LaneView,
        w: &WeatherState,
        t_gap: f64,
        dt: f64,
    ) -> f64 {
        let sit = common::situation(
            ego,
            lane,
            w,
            self.params.weather_response,
            self.params.road_context,
        );
        let d = sit.driver;
        let v = ego.speed_mps;
        let emergency = ego.class.spec().emergency_decel_mps2;
        let a = match leader {
            Some(l) if !l.is_vehicle() => {
                return self.fallback.accel(ego, Some(l), lane, w);
            }
            None => self.speed_control(v, d.desired_speed_mps),
            Some(l) => {
                let g = l.gap_m - d.min_gap_m;
                self.controller(v, d.desired_speed_mps, g, l.speed_mps, t_gap)
                    .min(self.safety_cap(v, g, l.speed_mps, emergency, dt))
            }
        };
        finish(a, v, &d, emergency, sit.grip_decel_mps2, dt)
    }

    /// The driver of `class`: the preset's, with the ACC's time gap.
    pub fn class_profile(&self, class: VehicleClass) -> DriverProfile {
        DriverProfile {
            time_headway_s: self.params.time_gap_s,
            ..self.params.preset.profile(class)
        }
    }
}

/// The common last step of every ACC/CACC law: never past the desired speed in one step,
/// never beyond `[−emergency, a_max]`, never harder than the surface grips.
pub(crate) fn finish(a: f64, v: f64, d: &DriverProfile, emergency: f64, grip: f64, dt: f64) -> f64 {
    a.min((d.desired_speed_mps - v) / dt)
        .clamp(-emergency, d.max_accel_mps2)
        .max(-grip)
}

impl v2xw_core::model::Model for Acc {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl CarFollowing for Acc {
    fn accel(
        &self,
        ego: &VehicleView,
        leader: Option<&LeaderView>,
        lane: &LaneView,
        w: &WeatherState,
    ) -> f64 {
        // The ACC's own setting, not the driver's drawn headway: a controller holds the gap
        // it is set to. The weather still lengthens it, as a careful driver would set it.
        let p = &self.params;
        let sit = common::situation(ego, lane, w, p.weather_response, p.road_context);
        let t_gap = p.time_gap_s * sit.headway_factor;
        self.accel_with_time_gap(ego, leader, lane, w, t_gap, self.clock.dt_s())
    }

    fn profile(&self, class: VehicleClass) -> DriverProfile {
        self.class_profile(class)
    }

    fn begin_step(&self, now: SimTime, dt: Duration) {
        self.clock.begin(now, dt);
    }
}

/// The sources every ACC/CACC card cites.
pub(crate) fn acc_sources() -> Vec<Source> {
    vec![
        Source::new(
            SourceKind::Paper,
            "Milanés & Shladover 2014, Modeling cooperative and autonomous adaptive cruise \
             control dynamic responses using experimental data, Transportation Research C \
             48:285-300 (DOI 10.1016/j.trc.2014.09.001)",
        ),
        Source::new(
            SourceKind::Paper,
            "Xiao, Wang & van Arem 2017, Realistic car-following models for microscopic \
             simulation of adaptive and cooperative adaptive cruise control vehicles, \
             Transportation Research Record 2623:1-9 (DOI 10.3141/2623-01)",
        ),
        Source::new(
            SourceKind::Paper,
            "Xiao, Wang, Schakel & van Arem 2018, Unravelling effects of cooperative adaptive \
             cruise control deactivation on traffic flow characteristics at merging \
             bottlenecks, Transportation Research C 96:380-397 (eq. 5, the dynamic margin d0)",
        ),
        Source {
            kind: SourceKind::Code,
            reference: "SUMO src/microsim/cfmodels/MSCFModel_ACC.cpp and MSCFModel_CACC.cpp \
                        (EPL-2.0, read only, never vendored): gains, thresholds, override"
                .to_string(),
            accessed: Some("2026-10-07".to_string()),
            note: None,
        },
    ]
}

/// The model card.
pub fn card(params: &AccParams) -> ModelCard {
    let src = acc_sources();
    let milanes = src[0].clone();
    let xiao = src[1].clone();
    let sumo = src[3].clone();
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        MODEL_VERSION,
        "Longitudinal acceleration of a vehicle under adaptive cruise control: the PATH \
         multi-mode controller identified from production ACC field data. Radar only — no \
         V2X. What every CACC in the library degrades to when its messages are lost or late.",
    );
    card.tier = vec![Tier::Medium];
    card.equations = vec![
        Equation::new("speed control", "a = −k_sc·(v − v0)   (g > 120 m)"),
        Equation::new(
            "gap control",
            "e = g − t_hw·v − d0, d0 = max(0, min(75/v − 5, 2)); a = k_Δv·Δv + k_e·e by mode \
             (gap / collision avoidance / closing)",
        ),
        Equation::new(
            "safety envelope",
            "a ≤ (v_safe(emergency decel, Δt) + min(2 m/s, g) − v)/Δt",
        ),
    ];
    card.parameters = vec![
        Parameter::new(
            "time_gap",
            "s",
            serde_json::json!(params.time_gap_s),
            milanes.clone(),
        ),
        Parameter::new(
            "speed_gain",
            "1/s",
            serde_json::json!(params.speed_gain),
            xiao.clone(),
        ),
        Parameter::new(
            "gap_gains",
            "1/s, 1/s²",
            serde_json::json!([params.gap_gains.0, params.gap_gains.1]),
            milanes,
        ),
        Parameter::new(
            "collision_gains",
            "1/s, 1/s²",
            serde_json::json!([params.collision_gains.0, params.collision_gains.1]),
            sumo.clone(),
        ),
        Parameter::new(
            "closing_gains",
            "1/s, 1/s²",
            serde_json::json!([params.closing_gains.0, params.closing_gains.1]),
            sumo.clone(),
        ),
        Parameter::new(
            "speed_control_gap",
            "m",
            serde_json::json!(params.speed_control_gap_m),
            sumo.clone(),
        ),
        Parameter::new(
            "gap_control_gap",
            "m",
            serde_json::json!(params.gap_control_gap_m),
            sumo.clone(),
        ),
        Parameter::new(
            "emergency_override",
            "m/s",
            serde_json::json!(params.emergency_override_mps),
            sumo,
        ),
        Parameter::new(
            "preset",
            "-",
            serde_json::json!(params.preset.label()),
            params.preset.source(),
        ),
    ];
    card.parameters.extend(crate::weather::fhwa_parameters());
    card.limitations = vec![
        "Between 100 m and 120 m the lower of speed and gap control applies, where SUMO \
         keeps the previously applied law (per-vehicle memory this trait does not carry)."
            .to_string(),
        "Virtual leaders (stop lines, signals, yields) are handed to the IDM with the same \
         driver: the ACC law was identified on vehicle targets only."
            .to_string(),
        "String stable in the platoon experiment at 1.1 s (acceleration L2 gain 0.967), \
         where Milanés & Shladover 2014 measured production ACCs to be string-unstable: the \
         heavily damped gap-closing mode (0.8·Δv) dominates the response. Not tuned away; \
         a calibration against their trajectories is the open item."
            .to_string(),
    ];
    card.sources = src;
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "carfollowing::acc::tests::settles_at_the_time_gap".to_string(),
            "carfollowing::acc::tests::stops_behind_a_standing_leader".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carfollowing::testkit::{lane, run_follow, vehicle};
    use v2xw_core::geom::Dims;
    use v2xw_core::ids::ActorId;

    #[test]
    fn stops_behind_a_standing_leader() {
        let m = Acc::default();
        let (min_gap, v) = run_follow(&m, 15.0, 80.0, 0.0, 600);
        assert!(min_gap > 1.0, "{min_gap}");
        assert!(v < 0.05, "{v}");
    }

    #[test]
    fn settles_at_the_time_gap() {
        let m = Acc::default();
        let w = WeatherState::default();
        let mut ego = vehicle(1, 20.0, m.profile(VehicleClass::Passenger));
        ego.driver.desired_speed_mps = 30.0;
        let mut lead = vehicle(0, 20.0, ego.driver);
        lead.dims = Dims::new(5.0, 1.8, 1.5);
        lead.actor = ActorId::new(0);
        let mut gap = 60.0;
        let dt = Duration::from_millis(100);
        for k in 0..3000u64 {
            m.begin_step(k * dt.as_nanos(), dt);
            let a = m.accel(&ego, Some(&LeaderView::of(lead, gap)), &lane(30.0), &w);
            let v1 = (ego.speed_mps + a * 0.1).max(0.0);
            gap += (20.0 - 0.5 * (ego.speed_mps + v1)) * 0.1;
            ego.accel_mps2 = a;
            ego.speed_mps = v1;
        }
        // e = g − s0 − t_hw·v − d0 → 0, with d0 = 0 at 20 m/s (75/20 − 5 < 0).
        let want = ego.driver.min_gap_m + 1.1 * 20.0;
        assert!((gap - want).abs() < 0.5, "gap {gap}, want {want}");
        assert!((ego.speed_mps - 20.0).abs() < 0.05);
    }
}
