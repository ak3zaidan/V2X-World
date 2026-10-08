//! The platoon experiment: how a cooperative controller's string stability, gaps and lane
//! throughput respond to the V2X channel.
//!
//! # What it runs
//!
//! `N` vehicles of one car-following model in one lane, the first scripted. After a
//! warm-up long enough for every controller to settle, the leader brakes and recovers;
//! every vehicle broadcasts its state every `interval_s` (a BSM, SAE J2945/1's 100 ms),
//! staggered across the interval, and each follower hears its predecessor and the platoon
//! leader through a channel with a loss probability and a latency. What a follower heard
//! goes into a [`crate::coop::CoopLink`] exactly as the kernel writes it in a full run, so
//! the controllers under test are the same objects, with the same degradation rule, that
//! drive the full engine.
//!
//! The channel here is deliberately the simplest thing that isolates the controller —
//! independent (Bernoulli) loss, or Gilbert–Elliott bursts, and a fixed latency — so a
//! sweep shows the controller's response to the channel and nothing else. In a full run the
//! same link is fed by the simulated radio, MAC, congestion control and security stack
//! (`v2xw-engine`'s `run/coop.rs`), and loss and latency are whatever those produce.
//!
//! # What it measures
//!
//! * **String stability** — the amplification of the leader's disturbance down the
//!   platoon: `max_i ‖a_i‖₂ / ‖a_{i−1}‖₂` over the followers (Ploeg, van de Wouw &
//!   Nijmeijer, "Lp string stability of cascaded systems", IEEE T-CST 22(2), 2014, use
//!   the L2 gain of acceleration), and the same for the spacing error's ∞-norm. Below one,
//!   a disturbance shrinks as it travels back; above one, it grows into a jam.
//! * **Gap error** — each follower's deviation from the gap it held just before the
//!   disturbance: RMS and maximum.
//! * **Lane throughput** — the platoon's time-averaged flow `3600·v̄/(mean gap + length)`,
//!   veh/h/lane: what the controller's gap policy, as actually held under this channel,
//!   would carry.
//! * **Cooperative share** — the fraction of follower steps that drove the cooperative
//!   law rather than its ACC fallback.
//! * **Safety** — the smallest gap and hardest braking seen.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use v2xw_core::geom::{Dims, Vec3};
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;
use v2xw_world::{ClassMask, LaneKind};

use super::common::keyed_uniform;
use super::library::{self, BuildCtx, CfChoice};
use crate::classes::VehicleClass;
use crate::coop::{CoopLink, CoopParams, CoopRecord};
use crate::views::{LaneView, LeaderView, VehicleView};

/// The id the channel's keyed draws are made under.
const CHANNEL_ID: &str = "mobility/car-following/platoon-channel";

/// The channel between platoon members.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Channel {
    /// The probability a message is lost, `0..=1` (the mean, for bursty loss).
    pub loss: f64,
    /// Mean burst length in messages, ≥ 1: 1 is independent loss; above 1, a two-state
    /// Gilbert–Elliott chain with this mean bad-state sojourn and `loss` as its stationary
    /// loss rate (every message in the bad state is lost).
    pub burst_len: f64,
    /// The one-way latency, seconds.
    pub latency_s: f64,
    /// The broadcast interval, seconds.
    pub interval_s: f64,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            loss: 0.0,
            burst_len: 1.0,
            latency_s: 0.01,
            interval_s: 0.1,
        }
    }
}

/// One platoon run's set-up.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlatoonSetup {
    /// The followers' model.
    pub model: CfChoice,
    /// How many vehicles, the scripted leader included.
    pub vehicles: usize,
    /// Cruise speed, m/s.
    pub cruise_mps: f64,
    /// The leader's braking, m/s², and for how long, seconds.
    pub brake: (f64, f64),
    /// The leader's recovery acceleration, m/s².
    pub recover_mps2: f64,
    /// Settling time before the disturbance, seconds.
    pub warmup_s: f64,
    /// Time after the disturbance starts, seconds.
    pub observe_s: f64,
    /// The mobility step, seconds.
    pub step_s: f64,
    /// The channel.
    pub channel: Channel,
    /// How the cooperative link ages messages.
    pub coop: CoopParams,
    /// The master seed.
    pub seed: u64,
}

impl Default for PlatoonSetup {
    fn default() -> Self {
        Self {
            model: CfChoice::of("cacc-ploeg"),
            vehicles: 8,
            cruise_mps: 25.0,
            brake: (2.0, 3.0),
            recover_mps2: 1.0,
            warmup_s: 90.0,
            observe_s: 40.0,
            step_s: 0.1,
            channel: Channel::default(),
            coop: CoopParams::default(),
            seed: 0x5EED,
        }
    }
}

/// What one run measured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatoonResult {
    /// `max_i ‖a_i‖₂/‖a_{i−1}‖₂` over followers `i ≥ 2`, over the observation window.
    pub accel_amplification: f64,
    /// `max_i ‖e_i‖∞/‖e_{i−1}‖∞` over followers `i ≥ 2`.
    pub gap_error_amplification: f64,
    /// RMS gap error over every follower and step of the observation window, metres.
    pub rms_gap_error_m: f64,
    /// The largest gap error, metres.
    pub max_gap_error_m: f64,
    /// The mean net gap held before the disturbance, metres.
    pub cruise_gap_m: f64,
    /// Time-averaged platoon flow over the whole run after warm-up, veh/h/lane.
    pub throughput_veh_h: f64,
    /// The fraction of follower steps on the cooperative law.
    pub cooperative_share: f64,
    /// The smallest net gap, metres.
    pub min_gap_m: f64,
    /// The hardest deceleration, m/s² (positive).
    pub max_decel_mps2: f64,
}

impl PlatoonResult {
    /// Whether the platoon damped the disturbance (acceleration L2 gain at most 1, with a
    /// 1 % numerical tolerance).
    pub fn string_stable(&self) -> bool {
        self.accel_amplification <= 1.01
    }
}

fn lane() -> LaneView {
    LaneView {
        id: LaneId::new(0),
        kind: LaneKind::Driving,
        speed_limit_mps: 36.0,
        width_m: 3.5,
        length_m: 1.0e7,
        allowed: ClassMask::MOTOR_TRAFFIC,
    }
}

/// Runs one platoon.
///
/// # Errors
///
/// The model name or its parameters, as [`library::build`] reports them.
pub fn run(setup: &PlatoonSetup) -> Result<PlatoonResult, String> {
    let n = setup.vehicles.max(2);
    let factory = library::factory(&setup.model.model)
        .ok_or_else(|| format!("unknown model `{}`", setup.model.model))?;
    let link = Arc::new(CoopLink::new(setup.coop, vec![VehicleClass::Passenger]));
    let model = (factory.build)(
        &setup.model.params,
        &BuildCtx {
            seed: setup.seed,
            link: Arc::clone(&link),
        },
    )?;
    let dt = setup.step_s;
    let step = Duration::from_secs_f64(dt);
    let dims = Dims::new(5.0, 1.8, 1.5);
    let mut driver = model.profile(VehicleClass::Passenger);
    driver.desired_speed_mps = setup.cruise_mps + 5.0;
    // Start at a generous gap; the warm-up settles every controller to its own.
    let start_gap = driver.min_gap_m + 1.2 * setup.cruise_mps;
    let mut s: Vec<f64> = (0..n)
        .map(|i| -(i as f64) * (start_gap + dims.length_m))
        .collect();
    let mut v = vec![setup.cruise_mps; n];
    let mut a = vec![0.0; n];
    let total = setup.warmup_s + setup.observe_s;
    let steps = (total / dt).round() as u64;
    let disturb_at = (setup.warmup_s / dt).round() as u64;
    let interval = setup.channel.interval_s.max(dt);
    let latency_ns = (setup.channel.latency_s * 1e9) as u64;
    // In flight: (deliver_at, receiver, sender, record).
    let mut flight: Vec<(SimTime, usize, usize, CoopRecord)> = Vec::new();
    // Gilbert–Elliott state per directed link, `bad` or not.
    let mut bad = vec![false; n * n];
    let ge = setup.channel.burst_len > 1.0 && setup.channel.loss > 0.0;
    let (p_exit_bad, p_enter_bad) = if ge {
        let r = 1.0 / setup.channel.burst_len;
        let l = setup.channel.loss.min(0.999);
        (r, r * l / (1.0 - l))
    } else {
        (1.0, 0.0)
    };
    let w = WeatherState::default();
    let lv = lane();
    let mut ref_gap = vec![0.0; n];
    let mut e_max = vec![0.0f64; n];
    let mut a_sq = vec![0.0f64; n];
    let mut e_sq_sum = 0.0;
    let mut e_count = 0u64;
    let mut coop_steps = 0u64;
    let mut follower_steps = 0u64;
    let mut flow_sum = 0.0;
    let mut flow_n = 0u64;
    let mut min_gap = f64::INFINITY;
    let mut max_decel = 0.0f64;
    let mut cruise_gap = 0.0;
    let view = |i: usize, s: &[f64], v: &[f64], a: &[f64]| VehicleView {
        actor: ActorId::new(i as u32),
        class: VehicleClass::Passenger,
        lane: LaneId::new(0),
        lane_index: 0,
        s_m: s[i],
        lateral_m: 0.0,
        speed_mps: v[i],
        accel_mps2: a[i],
        heading_rad: 0.0,
        dims,
        driver,
    };
    for k in 0..steps {
        let now: SimTime = k * step.as_nanos();
        let t = k as f64 * dt;
        // Deliveries due by now.
        flight.sort_by_key(|f| (f.0, f.1, f.2));
        let due = flight.partition_point(|f| f.0 <= now);
        for (_, rx, tx, rec) in flight.drain(..due) {
            let predecessor = tx + 1 == rx;
            link.record(
                ActorId::new(rx as u32),
                ActorId::new(tx as u32),
                rec,
                predecessor,
            );
        }
        // Broadcasts in [t, t + dt), staggered by vehicle.
        for i in 0..n {
            let offset = interval * (i as f64) / (n as f64);
            let phase = (t - offset) / interval;
            let next = phase.ceil();
            if next * interval + offset >= t + dt - 1e-12 {
                continue;
            }
            let msg = next as u64;
            let rec = CoopRecord {
                generated: now,
                received: now + latency_ns,
                speed_mps: v[i],
                accel_mps2: a[i],
                accel_from_message: true,
                claimed_pos: Vec3::new(s[i], 0.0, 0.0),
            };
            for rx in (i + 1)..n {
                // A follower hears its predecessor and the platoon leader.
                if rx != i + 1 && i != 0 {
                    continue;
                }
                let key = ActorId::new((rx * n + i) as u32);
                let u = keyed_uniform(setup.seed, CHANNEL_ID, key, msg, 0);
                let lost = if ge {
                    let st = &mut bad[rx * n + i];
                    let flip = keyed_uniform(setup.seed, CHANNEL_ID, key, msg, 1);
                    *st = if *st {
                        flip >= p_exit_bad
                    } else {
                        flip < p_enter_bad
                    };
                    *st
                } else {
                    u < setup.channel.loss
                };
                if !lost {
                    flight.push((
                        now + latency_ns,
                        rx,
                        i,
                        CoopRecord {
                            received: now + latency_ns,
                            ..rec
                        },
                    ));
                }
            }
        }
        model.begin_step(now, step);
        // Accelerations, Jacobi.
        let mut next_a = vec![0.0; n];
        next_a[0] = if k < disturb_at {
            0.0
        } else {
            let tt = (k - disturb_at) as f64 * dt;
            let (b, dur) = setup.brake;
            if tt < dur {
                -b
            } else if v[0] < setup.cruise_mps {
                setup.recover_mps2.min((setup.cruise_mps - v[0]) / dt)
            } else {
                0.0
            }
        };
        for i in 1..n {
            let gap = s[i - 1] - dims.length_m - s[i];
            let lead = LeaderView::of(view(i - 1, &s, &v, &a), gap);
            let ego = view(i, &s, &v, &a);
            next_a[i] = model.accel(&ego, Some(&lead), &lv, &w);
            if k >= disturb_at {
                follower_steps += 1;
                if link
                    .fresh(ActorId::new(i as u32), ActorId::new((i - 1) as u32), now)
                    .is_some()
                {
                    coop_steps += 1;
                }
            }
        }
        // Integrate (ballistic, never reversing).
        for i in 0..n {
            let v1 = (v[i] + next_a[i] * dt).max(0.0);
            s[i] += 0.5 * (v[i] + v1) * dt;
            a[i] = (v1 - v[i]) / dt;
            v[i] = v1;
        }
        // Measure.
        let gaps: Vec<f64> = (1..n).map(|i| s[i - 1] - dims.length_m - s[i]).collect();
        if k + 1 == disturb_at {
            for i in 1..n {
                ref_gap[i] = gaps[i - 1];
            }
            cruise_gap = gaps.iter().sum::<f64>() / gaps.len() as f64;
        }
        if k >= disturb_at {
            for i in 1..n {
                let e = gaps[i - 1] - ref_gap[i];
                e_max[i] = e_max[i].max(e.abs());
                a_sq[i] += a[i] * a[i] * dt;
                e_sq_sum += e * e;
                e_count += 1;
                min_gap = min_gap.min(gaps[i - 1]);
                max_decel = max_decel.max(-a[i]);
            }
            a_sq[0] += a[0] * a[0] * dt;
            let mean_v = v.iter().sum::<f64>() / n as f64;
            let mean_spacing = gaps.iter().sum::<f64>() / gaps.len() as f64 + dims.length_m;
            flow_sum += 3600.0 * mean_v / mean_spacing.max(1e-3);
            flow_n += 1;
        }
    }
    let mut accel_amp: f64 = 0.0;
    let mut gap_amp: f64 = 0.0;
    for i in 2..n {
        if a_sq[i - 1] > 1e-12 {
            accel_amp = accel_amp.max(math::sqrt(a_sq[i] / a_sq[i - 1]));
        }
        if e_max[i - 1] > 1e-9 {
            gap_amp = gap_amp.max(e_max[i] / e_max[i - 1]);
        }
    }
    Ok(PlatoonResult {
        accel_amplification: accel_amp,
        gap_error_amplification: gap_amp,
        rms_gap_error_m: math::sqrt(e_sq_sum / e_count.max(1) as f64),
        max_gap_error_m: e_max.iter().copied().fold(0.0, f64::max),
        cruise_gap_m: cruise_gap,
        throughput_veh_h: flow_sum / flow_n.max(1) as f64,
        cooperative_share: if model.cooperative().is_some() {
            coop_steps as f64 / follower_steps.max(1) as f64
        } else {
            0.0
        },
        min_gap_m: min_gap,
        max_decel_mps2: max_decel,
    })
}

/// One row of a sweep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepRow {
    /// The model's short name.
    pub model: String,
    /// The loss probability.
    pub loss: f64,
    /// The latency, seconds.
    pub latency_s: f64,
    /// What it measured.
    pub result: PlatoonResult,
}

/// Runs every `(model, loss, latency)` combination over `base`, in that nesting order.
///
/// # Errors
///
/// The first model a run refuses.
pub fn sweep(
    base: &PlatoonSetup,
    models: &[CfChoice],
    losses: &[f64],
    latencies_s: &[f64],
) -> Result<Vec<SweepRow>, String> {
    let mut rows = Vec::new();
    for m in models {
        for &loss in losses {
            for &lat in latencies_s {
                let setup = PlatoonSetup {
                    model: m.clone(),
                    channel: Channel {
                        loss,
                        latency_s: lat,
                        ..base.channel
                    },
                    ..base.clone()
                };
                rows.push(SweepRow {
                    model: if m.params.is_null() {
                        m.model.clone()
                    } else {
                        format!("{} {}", m.model, m.params)
                    },
                    loss,
                    latency_s: lat,
                    result: run(&setup)?,
                });
            }
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quick(model: &str, loss: f64) -> PlatoonResult {
        run(&PlatoonSetup {
            model: CfChoice::of(model),
            vehicles: 6,
            warmup_s: 60.0,
            observe_s: 30.0,
            channel: Channel {
                loss,
                ..Channel::default()
            },
            ..PlatoonSetup::default()
        })
        .unwrap()
    }

    #[test]
    fn cacc_string_stable_with_messages_degrades_without() {
        let good = quick("cacc-ploeg", 0.0);
        let dead = quick("cacc-ploeg", 1.0);
        assert!(good.cooperative_share > 0.99, "{good:?}");
        assert_eq!(dead.cooperative_share, 0.0, "{dead:?}");
        assert!(good.string_stable(), "{good:?}");
        assert!(
            good.min_gap_m > 0.0 && dead.min_gap_m > 0.0,
            "{good:?} {dead:?}"
        );
        // The cooperative platoon holds a tighter gap (0.7 s against the fallback's 1.1 s),
        // so it carries more, and it holds that gap more closely through the disturbance.
        assert!(
            good.throughput_veh_h > 1.2 * dead.throughput_veh_h,
            "{good:?} {dead:?}"
        );
        assert!(
            good.rms_gap_error_m < dead.rms_gap_error_m,
            "{good:?} {dead:?}"
        );
    }

    #[test]
    fn the_run_is_a_pure_function_of_its_setup() {
        let a = quick("cacc-milanes", 0.3);
        let b = quick("cacc-milanes", 0.3);
        assert_eq!(a, b);
    }
}
