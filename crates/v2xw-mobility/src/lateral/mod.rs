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

/// The narrow classes that may share a lane at all.
fn narrow(class: VehicleClass) -> bool {
    matches!(
        class,
        VehicleClass::Bicycle | VehicleClass::Motorcycle | VehicleClass::Moped | VehicleClass::Scooter
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

    /// The lateral clearance between two bodies at their current offsets, metres
    /// (negative when they overlap).
    pub fn clearance(a: &VehicleView, b: &VehicleView) -> f64 {
        (a.lateral_m - b.lateral_m).abs() - 0.5 * (a.dims.width_m + b.dims.width_m)
    }

    /// Whether `cand`, ahead of `ego` in its lane, is `ego`'s leader: always when the two
    /// may not share; otherwise only when they are closer laterally than their clearance.
    pub fn is_leader(&self, ego: &VehicleView, cand: &VehicleView) -> bool {
        match self.pair_gap(ego.class, cand.class) {
            None => true,
            Some(gap) => Self::clearance(ego, cand) < gap,
        }
    }
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
    use crate::carfollowing::testkit::vehicle;
    use crate::views::DriverProfile;
    use v2xw_core::geom::Dims;

    fn of(class: VehicleClass, lat: f64, width: f64) -> VehicleView {
        let mut v = vehicle(0, 5.0, DriverProfile {
            desired_speed_mps: 10.0,
            max_accel_mps2: 1.0,
            comfort_decel_mps2: 2.0,
            time_headway_s: 1.0,
            min_gap_m: 2.0,
        });
        v.class = class;
        v.lateral_m = lat;
        v.dims = Dims::new(2.0, width, 1.5);
        v
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
        let lawful = SublaneFilter { motorcycles_share: true, ..f };
        assert!(!lawful.is_leader(&of(VehicleClass::Motorcycle, 1.5, 0.8), &of(VehicleClass::Passenger, -0.5, 1.8)));
    }
}
