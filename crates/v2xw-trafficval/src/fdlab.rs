//! The traffic-calibration study: fundamental diagrams, shockwaves, headways and driver
//! heterogeneity on the engine's own mobility, against the published fits and SUMO's
//! default driver.
//!
//! Every run is a single-lane ring (`v2xw_mobility::worlds::ring`) driven by
//! [`NativeMobility`] exactly as a scenario drives it, loaded evenly at one density with one
//! car slowed, warmed up and then measured:
//!
//! * **flow and speed** — the space-mean speed over every car and every step of the window,
//!   and the flow `q = k·v̄` (Edie's definitions over the whole ring);
//! * **time headways** — every front passage of a fixed detector, interpolated inside the
//!   step;
//! * **the jam's drift** — the position of the slowest car while some car is below 2 m/s,
//!   whose least-squares slope is the stop-and-go wave's speed;
//! * **the smallest net gap** any car had, the collision check.
//!
//! The configurations: the engine's city drivers (`IdmPreset::UrbanHcm`) as one identical
//! driver and as the `human` population of [`HumanDriverParams::human`]; Treiber 2000's
//! freeway drivers on a 120 km/h ring (the "corridor"); and SUMO's default driver (Krauß
//! with σ = 0.5, τ = 1 s, the SUMO vType defaults) on both.

use std::collections::BTreeMap;
use std::sync::Arc;

use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::math;
use v2xw_core::rng::RngRegistry;
use v2xw_core::time::Duration;
use v2xw_mobility::carfollowing::idm::{Idm, IdmPreset};
use v2xw_mobility::carfollowing::krauss::{Krauss, KraussParams};
use v2xw_mobility::ctx::MobilityCtx;
use v2xw_mobility::demand::NoDemand;
use v2xw_mobility::drivers::{self, HumanDriverParams};
use v2xw_mobility::fd;
use v2xw_mobility::lanechange::mobil::MobilPreset;
use v2xw_mobility::traits::{CarFollowing, Mobility};
use v2xw_mobility::views::DriverProfile;
use v2xw_mobility::worlds::{RingParams, cycle_length_m, ring, ring_cycle};
use v2xw_mobility::{EngineParams, IntersectionMode, NativeMobility, VehicleClass};

/// Which car-following model drives the ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// The IDM with a preset's drivers.
    Idm(IdmPreset),
    /// SUMO's default: Krauß, σ = 0.5, the SUMO vType car.
    KraussSumo,
}

/// One ring configuration.
#[derive(Debug, Clone, Copy)]
pub struct RingSpec {
    /// The model.
    pub model: Model,
    /// The human-driver parameters (all zero: the ideal model).
    pub human: HumanDriverParams,
    /// The ring's length, metres.
    pub circumference_m: f64,
    /// The desired speed and the limit, m/s.
    pub v0_mps: f64,
    /// Seconds of warm-up.
    pub warmup_s: f64,
    /// Seconds measured.
    pub measure_s: f64,
    /// The master seed.
    pub seed: u64,
}

/// What one ring at one density produced.
#[derive(Debug, Clone)]
pub struct RingRun {
    /// The density, veh/km.
    pub density_veh_km: f64,
    /// Space-mean speed, m/s.
    pub speed_mps: f64,
    /// Flow, veh/h.
    pub flow_veh_h: f64,
    /// The standard deviation of every car's speed over the window, m/s.
    pub speed_sd_mps: f64,
    /// The slowest any car went, m/s.
    pub min_speed_mps: f64,
    /// The jam's drift, km/h (negative upstream), NaN without a jam.
    pub wave_kmh: f64,
    /// The time headways at the detector, seconds.
    pub headways_s: Vec<f64>,
    /// The smallest net gap any car had, metres.
    pub min_gap_m: f64,
}

/// Runs `spec` loaded to `density_veh_km`.
///
/// # Errors
/// When the ring cannot be built or a car cannot be placed.
pub fn run_ring(spec: &RingSpec, density_veh_km: f64) -> Result<RingRun, String> {
    let world = ring(&RingParams {
        circumference_m: spec.circumference_m,
        segments: 6,
        points_per_segment: 16,
        lanes: 1,
        lane_width_m: 3.5,
        speed_limit_mps: spec.v0_mps,
    })
    .map_err(|e| e.to_string())?;
    let cycle = ring_cycle(&world, 0);
    let length = cycle_length_m(&world, &cycle);
    let mut offsets = Vec::with_capacity(cycle.len());
    let mut acc = 0.0;
    for l in &cycle {
        offsets.push(acc);
        acc += world.lane(*l).length_m;
    }
    let n = ((density_veh_km / 1000.0) * length).round().max(2.0) as usize;
    let class = VehicleClass::Passenger;
    let body = class.spec().length_m;
    let (cf, base): (Arc<dyn CarFollowing + Send + Sync>, DriverProfile) = match spec.model {
        Model::Idm(p) => (Arc::new(Idm::new(p)), p.profile(class)),
        Model::KraussSumo => (
            Arc::new(Krauss::new(KraussParams::default(), spec.seed)),
            Krauss::class_profile(class),
        ),
    };
    let step = Duration::from_millis(100);
    let mut engine = NativeMobility::with_models(
        EngineParams {
            step,
            intersections: IntersectionMode::None,
            lane_changes: false,
            lookahead_m: (length / n as f64).max(80.0) * 1.5,
            dynamic_rerouting: false,
            human: spec.human,
            ..EngineParams::default()
        },
        cf,
        MobilPreset::Kesting2007,
    );
    let rng = RngRegistry::new(spec.seed);
    {
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        engine
            .init(&mut ctx, Box::new(NoDemand::new()))
            .map_err(|e| e.to_string())?;
    }
    let total_s = spec.warmup_s + spec.measure_s;
    let laps = ((spec.v0_mps * total_s / length).ceil() as usize + 3).max(3);
    let route: Vec<LaneId> = cycle
        .iter()
        .cycle()
        .take(cycle.len() * laps)
        .copied()
        .collect();
    let spacing = length / n as f64;
    let mut d = base;
    d.desired_speed_mps = spec.v0_mps;
    let ve = match spec.model {
        Model::Idm(p) => fd::equilibrium_speed_mps(&Idm::new(p), &d, spec.v0_mps, spacing - body),
        Model::KraussSumo => spec
            .v0_mps
            .min((spacing - body - d.min_gap_m).max(0.0) / 1.0),
    };
    let mut index: BTreeMap<ActorId, usize> = BTreeMap::new();
    for i in 0..n {
        let mut s = spacing * i as f64;
        let mut k = 0;
        while k + 1 < cycle.len() && s > world.lane(cycle[k]).length_m {
            s -= world.lane(cycle[k]).length_m;
            k += 1;
        }
        // Each driver their own parameters, as the engine draws them for a trip.
        let z = [0u64, 1, 2, 3].map(|slot| drivers::std_normal(spec.seed, i as u64, slot));
        let di = drivers::spread_profile(d, class, z, spec.human.parameter_spread);
        let id = engine
            .spawn_with_route(&world, 0, class, di, route[k..].to_vec(), s)
            .map_err(|e| e.to_string())?;
        engine
            .set_speed(id, if i == 0 { 0.8 * ve } else { ve })
            .map_err(|e| e.to_string())?;
        index.insert(id, i);
    }
    let lane_index: BTreeMap<LaneId, usize> =
        cycle.iter().enumerate().map(|(i, l)| (*l, i)).collect();
    let step_ns = step.as_nanos();
    let dt = step.as_secs_f64();
    let warm_ns = (spec.warmup_s * 1e9) as u64;
    let end_ns = (total_s * 1e9) as u64;
    let mut unwrapped: Vec<Option<f64>> = vec![None; n];
    let mut raw_last = vec![0.0f64; n];
    let mut speeds: Vec<f64> = Vec::new();
    let mut min_speed = f64::INFINITY;
    let mut min_gap = f64::INFINITY;
    let mut passages: Vec<f64> = Vec::new();
    // Jam outflow: (time, car, position) each time a car that stood below 1 m/s rises
    // past 3 m/s.
    let mut stood = vec![false; n];
    let mut departures: Vec<(f64, usize, f64)> = Vec::new();
    let mut t = 0u64;
    while t < end_ns {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        engine.step(&mut ctx, step);
        t += step_ns;
        let measuring = t > warm_ns;
        for (id, lane, s, v) in engine.longitudinal_states() {
            let Some(&i) = index.get(&id) else { continue };
            let raw = lane_index.get(&lane).map_or(f64::NAN, |k| offsets[*k] + s);
            if !raw.is_finite() {
                continue;
            }
            let u = match unwrapped[i] {
                None => raw,
                Some(prev) => {
                    let mut dlt = raw - raw_last[i];
                    if dlt < -length / 2.0 {
                        dlt += length;
                        // The front passed the detector at offset 0 this step.
                        if measuring && v > 0.0 {
                            let frac = (raw / (v * dt).max(1e-9)).clamp(0.0, 1.0);
                            passages.push(t as f64 / 1e9 - frac * dt);
                        }
                    } else if dlt > length / 2.0 {
                        dlt -= length;
                    }
                    prev + dlt
                }
            };
            unwrapped[i] = Some(u);
            raw_last[i] = raw;
            if measuring {
                speeds.push(v);
                min_speed = min_speed.min(v);
                if v < 1.0 {
                    stood[i] = true;
                } else if v > 3.0 && stood[i] {
                    stood[i] = false;
                    departures.push((t as f64 / 1e9, i, u));
                }
            }
        }
        if measuring {
            // Net gaps: car i follows car i + 1 (placed ahead of it), the last the first.
            for i in 0..n {
                let (Some(me), Some(ahead)) = (unwrapped[i], unwrapped[(i + 1) % n]) else {
                    continue;
                };
                let lead = if i + 1 == n { ahead + length } else { ahead };
                // Unwrapped positions all start inside one lap, so the leader is ahead by
                // less than one lap; fold any multiple out.
                let mut gap = lead - me;
                while gap > length {
                    gap -= length;
                }
                while gap < 0.0 {
                    gap += length;
                }
                min_gap = min_gap.min(gap - body);
            }
        }
    }
    let mean = math::sum_ordered(speeds.clone()) / speeds.len().max(1) as f64;
    let var = math::sum_ordered(
        speeds
            .iter()
            .map(|v| (v - mean) * (v - mean))
            .collect::<Vec<f64>>(),
    ) / speeds.len().max(1) as f64;
    passages.sort_by(f64::total_cmp);
    let headways: Vec<f64> = passages.windows(2).map(|w| w[1] - w[0]).collect();
    let wave = outflow_wave_kmh(&departures, n, length);
    let density = n as f64 / length * 1000.0;
    Ok(RingRun {
        density_veh_km: density,
        speed_mps: mean,
        flow_veh_h: density * mean * 3.6,
        speed_sd_mps: var.max(0.0).sqrt(),
        min_speed_mps: min_speed,
        wave_kmh: wave,
        headways_s: headways,
        min_gap_m: min_gap,
    })
}

/// The speed of a jam's outflow front, km/h (negative: upstream), from departures
/// `(t, car, unwrapped position)`: for each car leaving a jam, the next departure of the
/// car behind it (car `i − 1` follows car `i`) within 15 s gives one `Δx/Δt`, with `Δx`
/// folded into one lap; the median of at least five. NaN when there is no jam.
///
/// This is how a jam front is read off trajectories: the front is where cars accelerate
/// out, and it travels upstream one car at a time (Treiber & Kesting 2013 §18.1).
pub fn outflow_wave_kmh(departures: &[(f64, usize, f64)], n: usize, length: f64) -> f64 {
    let mut by_car: Vec<Vec<(f64, f64)>> = vec![Vec::new(); n];
    for &(t, i, u) in departures {
        by_car[i].push((t, u));
    }
    let mut speeds: Vec<f64> = Vec::new();
    for &(t, i, u) in departures {
        let follower = (i + n - 1) % n;
        if let Some(&(tf, uf)) = by_car[follower]
            .iter()
            .find(|(tf, _)| *tf > t && *tf - t < 15.0)
        {
            let mut dx = (uf - u).rem_euclid(length);
            if dx > length / 2.0 {
                dx -= length;
            }
            speeds.push(dx / (tf - t) * 3.6);
        }
    }
    if speeds.len() < 5 {
        return f64::NAN;
    }
    speeds.sort_by(f64::total_cmp);
    speeds[speeds.len() / 2]
}

/// A straight-line fit with its coefficient of determination.
#[derive(Debug, Clone, Copy)]
pub struct Fit {
    /// Slope.
    pub slope: f64,
    /// Intercept.
    pub intercept: f64,
    /// R².
    pub r2: f64,
}

/// Least squares with R².
pub fn fit(points: &[(f64, f64)]) -> Fit {
    let (slope, intercept) = fd::least_squares(points);
    let n = points.len().max(1) as f64;
    let mean = points.iter().map(|p| p.1).sum::<f64>() / n;
    let ss_tot: f64 = points.iter().map(|p| (p.1 - mean).powi(2)).sum();
    let ss_res: f64 = points
        .iter()
        .map(|p| (p.1 - (slope * p.0 + intercept)).powi(2))
        .sum();
    Fit {
        slope,
        intercept,
        r2: if ss_tot > 0.0 {
            1.0 - ss_res / ss_tot
        } else {
            f64::NAN
        },
    }
}

/// The Greenshields and triangular fits of one fundamental diagram.
#[derive(Debug, Clone, Copy)]
pub struct DiagramFits {
    /// Greenshields free speed, km/h.
    pub gs_free_kmh: f64,
    /// Greenshields jam density, veh/km.
    pub gs_jam_veh_km: f64,
    /// Greenshields capacity `v_f·k_j/4`, veh/h.
    pub gs_capacity_veh_h: f64,
    /// R² of the Greenshields speed-density line.
    pub gs_r2: f64,
    /// Triangular free speed (mean speed of the free-flow points), km/h.
    pub tri_free_kmh: f64,
    /// Triangular congested wave speed, km/h (negative).
    pub tri_wave_kmh: f64,
    /// Triangular jam density, veh/km.
    pub tri_jam_veh_km: f64,
    /// Triangular capacity, veh/h.
    pub tri_capacity_veh_h: f64,
    /// Triangular critical density, veh/km.
    pub tri_critical_veh_km: f64,
    /// R² of the congested branch's flow-density line.
    pub tri_r2: f64,
    /// The highest flow measured, veh/h.
    pub measured_capacity_veh_h: f64,
}

/// Fits Greenshields (speed linear in density) and a triangular diagram (free branch at
/// the free speed, congested branch a straight line in flow-density) to `runs`.
pub fn fit_diagram(runs: &[RingRun], v0_mps: f64) -> DiagramFits {
    let sk: Vec<(f64, f64)> = runs
        .iter()
        .map(|r| (r.density_veh_km, r.speed_mps * 3.6))
        .collect();
    let g = fit(&sk);
    let gs_free = g.intercept;
    let gs_jam = -g.intercept / g.slope;
    let free: Vec<&RingRun> = runs
        .iter()
        .filter(|r| r.speed_mps >= 0.9 * v0_mps)
        .collect();
    let cong: Vec<(f64, f64)> = runs
        .iter()
        .filter(|r| r.speed_mps < 0.9 * v0_mps)
        .map(|r| (r.density_veh_km, r.flow_veh_h))
        .collect();
    let tri_free = if free.is_empty() {
        v0_mps * 3.6
    } else {
        free.iter().map(|r| r.speed_mps).sum::<f64>() / free.len() as f64 * 3.6
    };
    let c = fit(&cong);
    let w = c.slope; // km/h: veh/h per veh/km
    let kj = -c.intercept / c.slope;
    // Intersection of q = v_f·k and q = w·(k − k_j).
    let kc = -w * kj / (tri_free - w);
    DiagramFits {
        gs_free_kmh: gs_free,
        gs_jam_veh_km: gs_jam,
        gs_capacity_veh_h: gs_free * gs_jam / 4.0,
        gs_r2: g.r2,
        tri_free_kmh: tri_free,
        tri_wave_kmh: w,
        tri_jam_veh_km: kj,
        tri_capacity_veh_h: tri_free * kc,
        tri_critical_veh_km: kc,
        tri_r2: c.r2,
        measured_capacity_veh_h: runs.iter().map(|r| r.flow_veh_h).fold(0.0, f64::max),
    }
}

/// Summary statistics of a set of time headways.
#[derive(Debug, Clone, Copy)]
pub struct HeadwayStats {
    /// How many.
    pub n: usize,
    /// Mean, seconds.
    pub mean_s: f64,
    /// Coefficient of variation.
    pub cv: f64,
    /// Share below 1 s.
    pub below_1s: f64,
    /// 15th, 50th and 85th percentiles, seconds.
    pub p15_s: f64,
    /// Median.
    pub p50_s: f64,
    /// 85th percentile.
    pub p85_s: f64,
}

/// The statistics of `h`.
pub fn headway_stats(h: &[f64]) -> HeadwayStats {
    let mut v: Vec<f64> = h
        .iter()
        .copied()
        .filter(|x| x.is_finite() && *x > 0.0)
        .collect();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n == 0 {
        return HeadwayStats {
            n,
            mean_s: f64::NAN,
            cv: f64::NAN,
            below_1s: f64::NAN,
            p15_s: f64::NAN,
            p50_s: f64::NAN,
            p85_s: f64::NAN,
        };
    }
    let mean = v.iter().sum::<f64>() / n as f64;
    let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64).sqrt();
    let q = |p: f64| v[((p * (n - 1) as f64).round() as usize).min(n - 1)];
    HeadwayStats {
        n,
        mean_s: mean,
        cv: sd / mean,
        below_1s: v.iter().filter(|x| **x < 1.0).count() as f64 / n as f64,
        p15_s: q(0.15),
        p50_s: q(0.5),
        p85_s: q(0.85),
    }
}
