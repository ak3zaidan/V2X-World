//! Cooperative adaptive cruise control and platooning that run on **received** V2X data.
//!
//! Three published control laws, each a plug-in with its own id and card, sharing one
//! degradation rule: when the follower holds no BSM or CAM from its radar target younger
//! than [`crate::coop::CoopParams::max_age_s`], it drives the [`super::acc::Acc`] law at the
//! ACC time gap instead. Nothing here reads the leader's true acceleration: the
//! cooperative terms come from [`crate::coop::CoopLink`], which only the simulated network
//! fills, so loss, latency, congestion and security outcomes all reach the platoon.
//!
//! | Law | Id | Uses from V2X | Spacing policy |
//! |---|---|---|---|
//! | [`CaccLaw::Milanes`] | `mobility/car-following/cacc-milanes` | predecessor speed (extrapolated by message age) | constant time gap 0.6 s |
//! | [`CaccLaw::Ploeg`] | `mobility/car-following/cacc-ploeg` | predecessor speed and acceleration (feed-forward) | constant time gap 0.7 s |
//! | [`CaccLaw::Path`] | `mobility/car-following/cacc-path` | predecessor **and platoon leader** speed and acceleration | constant spacing 5 m |
//!
//! # The laws
//!
//! **Milanés & Shladover 2014** (Transportation Research C 48:285–300), as Xiao, Wang &
//! van Arem 2017 (TRR 2623) made it a three-mode microsimulation model and SUMO's
//! `MSCFModel_CACC` implements it (gains read 2026-10-07, EPL-2.0, read only). With
//! `e = g − h·v` (`g` net gap less `s0`) and `ė = v_p − v − h·a`, the paper's speed
//! command `v_ref = v + k_p·e + k_d·ė` is issued every 0.1 s; written as an acceleration,
//! independent of the simulation step, `a = (k_p·e + k_d·ė)/0.1 s`:
//!
//! ```text
//! gap mode        0 < e < 0.2 m, v − v0 < 0.1 m/s:  (k_p, k_d) = (0.45, 0.0125)
//! collision       e < 0:                             (0.45, 0.05)
//! closing         otherwise:                         (0.005, 0.05)
//! speed control   g/v > 2 s and e > 1.66 m:          a = −0.4·(v − v0)
//! ```
//!
//! (SUMO applies the increment once per simulation step, so its CACC behaves differently
//! at Δt = 1 s and Δt = 0.1 s; dividing by the paper's own 0.1 s makes it step-independent.)
//!
//! **Ploeg, Scheepers, van Nunen, van de Wouw & Nijmeijer 2011**, "Design and experimental
//! evaluation of cooperative adaptive cruise control", IEEE ITSC 2011, pp. 260–265: the
//! controller state `u` obeys `h·u̇ = −u + k_p·e + k_d·ė + u_p` with `u_p` the predecessor's
//! *communicated* acceleration, `h` = 0.7 s, `k_p` = 0.2 s⁻², `k_d` = 0.7 s⁻¹, and a
//! first-order driveline lag `τ` = 0.1 s. Discretised forward-Euler at the mobility step
//! with the previous applied acceleration as `u`.
//!
//! **The PATH platoon controller** (Rajamani, *Vehicle Dynamics and Control*, 2nd ed.,
//! Springer 2012, §7.x; the parameterisation of Plexe, Segata et al. 2014, "PLEXE: A
//! platooning extension for Veins", IEEE VNC, defaults `C1` = 0.5, `ξ` = 1, `ω_n` =
//! 0.2, spacing 5 m, driveline lag 0.5 s):
//!
//! ```text
//! u = α1·u_p + α2·u_0 + α3·ε̇ + α4·(v − v_0) + α5·ε
//! ε = s_des − gap,  ε̇ = v − v_p
//! α1 = 1 − C1, α2 = C1, α3 = −(2ξ − C1(ξ + √(ξ²−1)))·ω_n, α4 = −C1(ξ + √(ξ²−1))·ω_n, α5 = −ω_n²
//! ```
//!
//! where `(u_0, v_0)` is the platoon leader's — the head of the follower's associated
//! predecessor chain ([`crate::coop::CoopLink::platoon_head`]); without a fresh message
//! from *both* the predecessor and the head the vehicle degrades to ACC.
//!
//! Every law ends in the same safety envelope as the ACC ([`super::acc::Acc::safety_cap`]),
//! the desired-speed cap and the surface's grip.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;

use super::acc::{self, Acc, AccParams};
use super::common::{self, StepClock};
use crate::classes::VehicleClass;
use crate::coop::{CoopInput, CoopLink};
use crate::traits::CarFollowing;
use crate::views::{DriverProfile, LaneView, LeaderView, VehicleView};

/// The model version (all three laws).
pub const MODEL_VERSION: &str = "1.0.0";

/// Which control law.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaccLaw {
    /// Milanés & Shladover 2014, the PATH field CACC.
    Milanes,
    /// Ploeg et al. 2011, acceleration feed-forward.
    Ploeg,
    /// The PATH leader–predecessor platoon controller (Rajamani 2012, Plexe).
    Path,
}

impl CaccLaw {
    /// Every law.
    pub const ALL: [CaccLaw; 3] = [CaccLaw::Milanes, CaccLaw::Ploeg, CaccLaw::Path];

    /// The model id.
    pub const fn model_id(self) -> &'static str {
        match self {
            CaccLaw::Milanes => "mobility/car-following/cacc-milanes",
            CaccLaw::Ploeg => "mobility/car-following/cacc-ploeg",
            CaccLaw::Path => "mobility/car-following/cacc-path",
        }
    }
}

/// One law's constants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaccParams {
    /// The cooperative time gap `h`, seconds (Milanés 0.6, Ploeg 0.7; unused by PATH).
    pub time_gap_s: f64,
    /// Milanés: `(k_p, k_d)` of gap mode.
    pub gap_gains: (f64, f64),
    /// Milanés: `(k_p, k_d)` of collision avoidance.
    pub collision_gains: (f64, f64),
    /// Milanés: `(k_p, k_d)` of gap closing.
    pub closing_gains: (f64, f64),
    /// Milanés: the interval the speed command is issued at, seconds.
    pub command_interval_s: f64,
    /// Milanés: the speed-control minimum spacing error, metres.
    pub speed_control_min_gap_m: f64,
    /// Ploeg: `(k_p, k_d)`.
    pub ploeg_gains: (f64, f64),
    /// PATH: `C1`.
    pub path_c1: f64,
    /// PATH: `ξ`.
    pub path_xi: f64,
    /// PATH: `ω_n`, rad/s.
    pub path_omega_n: f64,
    /// PATH: the constant spacing, metres (net gap).
    pub path_spacing_m: f64,
    /// PATH: the longest predecessor chain followed to find the head.
    pub path_max_platoon: usize,
    /// Graceful degradation: how long, after the last message went stale, the fallback
    /// ACC takes to open the gap from the cooperative time gap to its own, seconds. 0 is
    /// the abrupt switch.
    pub degrade_ramp_s: f64,
    /// First-order driveline lag applied to the law's output, seconds (0 = none).
    pub actuator_lag_s: f64,
    /// The ACC this law degrades to.
    pub acc: AccParams,
}

impl Default for CaccParams {
    /// The Milanés & Shladover set; a scenario's partial `params` are merged over the
    /// selected law's own set instead (`library::cacc_build`).
    fn default() -> Self {
        Self::for_law(CaccLaw::Milanes)
    }
}

impl CaccParams {
    /// The cited set for `law`.
    pub fn for_law(law: CaccLaw) -> Self {
        let base = Self {
            time_gap_s: 0.6,
            gap_gains: (0.45, 0.0125),
            collision_gains: (0.45, 0.05),
            closing_gains: (0.005, 0.05),
            command_interval_s: 0.1,
            speed_control_min_gap_m: 1.66,
            ploeg_gains: (0.2, 0.7),
            path_c1: 0.5,
            path_xi: 1.0,
            path_omega_n: 0.2,
            path_spacing_m: 5.0,
            path_max_platoon: 32,
            degrade_ramp_s: 5.0,
            actuator_lag_s: 0.0,
            acc: AccParams::default(),
        };
        match law {
            CaccLaw::Milanes => base,
            CaccLaw::Ploeg => Self {
                time_gap_s: 0.7,
                actuator_lag_s: 0.1,
                ..base
            },
            // A 5 m platoon that loses its messages must open the gap at once: ramping
            // the fallback ACC up from the platoon's ~0.2 s equivalent time gap left it
            // too close to brake for its predecessor, and the platoon sweep recorded
            // collisions at 50 % loss. The abrupt switch is the safe default here.
            CaccLaw::Path => Self {
                actuator_lag_s: 0.5,
                degrade_ramp_s: 0.0,
                ..base
            },
        }
    }
}

/// A cooperative car-following model.
#[derive(Debug, Clone)]
pub struct Cacc {
    law: CaccLaw,
    params: CaccParams,
    acc: Acc,
    link: Arc<CoopLink>,
    clock: StepClock,
    card: ModelCard,
}

impl Cacc {
    /// `law` with its cited constants, reading `link`.
    pub fn new(law: CaccLaw, link: Arc<CoopLink>) -> Self {
        Self::with_params(law, CaccParams::for_law(law), link)
    }

    /// `law` with constants of the caller's own.
    pub fn with_params(law: CaccLaw, params: CaccParams, link: Arc<CoopLink>) -> Self {
        Self {
            card: card(law, &params, link.params()),
            acc: Acc::new(params.acc),
            law,
            params,
            link,
            clock: StepClock::default(),
        }
    }

    /// The law.
    pub fn law(&self) -> CaccLaw {
        self.law
    }

    /// The parameters in force.
    pub fn params(&self) -> &CaccParams {
        &self.params
    }

    /// The cooperative law's raw output, given fresh predecessor data `p` (and, for PATH,
    /// head data `h0`). `g` is the net gap less `s0`; `gap` the net gap.
    #[allow(clippy::too_many_arguments)]
    pub fn law_output(
        &self,
        v: f64,
        a_prev: f64,
        v0: f64,
        g: f64,
        gap: f64,
        h: f64,
        p: CoopInput,
        h0: Option<CoopInput>,
        dt: f64,
    ) -> Option<f64> {
        let q = &self.params;
        match self.law {
            CaccLaw::Milanes => {
                let e = g - h * v;
                let v_err = v - v0;
                let speed = -q.acc.speed_gain * v_err;
                let gap_ctl = || {
                    let e_dot = p.speed_mps - v - h * a_prev;
                    let (kp, kd) = if e > 0.0 && e < 0.2 && v_err < 0.1 {
                        q.gap_gains
                    } else if e < 0.0 {
                        q.collision_gains
                    } else {
                        q.closing_gains
                    };
                    (kp * e + kd * e_dot) / q.command_interval_s
                };
                let time_gap = gap / v.max(1e-3);
                Some(if time_gap > 2.0 && e > q.speed_control_min_gap_m {
                    speed
                } else if time_gap < 1.5 {
                    gap_ctl()
                } else {
                    speed.min(gap_ctl())
                })
            }
            CaccLaw::Ploeg => {
                let (kp, kd) = q.ploeg_gains;
                let e = g - h * v;
                let e_dot = p.speed_mps - v - h * a_prev;
                Some(a_prev + dt / h.max(0.05) * (-a_prev + kp * e + kd * e_dot + p.accel_mps2))
            }
            CaccLaw::Path => {
                let h0 = h0?;
                let (c1, xi, wn) = (q.path_c1, q.path_xi, q.path_omega_n);
                let root = math::sqrt((xi * xi - 1.0).max(0.0));
                let a1 = 1.0 - c1;
                let a2 = c1;
                let a3 = -(2.0 * xi - c1 * (xi + root)) * wn;
                let a4 = -c1 * (xi + root) * wn;
                let a5 = -wn * wn;
                let eps = q.path_spacing_m - gap;
                let eps_dot = v - p.speed_mps;
                Some(
                    a1 * p.accel_mps2
                        + a2 * h0.accel_mps2
                        + a3 * eps_dot
                        + a4 * (v - h0.speed_mps)
                        + a5 * eps,
                )
            }
        }
    }

    /// The fallback ACC's time gap while degraded — graceful degradation.
    ///
    /// An abrupt switch from a 0.6–0.7 s cooperative gap to the ACC's 1.1 s makes every
    /// lost message a braking transient (the ACC sees a large negative spacing error and
    /// enters collision avoidance), and with 10 % loss the platoon goes string-unstable on
    /// the switching alone (the platoon sweep shows it). That is the problem of Ploeg,
    /// Semsar-Kazerooni, Lijster, van de Wouw & Nijmeijer 2015, "Graceful degradation of
    /// cooperative adaptive cruise
    /// control", IEEE T-ITS 16(1):488-497. Here the gap opens linearly from the
    /// cooperative one to the ACC's over [`CaccParams::degrade_ramp_s`], timed from when
    /// the last message from this leader went stale: stateless, because the link's record
    /// age is the timer. A leader never heard from gets the ACC gap at once.
    fn degraded_time_gap(&self, ego: &VehicleView, l: &LeaderView, h_coop: f64) -> f64 {
        let h_acc = self.acc.params().time_gap_s;
        let h_start = match self.law {
            CaccLaw::Path => self.params.path_spacing_m / ego.speed_mps.max(5.0),
            _ => h_coop,
        }
        .min(h_acc);
        let Some(lead) = l.vehicle.as_ref() else {
            return h_acc;
        };
        let Some(rec) = self.link.latest(ego.actor, lead.actor) else {
            return h_acc;
        };
        let ramp = self.params.degrade_ramp_s;
        if ramp <= 0.0 {
            return h_acc;
        }
        let stale_for = rec.age_s(self.clock.now()) - self.link.params().max_age_s;
        h_start + (h_acc - h_start) * (stale_for / ramp).clamp(0.0, 1.0)
    }

    /// What the step's data allow: the law's output, or `None` to degrade.
    fn cooperative_accel(
        &self,
        ego: &VehicleView,
        l: &LeaderView,
        d: &DriverProfile,
        h: f64,
        dt: f64,
    ) -> Option<f64> {
        let lead = l.vehicle.as_ref()?;
        let now = self.clock.now();
        let p = self.link.fresh(ego.actor, lead.actor, now)?;
        let h0 = if self.law == CaccLaw::Path {
            let head = self
                .link
                .platoon_head(ego.actor, self.params.path_max_platoon)
                .unwrap_or(lead.actor);
            if head == lead.actor {
                Some(p)
            } else {
                Some(self.link.fresh(ego.actor, head, now)?)
            }
        } else {
            None
        };
        let g = l.gap_m - d.min_gap_m;
        let u = self.law_output(
            ego.speed_mps,
            ego.accel_mps2,
            d.desired_speed_mps,
            g,
            l.gap_m,
            h,
            p,
            h0,
            dt,
        )?;
        let tau = self.params.actuator_lag_s;
        Some(if tau > 0.0 {
            ego.accel_mps2 + (u - ego.accel_mps2) * (dt / tau).min(1.0)
        } else {
            u
        })
    }
}

impl v2xw_core::model::Model for Cacc {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl CarFollowing for Cacc {
    fn accel(
        &self,
        ego: &VehicleView,
        leader: Option<&LeaderView>,
        lane: &LaneView,
        w: &WeatherState,
    ) -> f64 {
        let ap = self.acc.params();
        let sit = common::situation(ego, lane, w, ap.weather_response, ap.road_context);
        let dt = self.clock.dt_s();
        let degrade = |acc_gap: f64| self.acc.accel_with_time_gap(ego, leader, lane, w, acc_gap, dt);
        let factor = sit.headway_factor;
        let Some(l) = leader.filter(|l| l.is_vehicle()) else {
            return degrade(ap.time_gap_s * factor);
        };
        let h = self.params.time_gap_s * factor;
        match self.cooperative_accel(ego, l, &sit.driver, h, dt) {
            None => degrade(self.degraded_time_gap(ego, l, self.params.time_gap_s) * factor),
            Some(u) => {
                let emergency = ego.class.spec().emergency_decel_mps2;
                let g = l.gap_m - sit.driver.min_gap_m;
                let a = u.min(self.acc.safety_cap(ego.speed_mps, g, l.speed_mps, emergency, dt));
                acc::finish(
                    a,
                    ego.speed_mps,
                    &sit.driver,
                    emergency,
                    sit.grip_decel_mps2,
                    dt,
                )
            }
        }
    }

    fn profile(&self, class: VehicleClass) -> DriverProfile {
        DriverProfile {
            time_headway_s: self.params.time_gap_s,
            ..self.params.acc.preset.profile(class)
        }
    }

    fn begin_step(&self, now: SimTime, dt: Duration) {
        self.clock.begin(now, dt);
        self.acc.begin_step(now, dt);
    }

    fn cooperative(&self) -> Option<Arc<CoopLink>> {
        Some(Arc::clone(&self.link))
    }
}

/// The model card for one law.
pub fn card(law: CaccLaw, p: &CaccParams, coop: &crate::coop::CoopParams) -> ModelCard {
    let mut sources = acc::acc_sources();
    let (purpose, equations, params, law_src): (&str, Vec<Equation>, Vec<Parameter>, Source) =
        match law {
            CaccLaw::Milanes => {
                let s = sources[0].clone();
                (
                    "Cooperative adaptive cruise control by the PATH field controller \
                     (Milanés & Shladover 2014): a 0.6 s time gap held with the predecessor's \
                     speed as received over V2X, degrading to ACC when messages are lost or \
                     late.",
                    vec![
                        Equation::new(
                            "gap control",
                            "a = (k_p·e + k_d·ė)/0.1 s, e = g − h·v, ė = v_p(V2X) − v − h·a",
                        ),
                        Equation::new("speed control", "a = −0.4·(v − v0) when g/v > 2 s, e > 1.66 m"),
                    ],
                    vec![
                        Parameter::new("time_gap", "s", serde_json::json!(p.time_gap_s), s.clone()),
                        Parameter::new(
                            "gap_gains",
                            "1, s",
                            serde_json::json!([p.gap_gains.0, p.gap_gains.1]),
                            s.clone(),
                        ),
                        Parameter::new(
                            "collision_gains",
                            "1, s",
                            serde_json::json!([p.collision_gains.0, p.collision_gains.1]),
                            sources[3].clone(),
                        ),
                        Parameter::new(
                            "closing_gains",
                            "1, s",
                            serde_json::json!([p.closing_gains.0, p.closing_gains.1]),
                            sources[3].clone(),
                        ),
                        Parameter::new(
                            "command_interval",
                            "s",
                            serde_json::json!(p.command_interval_s),
                            s.clone(),
                        ),
                        Parameter::new(
                            "speed_control_min_gap",
                            "m",
                            serde_json::json!(p.speed_control_min_gap_m),
                            sources[3].clone(),
                        ),
                    ],
                    s,
                )
            }
            CaccLaw::Ploeg => {
                let s = Source::new(
                    SourceKind::Paper,
                    "Ploeg, Scheepers, van Nunen, van de Wouw & Nijmeijer 2011, Design and \
                     experimental evaluation of cooperative adaptive cruise control, IEEE ITSC \
                     2011, 260-265 (DOI 10.1109/ITSC.2011.6082981)",
                );
                (
                    "Cooperative adaptive cruise control with acceleration feed-forward (Ploeg \
                     et al. 2011): the predecessor's received acceleration enters the \
                     controller directly, so a 0.7 s gap is string stable while messages \
                     arrive and the vehicle degrades to ACC when they do not.",
                    vec![Equation::new(
                        "controller",
                        "h·u̇ = −u + k_p·e + k_d·ė + u_p(V2X),  e = g − h·v,  ė = v_p − v − h·a",
                    )],
                    vec![
                        Parameter::new("time_gap", "s", serde_json::json!(p.time_gap_s), s.clone()),
                        Parameter::new(
                            "ploeg_gains",
                            "1/s², 1/s",
                            serde_json::json!([p.ploeg_gains.0, p.ploeg_gains.1]),
                            s.clone(),
                        ),
                        Parameter::new(
                            "actuator_lag",
                            "s",
                            serde_json::json!(p.actuator_lag_s),
                            s.clone(),
                        ),
                    ],
                    s,
                )
            }
            CaccLaw::Path => {
                let s = Source::new(
                    SourceKind::Paper,
                    "Rajamani 2012, Vehicle Dynamics and Control, 2nd ed., Springer, ch. 7 \
                     (leader-predecessor constant-spacing controller); parameterised as Plexe, \
                     Segata, Joerer, Bloessl, Sommer, Dressler & Lo Cigno 2014, PLEXE: A \
                     platooning extension for Veins, IEEE VNC 2014, 53-60",
                );
                let plexe = Source {
                    kind: SourceKind::Code,
                    reference: "Plexe defaults caccC1 = 0.5, caccXi = 1, caccOmegaN = 0.2, \
                                caccSpacing = 5 m, engine lag 0.5 s"
                        .to_string(),
                    accessed: None,
                    note: Some("secondary: Plexe's documented defaults as recalled".to_string()),
                };
                (
                    "Platooning by the PATH leader-predecessor controller: constant 5 m \
                     spacing held with both the predecessor's and the platoon leader's speed \
                     and acceleration as received over V2X; degrades to ACC when either is \
                     lost or late.",
                    vec![Equation::new(
                        "controller",
                        "u = α1·u_p + α2·u_0 + α3·ε̇ + α4·(v − v_0) + α5·ε, ε = s_des − gap",
                    )],
                    vec![
                        Parameter::new("c1", "1", serde_json::json!(p.path_c1), plexe.clone()),
                        Parameter::new("xi", "1", serde_json::json!(p.path_xi), plexe.clone()),
                        Parameter::new(
                            "omega_n",
                            "rad/s",
                            serde_json::json!(p.path_omega_n),
                            plexe.clone(),
                        ),
                        Parameter::new(
                            "spacing",
                            "m",
                            serde_json::json!(p.path_spacing_m),
                            plexe.clone(),
                        ),
                        Parameter::new(
                            "actuator_lag",
                            "s",
                            serde_json::json!(p.actuator_lag_s),
                            plexe.clone(),
                        ),
                        Parameter::new(
                            "max_platoon",
                            "1",
                            serde_json::json!(p.path_max_platoon),
                            Source::new(
                                SourceKind::Code,
                                "housekeeping bound on the predecessor-chain walk; longer than \
                                 any platoon the PATH trials ran",
                            ),
                        ),
                    ],
                    s,
                )
            }
        };
    sources.insert(0, law_src);
    let mut card = ModelCard::new(law.model_id(), Family::Mobility, MODEL_VERSION, purpose);
    card.tier = vec![Tier::Medium];
    card.equations = equations;
    card.equations.push(Equation::new(
        "degradation",
        "no BSM/CAM from the radar target younger than coop.max_age_s (generation time) → \
         the ACC law (mobility/car-following/acc), its time gap opening from the \
         cooperative one to 1.1 s over degrade_ramp_s after the last message went stale",
    ));
    card.parameters = params;
    card.parameters.push(Parameter {
        name: "degrade_ramp_s".to_string(),
        unit: "s".to_string(),
        default: serde_json::json!(p.degrade_ramp_s),
        range: Some(vec![serde_json::json!(0.0), serde_json::json!(60.0)]),
        source: Source::todo_calibrate(
            "graceful degradation (Ploeg et al. 2015, IEEE T-ITS 16(1)): the gap opens from \
             the cooperative to the ACC time gap over this ramp; 5 s is a choice",
        ),
        calibration: Some(
            "sweep degrade_ramp_s in crate::carfollowing::platoon at 1-30 % loss; take the \
             shortest ramp whose string-stability gain stays <= 1, and compare with the \
             transition times of Ploeg et al. 2015"
                .to_string(),
        ),
    });
    card.parameters.extend(coop.card_parameters());
    card.parameters.extend(crate::weather::fhwa_parameters());
    card.assumptions = vec![
        "The radar measures the true net gap and the true range rate; only the cooperative \
         terms come from V2X."
            .to_string(),
        "A message is associated with the radar target by its claimed position \
         (crate::coop), never by the sender's simulation identity."
            .to_string(),
    ];
    card.limitations = vec![
        "No platoon management protocol (join, leave, split): a platoon is any chain of \
         cooperative vehicles each associated with its predecessor."
            .to_string(),
        "The controller state is the previous applied acceleration; the engine's own jerk \
         limit may differ from the law's command."
            .to_string(),
    ];
    card.sources = sources;
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "carfollowing::platoon::tests::cacc_string_stable_with_messages_degrades_without"
                .to_string(),
        ],
    };
    card
}
