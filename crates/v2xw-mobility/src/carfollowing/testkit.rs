//! Test helpers shared by the library's models (compiled for tests only).

use v2xw_core::geom::Dims;
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::time::Duration;
use v2xw_core::weather::WeatherState;
use v2xw_world::{ClassMask, LaneKind};

use crate::classes::VehicleClass;
use crate::traits::CarFollowing;
use crate::views::{DriverProfile, LaneView, LeaderView, VehicleView};

/// A straight driving lane with limit `limit`.
pub fn lane(limit: f64) -> LaneView {
    LaneView {
        id: LaneId::new(0),
        kind: LaneKind::Driving,
        speed_limit_mps: limit,
        width_m: 3.5,
        length_m: 5000.0,
        allowed: ClassMask::MOTOR_TRAFFIC,
    }
}

/// A passenger car `id` at `speed` with `driver`.
pub fn vehicle(id: u32, speed: f64, driver: DriverProfile) -> VehicleView {
    VehicleView {
        actor: ActorId::new(id),
        class: VehicleClass::Passenger,
        lane: LaneId::new(0),
        lane_index: 0,
        s_m: 0.0,
        lateral_m: 0.0,
        speed_mps: speed,
        accel_mps2: 0.0,
        heading_rad: 0.0,
        dims: Dims::new(5.0, 1.8, 1.5),
        driver,
    }
}

/// Drives `m` from `v_ego` behind an obstacle `gap` ahead holding `v_lead`, for `steps`
/// 100 ms steps. Returns the smallest net gap seen and the final speed.
pub fn run_follow(
    m: &dyn CarFollowing,
    v_ego: f64,
    gap: f64,
    v_lead: f64,
    steps: u32,
) -> (f64, f64) {
    let dt = Duration::from_millis(100);
    let mut ego = vehicle(1, v_ego, m.profile(VehicleClass::Passenger));
    ego.driver.desired_speed_mps = 30.0;
    let mut g = gap;
    let mut min_gap = gap;
    let w = WeatherState::default();
    for k in 0..steps {
        m.begin_step(u64::from(k) * dt.as_nanos(), dt);
        let lead = LeaderView::virtual_obstacle(g, v_lead);
        let a = m.accel(&ego, Some(&lead), &lane(30.0), &w);
        let v1 = (ego.speed_mps + a * 0.1).max(0.0);
        g += (v_lead - 0.5 * (ego.speed_mps + v1)) * 0.1;
        ego.accel_mps2 = (v1 - ego.speed_mps) / 0.1;
        ego.speed_mps = v1;
        min_gap = min_gap.min(g);
    }
    (min_gap, ego.speed_mps)
}
