//! The vehicle's own state as its on-board unit reads it off the vehicle bus, and what the
//! unit derives from it for its messages: J2945/1 path history and path prediction, the
//! J2735 event flags and exterior lights, the CAM's dynamics and low-frequency container.
//!
//! # Why this is not a firewall breach
//!
//! A fielded OBU is wired to its own vehicle's CAN bus: wheel speed, the accelerometers and
//! yaw-rate sensor of the stability control, the brake switch, the light and turn-signal
//! stalks, the hazard switch, and the navigation's route. Every field of [`VehicleBus`] is
//! one of those — the vehicle's own sensors about itself, like its GNSS fix — and none is
//! a view of any other road user (invariant I-C2). The engine hands it in each mobility
//! step, as it hands in the position belief.
//!
//! # Derivations, each with its source
//!
//! | Output | Rule | Source |
//! |---|---|---|
//! | brake lights / `wheelBrakes` | the service brake is applied when the vehicle decelerates harder than [`BRAKE_SWITCH_DECEL_MPS2`] | this build's threshold: engine drag and rolling resistance alone decelerate a car at up to about 0.5 m/s², so anything harder needs the brake |
//! | `eventHardBraking` | deceleration past 0.4 g (0.2 g heavy) | J2735 2024-09 §7.234 |
//! | `eventHazardLights` | the hazard switch is on | J2735 §7.234 |
//! | `eventDisabledVehicle` | the vehicle is broken down (hazards on and standing) | J2735 §7.234 ("any vehicle that considers itself disabled") |
//! | `eventLightsChanged` | the exterior lights changed in the last 2 s | J2735 §7.234 |
//! | turn signal | on from [`TURN_SIGNAL_LEAD_M`] before a turn until the turn ends | California Vehicle Code §22108 (100 ft); most US codes say the same |
//! | low beams | on when visibility is under [`HEADLIGHT_VISIBILITY_M`] or between 19:00 and 07:00 local | most US states' 1,000 ft rule; the clock hours stand in for sunset and sunrise, a stated simplification |

use v2xw_core::belief::PositionEstimate;
use v2xw_core::geo::GeoOrigin;
use v2xw_core::time::SimTime;
use v2xw_msg::j2735::bsm::{
    BrakeAppliedStatus, BrakeSystemStatus, ExteriorLights, VehicleEventFlags,
    VehicleSafetyExtensions,
};
use v2xw_msg::j2945::{
    self, Breadcrumb, PathHistoryBuilder, PathHistoryParams, PathPredictionParams, PathPredictor,
};

/// Deceleration beyond which the service brake is on, m/s² (see the module table).
pub const BRAKE_SWITCH_DECEL_MPS2: f64 = 0.7;

/// How far before a turn the driver signals, metres: 100 ft (California Vehicle Code
/// §22108, and most US state codes).
pub const TURN_SIGNAL_LEAD_M: f64 = 30.5;

/// Visibility below which headlights are required, metres: 1,000 ft, most US states'
/// rule.
pub const HEADLIGHT_VISIBILITY_M: f64 = 304.8;

/// A turn the driver intends, as the turn-signal stalk and the navigation know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnIntent {
    /// Straight on.
    Straight,
    /// Left (including a slight left or a U-turn).
    Left,
    /// Right (including a slight right).
    Right,
}

/// The vehicle's own movement at the next junction, from its navigation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnIntent {
    /// Which way.
    pub turn: TurnIntent,
    /// Metres to the start of the movement; zero or less once on it.
    pub distance_m: f64,
    /// The junction's reference point, world ENU metres, as the navigation's map has it.
    pub junction_pos: v2xw_core::geom::Vec3,
}

/// One reading of the vehicle's own bus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleBus {
    /// When it was read, on the simulation's timeline.
    pub t: SimTime,
    /// Wheel speed, m/s.
    pub speed_mps: f64,
    /// Longitudinal acceleration, m/s², positive forwards.
    pub a_long_mps2: f64,
    /// Lateral acceleration, m/s², positive to the left.
    pub a_lat_mps2: f64,
    /// Yaw rate, rad/s, positive counter-clockwise.
    pub yaw_rate_rad_s: f64,
    /// The hazard switch.
    pub hazard_lights: bool,
    /// The low-beam headlights.
    pub low_beam: bool,
    /// The navigation's next movement, when one is within its horizon.
    pub intent: Option<OwnIntent>,
    /// Whether this is a heavy vehicle for J2735's hard-braking rule (a Class 4 bus or a
    /// Class 7–13 truck).
    pub heavy: bool,
}

impl VehicleBus {
    /// Whether the service brake is applied.
    pub fn brake_applied(&self) -> bool {
        self.a_long_mps2 <= -BRAKE_SWITCH_DECEL_MPS2
    }

    /// The turn signal the driver has on.
    pub fn turn_signal(&self) -> Option<TurnIntent> {
        let i = self.intent?;
        (i.turn != TurnIntent::Straight && i.distance_m <= TURN_SIGNAL_LEAD_M).then_some(i.turn)
    }

    /// Whether the deceleration is past J2735's hard-braking threshold.
    pub fn hard_braking(&self) -> bool {
        self.a_long_mps2 <= -j2945::hard_braking_threshold_mps2(self.heavy)
    }
}

/// What the unit keeps about its own vehicle between messages.
#[derive(Debug, Clone)]
pub struct OwnVehicle {
    bus: Option<VehicleBus>,
    history: PathHistoryBuilder,
    predictor: PathPredictor,
    /// The exterior-light bits last seen, and when they last changed.
    lights: Option<(u16, SimTime)>,
    /// The identity the path history belongs to: cleared when it changes.
    identity: Option<[u8; 8]>,
    /// When the vehicle last stood still, for the disabled-vehicle flag.
    standing_since: Option<SimTime>,
    /// The vehicle's role, for the CAM.
    role: v2xw_msg::cam::VehicleRole,
}

impl Default for OwnVehicle {
    fn default() -> Self {
        Self {
            bus: None,
            history: PathHistoryBuilder::new(PathHistoryParams::J2945_1),
            predictor: PathPredictor::new(PathPredictionParams::J2945_1),
            lights: None,
            identity: None,
            standing_since: None,
            role: v2xw_msg::cam::VehicleRole::Default,
        }
    }
}

impl OwnVehicle {
    /// The vehicle's role.
    pub fn role(&self) -> v2xw_msg::cam::VehicleRole {
        self.role
    }

    /// Sets the vehicle's role.
    pub fn set_role(&mut self, role: v2xw_msg::cam::VehicleRole) {
        self.role = role;
    }

    /// The last bus reading.
    pub fn bus(&self) -> Option<&VehicleBus> {
        self.bus.as_ref()
    }

    /// Takes a bus reading.
    pub fn set_bus(&mut self, bus: VehicleBus) {
        self.predictor.push(bus.t, bus.yaw_rate_rad_s);
        let bits = self.light_bits_of(&bus);
        match self.lights {
            Some((old, _)) if old == bits => {}
            _ => self.lights = Some((bits, bus.t)),
        }
        if bus.speed_mps < 0.1 {
            self.standing_since.get_or_insert(bus.t);
        } else {
            self.standing_since = None;
        }
        self.bus = Some(bus);
    }

    /// Takes a position fix, for the path history.
    pub fn on_fix(&mut self, belief: &PositionEstimate) {
        if !belief.fix.has_position() {
            return;
        }
        self.history.push(Breadcrumb {
            t: belief.time_ns,
            pos: belief.pos,
            heading_rad: belief.heading_rad,
            speed_mps: belief.ground_speed_mps(),
        });
    }

    /// The pseudonym the messages go out under. A new one clears the path history, so the
    /// new identity's first messages do not carry the old identity's track (J2945/1's
    /// privacy rule, recalled).
    pub fn set_identity(&mut self, id: [u8; 8]) {
        if self.identity != Some(id) {
            if self.identity.is_some() {
                self.history.clear();
            }
            self.identity = Some(id);
        }
    }

    fn light_bits_of(&self, bus: &VehicleBus) -> u16 {
        let mut bits = ExteriorLights::NONE.0;
        if bus.low_beam {
            bits |= ExteriorLights::LOW_BEAM.0;
        }
        if bus.hazard_lights {
            bits |= ExteriorLights::HAZARD.0;
        }
        match bus.turn_signal() {
            Some(TurnIntent::Left) => bits |= ExteriorLights::LEFT_TURN.0,
            Some(TurnIntent::Right) => bits |= ExteriorLights::RIGHT_TURN.0,
            _ => {}
        }
        bits
    }

    /// The J2735 brake status: every wheel braked when the brake switch is on.
    pub fn brakes(&self) -> BrakeSystemStatus {
        let Some(bus) = self.bus else {
            return BrakeSystemStatus::UNAVAILABLE;
        };
        BrakeSystemStatus {
            wheel_brakes: if bus.brake_applied() {
                BrakeAppliedStatus::ALL_WHEELS
            } else {
                BrakeAppliedStatus::NONE
            },
            ..BrakeSystemStatus::UNAVAILABLE
        }
    }

    /// The J2735 event flags set at `now`, or `None` when none is (J2735 §7.234: the
    /// element is not sent unless a flag is set).
    pub fn events(&self, now: SimTime) -> Option<VehicleEventFlags> {
        let bus = self.bus?;
        let mut f = VehicleEventFlags::NONE.0;
        if bus.hazard_lights {
            f |= VehicleEventFlags::HAZARD_LIGHTS.0;
        }
        if bus.hard_braking() {
            f |= VehicleEventFlags::HARD_BRAKING.0;
        }
        if self
            .lights
            .is_some_and(|(_, at)| now.saturating_sub(at) < j2945::LIGHTS_CHANGED_HOLD_NS && at > 0)
        {
            f |= VehicleEventFlags::LIGHTS_CHANGED.0;
        }
        if bus.hazard_lights && self.standing_since.is_some() {
            f |= VehicleEventFlags::DISABLED_VEHICLE.0;
        }
        (f != 0).then_some(VehicleEventFlags(f))
    }

    /// The exterior lights, sent when any is on or they changed in the last 2 s.
    pub fn lights(&self, now: SimTime) -> Option<ExteriorLights> {
        let (bits, at) = self.lights?;
        let recent = now.saturating_sub(at) < j2945::LIGHTS_CHANGED_HOLD_NS && at > 0;
        (bits != 0 || recent).then_some(ExteriorLights(bits))
    }

    /// The path-history points a message anchored at `belief` carries, newest first.
    pub fn history_points(&self, belief: &PositionEstimate) -> Vec<Breadcrumb> {
        self.history.points(belief.pos, belief.time_ns)
    }

    /// The BSM's `VehicleSafetyExtensions`: path history and path prediction in every
    /// message (J2945/1), events and lights when there are any.
    pub fn safety_extensions(
        &self,
        belief: &PositionEstimate,
        now: SimTime,
        origin: GeoOrigin,
    ) -> Option<VehicleSafetyExtensions> {
        if !belief.fix.has_position() {
            return None;
        }
        let points = self.history_points(belief);
        let path_history = j2945::j2735_path_history(&points, belief.pos, belief.time_ns, origin);
        let path_prediction = self
            .bus
            .map(|b| self.predictor.predict(b.speed_mps, b.yaw_rate_rad_s));
        let ext = VehicleSafetyExtensions {
            events: self.events(now),
            path_history,
            path_prediction,
            lights: self.lights(now),
        };
        (!ext.is_empty()).then_some(ext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus(t_s: f64, a: f64) -> VehicleBus {
        VehicleBus {
            t: (t_s * 1e9) as SimTime,
            speed_mps: 10.0,
            a_long_mps2: a,
            a_lat_mps2: 0.0,
            yaw_rate_rad_s: 0.0,
            hazard_lights: false,
            low_beam: false,
            intent: None,
            heavy: false,
        }
    }

    #[test]
    fn hard_braking_sets_its_flag_and_the_brake_lights() {
        let mut v = OwnVehicle::default();
        v.set_bus(bus(1.0, -4.5));
        let e = v.events(1_000_000_000).expect("an event");
        assert_ne!(e.0 & VehicleEventFlags::HARD_BRAKING.0, 0);
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::ALL_WHEELS);
        v.set_bus(bus(2.0, -3.0));
        assert!(v.events(2_000_000_000).is_none());
        // Heavy vehicles flag at 0.2 g.
        let mut heavy = bus(3.0, -3.0);
        heavy.heavy = true;
        v.set_bus(heavy);
        assert!(v.events(3_000_000_000).is_some());
    }

    #[test]
    fn a_turn_signal_shows_100_feet_before_the_turn_and_changes_the_lights() {
        let mut v = OwnVehicle::default();
        let mut b = bus(1.0, 0.0);
        b.intent = Some(OwnIntent {
            turn: TurnIntent::Left,
            distance_m: 60.0,
            junction_pos: v2xw_core::geom::Vec3::ZERO,
        });
        v.set_bus(b);
        assert!(v.lights(1_000_000_000).is_none());
        b.t = 2_000_000_000;
        b.intent = Some(OwnIntent {
            distance_m: 20.0,
            ..b.intent.expect("set")
        });
        v.set_bus(b);
        let l = v.lights(2_000_000_000).expect("the signal is on");
        assert_ne!(l.0 & ExteriorLights::LEFT_TURN.0, 0);
        let e = v.events(2_500_000_000).expect("lights changed");
        assert_ne!(e.0 & VehicleEventFlags::LIGHTS_CHANGED.0, 0);
        assert!(v.events(4_500_000_000).is_none(), "the flag holds 2 s only");
    }
}
