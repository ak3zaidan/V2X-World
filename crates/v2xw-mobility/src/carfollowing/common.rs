//! Plumbing every model of the driving-model library shares.
//!
//! Three things, each of which the IDM does for itself in `idm.rs` and which the library's
//! other models must do *the same way* or a model comparison measures the plumbing rather
//! than the model:
//!
//! 1. **The driver in this weather on this lane** ([`situation`]): the desired speed is the
//!    lower of the driver's wish and the lane's limit, slowed by the cited weather table,
//!    and in fog capped to what the driver can stop from within the visibility; the
//!    headway grows and the deceleration is capped by the surface. Exactly the order
//!    [`crate::carfollowing::idm`] applies and records on its card (04-models.md §2.6).
//! 2. **The step clock** ([`StepClock`]): a discrete-time model (Krauss, Gipps, the PATH
//!    controllers) needs the step length and, for its keyed noise, the step index. The
//!    [`crate::traits::CarFollowing`] trait passes neither to `accel`, so the engine calls
//!    [`crate::traits::CarFollowing::begin_step`] once per mobility step and the model
//!    keeps the two numbers here. Written once per step, before any `accel` of that step,
//!    so every read in a step sees the same values whatever order actors are visited in.
//! 3. **Keyed noise** ([`keyed_uniform`]): Krauss's dawdling and Wiedemann's random
//!    thresholds are per-step random draws. They come from a ChaCha stream keyed by
//!    `(master seed, the model's plug-in domain, the actor)` and positioned at the step
//!    index, so a draw is a pure function of `(seed, model, actor, step, slot)`: the same
//!    on every thread count, unaffected by how many other actors drew, and the same for a
//!    hypothetical evaluation (MOBIL asking "what would the follower do?") as for the real
//!    one in the same step.

use core::sync::atomic::{AtomicU64, Ordering};

use v2xw_core::ids::ActorId;
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;

use crate::views::{DriverProfile, LaneView, VehicleView};
use crate::weather::{self, RoadContext, WeatherResponse};

/// The step length assumed before the engine has called `begin_step` once, seconds: the
/// native engine's default mobility step (`EngineParams::default().step`, 100 ms).
pub const DEFAULT_STEP_S: f64 = 0.1;

/// The driver a model should drive with on this lane in this weather.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Situation {
    /// The driver, with `desired_speed_mps` already the effective desired speed (limit,
    /// weather, sight distance) and `time_headway_s`/`comfort_decel_mps2` weather-adjusted.
    pub driver: DriverProfile,
    /// The hardest the surface lets any vehicle brake, m/s² (positive; infinite when the
    /// weather caps nothing).
    pub grip_decel_mps2: f64,
    /// The weather's multiplier on any time gap (1 in clear weather): what a controller
    /// with its own time-gap setting (ACC, CACC) applies to that setting.
    pub headway_factor: f64,
}

/// The effective driver: see the module documentation, point 1.
pub fn situation(
    ego: &VehicleView,
    lane: &LaneView,
    w: &WeatherState,
    response: WeatherResponse,
    road: RoadContext,
) -> Situation {
    let road = road.resolve(lane.speed_limit_mps);
    let effects = weather::driving_effects(response, w, road);
    let mut v0 =
        effects.apply_to_desired_speed(ego.driver.desired_speed_mps.min(lane.speed_limit_mps));
    if response == WeatherResponse::Fhwa {
        v0 = v0.min(weather::sight_limited_speed_mps(effects.visibility_m));
    }
    Situation {
        driver: DriverProfile {
            desired_speed_mps: v0.max(0.0),
            max_accel_mps2: ego.driver.max_accel_mps2,
            comfort_decel_mps2: effects.cap_decel(ego.driver.comfort_decel_mps2),
            time_headway_s: effects.apply_to_headway(ego.driver.time_headway_s),
            min_gap_m: ego.driver.min_gap_m,
        },
        grip_decel_mps2: effects.max_decel_mps2,
        headway_factor: effects.apply_to_headway(1.0),
    }
}

/// The step clock: see the module documentation, point 2.
///
/// Atomics rather than a lock because the models are shared as
/// `Arc<dyn CarFollowing + Send + Sync>`; `Relaxed` is enough because the engine writes
/// before the step's map begins and reads happen-after through the step's own ordering.
#[derive(Debug, Default)]
pub struct StepClock {
    now_ns: AtomicU64,
    dt_ns: AtomicU64,
}

impl StepClock {
    /// Records the step about to run.
    pub fn begin(&self, now: SimTime, dt: Duration) {
        self.now_ns.store(now, Ordering::Relaxed);
        self.dt_ns.store(dt.as_nanos(), Ordering::Relaxed);
    }

    /// The instant the current step started at.
    pub fn now(&self) -> SimTime {
        self.now_ns.load(Ordering::Relaxed)
    }

    /// The current step's length, seconds ([`DEFAULT_STEP_S`] before the first step).
    pub fn dt_s(&self) -> f64 {
        match self.dt_ns.load(Ordering::Relaxed) {
            0 => DEFAULT_STEP_S,
            ns => ns as f64 * 1e-9,
        }
    }

    /// The index of the current step: `now / dt`, the position of the keyed noise.
    pub fn step_index(&self) -> u64 {
        match self.dt_ns.load(Ordering::Relaxed) {
            0 => 0,
            dt => self.now() / dt,
        }
    }
}

impl Clone for StepClock {
    fn clone(&self) -> Self {
        let c = StepClock::default();
        c.now_ns.store(self.now(), Ordering::Relaxed);
        c.dt_ns
            .store(self.dt_ns.load(Ordering::Relaxed), Ordering::Relaxed);
        c
    }
}

/// A uniform draw in `[0, 1)`: see the module documentation, point 3.
///
/// `slot` separates two draws one model makes for one actor in one step (Wiedemann 99
/// makes two). Each slot is one 64-bit word, so a model may use up to 8 slots a step.
pub fn keyed_uniform(seed: u64, model_id: &str, actor: ActorId, step: u64, slot: u8) -> f64 {
    let mut s = RngStream::derive(seed, RngDomain::plugin(model_id), EntityRef::Actor(actor));
    s.set_word_pos(u128::from(step) * 16 + u128::from(slot.min(7)) * 2);
    s.f64()
}

/// The safe speed of Krauß's model: the highest speed from which the follower, reacting
/// after `tau`, can still stop behind a leader that brakes at the same deceleration `b`
/// (Krauß 1998, DLR-Forschungsbericht 98-08, eq. for `v_safe`; the form SUMO's
/// `MSCFModel_KraussOrig1` documents):
///
/// ```text
/// v_safe = v_l + (g − v_l·τ) / ( (v + v_l)/(2b) + τ )
/// ```
///
/// `gap_m` is the net gap. Never negative.
pub fn krauss_safe_speed(v: f64, v_lead: f64, gap_m: f64, b: f64, tau: f64) -> f64 {
    let v_bar = 0.5 * (v + v_lead);
    let denom = v_bar / b.max(0.1) + tau;
    (v_lead + (gap_m - v_lead * tau) / denom.max(1e-6)).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_noise_is_a_pure_function_of_its_key() {
        let a = keyed_uniform(7, "mobility/car-following/krauss", ActorId::new(3), 10, 0);
        let b = keyed_uniform(7, "mobility/car-following/krauss", ActorId::new(3), 10, 0);
        assert_eq!(a.to_bits(), b.to_bits());
        let c = keyed_uniform(7, "mobility/car-following/krauss", ActorId::new(3), 11, 0);
        let d = keyed_uniform(7, "mobility/car-following/krauss", ActorId::new(4), 10, 0);
        let e = keyed_uniform(7, "mobility/car-following/krauss", ActorId::new(3), 10, 1);
        assert!(a != c && a != d && a != e);
        assert!((0.0..1.0).contains(&a));
    }

    #[test]
    fn the_safe_speed_stops_behind_a_standing_leader() {
        // A standing leader 0 m ahead: the safe speed is 0.
        assert_eq!(krauss_safe_speed(10.0, 0.0, 0.0, 4.5, 1.0), 0.0);
        // Following at equal speed with the gap v·τ: the safe speed is the leader's.
        let v = krauss_safe_speed(20.0, 20.0, 20.0, 4.5, 1.0);
        assert!((v - 20.0).abs() < 1e-9, "{v}");
    }
}
