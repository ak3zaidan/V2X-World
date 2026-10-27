//! `mobility/car-following/w99` — Wiedemann's 1999 psycho-physical model (the ten-parameter
//! freeway model of PTV Vissim).
//!
//! # The model
//!
//! Wiedemann's model is a state machine over the `(Δx, Δv)` plane: thresholds of perception
//! split it into free driving, approaching, following and emergency regimes. Wiedemann's
//! 1999 formulation was never published as a paper with its equations; the executable
//! reference every open implementation follows is the MIT-licensed `w99-demo` (glgh,
//! 2016) as adopted by SUMO's `MSCFModel_W99` (EPL-2.0, read only, never vendored;
//! 2026-10-07). The thresholds, with `Δv = v_l − v` and `dx` the net gap:
//!
//! ```text
//! sdxc = CC0 + CC1·v_slow           v_slow = v if (Δv ≥ 0 or a_l < 1) else v_l + Δv·(η − ½)
//! sdxo = sdxc + CC2                 (upper end of following)
//! sdxv = sdxo + CC3·(Δv − CC4)      (perception of approach)
//! sdv  = CC6·dx²/10⁴
//! sdvc = CC4 − sdv  (v > 0)         sdvo = sdv + CC5 (v_l > CC5) else sdv
//! ```
//!
//! and the regimes, in order:
//!
//! | condition | regime | acceleration |
//! |---|---|---|
//! | `Δv < sdvo ∧ dx ≤ sdxc` | decelerate, increase distance | `min(a_l + Δv²/(CC0 − dx), 0)` (dx > CC0) or `min(a_l + ½(Δv − sdvo), 0)`, at least `CC7` braking, at most `10 − ½√v` |
//! | `Δv < sdvc ∧ dx < sdxv` | decelerate, decrease distance | `½·Δv²/(sdxc − dx − 0.1)` |
//! | `Δv < sdvo ∧ dx < sdxo` | keep distance | `min(a_prev, −CC7)` if `a_prev ≤ 0`, else `max(a_prev, CC7)` |
//! | otherwise, `dx > sdxc` | accelerate | `min(Δv²/(sdxo − dx), a_max(v))` within `sdxo`, `a_max(v)` beyond |
//!
//! # One deliberate difference from `w99-demo`
//!
//! `w99-demo` writes the acceleration ceiling as `CC8 + CC9·min(v, 80 km/h) + U[0,1)`,
//! which grows to 37 m/s² at 80 km/h and only works because SUMO caps it with the vType's
//! `accel` afterwards. Vissim's manual defines `CC8` as the desired acceleration from
//! standstill and `CC9` as the desired acceleration at 80 km/h; this implementation
//! interpolates between the two, `a_max(v) = CC8 + (CC9 − CC8)·min(v, 22.2)/22.2`. The
//! random term is dropped: under the cap it never mattered.
//!
//! # Parameter sets
//!
//! [`W99Preset::Vissim`] — PTV Vissim's documented defaults (CC0 1.50 m, CC1 0.9 s, CC2
//! 4.00 m, CC3 −8.00 s, CC4 −0.35 m/s, CC5 0.35 m/s, CC6 11.44, CC7 0.25 m/s², CC8
//! 3.50 m/s², CC9 1.50 m/s²; **secondary**: as quoted in the PTV Vissim user manual's
//! "Wiedemann 99" table and reproduced across the calibration literature, the manual itself
//! not re-read) and [`W99Preset::Sumo`] — SUMO's `MSCFModel_W99` defaults (CC1 1.30, CC2
//! 8.00, CC3 −12.00, CC4 −0.25, CC5 0.35, CC6 6.00, CC7 0.25, CC8 2.00, CC9 1.50, with
//! `minGap` 2.5 m as CC0), read from source.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;

use super::common::{self, StepClock};
use crate::classes::VehicleClass;
use crate::traits::CarFollowing;
use crate::views::{DriverProfile, LaneView, LeaderView, VehicleView};
use crate::weather::{RoadContext, WeatherResponse};

/// The model id.
pub const MODEL_ID: &str = "mobility/car-following/w99";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// 80 km/h in m/s: where `CC9` applies.
const V80_MPS: f64 = 80.0 / 3.6;

/// The two cited parameter sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum W99Preset {
    /// PTV Vissim's defaults.
    #[default]
    Vissim,
    /// SUMO's `MSCFModel_W99` defaults.
    Sumo,
}

impl W99Preset {
    /// A stable label.
    pub const fn label(self) -> &'static str {
        match self {
            W99Preset::Vissim => "vissim",
            W99Preset::Sumo => "sumo",
        }
    }

    /// `[CC0, …, CC9]`.
    pub const fn cc(self) -> [f64; 10] {
        match self {
            W99Preset::Vissim => [1.5, 0.9, 4.0, -8.0, -0.35, 0.35, 11.44, 0.25, 3.5, 1.5],
            W99Preset::Sumo => [2.5, 1.3, 8.0, -12.0, -0.25, 0.35, 6.0, 0.25, 2.0, 1.5],
        }
    }

    /// The citation.
    pub fn source(self) -> Source {
        match self {
            W99Preset::Vissim => Source {
                kind: SourceKind::Standard,
                reference: "PTV Vissim user manual, Wiedemann 99 car-following parameters \
                            CC0-CC9 (defaults)"
                    .to_string(),
                accessed: None,
                note: Some(
                    "secondary: the default table as reproduced across the Vissim calibration \
                     literature; the manual was not re-read"
                        .to_string(),
                ),
            },
            W99Preset::Sumo => Source {
                kind: SourceKind::Code,
                reference: "SUMO src/microsim/cfmodels/MSCFModel_W99.cpp constructor defaults \
                            (EPL-2.0, read only)"
                    .to_string(),
                accessed: Some("2026-10-07".to_string()),
                note: None,
            },
        }
    }
}

/// The model-wide constants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct W99Params {
    /// Which cited set (CC2–CC7 and CC9 come from it; CC0, CC1 and CC8 travel per driver
    /// as `min_gap_m`, `time_headway_s` and `max_accel_mps2`).
    pub preset: W99Preset,
    /// Which cited weather table applies.
    pub weather_response: WeatherResponse,
    /// Whether the FHWA freeway or arterial rows apply.
    pub road_context: RoadContext,
}

impl Default for W99Params {
    fn default() -> Self {
        Self {
            preset: W99Preset::Vissim,
            weather_response: WeatherResponse::Fhwa,
            road_context: RoadContext::BySpeedLimit,
        }
    }
}

/// The Wiedemann 99 model.
#[derive(Debug, Clone)]
pub struct W99 {
    params: W99Params,
    seed: u64,
    clock: StepClock,
    card: ModelCard,
}

impl W99 {
    /// The model with `params`, drawing its threshold noise from the run's master `seed`.
    pub fn new(params: W99Params, seed: u64) -> Self {
        Self {
            card: card(&params),
            params,
            seed,
            clock: StepClock::default(),
        }
    }

    /// The driver of `class`: CC0, CC1 and CC8 from the preset, the desired speed from the
    /// SUMO vType, and heavy classes accelerating no harder than their vType `accel`.
    pub fn class_profile(&self, class: VehicleClass) -> DriverProfile {
        let cc = self.params.preset.cc();
        let spec = class.spec();
        DriverProfile {
            desired_speed_mps: spec.desired_speed_mps(),
            max_accel_mps2: cc[8].min(spec.accel_mps2.max(1.0)),
            comfort_decel_mps2: spec.decel_mps2,
            time_headway_s: cc[1],
            min_gap_m: cc[0],
            sigma: None,
        }
    }

    /// The regime machine on bare numbers. `eta` is the threshold draw in `[0, 1)`.
    #[allow(clippy::too_many_arguments)]
    pub fn accel_of(
        &self,
        v: f64,
        a_prev: f64,
        leader: Option<(f64, f64, f64)>,
        d: &DriverProfile,
        eta: f64,
    ) -> f64 {
        let cc = self.params.preset.cc();
        let (cc0, cc1) = (d.min_gap_m, d.time_headway_s);
        let (cc2, cc3, cc4, cc5, cc6, cc7) = (cc[2], cc[3], cc[4], cc[5], cc[6], cc[7]);
        let cc8 = d.max_accel_mps2;
        let cc9 = cc[9].min(cc8);
        let a_max = cc8 + (cc9 - cc8) * v.min(V80_MPS) / V80_MPS;
        let Some((dx, v_l, a_l)) = leader else {
            return a_max;
        };
        let dv = v_l - v;
        let mut sdxc = cc0;
        if v_l > 0.0 {
            let v_slow = if dv >= 0.0 || a_l < 1.0 {
                v
            } else {
                v_l + dv * (eta - 0.5)
            };
            sdxc += cc1 * v_slow.max(0.0);
        }
        let sdxo = sdxc + cc2;
        let sdxv = sdxo + cc3 * (dv - cc4);
        let sdv = cc6 * dx * dx / 10_000.0;
        let sdvc = if v > 0.0 { cc4 - sdv } else { 0.0 };
        let sdvo = if v_l > cc5 { sdv + cc5 } else { sdv };
        if dv < sdvo && dx <= sdxc {
            let mut acc: f64 = 0.0;
            if v_l > 0.0 {
                if dv < 0.0 {
                    acc = if dx > cc0 {
                        (a_l + dv * dv / (cc0 - dx)).min(0.0)
                    } else {
                        (a_l + 0.5 * (dv - sdvo)).min(0.0)
                    };
                }
                acc = if acc > -cc7 {
                    -cc7
                } else {
                    acc.max(-10.0 + 0.5 * math::sqrt(v))
                };
            }
            acc
        } else if dv < sdvc && dx < sdxv {
            0.5 * dv * dv / (sdxc - dx - 0.1)
        } else if dv < sdvo && dx < sdxo {
            if a_prev <= 0.0 {
                a_prev.min(-cc7)
            } else {
                a_prev.max(cc7)
            }
        } else if dx > sdxc {
            if dx < sdxo {
                (dv * dv / (sdxo - dx)).min(a_max)
            } else {
                a_max
            }
        } else {
            0.0
        }
    }
}

impl v2xw_core::model::Model for W99 {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl CarFollowing for W99 {
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
        let dt = self.clock.dt_s();
        let v = ego.speed_mps;
        let eta = if v > 0.0 {
            common::keyed_uniform(self.seed, MODEL_ID, ego.actor, self.clock.step_index(), 0)
        } else {
            1.0
        };
        let mut a = self.accel_of(
            v,
            ego.accel_mps2,
            leader.map(|l| (l.gap_m, l.speed_mps, l.accel_mps2)),
            &d,
            eta,
        );
        // Capped at the desired speed, as SUMO's finalizeSpeed caps every model.
        a = a.min((d.desired_speed_mps - v) / dt);
        // SUMO's addition to w99-demo (issue #10472): never drive into the standstill gap's
        // last tenth within one step.
        if let Some(l) = leader
            && l.gap_m - v * dt < d.min_gap_m * 0.1
        {
            a = a.min(-v / dt);
        }
        let emergency = ego.class.spec().emergency_decel_mps2;
        a.clamp(-emergency, d.max_accel_mps2)
            .max(-sit.grip_decel_mps2)
    }

    fn profile(&self, class: VehicleClass) -> DriverProfile {
        self.class_profile(class)
    }

    fn begin_step(&self, now: SimTime, dt: Duration) {
        self.clock.begin(now, dt);
    }
}

/// The model card.
pub fn card(params: &W99Params) -> ModelCard {
    let demo = Source::new(
        SourceKind::Code,
        "w99-demo (glgh, 2016, MIT) as adopted by SUMO src/microsim/cfmodels/MSCFModel_W99.cpp \
         (EPL-2.0, read only, 2026-10-07): the regime thresholds and accelerations",
    );
    let src = params.preset.source();
    let cc = params.preset.cc();
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        MODEL_VERSION,
        "Longitudinal acceleration by Wiedemann's 1999 psycho-physical model: perception \
         thresholds in the gap/speed-difference plane switch between free driving, \
         approaching, following and emergency braking. The car-following model of PTV Vissim.",
    );
    card.tier = vec![Tier::Medium];
    card.equations = vec![
        Equation::new(
            "thresholds",
            "sdxc = CC0 + CC1·v_slow; sdxo = sdxc + CC2; sdxv = sdxo + CC3(Δv − CC4); \
             sdv = CC6·dx²/1e4; sdvc = CC4 − sdv; sdvo = sdv + CC5",
        ),
        Equation::new(
            "ceiling",
            "a_max(v) = CC8 + (CC9 − CC8)·min(v, 80 km/h)/(80 km/h)",
        ),
    ];
    let names = [
        "CC0", "CC1", "CC2", "CC3", "CC4", "CC5", "CC6", "CC7", "CC8", "CC9",
    ];
    let units = [
        "m", "s", "m", "s", "m/s", "m/s", "1/(m·s)", "m/s²", "m/s²", "m/s²",
    ];
    card.parameters = names
        .iter()
        .zip(units)
        .zip(cc)
        .map(|((n, u), v)| Parameter::new(*n, u, serde_json::json!(v), src.clone()))
        .collect();
    card.parameters.push(Parameter::new(
        "preset",
        "-",
        serde_json::json!(params.preset.label()),
        src.clone(),
    ));
    card.parameters.extend(crate::weather::fhwa_parameters());
    card.limitations = vec![
        "Wiedemann's own 1999 equations are unpublished; this is the open executable reading \
         (w99-demo / SUMO), with the acceleration ceiling interpolated between CC8 and CC9 \
         as Vissim's manual defines them rather than w99-demo's unbounded sum."
            .to_string(),
        "Known to violate the standstill gap occasionally (SUMO sets collisionMinGapFactor \
         0.1 for it); the one-step guard of SUMO issue #10472 is kept."
            .to_string(),
    ];
    card.sources = vec![src, demo];
    card.determinism = v2xw_core::card::Determinism {
        uses_rng: true,
        rng_domains: vec!["plugin".to_string()],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec!["carfollowing::w99::tests::stops_behind_a_standing_leader".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carfollowing::testkit::run_follow;

    #[test]
    fn stops_behind_a_standing_leader() {
        for preset in [W99Preset::Vissim, W99Preset::Sumo] {
            let m = W99::new(
                W99Params {
                    preset,
                    ..W99Params::default()
                },
                3,
            );
            let (min_gap, v) = run_follow(&m, 15.0, 120.0, 0.0, 900);
            assert!(min_gap > 0.0, "{preset:?} collided: {min_gap}");
            assert!(v < 0.3, "{preset:?} still moving at {v}");
        }
    }
}
