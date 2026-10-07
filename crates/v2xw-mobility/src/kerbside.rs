//! Kerbside stops: buses at their stops, taxis picking up and setting down, delivery vans
//! and trucks double-parked while they unload.
//!
//! # Why it exists
//!
//! A Midtown street is not a pipe. A good share of what slows it is vehicles that stop *in*
//! a travel lane on purpose: a bus serving a stop, a taxi letting a passenger out, a van
//! double-parked for a delivery. Each one turns a lane into a standing obstacle that the
//! traffic behind has to queue behind or go round, and the Highway Capacity Manual
//! counts exactly that as lost capacity (its bus-blockage and parking-manoeuvre
//! adjustments to the saturation flow). Without them the simulated street flows like a
//! test track, and the owner's complaint that the traffic does not look real is partly
//! that nothing ever stops except at a red.
//!
//! # What happens
//!
//! At spawn, a vehicle of a class that makes kerbside stops gets a [`KerbRole`] and the
//! distance into its trip at which its first stop falls due ([`KerbState::assign`]). Once
//! it has driven that far, it looks along its route — the lane it is on and the next few
//! — for the first stretch of **kerb lane** where it can stop (changing lanes to the right towards it first, when it is further out): the rightmost motorised lane
//! of its road, with another motorised lane beside it that the traffic behind can pass in,
//! and room to brake at [`KerbsideParams::stop_decel_mps2`] before the stop and to clear
//! the no-change zone after it ([`KerbState::advance`]). The stop is a virtual obstacle the
//! car-following model brakes for like a stop line; once standing at it, the vehicle dwells
//! for its drawn time and moves off with its own reaction time. While a stop is planned the
//! vehicle makes no discretionary lane change.
//!
//! The traffic behind treats a vehicle at the kerb as what it is: a vehicle standing in its
//! lane. Followers that reach it go round it ([`crate::engine`]'s pass rule), detouring if
//! the lane beside cannot make their next turn — which is what drivers stuck behind a
//! double-parked truck in Manhattan do.
//!
//! # The numbers, and where they come from
//!
//! | What | Default | Source |
//! |---|---|---|
//! | bus dwell | lognormal, mean 60 s, coefficient of variation 0.6 | TCRP Report 100, *Transit Capacity and Quality of Service Manual* 2nd ed. (2003), Part 4: in the absence of field data, 60 s at a downtown stop, 30 s at a major outlying stop, 15 s at a typical outlying stop; dwell-time c_v typically 0.4–0.8, 0.6 when unknown (**secondary**: as the manual is widely quoted; not re-read here) |
//! | bus stop spacing | 260 m (about 850 ft) | **a choice**: MTA New York City Transit local-bus stops are commonly described as two to three short blocks apart (750–1,000 ft); no MTA stop inventory was read |
//! | taxi share of passenger cars | 0 by default; 0.3 in the New York City preset ([`KerbsideParams::midtown`]) | **a choice**: taxis and app-based for-hire vehicles are widely reported as roughly a third of the vehicles in the Manhattan core (e.g. Schaller Consulting, *Empty Seats, Full Streets*, 2017; **not re-read**) — a share, not a count |
//! | taxi pick-up or drop-off | one per trip, lognormal median 30 s, shape 0.5 | **a choice**: no field distribution of kerbside passenger dwell was read |
//! | delivery stop | 60 % of delivery vans and trucks make one stop per trip, lognormal median 10 min, shape 0.6 | **a choice inside the range field studies report**: urban commercial-vehicle stops in a CBD last on the order of 10–30 minutes (University of Washington Urban Freight Lab, Seattle CBD studies 2018–2020; **not re-read**) |
//! | the stop is braked into at | 1.5 m/s² | **a choice**: a planned stop, well inside AASHTO's 3.4 m/s² comfortable deceleration |
//!
//! # Determinism
//!
//! Every draw is from a keyed stream: the role at spawn under `(plugin(MODEL_ID),
//! custom(MODEL_ID, seq))`, each dwell under `(plugin(MODEL_ID), custom(DWELL_ID, seq·256 +
//! stop))`. The state changes only in the engine's integration pass, from the actor's own
//! state, so the Jacobi property holds.

use serde::{Deserialize, Serialize};
use v2xw_core::ids::LaneId;
use v2xw_core::math;
use v2xw_core::rng::{EntityRef, RngDomain};
use v2xw_core::time::{SimTime, ns_to_secs};
use v2xw_world::{LaneKind, World};

use crate::classes::VehicleClass;
use crate::ctx::MobCtx;

/// The model id, and the plug-in domain every kerbside draw is taken from.
pub const MODEL_ID: &str = "mobility/kerbside";

/// The custom entity a stop's dwell is drawn under.
const DWELL_ID: &str = "mobility/kerbside/dwell";

/// How close to its stop point a standing vehicle must be to count as at the kerb,
/// metres: a stop is braked into at a constant deceleration and stands within centimetres
/// of the point; the slack covers a stop that a leader cut a little short.
const AT_STOP_M: f64 = 1.5;

/// How far a vehicle may overrun its stop before the stop is abandoned, metres.
const OVERRUN_M: f64 = 1.5;

/// How much further on a stop that found no kerb lane is looked for again, metres.
const RETRY_M: f64 = 40.0;

/// How many lanes of the route ahead a stop may be placed on.
const LOOK_LANES: usize = 5;

/// What a vehicle stops at the kerb for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum KerbRole {
    /// A bus serving its stops.
    Bus,
    /// A taxi or for-hire car picking up or setting down.
    Taxi,
    /// A delivery van or truck unloading, double-parked.
    Delivery,
}

impl KerbRole {
    /// Every role.
    pub const ALL: [KerbRole; 3] = [KerbRole::Bus, KerbRole::Taxi, KerbRole::Delivery];

    /// A stable label.
    pub const fn label(self) -> &'static str {
        match self {
            KerbRole::Bus => "bus",
            KerbRole::Taxi => "taxi",
            KerbRole::Delivery => "delivery",
        }
    }
}

/// How vehicles stop at the kerb.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KerbsideParams {
    /// Whether vehicles make kerbside stops at all.
    pub enabled: bool,
    /// Distance between one bus stop and the next, metres.
    pub bus_stop_spacing_m: f64,
    /// Mean bus dwell, seconds.
    pub bus_dwell_mean_s: f64,
    /// Coefficient of variation of the bus dwell.
    pub bus_dwell_cv: f64,
    /// The share of passenger cars that are taxis or for-hire vehicles.
    pub taxi_share: f64,
    /// Median taxi dwell at the kerb, seconds.
    pub taxi_dwell_median_s: f64,
    /// Lognormal shape of the taxi dwell.
    pub taxi_dwell_sigma: f64,
    /// The share of delivery vans and trucks that make a delivery stop on their trip.
    pub delivery_stop_share: f64,
    /// Median delivery dwell, seconds.
    pub delivery_dwell_median_s: f64,
    /// Lognormal shape of the delivery dwell.
    pub delivery_dwell_sigma: f64,
    /// The deceleration a kerbside stop is planned at, m/s².
    pub stop_decel_mps2: f64,
    /// How far beyond the engine's no-change zone a stop must end before the lane does,
    /// metres, so the traffic queued behind it is outside the zone and may pass.
    pub end_margin_m: f64,
}

impl Default for KerbsideParams {
    fn default() -> Self {
        Self {
            enabled: true,
            bus_stop_spacing_m: 260.0,
            bus_dwell_mean_s: 60.0,
            bus_dwell_cv: 0.6,
            taxi_share: 0.0,
            taxi_dwell_median_s: 30.0,
            taxi_dwell_sigma: 0.5,
            delivery_stop_share: 0.6,
            delivery_dwell_median_s: 600.0,
            delivery_dwell_sigma: 0.6,
            stop_decel_mps2: 1.5,
            end_margin_m: 8.0,
        }
    }
}

/// The share of passenger cars that are taxis or for-hire vehicles in the New York City
/// preset (see the module table: **a choice**).
pub const MIDTOWN_TAXI_SHARE: f64 = 0.3;

impl KerbsideParams {
    /// New York City: the defaults, with taxis.
    pub fn midtown() -> Self {
        Self {
            taxi_share: MIDTOWN_TAXI_SHARE,
            ..Self::default()
        }
    }

    /// The preset for a world's highway preset label: [`KerbsideParams::midtown`] for
    /// `urban-us-nyc`, the defaults (no taxis) otherwise.
    pub fn for_highway_preset(label: Option<&str>) -> Self {
        match label {
            Some("urban-us-nyc") => Self::midtown(),
            _ => Self::default(),
        }
    }

    /// The bus dwell's lognormal `(mu, sigma)` from its mean and coefficient of variation:
    /// `σ² = ln(1 + c_v²)`, `μ = ln(mean) − σ²/2`.
    pub fn bus_dwell_lognormal(&self) -> (f64, f64) {
        let cv = self.bus_dwell_cv.max(0.0);
        let s2 = math::ln(1.0 + cv * cv);
        (math::ln(self.bus_dwell_mean_s.max(1.0)) - 0.5 * s2, math::sqrt(s2))
    }

    /// The role a vehicle of `class` may take, before any share is drawn.
    pub const fn candidate_role(class: VehicleClass) -> Option<KerbRole> {
        match class {
            VehicleClass::Bus => Some(KerbRole::Bus),
            VehicleClass::Passenger => Some(KerbRole::Taxi),
            VehicleClass::Delivery | VehicleClass::Truck => Some(KerbRole::Delivery),
            _ => None,
        }
    }
}

/// A kerbside stop, planned or under way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KerbStop {
    /// The lane it is on.
    pub lane: LaneId,
    /// Where the vehicle's front stops, arc length along `lane`, metres.
    pub s_m: f64,
    /// How long it stays, seconds.
    pub dwell_s: f64,
    /// When it came to rest there; `None` while it is still on its way.
    pub since: Option<SimTime>,
}

/// One vehicle's kerbside stops.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KerbState {
    /// What it stops for, if anything.
    pub role: Option<KerbRole>,
    /// How far it has driven since it appeared, metres.
    pub driven_m: f64,
    /// The driven distance at which its next stop falls due; infinite when it has none
    /// left.
    pub next_due_m: f64,
    /// Stops completed.
    pub stops_made: u32,
    /// The stop planned or under way.
    pub stop: Option<KerbStop>,
}

impl Default for KerbState {
    fn default() -> Self {
        Self::NONE
    }
}

impl KerbState {
    /// A vehicle that never stops at the kerb.
    pub const NONE: KerbState = KerbState {
        role: None,
        driven_m: 0.0,
        next_due_m: f64::INFINITY,
        stops_made: 0,
        stop: None,
    };

    /// The kerbside role and first due distance of the vehicle of trip `seq`, of `class`,
    /// on a route `route_length_m` long.
    pub fn assign(
        ctx: &dyn MobCtx,
        params: &KerbsideParams,
        class: VehicleClass,
        seq: u64,
        route_length_m: f64,
    ) -> KerbState {
        let Some(role) = KerbsideParams::candidate_role(class).filter(|_| params.enabled) else {
            return KerbState::NONE;
        };
        let mut rng = ctx.rng(RngDomain::plugin(MODEL_ID), EntityRef::custom(MODEL_ID, seq));
        // Two draws whatever the role, so a role's draws do not depend on another's.
        let take = rng.uniform(0.0, 1.0);
        let where_ = rng.uniform(0.0, 1.0);
        let length = route_length_m.max(0.0);
        let (active, due) = match role {
            KerbRole::Bus => (true, where_ * params.bus_stop_spacing_m.max(1.0)),
            KerbRole::Taxi => (take < params.taxi_share, (0.15 + 0.7 * where_) * length),
            KerbRole::Delivery => (
                take < params.delivery_stop_share,
                (0.1 + 0.5 * where_) * length,
            ),
        };
        if !active {
            return KerbState::NONE;
        }
        KerbState {
            role: Some(role),
            next_due_m: due,
            ..KerbState::NONE
        }
    }

    /// True when a stop is due and none is planned yet: the vehicle is looking for the
    /// kerb, and moves towards it ([`crate::engine`] changes it to the right).
    pub fn due(&self) -> bool {
        self.role.is_some() && self.stop.is_none() && self.driven_m >= self.next_due_m
    }

    /// True while the vehicle stands at the kerb for its stop.
    pub fn at_kerb(&self) -> bool {
        self.stop.is_some_and(|s| s.since.is_some())
    }

    /// The role it is at the kerb in, if it is.
    pub fn at_kerb_as(&self) -> Option<KerbRole> {
        if self.at_kerb() { self.role } else { None }
    }

    /// True while the stop still holds the vehicle at `t`: planned and not reached, or
    /// reached and the dwell not over.
    pub fn holds(&self, t: SimTime) -> bool {
        match self.stop {
            None => false,
            Some(KerbStop { since: None, .. }) => true,
            Some(KerbStop {
                since: Some(since),
                dwell_s,
                ..
            }) => ns_to_secs(t.saturating_sub(since)) < dwell_s,
        }
    }

    /// The distance from the vehicle's front (on `lane`, at `s_m`) to its stop point,
    /// along `route` from `route_index`, metres; `None` with no stop or one not on the
    /// lanes ahead.
    pub fn stop_distance(
        &self,
        world: &World,
        lane: LaneId,
        s_m: f64,
        route: &[LaneId],
        route_index: usize,
    ) -> Option<f64> {
        let stop = self.stop?;
        if stop.lane == lane {
            return Some(stop.s_m - s_m);
        }
        let mut offset = world.try_lane(lane)?.length_m - s_m;
        for l in route.iter().skip(route_index + 1).take(LOOK_LANES + 2) {
            if *l == stop.lane {
                return Some(offset + stop.s_m);
            }
            offset += world.try_lane(*l)?.length_m;
        }
        None
    }

    /// The state after one integration step, from the vehicle's own state at its end:
    /// on `lane` at `s_m` (its front), at `speed_mps`, having driven `advanced_m`,
    /// following `route` from `route_index`; `changing` while a lane change is under way.
    #[allow(clippy::too_many_arguments)]
    pub fn advance(
        &mut self,
        ctx: &dyn MobCtx,
        params: &KerbsideParams,
        world: &World,
        seq: u64,
        length_m: f64,
        t1: SimTime,
        lane: LaneId,
        s_m: f64,
        speed_mps: f64,
        advanced_m: f64,
        route: &[LaneId],
        route_index: usize,
        changing: bool,
        no_change_zone_m: f64,
    ) {
        let Some(role) = self.role else { return };
        self.driven_m += advanced_m.max(0.0);
        match self.stop {
            Some(stop) => match stop.since {
                Some(since) => {
                    if ns_to_secs(t1.saturating_sub(since)) >= stop.dwell_s {
                        self.stop = None;
                        self.stops_made += 1;
                        self.next_due_m = match role {
                            KerbRole::Bus => {
                                let u = ctx
                                    .rng(
                                        RngDomain::plugin(MODEL_ID),
                                        EntityRef::custom(
                                            DWELL_ID,
                                            seq.wrapping_mul(256)
                                                .wrapping_add(u64::from(self.stops_made) + 128),
                                        ),
                                    )
                                    .uniform(0.75, 1.25);
                                self.driven_m + params.bus_stop_spacing_m * u
                            }
                            KerbRole::Taxi | KerbRole::Delivery => f64::INFINITY,
                        };
                    }
                }
                None => {
                    match self.stop_distance(world, lane, s_m, route, route_index) {
                        Some(d) if d.abs() <= AT_STOP_M && speed_mps < 0.1 && lane == stop.lane => {
                            self.stop = Some(KerbStop {
                                since: Some(t1),
                                ..stop
                            });
                        }
                        Some(d) if d >= -OVERRUN_M => {}
                        // Overrun, or the stop is no longer on the route ahead: given up,
                        // and looked for again a little further on.
                        _ => {
                            self.stop = None;
                            self.next_due_m = self.driven_m + RETRY_M;
                        }
                    }
                }
            },
            None => {
                if self.driven_m < self.next_due_m || changing {
                    return;
                }
                match plan_stop(
                    world,
                    params,
                    length_m,
                    lane,
                    s_m,
                    speed_mps,
                    route,
                    route_index,
                    no_change_zone_m,
                ) {
                    Some((stop_lane, stop_s)) => {
                        let mut rng = ctx.rng(
                            RngDomain::plugin(MODEL_ID),
                            EntityRef::custom(
                                DWELL_ID,
                                seq.wrapping_mul(256).wrapping_add(u64::from(self.stops_made)),
                            ),
                        );
                        let dwell_s = match role {
                            KerbRole::Bus => {
                                let (mu, sigma) = params.bus_dwell_lognormal();
                                rng.lognormal(mu, sigma).clamp(5.0, 300.0)
                            }
                            KerbRole::Taxi => rng
                                .lognormal(
                                    math::ln(params.taxi_dwell_median_s.max(1.0)),
                                    params.taxi_dwell_sigma,
                                )
                                .clamp(5.0, 180.0),
                            KerbRole::Delivery => rng
                                .lognormal(
                                    math::ln(params.delivery_dwell_median_s.max(1.0)),
                                    params.delivery_dwell_sigma,
                                )
                                .clamp(60.0, 3600.0),
                        };
                        self.stop = Some(KerbStop {
                            lane: stop_lane,
                            s_m: stop_s,
                            dwell_s,
                            since: None,
                        });
                    }
                    // On the kerb lane with no room left on it: looked for again a little
                    // further on. In a lane further out: still due, so the vehicle keeps
                    // moving towards the kerb.
                    None if is_kerb_lane(world, lane) => {
                        self.next_due_m = self.driven_m + RETRY_M;
                    }
                    None => {}
                }
            }
        }
    }
}

/// True if `lane` is a kerb lane a vehicle may stop in: a driving or bus lane that is the
/// rightmost motorised lane of its road, with a driving lane beside it in the same
/// direction for the traffic behind to pass in.
pub fn is_kerb_lane(world: &World, lane: LaneId) -> bool {
    let Some(l) = world.try_lane(lane) else {
        return false;
    };
    if !matches!(l.kind, LaneKind::Driving | LaneKind::Bus) {
        return false;
    }
    let edge = world.edge(l.edge);
    let mut motorised: Vec<(u8, LaneKind)> = edge
        .lanes
        .iter()
        .filter_map(|id| world.try_lane(*id))
        .filter(|o| matches!(o.kind, LaneKind::Driving | LaneKind::Bus))
        .map(|o| (o.index, o.kind))
        .collect();
    motorised.sort_by_key(|(i, _)| *i);
    let Some((first, _)) = motorised.first().copied() else {
        return false;
    };
    first == l.index
        && motorised
            .iter()
            .skip(1)
            .any(|(_, kind)| *kind == LaneKind::Driving)
}

/// Where a vehicle due a stop now stops: the first kerb lane on its route ahead (the lane
/// it is on included) with a stretch where the front can come to rest braking at the
/// planned deceleration, the body clear of the lane's start, and the stop at least the
/// no-change zone plus [`KerbsideParams::end_margin_m`] before the lane's end — so the
/// traffic queued behind it may still change lanes to pass. The earliest such point.
#[allow(clippy::too_many_arguments)]
pub fn plan_stop(
    world: &World,
    params: &KerbsideParams,
    length_m: f64,
    lane: LaneId,
    s_m: f64,
    speed_mps: f64,
    route: &[LaneId],
    route_index: usize,
    no_change_zone_m: f64,
) -> Option<(LaneId, f64)> {
    let braking = speed_mps * speed_mps / (2.0 * params.stop_decel_mps2.max(0.1)) + 1.0;
    // The lanes ahead, each with the distance from the front to its start.
    let mut offset = -s_m;
    let mut lanes: Vec<(LaneId, f64)> = vec![(lane, offset)];
    if let Some(l) = world.try_lane(lane) {
        offset += l.length_m;
    }
    for l in route.iter().skip(route_index + 1).take(LOOK_LANES) {
        lanes.push((*l, offset));
        offset += world.try_lane(*l).map_or(0.0, |x| x.length_m);
    }
    for (id, start) in lanes {
        let Some(l) = world.try_lane(id) else { continue };
        if l.kind == LaneKind::Internal || !is_kerb_lane(world, id) {
            continue;
        }
        let lo = (length_m + 3.0).max(braking - start);
        let hi = l.length_m - (no_change_zone_m.max(0.0) + params.end_margin_m.max(0.0));
        if lo <= hi {
            return Some((id, lo));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bus_dwell_has_the_mean_and_spread_it_was_given() {
        let p = KerbsideParams::default();
        let (mu, sigma) = p.bus_dwell_lognormal();
        // A lognormal's mean is exp(μ + σ²/2) and its c_v sqrt(exp(σ²) − 1).
        let mean = math::exp(mu + 0.5 * sigma * sigma);
        let cv = math::sqrt(math::exp(sigma * sigma) - 1.0);
        assert!((mean - 60.0).abs() < 1e-9, "{mean}");
        assert!((cv - 0.6).abs() < 1e-9, "{cv}");
    }

    #[test]
    fn only_new_york_has_taxis_by_default() {
        assert_eq!(KerbsideParams::default().taxi_share, 0.0);
        assert_eq!(
            KerbsideParams::for_highway_preset(Some("urban-us-nyc")).taxi_share,
            MIDTOWN_TAXI_SHARE
        );
        assert_eq!(
            KerbsideParams::for_highway_preset(Some("urban-us-portland")).taxi_share,
            0.0
        );
        assert_eq!(KerbsideParams::candidate_role(VehicleClass::Bus), Some(KerbRole::Bus));
        assert_eq!(KerbsideParams::candidate_role(VehicleClass::Motorcycle), None);
    }

    #[test]
    fn a_stop_holds_until_its_dwell_is_over() {
        let stop = KerbStop {
            lane: LaneId::new(0),
            s_m: 20.0,
            dwell_s: 30.0,
            since: Some(10 * v2xw_core::time::NS_PER_S),
        };
        let s = KerbState {
            role: Some(KerbRole::Bus),
            stop: Some(stop),
            ..KerbState::NONE
        };
        assert!(s.at_kerb());
        assert!(s.holds(39 * v2xw_core::time::NS_PER_S));
        assert!(!s.holds(40 * v2xw_core::time::NS_PER_S));
        assert!(!KerbState::NONE.holds(0));
    }
}
