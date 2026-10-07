//! `mobility/lateral/sublane-sl2015` — a sub-lane lateral model after SUMO's SL2015.
//!
//! # The model
//!
//! Semrau & Erdmann, "Simulation framework for testing ADAS in Chinese traffic
//! situations", SUMO 2016 – Traffic, Mobility, and Logistics (DLR), pp. 103–115, describe
//! SUMO's sub-lane model: a lane is cut into sub-lanes of a fixed lateral resolution, each
//! sub-lane gets an *expected speed* from the leaders that occupy it, and a vehicle steers
//! its lateral position towards the band of sub-lanes, as wide as its body, with the
//! highest expected speed, at a bounded lateral speed. The expected speed of a sub-lane is
//! read from `MSLCM_SL2015::updateExpectedSublaneSpeeds` and `forecastAverageSpeed`
//! (EPL-2.0, read only, never vendored; 2026-10-07):
//!
//! ```text
//! v_exp(lane band) = min(v_max, min over leaders overlapping the band of v_safe')
//! v_safe  = Krauß safe speed behind the leader (the follower's b and τ)
//! v_safe' = (t_c·v_safe + (2L − t_c)·v_l)/(2L)   when the gap closes within L = 5 s
//!           (t_c = gap/(v_max − v_l), L = lcSpeedGainLookahead)
//! leader accelerating harder than half the follower's a_max: v_safe = v_max
//! ```
//!
//! Here the bands are candidate centre positions every `resolution_m` across the lane,
//! plus the vehicle's preferred position, and a candidate is **infeasible** while a
//! vehicle alongside (longitudinally overlapping, within one body length) would be nearer
//! than the pair's lawful clearance ([`super::SublaneFilter`]). Among feasible candidates
//! the vehicle takes the highest expected speed; a candidate must beat the preferred
//! position by `speed_gain_threshold_mps` to draw the vehicle off it (SUMO's
//! `lcSpeedGainRight` default 0.1, applied symmetrically); ties go to the candidate nearest
//! the preferred position, then nearest the current one, then the rightmost — a total
//! order, so the decision is a pure function of its inputs.
//!
//! The lateral speed is SUMO's vType `maxSpeedLat` default, 1.0 m/s.
//!
//! # What it leaves out
//!
//! SL2015's expected-speed memory (`SPEEDGAIN_MEMORY_FACTOR`), its strategic and
//! cooperative sub-lane motivations, `lcPushy` and `lcImpatience`, and leaders on the next
//! lane. Lane changes stay MOBIL's ([`crate::lanechange`]): this model moves vehicles
//! **within** their lane only.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_core::model::Model;

use super::{LateralDecision, LateralModel, LateralNeighbor, SublaneFilter};
use crate::carfollowing::common::krauss_safe_speed;
use crate::views::{LaneView, VehicleView};

/// The model id.
pub const MODEL_ID: &str = "mobility/lateral/sublane-sl2015";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// The model's constants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sl2015Params {
    /// The lateral spacing of candidate positions, metres (SUMO's `--lateral-resolution`;
    /// its documentation suggests values around a bicycle's width).
    pub resolution_m: f64,
    /// The lateral speed, m/s (SUMO vType `maxSpeedLat` default).
    pub max_speed_lat_mps: f64,
    /// How far ahead the expected speed anticipates, seconds (`lcSpeedGainLookahead`).
    pub speed_gain_lookahead_s: f64,
    /// The expected-speed gain needed to leave the preferred position, m/s
    /// (`lcSpeedGainRight`).
    pub speed_gain_threshold_mps: f64,
    /// How far ahead same-lane vehicles are considered, metres.
    pub lookahead_m: f64,
    /// The pair rule: who shares a lane with whom, and at what clearance.
    pub filter: SublaneFilter,
}

impl Default for Sl2015Params {
    fn default() -> Self {
        Self {
            resolution_m: 0.4,
            max_speed_lat_mps: 1.0,
            speed_gain_lookahead_s: 5.0,
            speed_gain_threshold_mps: 0.1,
            lookahead_m: 100.0,
            filter: SublaneFilter::default(),
        }
    }
}

/// The sub-lane model.
#[derive(Debug, Clone)]
pub struct Sl2015 {
    params: Sl2015Params,
    card: ModelCard,
}

impl Sl2015 {
    /// The model with `params`.
    pub fn new(params: Sl2015Params) -> Self {
        Self {
            card: card(&params),
            params,
        }
    }

    /// The parameters in force.
    pub fn params(&self) -> &Sl2015Params {
        &self.params
    }

    /// The expected speed at lateral centre `c`.
    fn expected_speed(
        &self,
        ego: &VehicleView,
        v_max: f64,
        c: f64,
        neighbours: &[LateralNeighbor],
    ) -> f64 {
        let at = VehicleView {
            lateral_m: c,
            ..*ego
        };
        let mut v_exp = v_max;
        for n in neighbours {
            if n.gap_m <= 0.0 || n.gap_m > self.params.lookahead_m {
                continue;
            }
            if !self.params.filter.is_leader(&at, &n.view) {
                continue;
            }
            let v_l = n.view.speed_mps;
            let v_safe = if n.view.accel_mps2 > 0.5 * ego.driver.max_accel_mps2 {
                v_max
            } else {
                let raw = krauss_safe_speed(
                    ego.speed_mps,
                    v_l,
                    n.gap_m - ego.driver.min_gap_m,
                    ego.driver.comfort_decel_mps2,
                    ego.driver.time_headway_s,
                );
                let look = self.params.speed_gain_lookahead_s;
                let dv = v_max - v_l;
                if dv > 0.0 && look > 0.0 && n.gap_m / dv < look {
                    let tc = (n.gap_m / dv).max(0.0);
                    (tc * raw + (2.0 * look - tc) * v_l) / (2.0 * look)
                } else {
                    raw
                }
            };
            v_exp = v_exp.min(v_safe);
        }
        v_exp
    }

    /// Whether centre `c` keeps the lawful clearance to every vehicle alongside.
    fn feasible(&self, ego: &VehicleView, c: f64, neighbours: &[LateralNeighbor]) -> bool {
        let at = VehicleView {
            lateral_m: c,
            ..*ego
        };
        neighbours.iter().all(|n| {
            let alongside = n.gap_m.abs() < 0.5 && n.gap_m > -(n.view.dims.length_m + ego.dims.length_m);
            if !alongside {
                return true;
            }
            match self.params.filter.pair_gap(ego.class, n.view.class) {
                None => true,
                Some(g) => SublaneFilter::clearance(&at, &n.view) >= g,
            }
        })
    }
}

impl Model for Sl2015 {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl LateralModel for Sl2015 {
    fn decide(
        &self,
        ego: &VehicleView,
        desired_speed_mps: f64,
        lane: &LaneView,
        neighbours: &[LateralNeighbor],
        preferred_m: f64,
    ) -> LateralDecision {
        let half = 0.5 * (lane.width_m - ego.dims.width_m).max(0.0);
        let clamp = |x: f64| x.clamp(-half, half);
        let preferred = clamp(preferred_m);
        let current = clamp(ego.lateral_m);
        let v_max = desired_speed_mps.min(lane.speed_limit_mps);
        let res = self.params.resolution_m.max(0.05);
        let mut candidates = vec![preferred, current];
        let n = (2.0 * half / res).floor() as i64;
        for k in 0..=n {
            candidates.push(clamp(-half + k as f64 * res));
        }
        candidates.push(half);
        let pref_speed = if self.feasible(ego, preferred, neighbours) {
            Some(self.expected_speed(ego, v_max, preferred, neighbours))
        } else {
            None
        };
        let mut best: Option<(f64, f64)> = None; // (speed, centre)
        for &c in &candidates {
            if !self.feasible(ego, c, neighbours) {
                continue;
            }
            let v = self.expected_speed(ego, v_max, c, neighbours);
            let better = match best {
                None => true,
                Some((bv, bc)) => {
                    if (v - bv).abs() > 1e-9 {
                        v > bv
                    } else {
                        let key = |x: f64| ((x - preferred).abs(), (x - current).abs(), x);
                        let (a, b) = (key(c), key(bc));
                        a.0.total_cmp(&b.0)
                            .then(a.1.total_cmp(&b.1))
                            .then(a.2.total_cmp(&b.2))
                            .is_lt()
                    }
                }
            };
            if better {
                best = Some((v, c));
            }
        }
        let target = match (best, pref_speed) {
            (Some((v, c)), Some(vp)) if v - vp > self.params.speed_gain_threshold_mps => c,
            (_, Some(_)) => preferred,
            (Some((_, c)), None) => c,
            (None, None) => current,
        };
        LateralDecision {
            target_m: target,
            speed_mps: self.params.max_speed_lat_mps,
        }
    }

    fn filter(&self) -> SublaneFilter {
        self.params.filter
    }
}

/// The model card.
pub fn card(p: &Sl2015Params) -> ModelCard {
    let paper = Source::new(
        SourceKind::Paper,
        "Semrau & Erdmann 2016, Simulation framework for testing ADAS in Chinese traffic \
         situations, SUMO 2016 - Traffic, Mobility, and Logistics, pp. 103-115 (the SL2015 \
         sub-lane model)",
    );
    let sumo = Source {
        kind: SourceKind::Code,
        reference: "SUMO src/microsim/lcmodels/MSLCM_SL2015.cpp (EPL-2.0, read only): \
                    updateExpectedSublaneSpeeds, forecastAverageSpeed, lcSpeedGainLookahead \
                    = 5 s, lcSpeedGainRight = 0.1; vType defaults maxSpeedLat = 1.0 m/s, \
                    minGapLat = 0.6 m"
            .to_string(),
        accessed: Some("2026-10-07".to_string()),
        note: None,
    };
    let three_feet = Source::new(
        SourceKind::Standard,
        "California Vehicle Code §21760 (Three Feet for Safety Act, 2014): a motorist passes \
         a cyclist in the same direction at no less than three feet; most US states have \
         the same rule",
    );
    let uvc = Source::new(
        SourceKind::Standard,
        "Uniform Vehicle Code §11-1205: a cyclist rides as near the right side of the roadway \
         as practicable, sharing the lane",
    );
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        MODEL_VERSION,
        "Where across its lane a vehicle drives: the lateral position with the highest \
         expected speed among those that keep a lawful clearance to vehicles alongside, \
         reached at a bounded lateral speed. With it, a car passes a cyclist within a wide \
         lane, cyclists ride side by side with traffic, and motorcycles filter where the \
         jurisdiction allows it.",
    );
    card.tier = vec![Tier::Medium];
    card.equations = vec![
        Equation::new(
            "expected speed",
            "v_exp(c) = min(v_max, min over leaders overlapping c of v_safe'); v_safe' = \
             (t_c·v_safe + (2L − t_c)·v_l)/(2L) when t_c = gap/(v_max − v_l) < L",
        ),
        Equation::new(
            "choice",
            "argmax over feasible c of v_exp(c); leave the preferred position only for a gain \
             above the threshold; ties: nearest preferred, nearest current, rightmost",
        ),
        Equation::new("motion", "|dc/dt| ≤ maxSpeedLat"),
    ];
    card.parameters = vec![
        Parameter::new("resolution", "m", serde_json::json!(p.resolution_m), sumo.clone()),
        Parameter::new(
            "max_speed_lat",
            "m/s",
            serde_json::json!(p.max_speed_lat_mps),
            sumo.clone(),
        ),
        Parameter::new(
            "speed_gain_lookahead",
            "s",
            serde_json::json!(p.speed_gain_lookahead_s),
            sumo.clone(),
        ),
        Parameter::new(
            "speed_gain_threshold",
            "m/s",
            serde_json::json!(p.speed_gain_threshold_mps),
            sumo.clone(),
        ),
        Parameter::new(
            "lookahead",
            "m",
            serde_json::json!(p.lookahead_m),
            Source::new(
                SourceKind::Code,
                "the neighbour window; at least SL2015's 5 s lookahead at urban speeds",
            ),
        ),
        Parameter::new(
            "min_gap_lat",
            "m",
            serde_json::json!(p.filter.min_gap_lat_m),
            sumo.clone(),
        ),
        Parameter::new(
            "bicycle_passing_gap",
            "m",
            serde_json::json!(p.filter.bicycle_passing_gap_m),
            three_feet.clone(),
        ),
        Parameter::new(
            "motorcycles_share",
            "-",
            serde_json::json!(p.filter.motorcycles_share),
            Source::new(
                SourceKind::Standard,
                "the jurisdiction's filtering rule (crate::rules): false unless both the \
                 scenario and the rules allow it",
            ),
        ),
        Parameter::new(
            "bicycles_share",
            "-",
            serde_json::json!(p.filter.bicycles_share),
            uvc.clone(),
        ),
    ];
    card.limitations = vec![
        "No expected-speed memory, no strategic or cooperative sub-lane motivation, no \
         lcPushy/lcImpatience, no leaders on the next lane: SL2015's lateral choice reduced \
         to its speed-gain core."
            .to_string(),
        "Moves vehicles within their lane only; lane changes remain MOBIL's.".to_string(),
    ];
    card.sources = vec![paper, sumo, three_feet, uvc];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "lateral::sl2015::tests::a_car_moves_left_to_pass_a_cyclist_on_a_wide_lane".to_string(),
            "lateral::sl2015::tests::a_car_stays_behind_a_cyclist_on_a_narrow_lane".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carfollowing::testkit::{lane, vehicle};
    use crate::classes::VehicleClass;
    use crate::views::DriverProfile;
    use v2xw_core::geom::Dims;

    fn driver() -> DriverProfile {
        DriverProfile {
            desired_speed_mps: 13.9,
            max_accel_mps2: 1.7,
            comfort_decel_mps2: 2.0,
            time_headway_s: 1.0,
            min_gap_m: 2.0,
        }
    }

    fn cyclist(lat: f64, gap: f64) -> LateralNeighbor {
        let mut v = vehicle(2, 4.5, driver());
        v.class = VehicleClass::Bicycle;
        v.dims = Dims::new(1.6, 0.65, 1.7);
        v.lateral_m = lat;
        LateralNeighbor { view: v, gap_m: gap }
    }

    #[test]
    fn a_car_moves_left_to_pass_a_cyclist_on_a_wide_lane() {
        let m = Sl2015::new(Sl2015Params::default());
        let mut l = lane(13.9);
        l.width_m = 4.6;
        let car = vehicle(1, 10.0, driver());
        let d = m.decide(&car, 13.9, &l, &[cyclist(-1.9, 20.0)], 0.0);
        // Half free width (4.6 − 1.8)/2 = 1.4: the car shifts left so the clearance to
        // the cyclist reaches three feet.
        assert!(d.target_m > 0.0, "{d:?}");
        let at = VehicleView { lateral_m: d.target_m, ..car };
        assert!(!m.filter().is_leader(&at, &cyclist(-1.9, 20.0).view), "{d:?}");
    }

    #[test]
    fn a_car_stays_behind_a_cyclist_on_a_narrow_lane() {
        let m = Sl2015::new(Sl2015Params::default());
        let l = lane(13.9); // 3.5 m
        let car = vehicle(1, 10.0, driver());
        let d = m.decide(&car, 13.9, &l, &[cyclist(-1.4, 20.0)], 0.0);
        assert_eq!(d.target_m, 0.0, "{d:?}");
    }
}
