//! The V2X source of re-routing information (`traffic.reroute_source: v2x`, WP-05,
//! frameworks/sumo.md P14).
//!
//! A driver whose own on-board unit has received and verified a DENM announcing an event
//! that blocks a lane (an accident, road works, an obstacle on the road, a stationary
//! vehicle; ETSI EN 302 637-3 with the cause codes of ETSI TS 102 894-2) learns that the
//! lane at the event's position is blocked, and re-routes round it if it re-routes at all
//! (`traffic.reroute_share`). Everything here is the receiver's belief: the event
//! position is the one the message carries (its sender's own fix), matched to the
//! receiver's map, and a vehicle that did not receive the message learns nothing.
//!
//! J2735 TIM is not in this engine's message set yet; when it is, its advisory region
//! maps to lanes here in the same way.

use v2xw_core::geo::GeoOrigin;
use v2xw_core::ids::LaneId;
use v2xw_msg::asn1::cdd::CauseCodeChoice;
use v2xw_world::{ClassMask, World};

/// How far from the announced event position, metres, the receiver looks for the lane it
/// is on: a few GNSS errors and half a lane. This crate's choice.
pub const EVENT_MATCH_RADIUS_M: f64 = 10.0;

/// Whether a DENM's cause announces an event that blocks a lane: accident (2),
/// roadworks (3), obstacle on the road (10), stationary vehicle (94). A hard-braking
/// warning (dangerous situation, 99), a traffic condition or slippery road does not.
pub fn is_blocking(cause: &CauseCodeChoice) -> bool {
    matches!(
        cause,
        CauseCodeChoice::accident2(_)
            | CauseCodeChoice::roadworks3(_)
            | CauseCodeChoice::hazardousLocation_ObstacleOnTheRoad10(_)
            | CauseCodeChoice::stationaryVehicle94(_)
    )
}

/// The lane a received DENM's payload places a lane-blocking event on, matched on the
/// receiver's map; `None` for a payload that does not decode, a termination, a cause that
/// blocks nothing, or an event position on no motor lane.
pub fn blocking_event_lane(world: &World, payload: &[u8]) -> Option<LaneId> {
    let denm = v2xw_msg::denm::decode_denm(payload).ok()?;
    let m = &denm.denm.management;
    if m.termination.is_some() {
        return None;
    }
    let situation = denm.denm.situation.as_ref()?;
    if !is_blocking(&situation.event_type.cc_and_scc) {
        return None;
    }
    let lat = f64::from(m.event_position.latitude.0) * 1e-7;
    let lon = f64::from(m.event_position.longitude.0) * 1e-7;
    let origin: GeoOrigin = world.origin.into();
    let pos = origin.to_enu(lat, lon, 0.0);
    world
        .nearest_lane_within(pos, EVENT_MATCH_RADIUS_M, Some(ClassMask::MOTOR_TRAFFIC))
        .map(|hit| hit.pos.lane)
}
