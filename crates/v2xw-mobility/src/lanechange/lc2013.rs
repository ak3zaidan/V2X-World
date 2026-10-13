//! SUMO LC2013's discretionary motivations, on top of the native strategic layer.
//!
//! Ported from SUMO `src/microsim/lcmodels/MSLCM_LC2013.cpp` (`_wantsChange`), Erdmann
//! (2015), "SUMO's lane-changing model", Lecture Notes in Control and Information Sciences
//! 13. What is carried over:
//!
//! * **Strategic look-ahead.** A change the route needs becomes due at
//!   `laDist = lookAheadSpeed · 10 s · lcStrategic · (1 right, 2 left) + 2 · (length + minGap)`
//!   before the end of the lane (`LOOK_FORWARD`, `lcLookaheadLeft` = 2). Further out the
//!   vehicle moves to its best lane only when that lane is no slower (`changeToBest`).
//!   `lookAheadSpeed` rises at once with the speed and decays at `LOOK_AHEAD_SPEED_MEMORY`
//!   = 0.9 per second.
//! * **Speed gain.** Each side keeps an accumulator. Every step it adds
//!   `dt · (vSafe_neigh − vSafe_this) / max(vSafe_neigh, 10 m/s)`. It halves per second
//!   when the own lane is faster; on the right, this needs a margin of more than 5 km/h.
//!   On the left it decays at 0.8 per second when the two are equal. A change is wanted
//!   once the accumulator passes `0.2 / lcSpeedGain` (left) or
//!   `0.2 / lcSpeedGainRight / lcSpeedGain` (right, `lcSpeedGainRight` = 0.1), and the
//!   target lane is usable for `lcSpeedGainRemainTime` = 20 s.
//! * **Keep right.** While the right lane is not more than 5 km/h slower, a second
//!   accumulator falls by `dt · thrRight · (fullSpeedDrivingSeconds / acceptanceTime) / 5 s`
//!   (`KEEP_RIGHT_TIME`). It asks for the change once `acc · lcKeepRight < −thrRight`.
//!   Here `acceptanceTime = 7 · roadSpeedFactor · max(1, v)`.
//! * **Cooperation.** A vehicle that blocks a neighbour's urgent strategic change, as that
//!   neighbour's target-lane follower, moves out of the way to its other side. It does so
//!   unless the inconvenience exceeds `lcCooperative`. The inconvenience is the larger of
//!   the relative speed loss and the opposite speed-gain accumulator over its threshold,
//!   capped at 1.
//!
//! Both lanes' `vSafe` are Krauss's safe following speed behind the lane's leader
//! (Krauss 1998, eq. 2.7; SUMO `maximumSafeFollowSpeed`, τ = 1 s), capped at the
//! vehicle's free speed.
//!
//! Not ported: the lane-change follower's courtesy braking (`informFollower`, tried and
//! dropped; see `strategic_fall_back` in the engine), sublane, opposite-direction
//! overtaking and roundabout bonus.

use serde::{Deserialize, Serialize};

use crate::views::{LeaderView, Side};

/// SUMO `LOOK_FORWARD`, seconds.
pub const LOOK_FORWARD_S: f64 = 10.0;
/// SUMO `lcLookaheadLeft` default.
pub const LOOKAHEAD_LEFT: f64 = 2.0;
/// SUMO `LOOK_AHEAD_SPEED_MEMORY`.
pub const LOOK_AHEAD_SPEED_MEMORY: f64 = 0.9;
/// SUMO `lcSpeedGainRight` default.
pub const SPEED_GAIN_RIGHT: f64 = 0.1;
/// SUMO `lcSpeedGainRemainTime` default, seconds.
pub const SPEED_GAIN_REMAIN_TIME_S: f64 = 20.0;
/// SUMO `RELGAIN_NORMALIZATION_MIN_SPEED`, m/s.
pub const RELGAIN_NORMALIZATION_MIN_SPEED: f64 = 10.0;
/// SUMO `KEEP_RIGHT_TIME`, seconds.
pub const KEEP_RIGHT_TIME_S: f64 = 5.0;
/// The deceleration `vSafe` is judged at: SUMO's passenger-car `decel` default, m/s²
/// (SUMO vehicle type defaults, `SUMOVTypeParameter`).
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
    /// `mySpeedGainProbabilityLeft`.
    pub gain_left: f64,
    /// `mySpeedGainProbabilityRight`.
    pub gain_right: f64,
    /// `myKeepRightProbability` (falls below zero).
    pub keep_right: f64,
    /// `myLookAheadSpeed`, m/s.
    pub look_ahead_speed: f64,
}

impl Lc2013State {
    /// After a lane change (SUMO `changed()`): the accumulators restart.
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
    /// `LCA_SPEEDGAIN`.
    SpeedGain,
    /// `LCA_KEEPRIGHT`.
    KeepRight,
}

impl Lc2013Params {
    /// The speed-gain threshold on the left (`myChangeProbThresholdLeft`).
    pub fn threshold_left(&self) -> f64 {
        if self.speed_gain <= 0.0 {
            f64::INFINITY
        } else {
            0.2 / self.speed_gain
        }
    }

    /// The speed-gain threshold on the right (`myChangeProbThresholdRight`).
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

    /// The inconvenience of a cooperative change towards `side`, 0 to 1
    /// (MSLCM_LC2013.cpp, `inconvenience`).
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

    /// One step of the speed-gain and keep-right accumulators, right side first as SUMO's
    /// lane changer asks, and the change wanted if any.
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
        let remain_ok = |o: &SideOffer| o.dist_m / speed.max(0.1) > SPEED_GAIN_REMAIN_TIME_S;
        if let Some(r) = right {
            let rel = (r.v_safe - v_this) / r.v_safe.max(RELGAIN_NORMALIZATION_MIN_SPEED);
            if v_this - RIGHT_MARGIN_MPS > r.v_safe {
                state.gain_right *= 0.5_f64.powf(dt);
            } else {
                state.gain_right += dt * rel;
                let road_factor = if speed_limit > 0.0 { v_max / speed_limit } else { 1.0 };
                let acceptance = 7.0 * road_factor * speed.max(1.0);
                let vm = v_max.max(0.1);
                let mut gap = (r.dist_m - vm * vm / (2.0 * decel.max(0.1))).max(0.0);
                let mut secs = acceptance.min(gap / vm);
                if let Some(l) = r.leader
                    && l.speed_mps < vm
                {
                    let secure = (vm * TAU_S + (vm * vm - l.speed_mps * l.speed_mps) / (2.0 * decel.max(0.1))).max(0.0);
                    gap = gap.min(l.gap_m - secure).max(0.0);
                    secs = secs.min(gap / (vm - l.speed_mps));
                }
                let thr = self.threshold_right();
                if thr.is_finite() {
                    state.keep_right -= dt * thr * (secs / acceptance) / KEEP_RIGHT_TIME_S;
                    if state.keep_right * self.keep_right < -thr {
                        return Some((Side::Right, Motive::KeepRight));
                    }
                }
            }
            if state.gain_right > self.threshold_right() && remain_ok(&r) {
                return Some((Side::Right, Motive::SpeedGain));
            }
        }
        if let Some(l) = left {
            let rel = (l.v_safe - v_this) / l.v_safe.max(RELGAIN_NORMALIZATION_MIN_SPEED);
            if v_this > l.v_safe {
                state.gain_left *= 0.5_f64.powf(dt);
            } else if v_this == l.v_safe {
                state.gain_left *= 0.8_f64.powf(dt);
            } else {
                state.gain_left += dt * rel;
            }
            if state.gain_left > self.threshold_left() && rel > 1e-9 && remain_ok(&l) {
                return Some((Side::Left, Motive::SpeedGain));
            }
        }
        None
    }
}

/// Krauss's safe following speed behind a leader at `gap_m` doing `v_leader`, for a
/// driver decelerating at `decel` (Krauss 1998; SUMO `maximumSafeFollowSpeed`).
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
        let left = SideOffer { v_safe: 14.0, dist_m: 2000.0, leader: None };
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
        let right = SideOffer { v_safe: 14.0, dist_m: 3000.0, leader: None };
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
            assert_eq!(off.step(&mut s, 0.1, 14.0, 14.0, 14.0, 4.5, 14.0, Some(right), None), None);
        }
    }
}
