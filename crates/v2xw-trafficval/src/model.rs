//! Group 1: the models against their papers.
//!
//! * The IDM's acceleration equation and its published parameter set (Treiber, Hennecke &
//!   Helbing 2000, Eq. 1–2 and Table 1), and its defining approach behaviour: a car closing
//!   on a standing obstacle comes to rest at the jam distance `s0`.
//! * String stability: the linear criterion for a car-following model `a = f(s, v, Δv)`
//!   against a simulated platoon, density by density, for the city drivers and the
//!   Treiber 2000 set.
//! * The ring-road fundamental diagram against Treiber 2000's jam density and wave speed
//!   and Kesting 2010's capacity formula.
//! * The Sugiyama et al. 2008 ring experiment: 22 cars on a 230 m circuit form a jam with
//!   no bottleneck.
//! * MOBIL's decision against an independent evaluation of Kesting, Treiber & Helbing
//!   2007's criteria, and its parameters against the paper's table.
//! * The gap-acceptance table against the HCM, and the social-force parameters against
//!   Helbing & Molnár 1995.

use std::sync::Arc;

use v2xw_core::geom::Dims;
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::math;
use v2xw_core::rng::RngRegistry;
use v2xw_core::weather::WeatherState;
use v2xw_mobility::carfollowing::idm::{Idm, IdmParams, IdmPreset};
use v2xw_mobility::ctx::MobilityCtx;
use v2xw_mobility::fd::{self, FdParams};
use v2xw_mobility::intersection::gap_acceptance::{HcmGaps, Movement};
use v2xw_mobility::lanechange::mobil::{Mobil, MobilParams, MobilPreset};
use v2xw_mobility::traits::{CarFollowing, LaneChange};
use v2xw_mobility::views::{
    DriverProfile, LaneChangeDecision, LaneNeighbors, LeaderView, Side, SideNeighbors, VehicleView,
};
use v2xw_mobility::vru::social_force::SocialForceParams;
use v2xw_mobility::VehicleClass;

use crate::{Check, Group, Lab, Outcome, Row, Variant};

/// The checks of this group.
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "model/idm-equation",
            group: Group::Model,
            title: "The IDM evaluates Treiber 2000's equation with Table 1's parameters",
            procedure: "The engine's `Idm::accel_of` is evaluated on a grid of speeds \
                (0–30 m/s), net gaps (3–150 m) and approach rates (−5 to +5 m/s) and compared \
                with the equation written out independently here: \
                `a·[1 − (v/v0)^δ − (s*/s)²]`, `s* = s0 + max(0, v·T + v·Δv/(2√(ab)))` (the \
                max is Treiber & Kesting 2013's §11.3.6 guard against a negative desired \
                gap), clamped to the model's documented physical floor. The Treiber 2000 \
                preset's driver is compared with the paper's Table 1.",
            faults: &[
                "the acceleration exponent δ set to 2 instead of 4",
                "the Kesting 2010 drivers (T 1.5 s, a 1.4, b 2.0) read as the Treiber 2000 set",
                "the paper's 120 km/h entered as a rounded 33 m/s and s0 taken from the truck column (4 m)",
            ],
            run: idm_equation,
        },
        Check {
            id: "model/idm-stops-at-s0",
            group: Group::Model,
            title: "A city driver closing on a standing car stops at the jam distance s0",
            procedure: "One car at 50 km/h (13.9 m/s, the city driver of the UrbanHcm set) \
                drives at a standing obstacle 150 m ahead; the IDM is integrated with the \
                engine's step (0.1 s, ballistic). Treiber 2000 §II: the model's desired gap \
                tends to `s0` as `v → 0`, so the car must come to rest `s0` short of the \
                obstacle and never closer.",
            faults: &["the model evaluated with s0 = 0.5 m instead of the driver's 2 m"],
            run: idm_stop,
        },
        Check {
            id: "model/string-stability",
            group: Group::Model,
            title: "Platoons amplify or damp a disturbance exactly where linear string-stability theory says",
            procedure: "For `a = f(s, v, Δv)` (Δv = v − v_leader) a platoon in equilibrium is \
                string stable iff `f_v²/2 + f_v·f_Δv − f_s ≥ 0` (Wilson & Ward 2011, Transp. \
                Res. C 19; Treiber & Kesting 2013 §15.5: |G(iω)| ≤ 1 for all ω). The partial \
                derivatives are taken numerically from the engine's IDM at each density's \
                equilibrium, and give the linearised per-car gain |G(iω)|. A 16-car platoon \
                at that equilibrium is then simulated with the model while the leader's \
                speed oscillates 0.05 m/s at ω = √|criterion| (inside the unstable band \
                when there is one); the per-car amplitude gain, read by projecting the \
                speeds onto sin and cos at ω, is held to |G(iω)| and must agree on growth. \
                (A single dip is not used: its spectrum is mostly outside the band, so a \
                weakly unstable platoon damps it.) Densities where the criterion is within \
                0.01 s⁻² of zero are marginal and skipped.",
            faults: &[
                "the simulated drivers react 0.8 s late (a delay the ODE criterion does not have)",
            ],
            run: string_stability,
        },
        Check {
            id: "model/ring-fundamental-diagram",
            group: Group::Model,
            title: "A single-lane ring reproduces Treiber 2000's jam density and wave speed and Kesting 2010's capacity",
            procedure: "`v2xw_mobility::fd::measure(FdParams::quick())`: a 600 m single-lane \
                ring loaded to twelve densities from 5 to 140 veh/km, each from a homogeneous \
                start with one perturbed car and from a held jam, measured in one-minute bins \
                with Edie's definitions; the congested branch is fitted by least squares. \
                Treiber 2000 parameter set, the set the paper's figures were made with.",
            faults: &["the ring filled with 12 m trucks instead of 5 m cars"],
            run: ring_fd,
        },
        Check {
            id: "model/sugiyama-ring",
            group: Group::Model,
            title: "22 city drivers on a 230 m circuit form a stop-and-go jam with no bottleneck (Sugiyama et al. 2008)",
            procedure: "The engine's mobility (NativeMobility) on a 230 m single-lane ring \
                with 22 cars of the shipped city drivers (UrbanHcm) as the `human` driver \
                population (own parameters, 0.8 s reaction, HDM perception errors), desired speed 30 km/h \
                (the experiment's instruction), started evenly spaced at the equilibrium \
                speed with one car 20 % slow. After 300 s of warm-up, the next 300 s are \
                sampled every step: the slowest car's speed, and the jam's propagation \
                speed from its outflow front: the median of Δx/Δt between a car leaving \
                the jam (rising past 3 m/s after standing below 1 m/s) and the car behind \
                it leaving next.",
            faults: &["the ideal driver: no reaction time, no perception errors, every driver alike"],
            run: sugiyama,
        },
        Check {
            id: "model/mobil-criterion",
            group: Group::Model,
            title: "MOBIL changes lane exactly when Kesting 2007's safety and incentive criteria say",
            procedure: "400 lane-change situations (ego, its leader and follower, the target \
                lane's leader and follower, speeds 4–14 m/s, gaps 2–80 m, drawn from a fixed \
                linear congruential sequence) are given to the engine's `Mobil::decide` with \
                the city drivers. Each is also decided independently here: safety `ã_n ≥ \
                −b_safe`, incentive `(ã_c − a_c) + p·[(ã_n − a_n) + (ã_o − a_o)] > Δa_th`, \
                every acceleration from the IDM with that vehicle's own driver; the \
                verdicts (stay, left or right) must agree. The preset's parameters are \
                compared with Kesting 2007's Table 1.",
            faults: &[
                "the engine's MOBIL run with politeness 0 (a selfish driver) against the paper's p",
                "the engine's MOBIL run with b_safe = 9 m/s² against the paper's 4",
                "the preset's table misread: Δa_th and Δa_bias swapped, b_safe read from the comfortable deceleration b (2 m/s²)",
            ],
            run: mobil_criterion,
        },
        Check {
            id: "model/hcm-critical-gaps",
            group: Group::Model,
            title: "Gap acceptance uses the HCM's base critical headways and follow-up times",
            procedure: "`HcmGaps::of` for each movement at a two-way stop against HCM 2010 \
                Exhibit 19-10 (repeated as HCM 6th ed. Exhibit 20-11): left from the major \
                road 4.1 s (follow-up 2.2), right from the minor 6.2 / 6.9 s on a 2- / 4-lane \
                major (3.3), through from the minor 6.5 s (4.0), left from the minor 7.1 / \
                7.5 s (3.5). The values are the model's own table, so the fault replaces it.",
            faults: &[
                "the minor-road left and through rows swapped",
                "the base values adjusted as if every vehicle were heavy (HCM 2010 Eq. 19-30/19-31, Exhibit 19-11)",
            ],
            run: hcm_gaps,
        },
        Check {
            id: "model/social-force-parameters",
            group: Group::Model,
            title: "The pedestrian model carries Helbing & Molnár 1995's parameter set",
            procedure: "`SocialForceParams::default()` against Helbing & Molnár 1995 (Phys. \
                Rev. E 51, 4282), §III: desired speed N(1.34, 0.26) m/s, maximum 1.3·v0, \
                relaxation time τ 0.5 s, pedestrian repulsion V0 2.1 m²/s² over σ 0.3 m, \
                border repulsion U0 10 m²/s² over R 0.2 m, step Δt 2 s, field of view 200° \
                with weight c 0.5 behind. Walking speeds in a run are group 2's.",
            faults: &[
                "a parameter set with τ = 1.0 s and σ = 0.5 m",
                "the parameter table transcribed one row out of step",
            ],
            run: social_force,
        },
    ]
}

// ---------------------------------------------------------------------------------------
// The IDM equation
// ---------------------------------------------------------------------------------------

/// The IDM written out from Treiber 2000, with the guard and the clamps the model card
/// documents.
fn idm_reference(p: &IdmParams, d: &DriverProfile, v: f64, v0: f64, gap: f64, v_lead: f64) -> f64 {
    let v0 = v0.max(p.v0_floor_mps);
    let a = d.max_accel_mps2;
    let free = 1.0 - math::pow(v / v0, p.delta);
    let raw = if gap.is_infinite() {
        a * free
    } else {
        let s = gap.max(p.gap_floor_m);
        let dv = v - v_lead;
        let dynamic =
            (v * d.time_headway_s + v * dv / (2.0 * math::sqrt(a * d.comfort_decel_mps2))).max(0.0);
        let s_star = d.min_gap_m + p.s1_m * math::sqrt(v / v0) + dynamic;
        a * (free - (s_star / s).powi(2))
    };
    raw.clamp(p.a_min_mps2, a)
}

fn idm_equation(_lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let reference_params = IdmParams::default();
    let model = match variant {
        Variant::Fault(0) => Idm::with_params(
            IdmPreset::Treiber2000,
            IdmParams {
                delta: 2.0,
                ..IdmParams::default()
            },
        ),
        _ => Idm::new(IdmPreset::Treiber2000),
    };
    let driver = match variant {
        Variant::Fault(1) => IdmPreset::Kesting2010.profile(VehicleClass::Passenger),
        Variant::Fault(2) => DriverProfile {
            desired_speed_mps: 33.0,
            min_gap_m: 4.0,
            ..IdmPreset::Treiber2000.profile(VehicleClass::Passenger)
        },
        _ => IdmPreset::Treiber2000.profile(VehicleClass::Passenger),
    };
    let mut worst = 0.0f64;
    let mut n = 0usize;
    for v in [0.0, 2.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0] {
        for gap in [3.0, 5.0, 10.0, 20.0, 40.0, 80.0, 150.0, f64::INFINITY] {
            for dv in [-5.0, -1.0, 0.0, 1.0, 5.0] {
                let v_lead = f64::max(v - dv, 0.0);
                let got = model.accel_of(v, driver.desired_speed_mps, gap, v_lead, &driver);
                let want = idm_reference(
                    &reference_params,
                    &driver,
                    v,
                    driver.desired_speed_mps,
                    gap,
                    v_lead,
                );
                worst = worst.max((got - want).abs());
                n += 1;
            }
        }
    }
    let t2000 = "Treiber, Hennecke & Helbing 2000, Phys. Rev. E 62, 1805, Table 1";
    Ok(Outcome {
        rows: vec![
            Row::held(
                "largest |engine − equation|, m/s²",
                worst,
                (0.0, 1e-9),
                "Treiber 2000 Eq. 1–2",
            )
            .n(n),
            Row::held(
                "desired speed v0, km/h",
                driver.desired_speed_mps * 3.6,
                (119.5, 120.5),
                t2000,
            ),
            Row::held("time gap T, s", driver.time_headway_s, (1.6, 1.6), t2000),
            Row::held(
                "maximum acceleration a, m/s²",
                driver.max_accel_mps2,
                (0.73, 0.73),
                t2000,
            ),
            Row::held(
                "comfortable deceleration b, m/s²",
                driver.comfort_decel_mps2,
                (1.67, 1.67),
                t2000,
            ),
            Row::held("jam distance s0, m", driver.min_gap_m, (2.0, 2.0), t2000),
            Row::held(
                "acceleration exponent δ",
                model.params().delta,
                (4.0, 4.0),
                t2000,
            ),
        ],
        notes: vec![format!(
            "{n} points compared; the model's floor on the acceleration is {} m/s² and on the gap {} m, both on its card.",
            reference_params.a_min_mps2, reference_params.gap_floor_m
        )],
    })
}

// ---------------------------------------------------------------------------------------
// Approach to a standing obstacle
// ---------------------------------------------------------------------------------------

/// One ballistic step, as the engine integrates: speed first, never negative, then
/// position with the mean of the two speeds.
fn ballistic(x: &mut f64, v: &mut f64, a: f64, dt: f64) {
    let v1 = (*v + a * dt).max(0.0);
    *x += 0.5 * (*v + v1) * dt;
    *v = v1;
}

/// Integrates the approach to a standing obstacle `obstacle` m ahead from `v_start`, with
/// `accel(v, gap)`, and returns `(gap at the end, smallest gap, final speed, peak decel)`.
fn approach(
    accel: impl Fn(f64, f64) -> f64,
    v_start: f64,
    obstacle: f64,
    dt: f64,
    t_end: f64,
) -> (f64, f64, f64, f64) {
    let (mut x, mut v) = (0.0, v_start);
    let mut min_gap = f64::INFINITY;
    let mut peak_decel = 0.0f64;
    for _ in 0..(t_end / dt).round() as usize {
        let a = accel(v, obstacle - x);
        peak_decel = peak_decel.max(-a);
        ballistic(&mut x, &mut v, a, dt);
        min_gap = min_gap.min(obstacle - x);
    }
    (obstacle - x, min_gap, v, peak_decel)
}

fn idm_stop(_lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let idm = Idm::new(IdmPreset::UrbanHcm);
    let driver = IdmPreset::UrbanHcm.profile(VehicleClass::Passenger);
    let mut model_driver = driver;
    if variant == Variant::Fault(0) {
        model_driver.min_gap_m = 0.5;
    }
    let v0 = 13.9;
    let obstacle = 150.0;
    let (gap, min_gap, v, peak_decel) = approach(
        |v, gap| idm.accel_of(v, v0, gap, 0.0, &model_driver),
        v0,
        obstacle,
        0.1,
        120.0,
    );
    // The same approach through the equation written out here, at the engine's step and at
    // 1 ms (the continuous limit). With speeds floored at zero the IDM does not reach s0
    // from above: it is underdamped near standstill and stops a little short (Treiber &
    // Kesting 2013 §11.3.6 on the negative-speed guard), so the gap at rest is held to the
    // independent integration, and to s0 only within the continuous model's own shortfall.
    let params = IdmParams::default();
    let reference = |v: f64, gap: f64| idm_reference(&params, &driver, v, v0, gap, 0.0);
    let (ref_gap, _, _, _) = approach(reference, v0, obstacle, 0.1, 120.0);
    let (limit_gap, _, _, _) = approach(reference, v0, obstacle, 0.001, 120.0);
    let s0 = driver.min_gap_m;
    let shortfall = (s0 - limit_gap).max(0.0);
    let cite = "Treiber 2000 §II: s*(v→0) = s0";
    Ok(Outcome {
        rows: vec![
            Row::held(
                "gap at rest, |engine − independent IDM integration at 0.1 s|, m",
                (gap - ref_gap).abs(),
                (0.0, 1e-9),
                "Treiber 2000 Eq. 1–2 integrated ballistically (Treiber & Kanagaraj 2015) by the check itself",
            ),
            Row::held(
                "gap at rest, m",
                gap,
                (s0 - 0.1, s0 + 0.5),
                format!(
                    "{cite}; the continuous model with v ≥ 0 stops {shortfall:.3} m short of s0 (1 ms integration), so the band opens 0.1 m below s0"
                ),
            ),
            Row::held(
                "smallest gap on the approach, m",
                min_gap,
                (s0 - 0.1, f64::INFINITY),
                cite,
            ),
            Row::held("speed after 120 s, m/s", v, (0.0, 0.05), "at rest"),
            Row::reported(
                "peak deceleration, m/s²",
                peak_decel,
                "IDM: about b = 2 m/s² when the kinematic deceleration v²/2s is below b (Treiber & Kesting 2013 §11.3.5)",
            ),
        ],
        notes: vec![format!(
            "independent integration: {ref_gap:.4} m at 0.1 s, {limit_gap:.4} m at 1 ms"
        )],
    })
}

// ---------------------------------------------------------------------------------------
// String stability
// ---------------------------------------------------------------------------------------

/// The equilibrium speed at gap `s`, `f(s, v, v) = 0`, by bisection to 1e-13 m/s on the
/// engine's own function: a platoon started a hair off it relaxes over minutes at low
/// density (the gap mode's time constant is |f_v|/f_s), and that slow drift would leak into
/// the frequency-response reading.
fn equilibrium(idm: &Idm, d: &DriverProfile, v0: f64, s: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, v0.max(0.1) * 1.5);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if idm.accel_of(mid, v0, s, mid, d) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The partial derivatives `(f_s, f_v, f_Δv)` of the model at the equilibrium of gap `s`
/// (Δv = v − v_leader; f_v with Δv held), from central differences.
fn partials(idm: &Idm, d: &DriverProfile, v0: f64, s: f64) -> (f64, f64, f64) {
    let v = equilibrium(idm, d, v0, s);
    let h = 1e-4;
    let f = |s: f64, v: f64, vl: f64| idm.accel_of(v, v0, s, vl, d);
    let fs = (f(s + h, v, v) - f(s - h, v, v)) / (2.0 * h);
    // ∂/∂v with Δv held: move v and the leader together.
    let fv = (f(s, v + h, v + h) - f(s, v - h, v - h)) / (2.0 * h);
    // ∂/∂Δv: move the leader the other way.
    let fdv = (f(s, v, v - h) - f(s, v, v + h)) / (2.0 * h);
    (fs, fv, fdv)
}

/// The linear string-stability criterion `f_v²/2 + f_v·f_Δv − f_s` at the equilibrium of
/// gap `s`.
fn criterion(idm: &Idm, d: &DriverProfile, v0: f64, s: f64) -> (f64, f64) {
    let v = equilibrium(idm, d, v0, s);
    let (fs, fv, fdv) = partials(idm, d, v0, s);
    (v, fv * fv / 2.0 + fv * fdv - fs)
}

/// The linearised platoon's per-car transfer-function gain `|G(iω)|`,
/// `G(λ) = (f_s − f_Δv·λ) / (λ² − (f_v + f_Δv)·λ + f_s)` (Treiber & Kesting 2013 §15.5;
/// Wilson & Ward 2011 Eq. 9 in this sign convention).
fn transfer_gain(fs: f64, fv: f64, fdv: f64, omega: f64) -> f64 {
    let num = fs * fs + fdv * fdv * omega * omega;
    let re = fs - omega * omega;
    let im = (fv + fdv) * omega;
    math::sqrt(num / (re * re + im * im))
}

/// A platoon of `N` cars at the equilibrium of gap `s` whose leader's speed oscillates
/// `0.05·sin(ωt)` m/s: the speed amplitude at each car is read by projecting the last
/// whole periods onto sin and cos at ω, and the per-car gain is the geometric mean of the
/// ratios from the leader to the last car the probe still reaches. `delay_s` makes every follower see the platoon that
/// long ago (the fault).
fn platoon_gain(
    idm: &Idm,
    d: &DriverProfile,
    v0: f64,
    s: f64,
    length: f64,
    delay_s: f64,
    omega: f64,
) -> (f64, Vec<f64>) {
    const N: usize = 16;
    const AMP: f64 = 0.05;
    let dt = 0.02;
    let ve = equilibrium(idm, d, v0, s);
    let period = 2.0 * core::f64::consts::PI / omega;
    let settle = 200.0f64.max(4.0 * period);
    let cycles = (3.0f64).max((200.0 / period).floor());
    let t_end = settle + cycles * period;
    let mut x: Vec<f64> = (0..N).map(|i| -(i as f64) * (s + length)).collect();
    let mut v = vec![ve; N];
    let delay_steps = (delay_s / dt).round() as usize;
    let mut history: std::collections::VecDeque<(Vec<f64>, Vec<f64>)> =
        std::collections::VecDeque::new();
    let (mut sin_acc, mut cos_acc) = (vec![0.0f64; N], vec![0.0f64; N]);
    let steps = (t_end / dt).round() as usize;
    for k in 0..steps {
        let t = k as f64 * dt;
        history.push_back((x.clone(), v.clone()));
        while history.len() > delay_steps + 1 {
            history.pop_front();
        }
        let seen = &history[0];
        let mut acc = vec![0.0; N];
        for i in 1..N {
            let gap = seen.0[i - 1] - seen.0[i] - length;
            acc[i] = idm.accel_of(seen.1[i], v0, gap, seen.1[i - 1], d);
        }
        let target = ve + AMP * math::sin(omega * (t + dt));
        acc[0] = (target - v[0]) / dt;
        for i in 0..N {
            ballistic(&mut x[i], &mut v[i], acc[i], dt);
        }
        let t1 = t + dt;
        if t1 > settle {
            #[allow(clippy::needless_range_loop)]
            for i in 0..N {
                sin_acc[i] += (v[i] - ve) * math::sin(omega * t1) * dt;
                cos_acc[i] += (v[i] - ve) * math::cos(omega * t1) * dt;
            }
        }
    }
    let amp = |i: usize| math::sqrt(sin_acc[i] * sin_acc[i] + cos_acc[i] * cos_acc[i]);
    // A strongly damped platoon carries the probe only a few cars before its amplitude
    // sinks under what slow transients leak into the projection, so the ratio is taken
    // from the leader to the last car still carrying a hundredth of the probe (below that,
    // a residue of about 1e-4 of the probe from the settling transient dominates).
    let a0 = amp(0);
    let floor = a0 * 1e-2;
    let k = (1..N).take_while(|&i| amp(i) >= floor).last();
    let ratios: Vec<f64> = (1..5.min(N))
        .map(|i| amp(i) / amp(i - 1).max(1e-300))
        .collect();
    let g = match k {
        Some(k) if a0 > 0.0 => math::pow(amp(k) / a0, 1.0 / k as f64),
        _ => f64::NAN,
    };
    (g, ratios)
}

fn string_stability(_lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let delay = if variant == Variant::Fault(0) {
        0.8
    } else {
        0.0
    };
    let length = VehicleClass::Passenger.spec().length_m;
    let mut rows = Vec::new();
    let mut notes = Vec::new();
    let mut total_disagree = 0u64;
    let mut worst_gain_err = 0.0f64;
    let mut compared = 0usize;
    for (preset, v0, label) in [
        (
            IdmPreset::UrbanHcm,
            11.176,
            "city drivers (UrbanHcm), 25 mph",
        ),
        (IdmPreset::Treiber2000, 33.3, "Treiber 2000, 120 km/h"),
    ] {
        let idm = Idm::new(preset);
        let d = preset.profile(VehicleClass::Passenger);
        let mut unstable = Vec::new();
        let mut line = Vec::new();
        for rho in [
            10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 110.0, 120.0,
        ] {
            let s = 1000.0 / rho - length;
            if s <= d.min_gap_m + 0.2 {
                continue;
            }
            let (_ve, c) = criterion(&idm, &d, v0, s);
            if c < 0.0 {
                unstable.push(rho);
            }
            if c.abs() < 0.01 {
                line.push(format!(
                    "{rho:.0}: criterion {c:+.3} (marginal, not compared)"
                ));
                continue;
            }
            // ω inside the unstable band ω² < −2c when c < 0, at the same distance from 0
            // when c > 0, so both verdicts are probed where |G| differs from 1 the most.
            let omega = math::sqrt(c.abs());
            let (fs, fv, fdv) = partials(&idm, &d, v0, s);
            let theory = transfer_gain(fs, fv, fdv, omega);
            let (sim, ratios) = platoon_gain(&idm, &d, v0, s, length, delay, omega);
            let r: Vec<String> = ratios.iter().map(|x| format!("{x:.4}")).collect();
            line.push(format!("{rho:.0}: criterion {c:+.3}, ω {omega:.3} rad/s, |G| theory {theory:.4}, simulated {sim:.4} (car-to-car {})", r.join(" ")));
            compared += 1;
            if (theory > 1.0) != (sim > 1.0) || !sim.is_finite() {
                total_disagree += 1;
            }
            let err = if sim.is_finite() {
                (sim - theory).abs()
            } else {
                f64::INFINITY
            };
            worst_gain_err = worst_gain_err.max(err);
        }
        rows.push(Row::reported(
            format!("{label}: lowest unstable density, veh/km"),
            unstable.first().copied().unwrap_or(f64::NAN),
            "where homogeneous traffic of these drivers breaks into stop-and-go waves (NaN: none up to 120)",
        ));
        if preset == IdmPreset::Treiber2000 {
            rows.push(Row::held(
                "Treiber 2000: densities (of 12) where the platoon is unstable",
                unstable.len() as f64,
                (1.0, 11.0),
                "Treiber 2000 §IV: with Table 1's parameters homogeneous traffic is unstable over a range of densities and stable outside it (free flow)",
            ));
        }
        notes.push(format!("{label}: {}", line.join("; ")));
    }
    rows.insert(
        0,
        Row::held(
            "largest |simulated − linear-theory| per-car gain at the probe frequency",
            worst_gain_err,
            (0.0, 0.003),
            "Treiber & Kesting 2013 §15.5 transfer function; tolerance: the ballistic step's phase error at 0.02 s and the 0.05 m/s probe's nonlinearity, both under 0.1 % of the gain",
        )
        .n(compared),
    );
    rows.insert(
        0,
        Row::zero(
            "densities where simulation and theory disagree on growth (|G| > 1)",
            total_disagree,
            "Wilson & Ward 2011; Treiber & Kesting 2013 §15.5",
        )
        .n(compared),
    );
    Ok(Outcome { rows, notes })
}

// ---------------------------------------------------------------------------------------
// Ring fundamental diagram
// ---------------------------------------------------------------------------------------

fn ring_fd(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let class = if variant == Variant::Fault(0) {
        VehicleClass::Truck
    } else {
        VehicleClass::Passenger
    };
    let key = format!("ring-fd-{}", class.as_str());
    let r = lab.study(&key, |_| {
        fd::measure(&FdParams {
            class,
            ..FdParams::quick()
        })
        .map_err(|e| e.to_string())
    })?;
    let t = fd::targets::ABOUT_TOLERANCE;
    let jam = fd::targets::JAM_DENSITY_VEH_KM;
    let w = fd::targets::WAVE_SPEED_KMH;
    Ok(Outcome {
        rows: vec![
            Row::held(
                "jam density, veh/km",
                r.jam_density_veh_km,
                (jam * (1.0 - t), jam * (1.0 + t)),
                "Treiber 2000: about 140 veh/km (± 20 % read as 'about')",
            ),
            Row::held(
                "jam wave speed, km/h",
                r.wave_speed_kmh,
                (w * (1.0 + t), w * (1.0 - t)),
                "Treiber 2000: about −15 km/h (± 20 %)",
            ),
            Row::held(
                "capacity below Kesting 2010's Q_max, fraction",
                r.capacity_below_theory(),
                (0.0, fd::targets::CAPACITY_BELOW_THEORY_TOLERANCE),
                "Kesting 2010 Eq. 4.1: Q_max = (1/T)(1 − l_eff/(v0·T + l_eff)); a perturbed ring sits at or below it",
            ),
            Row::reported(
                "capacity, veh/h",
                r.capacity_veh_h,
                "Q_max from the parameters below",
            ),
            Row::reported(
                "Q_max from the parameters, veh/h",
                r.theoretical_capacity_veh_h,
                "Kesting 2010 Eq. 4.1",
            ),
            Row::reported(
                "capacity drop (same density), fraction",
                r.capacity_drop,
                "5–20 % at freeway bottlenecks (04-models.md §2.9); the plain IDM is known to show little",
            ),
            Row::reported("critical density, veh/km", r.critical_density_veh_km, "—"),
        ],
        notes: vec![format!(
            "{} bins, {} on the congested branch; the parameters predict jam density {:.1} veh/km and wave speed {:.1} km/h.",
            r.bins.len(),
            r.congested_bins,
            r.theoretical_jam_density_veh_km,
            r.theoretical_wave_speed_kmh
        )],
    })
}

// ---------------------------------------------------------------------------------------
// Sugiyama ring
// ---------------------------------------------------------------------------------------

/// What the Sugiyama ring produced.
#[derive(Debug, Clone)]
pub struct RingJam {
    /// The slowest any car went in the measurement window, m/s.
    pub min_speed_mps: f64,
    /// Mean speed over the window, m/s.
    pub mean_speed_mps: f64,
    /// The jam's drift along the road, km/h (negative: upstream), NaN when no jam.
    pub wave_kmh: f64,
    /// Standard deviation of the speeds in the window, m/s.
    pub speed_sd_mps: f64,
}

/// Runs `n` city drivers (UrbanHcm) of population `human` on a `length_m` ring at a
/// desired speed of `v0`: 300 s of warm-up, 300 s measured
/// ([`crate::fdlab::run_ring`]; the jam's speed is its outflow front's, from the
/// departures of consecutive cars).
pub fn ring_jam(
    n: usize,
    length_m: f64,
    v0: f64,
    human: v2xw_mobility::drivers::HumanDriverParams,
) -> Result<RingJam, String> {
    let spec = crate::fdlab::RingSpec {
        model: crate::fdlab::Model::Idm(IdmPreset::UrbanHcm),
        human,
        circumference_m: length_m,
        v0_mps: v0,
        warmup_s: 300.0,
        measure_s: 300.0,
        seed: 0x5_0617_A3A2,
    };
    let r = crate::fdlab::run_ring(&spec, n as f64 / length_m * 1000.0)?;
    Ok(RingJam {
        min_speed_mps: r.min_speed_mps,
        mean_speed_mps: r.speed_mps,
        wave_kmh: r.wave_kmh,
        speed_sd_mps: r.speed_sd_mps,
    })
}

fn sugiyama(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    // Sugiyama's drivers were people: the engine's `human` population (per-driver
    // parameters, a 0.8 s reaction time and the Human Driver Model's perception errors,
    // `v2xw_mobility::drivers`), which a scenario asks for with
    // `actors.vehicles.drivers.realism: human`.
    let human = if variant == Variant::Fault(0) {
        v2xw_mobility::drivers::HumanDriverParams::default()
    } else {
        v2xw_mobility::drivers::HumanDriverParams::human()
    };
    let key = format!("sugiyama-{}", human.reaction_time_s);
    let r = lab.study(&key, |_| ring_jam(22, 230.0, 30.0 / 3.6, human))?;
    let src = "Sugiyama et al. 2008, New J. Phys. 10 033001: 22 cars, 230 m, asked to cruise at 30 km/h; a jam formed and moved backwards 'with the same speed as a jam cluster on a highway'";
    Ok(Outcome {
        rows: vec![
            Row::held(
                "slowest car in the jam, m/s",
                r.min_speed_mps,
                (0.0, 2.0),
                src,
            ),
            Row::held(
                "jam propagation speed, km/h",
                r.wave_kmh,
                (-25.0, -10.0),
                "highway jam fronts move upstream at 15–20 km/h (Kerner & Rehborn 1996; Treiber & Kesting 2013 §18.1, secondary); about 20 km/h on Sugiyama's circuit (secondary)",
            ),
            Row::reported("mean speed, km/h", r.mean_speed_mps * 3.6, "—"),
            Row::reported(
                "speed standard deviation, m/s",
                r.speed_sd_mps,
                "stop-and-go amplitude",
            ),
        ],
        notes: vec![],
    })
}

// ---------------------------------------------------------------------------------------
// MOBIL
// ---------------------------------------------------------------------------------------

/// A fixed linear congruential sequence for drawing test situations (not the engine's
/// RNG: these are inputs, and a fixed sequence makes the check reproducible anywhere).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

fn mobil_criterion(_lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let params = v2xw_world::procedural::GridParams {
        lanes_per_direction: 3,
        ..v2xw_world::procedural::GridParams::legacy()
    };
    let world = v2xw_world::procedural::grid(&params, &v2xw_world::ImportOptions::default())
        .map_err(|e| e.to_string())?;
    // A middle lane with a neighbour each side.
    let mut lanes = None;
    for lane in world.roads.lanes() {
        if lane.kind != v2xw_world::LaneKind::Driving || lane.index != 1 || lane.junction.is_some()
        {
            continue;
        }
        let edge = world.edge(lane.edge);
        let right = edge
            .lanes
            .iter()
            .find(|l| world.lane(**l).index == 0)
            .copied();
        let left = edge
            .lanes
            .iter()
            .find(|l| world.lane(**l).index == 2)
            .copied();
        if let (Some(r), Some(l)) = (right, left) {
            lanes = Some((lane.id, l, r));
            break;
        }
    }
    let (mid, left, right) = lanes.ok_or("no three-lane edge in the grid")?;
    let paper = MobilPreset::Kesting2007.params();
    let mut model_params = paper;
    model_params.step_s = 1.0;
    model_params.reconsider_rate_per_s = 1.0;
    match variant {
        Variant::Fault(0) => model_params.politeness = 0.0,
        Variant::Fault(1) => model_params.b_safe_mps2 = 9.0,
        _ => {}
    }
    let idm = Idm::new(IdmPreset::UrbanHcm);
    let cf: Arc<dyn CarFollowing + Send + Sync> = Arc::new(idm.clone());
    let mobil = Mobil::with_params(MobilPreset::Kesting2007, model_params, cf);
    let driver = IdmPreset::UrbanHcm.profile(VehicleClass::Passenger);
    let rng = RngRegistry::new(11);
    let w = WeatherState::CLEAR;
    let veh = |id: u32, lane: LaneId, idx: u8, v: f64| VehicleView {
        actor: ActorId::new(id),
        class: VehicleClass::Passenger,
        lane,
        lane_index: idx,
        s_m: 50.0,
        lateral_m: 0.0,
        speed_mps: v,
        accel_mps2: 0.0,
        heading_rad: 0.0,
        dims: Dims::new(5.0, 1.8, 1.5),
        driver,
    };
    // The independent evaluation: Kesting 2007's criteria on the IDM, with the speed the
    // lane allows.
    let acc = |v: &VehicleView, leader: Option<&LeaderView>, lane: LaneId| {
        let v0 = v
            .driver
            .desired_speed_mps
            .min(world.lane(lane).speed_limit_mps);
        match leader {
            None => idm.accel_of(v.speed_mps, v0, f64::INFINITY, 0.0, &v.driver),
            Some(l) => idm.accel_of(v.speed_mps, v0, l.gap_m, l.speed_mps, &v.driver),
        }
    };
    let mut seq = Lcg(0x0B11_7E57);
    let mut disagree = 0u64;
    let mut changes = 0u64;
    // 400 random situations, then 100 built on the safety boundary: a slow leader close
    // ahead (a strong incentive), an empty left lane but for a follower closing at a gap
    // that puts its post-change deceleration either side of b_safe, and a right lane
    // blocked outright. The random draws seldom land there, and without them a wrong
    // b_safe would go unseen.
    let n = 500;
    let mut examples = Vec::new();
    for case in 0..n {
        let boundary = case >= 400;
        let v_ego = if boundary { 8.0 } else { seq.range(4.0, 14.0) };
        let ego = veh(0, mid, 1, v_ego);
        let lv = |id: u32, lane: LaneId, idx: u8, seq: &mut Lcg| -> Option<LeaderView> {
            if seq.next() < 0.15 {
                return None;
            }
            let v = seq.range(0.0, 14.0);
            Some(LeaderView::of(veh(id, lane, idx, v), seq.range(2.0, 80.0)))
        };
        let (leader, follower, l_leader, l_follower, r_leader, r_follower) = if boundary {
            (
                Some(LeaderView::of(veh(1, mid, 1, 2.0), 6.0)),
                None,
                None,
                Some(LeaderView::of(
                    veh(4, left, 2, seq.range(9.0, 14.0)),
                    seq.range(2.0, 20.0),
                )),
                None,
                Some(LeaderView::of(veh(6, right, 0, 14.0), 1.0)),
            )
        } else {
            (
                lv(1, mid, 1, &mut seq),
                lv(2, mid, 1, &mut seq),
                lv(3, left, 2, &mut seq),
                lv(4, left, 2, &mut seq),
                lv(5, right, 0, &mut seq),
                lv(6, right, 0, &mut seq),
            )
        };
        let nbrs = LaneNeighbors {
            lane: mid,
            leader,
            follower,
            left: Some(SideNeighbors {
                lane: left,
                side: Side::Left,
                leader: l_leader,
                follower: l_follower,
            }),
            right: Some(SideNeighbors {
                lane: right,
                side: Side::Right,
                leader: r_leader,
                follower: r_follower,
            }),
        };
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        let got = match mobil.decide(&mut ctx, &ego, &nbrs, &w) {
            LaneChangeDecision::Change { side, .. } => Some(side),
            _ => None,
        };
        // Independent.
        let mut best: Option<(f64, Side)> = None;
        let a_c = acc(&ego, nbrs.leader.as_ref(), mid);
        let old = nbrs
            .follower
            .as_ref()
            .and_then(|f| f.vehicle.map(|v| (v, f.gap_m)));
        for side in [Side::Left, Side::Right] {
            let t = nbrs.side(side).expect("both sides");
            let a_c_new = acc(&ego, t.leader.as_ref(), t.lane);
            let mut polite = 0.0;
            let mut safe = true;
            if let Some((fv, fgap)) = t
                .follower
                .as_ref()
                .and_then(|f| f.vehicle.map(|v| (v, f.gap_m)))
            {
                let after = acc(&fv, Some(&LeaderView::of(ego, fgap)), t.lane);
                if after < -paper.b_safe_mps2 {
                    safe = false;
                }
                let before = match t.leader.as_ref() {
                    Some(l) => acc(&fv, Some(&l.at_gap(fgap + 5.0 + l.gap_m)), t.lane),
                    None => acc(&fv, None, t.lane),
                };
                polite += after - before;
            }
            if let Some((ov, ogap)) = old {
                let before = acc(&ov, Some(&LeaderView::of(ego, ogap)), mid);
                let after = match nbrs.leader.as_ref() {
                    Some(l) => acc(&ov, Some(&l.at_gap(ogap + 5.0 + l.gap_m)), mid),
                    None => acc(&ov, None, mid),
                };
                polite += after - before;
            }
            if !safe {
                continue;
            }
            let incentive = a_c_new - a_c + paper.politeness * polite;
            if incentive > paper.threshold_mps2 && best.is_none_or(|(b, _)| incentive > b) {
                best = Some((incentive, side));
            }
        }
        let want = best.map(|(_, s)| s);
        if want.is_some() {
            changes += 1;
        }
        if got != want {
            disagree += 1;
            if examples.len() < 3 {
                examples.push(format!("case {case}: engine {got:?}, criterion {want:?}"));
            }
        }
    }
    let k07 = "Kesting, Treiber & Helbing 2007, TRR 1999, Table 1";
    // The table rows as the preset carries them, or misread (the third fault).
    let table = if variant == Variant::Fault(2) {
        MobilParams {
            threshold_mps2: paper.right_bias_mps2,
            right_bias_mps2: paper.threshold_mps2,
            b_safe_mps2: driver.comfort_decel_mps2,
            ..paper
        }
    } else {
        paper
    };
    let mut notes = vec![format!(
        "{changes} of {n} situations call for a change by the criterion."
    )];
    notes.extend(examples);
    Ok(Outcome {
        rows: vec![
            Row::zero(
                "verdicts that differ from the criterion",
                disagree,
                "Kesting 2007 Eq. 1–4",
            )
            .n(n),
            Row::held(
                "share of situations that change lane",
                changes as f64 / n as f64,
                (0.05, 0.95),
                "the sample must exercise both verdicts, or agreement proves nothing",
            ),
            Row::held(
                "changing threshold Δa_th, m/s²",
                table.threshold_mps2,
                (0.1, 0.1),
                k07,
            ),
            Row::held(
                "safe deceleration b_safe, m/s²",
                table.b_safe_mps2,
                (4.0, 4.0),
                k07,
            ),
            Row::held(
                "keep-right bias Δa_bias, m/s²",
                table.right_bias_mps2,
                (0.3, 0.3),
                k07,
            ),
            Row::reported(
                "politeness p",
                paper.politeness,
                "Kesting 2007 studies p from 0 to 1 and recommends 0–0.5; 0.2 is the legacy default, not a calibration",
            ),
        ],
        notes,
    })
}

// ---------------------------------------------------------------------------------------
// HCM gaps, social force
// ---------------------------------------------------------------------------------------

fn hcm_gaps(_lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let of = |m: Movement| -> HcmGaps {
        match (variant, m) {
            (Variant::Fault(0), Movement::MinorLeft) => HcmGaps::of(Movement::MinorThrough),
            (Variant::Fault(0), Movement::MinorThrough) => HcmGaps::of(Movement::MinorLeft),
            (Variant::Fault(1), m) => {
                // HCM 2010 Eq. 19-30/19-31 with Exhibit 19-11's heavy-vehicle terms applied as
                // if every vehicle were heavy: +1.0 s (2-lane) / +2.0 s (4-lane) critical,
                // +0.9 s follow-up.
                let g = HcmGaps::of(m);
                HcmGaps {
                    critical_gap_narrow_s: g.critical_gap_narrow_s + 1.0,
                    critical_gap_wide_s: g.critical_gap_wide_s + 2.0,
                    follow_up_s: g.follow_up_s + 0.9,
                }
            }
            _ => HcmGaps::of(m),
        }
    };
    let src = "HCM 2010 Exhibit 19-10 (HCM 6th ed. Exhibit 20-11), base values";
    let mut rows = Vec::new();
    for (m, name, narrow, wide, follow) in [
        (
            Movement::MajorLeft,
            "left from the major road",
            4.1,
            4.1,
            2.2,
        ),
        (
            Movement::MinorRight,
            "right from the minor road",
            6.2,
            6.9,
            3.3,
        ),
        (
            Movement::MinorThrough,
            "through from the minor road",
            6.5,
            6.5,
            4.0,
        ),
        (
            Movement::MinorLeft,
            "left from the minor road",
            7.1,
            7.5,
            3.5,
        ),
    ] {
        let g = of(m);
        rows.push(Row::held(
            format!("{name}: critical headway, 2-lane major, s"),
            g.critical_gap_narrow_s,
            (narrow, narrow),
            src,
        ));
        rows.push(Row::held(
            format!("{name}: critical headway, 4-lane major, s"),
            g.critical_gap_wide_s,
            (wide, wide),
            src,
        ));
        rows.push(Row::held(
            format!("{name}: follow-up headway, s"),
            g.follow_up_s,
            (follow, follow),
            src,
        ));
    }
    Ok(Outcome {
        rows,
        notes: vec![],
    })
}

fn social_force(_lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let mut p = SocialForceParams::default();
    if variant == Variant::Fault(0) {
        p.tau_s = 1.0;
        p.sigma_m = 0.5;
    }
    let src = "Helbing & Molnár 1995, Phys. Rev. E 51, 4282";
    let mut values = [
        p.desired_speed_mean_mps,
        p.desired_speed_std_mps,
        p.max_speed_factor,
        p.tau_s,
        p.v0_m2_s2,
        p.sigma_m,
        p.u0_m2_s2,
        p.r_m,
        p.step_width_s,
        p.field_of_view_deg,
        p.behind_weight,
    ];
    if variant == Variant::Fault(1) {
        // The table transcribed one row out of step.
        values.rotate_left(1);
    }
    let names = [
        ("desired speed mean, m/s", 1.34),
        ("desired speed standard deviation, m/s", 0.26),
        ("maximum speed / desired speed", 1.3),
        ("relaxation time τ, s", 0.5),
        ("pedestrian repulsion V0, m²/s²", 2.1),
        ("pedestrian repulsion range σ, m", 0.3),
        ("border repulsion U0, m²/s²", 10.0),
        ("border repulsion range R, m", 0.2),
        ("step width Δt, s", 2.0),
        ("field of view 2φ, degrees", 200.0),
        ("weight behind c", 0.5),
    ];
    Ok(Outcome {
        rows: names
            .iter()
            .zip(values)
            .map(|((name, want), v)| Row::held(*name, v, (*want, *want), src))
            .collect(),
        notes: vec![],
    })
}
