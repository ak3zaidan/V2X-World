//! The LC2013 lane-change motivations, on top of the native strategic layer.
//!
//! Re-implemented from J. Erdmann (2015), "SUMO's Lane-Changing Model", in M. Behrisch and
//! M. Weber (eds.), *Modeling Mobility with Open Data*, Lecture Notes in Mobility,
//! Springer, pp. 105-123, doi:10.1007/978-3-319-15024-6_7. The constants and defaults are
//! SUMO's documented ones (vType `lcStrategic`, `lcCooperative`, `lcSpeedGain`,
//! `lcKeepRight`, `lcSpeedGainRight`, `lcLookaheadLeft`, `lcSpeedGainRemainTime`), used as
//! facts with this citation. SUMO (EPL-2.0 OR GPL-2.0-or-later) was read for behaviour
//! only and no code was copied (licence audit, `docs/design/frameworks/licence-audit.md`).
//!
//! The model, as the paper sets it out:
//!
//! * **Strategic look-ahead.** A change the route needs becomes due within
//!   `lookAheadSpeed · 10 s · lcStrategic · (1 right, 2 left) + 2 · (length + minGap)`
//!   of the end of the lane. Further out the vehicle moves to its best lane only when that
//!   lane is no slower. The look-ahead speed follows a rise in speed at once and a fall
//!   with a memory of 0.9 per second, so a braking driver keeps planning ahead.
//! * **Speed gain.** The benefit of each side is accumulated over time: every step adds
//!   `dt ·` the side's relative gain `(v_side − v_own) / max(v_side, 10 m/s)`. The benefit
//!   halves per second while the own lane is faster (on the right, faster by more than
//!   5 km/h) and on the left fades at 0.8 per second while the two are equal. A change is
//!   wanted once it passes `0.2 / lcSpeedGain` (left) or
//!   `0.2 / lcSpeedGainRight / lcSpeedGain` (right, `lcSpeedGainRight` = 0.1), and only
//!   onto a lane that can be kept for `lcSpeedGainRemainTime` = 20 s.
//! * **Keep right.** While the right lane is not more than 5 km/h slower, a second
//!   accumulator falls in proportion to how long the driver could hold its full speed on
//!   the right lane, out of an acceptance time `7 · roadSpeedFactor · max(1, v)` seconds,
//!   reaching the right-side threshold over 5 s. It asks for the change once
//!   `acc · lcKeepRight < −threshold`.
//! * **Cooperation.** A vehicle that blocks a neighbour's urgent strategic change, as that
//!   neighbour's target-lane follower, moves out of the way to its other side unless the
//!   inconvenience exceeds `lcCooperative`. The inconvenience is the larger of the
//!   relative speed loss and the opposite side's speed-gain benefit over its threshold,
//!   capped at 1.
//!
//! Both lanes' safe speeds are Krauss's safe following speed behind the lane's leader
//! (Krauss 1998, eq. 2.7, τ = 1 s), capped at the vehicle's free speed.
//!
//! Not modelled: the target-lane follower's courtesy braking (tried and dropped; see
//! `strategic_fall_back` in the engine), sublane, opposite-direction overtaking and the
//! roundabout bonus.

use serde::{Deserialize, Serialize};

use crate::views::{LeaderView, Side};

/// The strategic look-ahead horizon, seconds (Erdmann 2015).
pub const LOOK_FORWARD_S: f64 = 10.0;
/// The left-side look-ahead factor (SUMO vType `lcLookaheadLeft` default).
pub const LOOKAHEAD_LEFT: f64 = 2.0;
/// The look-ahead speed's memory per second when the speed falls.
pub const LOOK_AHEAD_SPEED_MEMORY: f64 = 0.9;
/// The right-side speed-gain eagerness (SUMO vType `lcSpeedGainRight` default).
pub const SPEED_GAIN_RIGHT: f64 = 0.1;
/// How long a speed-gain target lane must stay usable (SUMO vType
/// `lcSpeedGainRemainTime` default), seconds.
pub const SPEED_GAIN_REMAIN_TIME_S: f64 = 20.0;
/// The floor of the speed that normalises a relative gain, m/s.
pub const RELGAIN_NORMALIZATION_MIN_SPEED: f64 = 10.0;
/// Turn-lane distance, metres: a driver on a lane that keeps its route does not leave
/// it for one that turns off within this distance.
pub const TURN_LANE_DIST_M: f64 = 200.0;
/// The time over which the keep-right accumulator reaches its threshold, seconds.
pub const KEEP_RIGHT_TIME_S: f64 = 5.0;
/// The deceleration `vSafe` is judged at: SUMO's passenger-car `decel` default, m/s²
/// (SUMO vehicle type defaults).
pub const DECEL_MPS2: f64 = 4.5;
/// Krauss's reaction time τ as SUMO uses it (`tau` default), seconds.
pub const TAU_S: f64 = 1.0;
/// The right-lane speed margin, 5 km/h, below which keep right and right speed gain stop.
pub const RIGHT_MARGIN_MPS: f64 = 5.0 / 3.6;

/// LC2013's four eagerness factors (SUMO vType `lcStrategic`, `lcCooperative`,
/// `lcSpeedGain`, `lcKeepRight`; all default 1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Lc2013Params {
    /// Eagerness for strategic changes: scales the look-ahead distance.
    pub strategic: f64,
    /// Willingness to change lane to let a neighbour in (0 to 1).
    pub cooperative: f64,
    /// Eagerness for changes that gain speed.
    pub speed_gain: f64,
    /// Eagerness to return to the right lane.
    pub keep_right: f64,
}

impl Default for Lc2013Params {
    fn default() -> Self {
        Self {
            strategic: 1.0,
            cooperative: 1.0,
            speed_gain: 1.0,
            keep_right: 1.0,
        }
    }
}

/// One vehicle's LC2013 memory.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Lc2013State {
    /// The accumulated speed-gain benefit of the left lane.
    pub gain_left: f64,
    /// The accumulated speed-gain benefit of the right lane.
    pub gain_right: f64,
    /// The keep-right accumulator (falls below zero).
    pub keep_right: f64,
    /// The look-ahead speed, m/s.
    pub look_ahead_speed: f64,
}

impl Lc2013State {
    /// After a lane change the accumulators restart.
    pub fn changed(&mut self) {
        self.gain_left = 0.0;
        self.gain_right = 0.0;
        self.keep_right = 0.0;
        self.look_ahead_speed = 0.0;
    }

    /// The look-ahead speed after one step at `speed`.
    pub fn track_speed(&mut self, speed: f64, dt: f64) {
        if speed > self.look_ahead_speed {
            self.look_ahead_speed = speed;
        } else {
            let m = 1.0 - (1.0 - LOOK_AHEAD_SPEED_MEMORY) * dt;
            self.look_ahead_speed = (m * self.look_ahead_speed + (1.0 - m) * speed).max(0.0);
        }
    }
}

/// What a side offers: its anticipated speed, how far it can be driven on the route,
/// and its leader.
#[derive(Debug, Clone, Copy)]
pub struct SideOffer {
    /// Anticipated safe speed on the lane, m/s.
    pub v_safe: f64,
    /// How far the lane can be followed on the route, m.
    pub dist_m: f64,
    /// The lane's leader alongside or ahead.
    pub leader: Option<LeaderView>,
}

/// Why LC2013 wants a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motive {
    /// A faster lane.
    SpeedGain,
    /// The keep-right rule.
    KeepRight,
}

impl Lc2013Params {
    /// The speed-gain threshold on the left.
    pub fn threshold_left(&self) -> f64 {
        if self.speed_gain <= 0.0 {
            f64::INFINITY
        } else {
            0.2 / self.speed_gain
        }
    }

    /// The speed-gain threshold on the right.
    pub fn threshold_right(&self) -> f64 {
        if self.speed_gain <= 0.0 {
            f64::INFINITY
        } else {
            0.2 / SPEED_GAIN_RIGHT / self.speed_gain
        }
    }

    /// The distance before the end of the lane at which a strategic change towards
    /// `side` becomes due, metres.
    pub fn look_ahead_m(&self, state: &Lc2013State, length_with_gap_m: f64, side: Side) -> f64 {
        let factor = match side {
            Side::Right => 1.0,
            Side::Left => LOOKAHEAD_LEFT,
        };
        state.look_ahead_speed * LOOK_FORWARD_S * self.strategic * factor + 2.0 * length_with_gap_m
    }

    /// The inconvenience of a cooperative change towards `side`, 0 to 1.
    pub fn inconvenience(&self, state: &Lc2013State, side: Side, v_this: f64, v_neigh: f64) -> f64 {
        let acc = match side {
            Side::Right => state.gain_left / self.threshold_right(),
            Side::Left => state.gain_right / self.threshold_left(),
        };
        let rel = if v_this == 0.0 {
            0.0
        } else {
            (v_this - v_neigh) / v_this.max(v_neigh)
        };
        rel.max(if acc.is_finite() { acc } else { 0.0 }).min(1.0)
    }

    /// One step of the speed-gain and keep-right accumulators, the right side first, and
    /// the change wanted if any.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &self,
        state: &mut Lc2013State,
        dt: f64,
        speed: f64,
        v_max: f64,
        speed_limit: f64,
        decel: f64,
        v_this: f64,
        right: Option<SideOffer>,
        left: Option<SideOffer>,
    ) -> Option<(Side, Motive)> {
        let keepable = |o: &SideOffer| o.dist_m / speed.max(0.1) > SPEED_GAIN_REMAIN_TIME_S;
        if let Some(r) = right {
            let road = RoadContext {
                speed,
                v_max,
                speed_limit,
                decel,
            };
            if let Some(m) = self.right_motive(state, dt, &road, v_this, &r, keepable(&r)) {
                return Some((Side::Right, m));
            }
        }
        if let Some(l) = left
            && self.left_wanted(state, dt, v_this, &l, keepable(&l))
        {
            return Some((Side::Left, Motive::SpeedGain));
        }
        None
    }

    /// The right side's step: its speed-gain benefit, then keep right, then speed gain.
    fn right_motive(
        &self,
        state: &mut Lc2013State,
        dt: f64,
        road: &RoadContext,
        v_this: f64,
        r: &SideOffer,
        keepable: bool,
    ) -> Option<Motive> {
        if v_this - RIGHT_MARGIN_MPS > r.v_safe {
            // The right lane is clearly slower: its benefit halves per second.
            state.gain_right *= 0.5_f64.powf(dt);
        } else {
            state.gain_right += dt * relative_gain(r.v_safe, v_this);
            if self.keep_right_due(state, dt, road, r) {
                return Some(Motive::KeepRight);
            }
        }
        (state.gain_right > self.threshold_right() && keepable).then_some(Motive::SpeedGain)
    }

    /// The keep-right accumulator's step, and whether it now asks for the change.
    fn keep_right_due(
        &self,
        state: &mut Lc2013State,
        dt: f64,
        road: &RoadContext,
        r: &SideOffer,
    ) -> bool {
        let threshold = self.threshold_right();
        if !threshold.is_finite() {
            return false;
        }
        let acceptance = road.acceptance_s();
        let horizon = full_speed_horizon_s(r, road.v_max, road.decel, acceptance);
        state.keep_right -= dt * threshold * (horizon / acceptance) / KEEP_RIGHT_TIME_S;
        state.keep_right * self.keep_right < -threshold
    }

    /// The left side's step: its speed-gain benefit, and whether a change is wanted.
    fn left_wanted(
        &self,
        state: &mut Lc2013State,
        dt: f64,
        v_this: f64,
        l: &SideOffer,
        keepable: bool,
    ) -> bool {
        let gain = relative_gain(l.v_safe, v_this);
        state.gain_left = if v_this > l.v_safe {
            state.gain_left * 0.5_f64.powf(dt)
        } else if v_this == l.v_safe {
            state.gain_left * 0.8_f64.powf(dt)
        } else {
            state.gain_left + dt * gain
        };
        state.gain_left > self.threshold_left() && gain > 1e-9 && keepable
    }
}

/// What the keep-right rule needs to know about the own vehicle and road.
struct RoadContext {
    /// Current speed, m/s.
    speed: f64,
    /// The vehicle's free speed on this road, m/s.
    v_max: f64,
    /// The lane's speed limit, m/s.
    speed_limit: f64,
    /// The deceleration safe speeds are judged at, m/s².
    decel: f64,
}

impl RoadContext {
    /// The acceptance time, seconds: `7 · roadSpeedFactor · max(1, v)`, where the road
    /// speed factor is the free speed over the limit.
    fn acceptance_s(&self) -> f64 {
        let road_speed_factor = if self.speed_limit > 0.0 {
            self.v_max / self.speed_limit
        } else {
            1.0
        };
        7.0 * road_speed_factor * self.speed.max(1.0)
    }
}

/// A side's relative speed gain over the own lane, `(v_side − v_own) / max(v_side, 10 m/s)`.
fn relative_gain(v_side: f64, v_own: f64) -> f64 {
    (v_side - v_own) / v_side.max(RELGAIN_NORMALIZATION_MIN_SPEED)
}

/// How long the vehicle could hold its free speed `v_max` on the offered lane, seconds, at
/// most `cap`: until it must start braking for the end of the lane it can use, or until
/// it closes to a secure gap behind a slower leader there.
fn full_speed_horizon_s(offer: &SideOffer, v_max: f64, decel: f64, cap: f64) -> f64 {
    let v = v_max.max(0.1);
    let b = decel.max(0.1);
    let braking_m = v * v / (2.0 * b);
    let mut room_m = (offer.dist_m - braking_m).max(0.0);
    let mut horizon = cap.min(room_m / v);
    if let Some(l) = offer.leader
        && l.speed_mps < v
    {
        let secure_m = (v * TAU_S + (v * v - l.speed_mps * l.speed_mps) / (2.0 * b)).max(0.0);
        room_m = room_m.min(l.gap_m - secure_m).max(0.0);
        horizon = horizon.min(room_m / (v - l.speed_mps));
    }
    horizon
}

/// Krauss's safe following speed behind a leader at `gap_m` doing `v_leader`, for a
/// driver decelerating at `decel` (Krauss 1998, eq. 2.7).
pub fn safe_follow_speed(gap_m: f64, v_leader: f64, decel: f64) -> f64 {
    let bt = decel * TAU_S;
    (-bt + (bt * bt + v_leader * v_leader + 2.0 * decel * gap_m.max(0.0)).sqrt()).max(0.0)
}

/// A lane's anticipated speed: the free speed, capped by the safe speed behind its leader.
pub fn lane_v_safe(v_max: f64, leader: Option<&LeaderView>, decel: f64, min_gap_m: f64) -> f64 {
    match leader {
        None => v_max,
        Some(l) => v_max.min(safe_follow_speed(l.gap_m - min_gap_m, l.speed_mps, decel)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_faster_left_lane_is_wanted_after_the_accumulator_fills() {
        let p = Lc2013Params::default();
        let mut s = Lc2013State::default();
        let left = SideOffer {
            v_safe: 14.0,
            dist_m: 2000.0,
            leader: None,
        };
        let mut wanted = None;
        let mut t = 0.0;
        while wanted.is_none() && t < 10.0 {
            wanted = p.step(&mut s, 0.1, 8.0, 14.0, 14.0, 4.5, 8.0, None, Some(left));
            t += 0.1;
        }
        assert_eq!(wanted, Some((Side::Left, Motive::SpeedGain)));
        // 0.2 / (6/14) per second: about 0.47 s.
        assert!(t < 1.0, "{t}");
    }

    #[test]
    fn an_empty_right_lane_draws_a_free_driver_back() {
        let p = Lc2013Params::default();
        let mut s = Lc2013State::default();
        let right = SideOffer {
            v_safe: 14.0,
            dist_m: 3000.0,
            leader: None,
        };
        let mut wanted = None;
        let mut t = 0.0;
        while wanted.is_none() && t < 120.0 {
            wanted = p.step(&mut s, 0.1, 14.0, 14.0, 14.0, 4.5, 14.0, Some(right), None);
            t += 0.1;
        }
        assert_eq!(wanted.map(|w| w.0), Some(Side::Right));
        let mut off = Lc2013Params::default();
        off.keep_right = 0.0;
        let mut s = Lc2013State::default();
        for _ in 0..1200 {
            assert_eq!(
                off.step(&mut s, 0.1, 14.0, 14.0, 14.0, 4.5, 14.0, Some(right), None),
                None
            );
        }
    }
}
