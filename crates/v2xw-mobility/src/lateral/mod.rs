//! Lateral models: where across its lane a vehicle drives, and which vehicles in its own
//! lane it must follow.
//!
//! Without a lateral model every vehicle drives its lane's centreline (two-wheelers their
//! fixed tyre track) and follows whatever is ahead in the lane, so a car stays behind a
//! cyclist on a 4.5 m lane and a motorcycle queues behind a car. A [`LateralModel`] gives
//! each vehicle a continuous position across the lane and, through [`SublaneFilter`],
//! makes the leader search ignore a vehicle in the same lane whose body it can pass with a
//! lawful lateral clearance — SUMO's sub-lane model (SL2015) in essence.
//!
//! # The pieces
//!
//! * [`SublaneFilter`] — which same-lane vehicle is a leader: the pair rule (who may share
//!   a lane with whom, and at what clearance), applied by
//!   [`crate::snapshot::ActorSnapshot::leader_on_route`] when
//!   [`crate::snapshot::NeighborOptions::sublane`] is set.
//! * [`LateralModel`] — the plug-in trait: given the ego and its same-lane neighbours, the
//!   lateral position it steers for and how fast.
//! * [`sl2015::Sl2015`] — the one implementation, after Semrau & Erdmann 2016.
//!
//! # How to add another lateral model
//!
//! Implement [`LateralModel`] (with a [`v2xw_core::model::Model`] card), add a variant to
//! [`LateralKind`] and a line to [`LateralSpec::build`]. The engine calls the trait only.

pub mod sl2015;

use serde::{Deserialize, Serialize};
use v2xw_core::model::Model;

use crate::classes::VehicleClass;
use crate::views::{LaneView, VehicleView};

pub use sl2015::{Sl2015, Sl2015Params};

/// Who may share a lane with whom, and at what lateral clearance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SublaneFilter {
    /// The lateral clearance two vehicles keep side by side in one lane, metres (SUMO's
    /// vType `minGapLat` default, 0.6 m).
    pub min_gap_lat_m: f64,
    /// The clearance a motor vehicle keeps passing a cyclist in the same lane, metres:
    /// three feet, the passing distance of most US states' safe-passing laws (for example
    /// California Vehicle Code §21760, the "Three Feet for Safety Act", 2014).
    pub bicycle_passing_gap_m: f64,
    /// Whether motorcycles and mopeds may share a lane with a car — filtering, lawful only
    /// where the jurisdiction allows it ([`crate::rules::TrafficRules::motorcycle_filtering`]).
    pub motorcycles_share: bool,
    /// Whether cyclists may share a lane with motor traffic (UVC §11-1205: ride as near
    /// the right side as practicable — sharing is the rule wherever a cyclist may use the
    /// lane at all).
    pub bicycles_share: bool,
}

impl Default for SublaneFilter {
    fn default() -> Self {
        Self {
            min_gap_lat_m: 0.6,
            bicycle_passing_gap_m: 0.914_4,
            motorcycles_share: false,
            bicycles_share: true,
        }
    }
}

/// Below this speed a motor vehicle counts as standing in a queue for filtering, m/s:
/// 5 mi/h, the HCM's queued-vehicle threshold (HCM 7th ed. ch. 31), the same one the
/// headway metrics use.
pub const FILTER_QUEUED_MPS: f64 = 2.235;

/// The lateral clearance a filtering motorcycle keeps passing a queued vehicle, metres.
/// **A choice**: no measured filtering clearance was read. SUMO's moving-traffic
/// `minGapLat` (0.6 m) cannot be met by a 0.9 m motorcycle beside a 1.8 m car in a
/// 3.5 m lane even on the lane line, so with it filtering never happened.
pub const FILTER_STANDING_GAP_M: f64 = 0.3;

/// The narrow classes that may share a lane at all.
fn narrow(class: VehicleClass) -> bool {
    matches!(
        class,
        VehicleClass::Bicycle
            | VehicleClass::Motorcycle
            | VehicleClass::Moped
            | VehicleClass::Scooter
    )
}

impl SublaneFilter {
    /// The clearance `a` and `b` must keep side by side in one lane, or `None` when the two
    /// may not share a lane at all (then each is the other's leader whatever the offsets).
    pub fn pair_gap(&self, a: VehicleClass, b: VehicleClass) -> Option<f64> {
        let share = |c: VehicleClass| match c {
            VehicleClass::Bicycle | VehicleClass::Scooter => self.bicycles_share,
            VehicleClass::Motorcycle | VehicleClass::Moped => self.motorcycles_share,
            _ => false,
        };
        let (na, nb) = (narrow(a), narrow(b));
        if !na && !nb {
            return None;
        }
        // At least one is narrow; the narrow one(s) must be allowed to share.
        if (na && !share(a)) || (nb && !share(b)) {
            return None;
        }
        let bike = |c: VehicleClass| matches!(c, VehicleClass::Bicycle | VehicleClass::Scooter);
        if (bike(a) && !nb) || (bike(b) && !na) {
            Some(self.bicycle_passing_gap_m)
        } else {
            Some(self.min_gap_lat_m)
        }
    }

    /// [`Self::pair_gap`] for two vehicles as they are now: a filtering motorcycle or
    /// moped passes a motor vehicle standing in a queue ([`FILTER_QUEUED_MPS`]) at
    /// [`FILTER_STANDING_GAP_M`], not at the moving-traffic `min_gap_lat_m`. Filtering is
    /// lawful only between stopped or slow vehicles where it is lawful at all (CVC
    /// §21658.1; Utah Code §41-6a-704.5, at most 15 mph, others stopped).
    pub fn pair_gap_of(&self, a: &VehicleView, b: &VehicleView) -> Option<f64> {
        let gap = self.pair_gap(a.class, b.class)?;
        let moto = |c: VehicleClass| matches!(c, VehicleClass::Motorcycle | VehicleClass::Moped);
        let filtering_past = |m: &VehicleView, other: &VehicleView| {
            moto(m.class) && !narrow(other.class) && other.speed_mps < FILTER_QUEUED_MPS
        };
        if self.motorcycles_share && (filtering_past(a, b) || filtering_past(b, a)) {
            Some(gap.min(FILTER_STANDING_GAP_M))
        } else {
            Some(gap)
        }
    }

    /// How far across its lane a vehicle of `class` may hold its centre, metres either
    /// side of the centreline, in a lane `lane_width_m` wide: a filtering motorcycle rides
    /// up to the lane line, the space between two queues (lane splitting); everyone else
    /// keeps its body inside the lane.
    pub fn half_range_m(&self, class: VehicleClass, width_m: f64, lane_width_m: f64) -> f64 {
        if self.motorcycles_share && matches!(class, VehicleClass::Motorcycle | VehicleClass::Moped)
        {
            0.5 * lane_width_m
        } else {
            0.5 * (lane_width_m - width_m).max(0.0)
        }
    }

    /// The lateral clearance between two bodies at their current offsets, metres
    /// (negative when they overlap).
    pub fn clearance(a: &VehicleView, b: &VehicleView) -> f64 {
        (a.lateral_m - b.lateral_m).abs() - 0.5 * (a.dims.width_m + b.dims.width_m)
    }

    /// Whether `cand`, ahead of `ego` in its lane, is `ego`'s leader: always when the two
    /// may not share; otherwise only when they are closer laterally than their clearance.
    pub fn is_leader(&self, ego: &VehicleView, cand: &VehicleView) -> bool {
        match self.pair_gap_of(ego, cand) {
            None => true,
            Some(gap) => Self::clearance(ego, cand) < gap,
        }
    }
}

/// The lateral speed a driver pulls over at, m/s: SUMO's vType `maxSpeedLat` default
/// (1.0 m/s), the same figure [`Sl2015Params::max_speed_lat_mps`] defaults to.
pub const PULL_OVER_SPEED_LAT_MPS: f64 = 1.0;

/// The offset from the lane centreline at which a body `width_m` wide stands against the
/// lane's left (`left`) or right edge in a lane `lane_width_m` wide, metres, positive
/// left: the body's outer side on the lane line, never past it, so no wheel leaves the
/// carriageway (WP-20's pull-over for an emergency vehicle). A body as wide as the lane
/// or wider stays on the centreline.
pub fn pull_over_offset(left: bool, width_m: f64, lane_width_m: f64) -> f64 {
    let half = 0.5 * (lane_width_m - width_m).max(0.0);
    if left { half } else { -half }
}

/// A same-lane neighbour as a lateral model sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LateralNeighbor {
    /// Its view.
    pub view: VehicleView,
    /// Longitudinal net gap, metres: positive ahead (its rear minus the ego's front),
    /// negative behind (its front minus the ego's rear, negated), zero when alongside.
    pub gap_m: f64,
}

/// What a lateral model decides for one step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LateralDecision {
    /// The offset from the lane centreline to steer for, metres, positive left.
    pub target_m: f64,
    /// The lateral speed to steer at, m/s (positive magnitude).
    pub speed_mps: f64,
}

/// A lateral (sub-lane) model.
pub trait LateralModel: Model + Send + Sync {
    /// Where across `lane` the ego steers for, given its same-lane neighbours (sorted by
    /// actor id) and its desired speed.
    fn decide(
        &self,
        ego: &VehicleView,
        desired_speed_mps: f64,
        lane: &LaneView,
        neighbours: &[LateralNeighbor],
        preferred_m: f64,
    ) -> LateralDecision;

    /// The pair rule this model drives with.
    fn filter(&self) -> SublaneFilter;
}

/// Asks `model` where `ego` steers across its lane, collecting its same-lane neighbours
/// from the frozen snapshot: everything on the ego's lane from one lookahead behind to one
/// lookahead ahead, with the longitudinal net gap signed as [`LateralNeighbor::gap_m`]
/// says, sorted by actor id.
pub fn decide_in_lane(
    model: &dyn LateralModel,
    snapshot: &crate::snapshot::ActorSnapshot,
    ego: &VehicleView,
    desired_speed_mps: f64,
    lane: &LaneView,
    preferred_m: f64,
) -> LateralDecision {
    const WINDOW_M: f64 = 100.0;
    let mut neighbours: Vec<LateralNeighbor> = snapshot
        .on_lane(ego.lane)
        .iter()
        .filter(|(s, a)| *a != ego.actor && (*s - ego.s_m).abs() <= WINDOW_M)
        .filter_map(|(s_front, a)| {
            let view = *snapshot.view(*a)?;
            let rear = s_front - view.dims.length_m;
            let gap_m = if rear > ego.s_m {
                rear - ego.s_m
            } else if *s_front < ego.rear_s_m() {
                -(ego.rear_s_m() - s_front)
            } else {
                0.0
            };
            Some(LateralNeighbor { view, gap_m })
        })
        .collect();
    neighbours.sort_by_key(|n| n.view.actor);
    neighbours.dedup_by_key(|n| n.view.actor);
    model.decide(ego, desired_speed_mps, lane, &neighbours, preferred_m)
}

/// Which lateral model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LateralKind {
    /// SUMO's sub-lane model, after Semrau & Erdmann 2016.
    #[default]
    Sl2015,
}

/// The scenario's `actors.vehicles.driving.lateral` block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LateralSpec {
    /// Which model.
    pub model: LateralKind,
    /// Its parameters.
    pub params: Sl2015Params,
}

impl LateralSpec {
    /// Builds the model, honouring the jurisdiction's filtering rule: motorcycles share
    /// lanes only if the scenario allows it **and** the rules do.
    pub fn build(&self, rules: &crate::rules::TrafficRules) -> std::sync::Arc<dyn LateralModel> {
        let mut p = self.params;
        if rules.motorcycle_filtering == crate::rules::Filtering::Prohibited {
            p.filter.motorcycles_share = false;
        }
        match self.model {
            LateralKind::Sl2015 => std::sync::Arc::new(Sl2015::new(p)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_over_offset_is_clamped_to_the_edge() {
        // A 1.8 m car in a 3.5 m lane: its right side on the right lane line.
        let r = pull_over_offset(false, 1.8, 3.5);
        assert!((r + 0.85).abs() < 1e-12, "{r}");
        assert!((pull_over_offset(true, 1.8, 3.5) - 0.85).abs() < 1e-12);
        // The body never crosses the line, however narrow the lane.
        for (w, lane) in [(2.55, 3.0), (2.16, 2.5), (0.9, 3.5), (2.6, 2.6), (2.6, 2.4)] {
            for left in [false, true] {
                let d = pull_over_offset(left, w, lane);
                assert!(
                    d.abs() + 0.5 * w <= 0.5 * lane.max(w) + 1e-12,
                    "{w} in {lane}: {d}"
                );
            }
        }
        assert_eq!(pull_over_offset(false, 2.6, 2.4), 0.0);
    }
    use crate::carfollowing::testkit::vehicle;
    use crate::views::DriverProfile;
    use v2xw_core::geom::Dims;

    fn of(class: VehicleClass, lat: f64, width: f64) -> VehicleView {
        let mut v = vehicle(
            0,
            5.0,
            DriverProfile {
                desired_speed_mps: 10.0,
                max_accel_mps2: 1.0,
                comfort_decel_mps2: 2.0,
                time_headway_s: 1.0,
                min_gap_m: 2.0,
                sigma: None,
            },
        );
        v.class = class;
        v.lateral_m = lat;
        v.dims = Dims::new(2.0, width, 1.5);
        v
    }

    /// The engine end to end: on a 5.0 m ring lane a car closes on a slow motorcycle.
    /// Lane-based (no lateral model) it queues behind; with the sub-lane model and lawful
    /// filtering it steers across and passes within the lane. Returns how far the car got
    /// past the motorcycle, metres (negative: still behind).
    fn car_gain_over_motorcycle(lateral: bool) -> f64 {
        use crate::carfollowing::idm::{Idm, IdmPreset};
        use crate::ctx::MobilityCtx;
        use crate::traits::Mobility;
        use crate::worlds::{RingParams, ring, ring_cycle};
        use crate::{EngineParams, IntersectionMode, MobilPreset, NativeMobility, NoDemand};
        use std::sync::Arc;
        use v2xw_core::rng::RngRegistry;
        use v2xw_core::time::Duration;

        let world = ring(&RingParams {
            lane_width_m: 5.0,
            speed_limit_mps: 13.9,
            ..RingParams::default()
        })
        .unwrap();
        let cycle = ring_cycle(&world, 0);
        let route: Vec<_> = cycle
            .iter()
            .cycle()
            .take(cycle.len() * 4)
            .copied()
            .collect();
        let rng = RngRegistry::new(7);
        let step = Duration::from_millis(100);
        let mut engine = NativeMobility::with_models(
            EngineParams {
                step,
                intersections: IntersectionMode::None,
                lane_changes: false,
                dynamic_rerouting: false,
                ..EngineParams::default()
            },
            Arc::new(Idm::new(IdmPreset::UrbanHcm)),
            MobilPreset::Kesting2007,
        );
        if lateral {
            engine = engine.with_lateral(Arc::new(Sl2015::new(Sl2015Params {
                filter: SublaneFilter {
                    motorcycles_share: true,
                    ..SublaneFilter::default()
                },
                ..Sl2015Params::default()
            })));
        }
        engine
            .init(
                &mut MobilityCtx::new(0, &world, &rng),
                Box::new(NoDemand::new()),
            )
            .unwrap();
        let mut moto_driver = IdmPreset::UrbanHcm.profile(VehicleClass::Motorcycle);
        moto_driver.desired_speed_mps = 4.0;
        let mut car_driver = IdmPreset::UrbanHcm.profile(VehicleClass::Passenger);
        car_driver.desired_speed_mps = 13.9;
        let moto = engine
            .spawn_with_route(
                &world,
                0,
                VehicleClass::Motorcycle,
                moto_driver,
                route.clone(),
                45.0,
            )
            .unwrap();
        let car = engine
            .spawn_with_route(&world, 0, VehicleClass::Passenger, car_driver, route, 5.0)
            .unwrap();
        engine.set_speed(moto, 4.0).unwrap();
        engine.set_speed(car, 8.0).unwrap();
        let (mut d_moto, mut d_car) = (0.0, 0.0);
        let mut t = 0;
        for _ in 0..400 {
            engine.step(&mut MobilityCtx::new(t, &world, &rng), step);
            t += step.as_nanos();
            for (id, _, _, v) in engine.longitudinal_states() {
                if id == moto {
                    d_moto += v * 0.1;
                } else if id == car {
                    d_car += v * 0.1;
                }
            }
        }
        // The car's front started 40 m behind the motorcycle's front.
        d_car - d_moto - 40.0
    }

    #[test]
    fn the_sub_lane_model_lets_a_car_pass_a_filtering_motorcycle_in_lane() {
        let queued = car_gain_over_motorcycle(false);
        let passed = car_gain_over_motorcycle(true);
        assert!(
            queued < 0.0,
            "lane-based: the car should stay behind, gained {queued}"
        );
        assert!(
            passed > 10.0,
            "sub-lane: the car should pass, gained {passed}"
        );
    }

    #[test]
    fn cars_never_share_and_a_cyclist_is_passed_only_with_three_feet() {
        let f = SublaneFilter::default();
        let car = of(VehicleClass::Passenger, 0.5, 1.8);
        assert!(f.is_leader(&car, &of(VehicleClass::Passenger, -3.0, 1.8)));
        // Cyclist at the right edge of a 4.5 m lane: car 0.5 m left of centre. Clearance
        // = |0.5 − (−1.8)| − (1.8 + 0.65)/2 = 2.3 − 1.225 = 1.075 m ≥ 0.914: passable.
        assert!(!f.is_leader(&car, &of(VehicleClass::Bicycle, -1.8, 0.65)));
        // On a 3.5 m lane the cyclist rides at −1.2: clearance 0.475 < 0.914, a leader.
        assert!(f.is_leader(&car, &of(VehicleClass::Bicycle, -1.2, 0.65)));
        // Motorcycles do not filter unless allowed.
        assert!(f.is_leader(&of(VehicleClass::Motorcycle, 1.2, 0.8), &car));
        let lawful = SublaneFilter {
            motorcycles_share: true,
            ..f
        };
        assert!(!lawful.is_leader(
            &of(VehicleClass::Motorcycle, 1.5, 0.8),
            &of(VehicleClass::Passenger, -0.5, 1.8)
        ));
    }
}
