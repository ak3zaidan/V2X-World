//! The two seams a demand model may use beyond [`crate::traits::Demand::spawns_in`]:
//! a route it planned itself, and a look at its vehicles after every step.
//!
//! Both are defaulted on the trait, so every existing demand model is untouched and the
//! engine behaves exactly as before unless a model opts in.
//!
//! * **A planned route.** A demand model that carries routes — an assignment's saved
//!   routes, a bus line through its stops, a route sampled to match counts — returns them
//!   from [`crate::traits::Demand::planned_route`]. The engine checks the route with
//!   [`planned_route_is_drivable`] and falls back to its own router when the check fails,
//!   so a stale route file can slow a run down but never put a car where it cannot go.
//! * **Progress.** A demand model that has to act on its vehicles while they drive — a
//!   bus that must stop at its stops and dwell — says so with
//!   [`crate::traits::Demand::wants_progress`], is shown every one of its vehicles at the
//!   end of each step as an [`ActorProgress`], and answers with
//!   [`crate::views::MobilityCommand`]s that the engine applies at the start of the next
//!   step, exactly like a command from the scenario timeline.
//!
//! The demand-and-route-choice crate (`v2xw-demand`) is the user of both.

use serde::{Deserialize, Serialize};
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_world::{TurnDirection, World};

use crate::classes::VehicleClass;

/// Where one vehicle is at the end of a step, for a demand model that follows its own
/// vehicles ([`crate::traits::Demand::observe`]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ActorProgress {
    /// The actor.
    pub actor: ActorId,
    /// The demand-stream position its trip came from ([`crate::views::TripRequest::seq`]).
    pub seq: u64,
    /// Its class.
    pub class: VehicleClass,
    /// The lane it is on.
    pub lane: LaneId,
    /// How far along that lane its front bumper is, metres.
    pub s_m: f64,
    /// Its speed, m/s.
    pub speed_mps: f64,
    /// Its index into its route: `route[route_index] == lane`.
    pub route_index: usize,
}

/// True if `lanes` is a route a vehicle of `class` may drive from end to end: non-empty,
/// every lane exists, admits the class and is a motor-traffic lane (for a motorised
/// class), and every consecutive pair is joined by a permitted connection — the same
/// filters [`crate::routing::dijkstra::Dijkstra`] applies, U-turns included only when
/// `allow_uturn`.
pub fn planned_route_is_drivable(
    world: &World,
    class: VehicleClass,
    lanes: &[LaneId],
    allow_uturn: bool,
) -> bool {
    if lanes.is_empty() {
        return false;
    }
    let mask = class.class_mask();
    let motorised = mask.contains_any(v2xw_world::ClassMask::MOTOR_TRAFFIC);
    for lane in lanes {
        let Some(l) = world.try_lane(*lane) else {
            return false;
        };
        if !l.admits(mask) || (motorised && !l.kind.is_motorised()) {
            return false;
        }
    }
    lanes.windows(2).all(|pair| {
        world.successors(pair[0]).iter().any(|c| {
            c.permitted
                && c.via.unwrap_or(c.to_lane) == pair[1]
                && (allow_uturn || c.direction != TurnDirection::UTurn)
        })
    })
}
