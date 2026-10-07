//! Scripted safety events on the scenario timeline: `safety.hard-brake`,
//! `safety.breakdown` and `safety.cut-in`.
//!
//! # Why a scenario needs them
//!
//! The calibrated drivers are careful, which is what makes them realistic: on the dense
//! `connected-intersections` run no driver braked harder than 3.02 m/s² in 18,052 samples,
//! so nothing ever reached the 0.4 g at which J2945/1 sets the hard-braking event flag and
//! ETSI's emergency-electronic-brake-light triggering raises a DENM. The applications
//! that react to emergencies (EEBL, FCW, a stationary-vehicle warning) can then only be
//! exercised by an emergency that is put there, at a known instant, on a known vehicle —
//! which is how their field tests do it too (CAMP VSC-A's objective tests script the lead
//! vehicle's braking; Euro NCAP's AEB tests script the cut-in).
//!
//! # What each does
//!
//! | Kind | The vehicle | Defaults |
//! |---|---|---|
//! | `safety.hard-brake` | brakes at `decel_mps2` to a stop, stands `hold_s`, drives on | 0.5 g (4.90 m/s²), 2 s |
//! | `safety.breakdown` | brakes at `decel_mps2` to a stop, hazard lights on, stands until `until` | 3 m/s² |
//! | `safety.cut-in` | changes lane to `side` into the gap ahead of that lane's follower | `left` |
//!
//! 0.5 g is the midpoint of the hard-braking decelerations CAMP's EEBL objective tests
//! used and sits above J2945/1's 0.4 g flag threshold, so the event is unambiguous; 3 m/s²
//! is a firm service stop (AASHTO's 3.4 m/s² comfortable-deceleration design value, less a
//! margin), what a driver whose engine has died does on the way to the kerb. Both are
//! parameters, and both are capped by the road surface's grip.
//!
//! The two stacks react differently to the same stop, as deployed vehicles do: on the US
//! stack 0.5 g sets the BSM's hard-braking flag (J2735's 0.4 g), but on the European stack
//! the emergency-brake-light DENM follows the vehicle's emergency stop signal, which UN
//! R48 does not allow below 6 m/s² — so a scripted stop meant to raise a DENM sets
//! `decel_mps2` to 6 or more (a full ABS stop on dry asphalt reaches 8–9 m/s²).
//!
//! # Which vehicle
//!
//! `target` names a node id. `"auto"` (the default) picks the first vehicle, in actor-id
//! order, that fits the event, so a scenario need not know which vehicles the demand will
//! have produced by then:
//!
//! * a hard brake: a moving vehicle (≥ [`AUTO_MIN_SPEED_MPS`]) with a vehicle behind it in
//!   the same lane within [`AUTO_FOLLOWER_RANGE_M`] — an emergency somebody has to react to;
//! * a breakdown: a moving vehicle;
//! * a cut-in: a moving vehicle with a lane on `side` of the same road and a vehicle on it
//!   between [`CUT_IN_MIN_GAP_M`] and [`AUTO_FOLLOWER_RANGE_M`] behind it.
//!
//! The pick reads ground truth, which is the scenario author's hand, not any node's view.
//!
//! When no vehicle fits at the item's instant, an `"auto"` item **waits**: it is tried
//! again at every mobility step for up to `within_s` (default [`AUTO_WAIT_DEFAULT_S`]) and
//! fires at the first step a vehicle fits, so its `scenario.event` record says `waiting`
//! first and then `start` with the node it acted on (or `expired`, with nothing done).
//! A light-traffic run therefore still gets its emergency, a little later, rather than
//! none; a named `target` that is not in the run does not wait.

use std::collections::BTreeMap;

use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::kinematics::Kinematics;
use v2xw_world::World;

/// Default deceleration of a `safety.hard-brake`: 0.5 g, m/s².
pub const HARD_BRAKE_DEFAULT_DECEL_MPS2: f64 = 0.5 * 9.806_65;

/// Default time a hard-braking vehicle stands before it drives on, seconds.
pub const HARD_BRAKE_DEFAULT_HOLD_S: f64 = 2.0;

/// Default deceleration of a `safety.breakdown`, m/s².
pub const BREAKDOWN_DEFAULT_DECEL_MPS2: f64 = 3.0;

/// How long an `"auto"` safety event waits for a vehicle that fits, seconds.
pub const AUTO_WAIT_DEFAULT_S: f64 = 10.0;

/// The slowest a vehicle `"auto"` may pick is moving, m/s (about 29 km/h): an emergency
/// stop from a crawl is not an emergency.
pub const AUTO_MIN_SPEED_MPS: f64 = 8.0;

/// How far behind the picked vehicle its follower may be, metres.
pub const AUTO_FOLLOWER_RANGE_M: f64 = 50.0;

/// The nearest a cut-in's new follower may be behind the vehicle cutting in, metres: the
/// lane-change resolution refuses a gap the body does not fit, so the pick does too.
pub const CUT_IN_MIN_GAP_M: f64 = 8.0;

/// Which vehicle a safety event acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// A node, by id.
    Node(NodeId),
    /// The first vehicle that fits (see the module notes).
    Auto,
}

impl Pick {
    /// Reads a `target` parameter: absent or `"auto"`, or a node id.
    ///
    /// # Errors
    /// A sentence naming the accepted spellings.
    pub fn parse(value: Option<&serde_json::Value>) -> Result<Pick, String> {
        match value {
            None => Ok(Pick::Auto),
            Some(v) if v.as_str() == Some("auto") => Ok(Pick::Auto),
            Some(v) => v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .map(|n| Pick::Node(NodeId::new(n)))
                .ok_or_else(|| format!("{v} is not a node id or \"auto\"")),
        }
    }
}

/// The side a cut-in goes to, from its `side` parameter (default left).
///
/// # Errors
/// A sentence naming the accepted values.
pub fn parse_side(value: Option<&serde_json::Value>) -> Result<v2xw_mobility::Side, String> {
    match value.and_then(serde_json::Value::as_str) {
        None if value.is_none() => Ok(v2xw_mobility::Side::Left),
        Some("left") => Ok(v2xw_mobility::Side::Left),
        Some("right") => Ok(v2xw_mobility::Side::Right),
        _ => Err(format!(
            "{} is not a side; write \"left\" or \"right\"",
            value.map(ToString::to_string).unwrap_or_default()
        )),
    }
}

/// A positive number parameter, or its default.
///
/// # Errors
/// A sentence when it is present and not a positive finite number.
pub fn positive(value: Option<&serde_json::Value>, default: f64) -> Result<f64, String> {
    match value {
        None => Ok(default),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite() && *x > 0.0)
            .ok_or_else(|| format!("{v} is not a positive number")),
    }
}

/// One vehicle as the pick sees it.
#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    /// The actor.
    pub actor: ActorId,
    /// Its node, when it carries one.
    pub node: Option<NodeId>,
    /// Its last published state.
    pub state: Kinematics,
}

/// What a pick is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A hard brake: needs a follower.
    HardBrake,
    /// A breakdown: any moving vehicle.
    Breakdown,
    /// A cut-in to this side: needs a follower on that lane.
    CutIn(v2xw_mobility::Side),
}

/// The vehicle an event acts on, from `candidates` in actor-id order.
pub fn pick(
    world: &World,
    candidates: &BTreeMap<ActorId, Candidate>,
    pick: Pick,
    purpose: Purpose,
) -> Option<Candidate> {
    if let Pick::Node(n) = pick {
        return candidates.values().find(|c| c.node == Some(n)).copied();
    }
    let speed = |k: &Kinematics| v2xw_core::math::hypot(k.vel.x, k.vel.y);
    // Vehicles by lane, for the follower test: (lane, s) of each.
    let mut by_lane: BTreeMap<u32, Vec<(f64, ActorId)>> = BTreeMap::new();
    for c in candidates.values() {
        if let Some(l) = c.state.lane {
            by_lane
                .entry(l.lane.index())
                .or_default()
                .push((l.s_m, c.actor));
        }
    }
    let follower_on = |lane: u32, s: f64, min_gap: f64| {
        by_lane.get(&lane).is_some_and(|v| {
            v.iter()
                .any(|(os, _)| *os < s - min_gap && *os >= s - AUTO_FOLLOWER_RANGE_M)
        })
    };
    // An equipped vehicle first: a scripted emergency is there to be announced and
    // reacted to, so the pick is a vehicle with an on-board unit when one fits.
    let fits = |c: &&Candidate| {
        if speed(&c.state) < AUTO_MIN_SPEED_MPS {
            return false;
        }
        let Some(l) = c.state.lane else {
            return false;
        };
        match purpose {
            Purpose::Breakdown => true,
            Purpose::HardBrake => follower_on(l.lane.index(), l.s_m, 0.0),
            Purpose::CutIn(side) => {
                let Some(lane) = world.roads.try_lane(l.lane) else {
                    return false;
                };
                let want = match side {
                    v2xw_mobility::Side::Left => i32::from(lane.index) + 1,
                    v2xw_mobility::Side::Right => i32::from(lane.index) - 1,
                };
                let Some(edge) = world.roads.try_edge(lane.edge) else {
                    return false;
                };
                edge.lanes.iter().any(|other| {
                    world.roads.try_lane(*other).is_some_and(|o| {
                        i32::from(o.index) == want
                            && o.kind == lane.kind
                            && follower_on(o.id.index(), l.s_m, CUT_IN_MIN_GAP_M)
                    })
                })
            }
        }
    };
    candidates
        .values()
        .filter(|c| c.node.is_some())
        .find(fits)
        .or_else(|| candidates.values().find(fits))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_is_auto_or_a_node() {
        assert_eq!(Pick::parse(None), Ok(Pick::Auto));
        assert_eq!(
            Pick::parse(Some(&serde_json::json!("auto"))),
            Ok(Pick::Auto)
        );
        assert_eq!(
            Pick::parse(Some(&serde_json::json!(7))),
            Ok(Pick::Node(NodeId::new(7)))
        );
        assert!(Pick::parse(Some(&serde_json::json!("car 7"))).is_err());
        assert!(parse_side(Some(&serde_json::json!("up"))).is_err());
        assert_eq!(parse_side(None), Ok(v2xw_mobility::Side::Left));
        assert!(positive(Some(&serde_json::json!(-1.0)), 1.0).is_err());
        assert_eq!(positive(None, 4.0), Ok(4.0));
    }
}
