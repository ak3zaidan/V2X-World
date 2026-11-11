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
//! | brake hold (opt-in, `messages.bsm.brake_hold`) | below [`BRAKE_LOW_SPEED_MPS`] the switch comes on at a deceleration of [`BRAKE_LOW_DECEL_MPS2`] and stays on until the commanded acceleration turns positive, plus [`BRAKE_RELEASE_OVERLAP_S`] (cut short once it passes [`BRAKE_RELEASE_ACCEL_MPS2`]), and at any speed past [`BRAKE_HIGH_DECEL_MPS2`]; at each stop (below [`STANDSTILL_MPS`]) one keyed draw holds it while standing with probability [`BRAKE_HOLD_STOP_P`], released [`BRAKE_PULL_AWAY_OVERLAP_S`] after the acceleration turns positive ([`brake_hold_card`]) | fitted to the Orem ev64-jn8c share of BSMs with the brake on, and with the accelerometer non-negative while on, by speed band; the SM-MBR `BrakeAcceleration` rate (5.4 per ID-minute at 0.5–5 m/s, 0.86 at 0–0.5 m/s) is the held-out check |
//! | `eventHardBraking` | deceleration past 0.4 g (0.2 g heavy) | J2735 2024-09 §7.234 |
//! | `eventABSactivated` | while the anti-lock brakes regulate ([`VehicleBus::abs_active`]) | J2945/1 §6.3.6.15 `DATAACC-035`: set the ABS flag when the information is available |
//! | critical event | hard braking, ABS, traction control or stability control flag set ([`OwnVehicle::critical_event`]) | J2945/1 §3.1.2 |
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
    self, Breadcrumb, PathHistoryBuilder, PathHistoryOnChange, PathHistoryParams,
    PathPredictionParams, PathPredictor,
};

/// Deceleration beyond which the service brake is on, m/s² (see the module table).
pub const BRAKE_SWITCH_DECEL_MPS2: f64 = 0.7;

/// Speed below which the vehicle stands for the brake hold, m/s: J2735's `Speed` LSB
/// (0.02 m/s), the smallest speed a BSM can tell from zero.
pub const STANDSTILL_MPS: f64 = 0.02;

/// The source of every fitted brake-hold parameter ([`brake_hold_card`]).
pub const BRAKE_HOLD_FIT_SOURCE: &str = "fit to Orem ev64-jn8c 2025-02-02..08 \
    wheel_brakes_flags by speed band (USDOT ITS CV Pilot BSMs, data.transportation.gov)";

/// Probability that a driver holds the brake for the whole of a stop (one draw per stop,
/// [`brake_hold_card`]); fitted to the share of standstill BSMs with the brake on.
pub const BRAKE_HOLD_STOP_P: f64 = 0.57;

/// Speed below which a driver slowing down is on the brake, m/s ([`brake_hold_card`]).
pub const BRAKE_LOW_SPEED_MPS: f64 = 9.0;

/// Deceleration beyond which a driver below [`BRAKE_LOW_SPEED_MPS`] is on the brake, m/s²
/// ([`brake_hold_card`]); once on, the brake stays on until the driver's commanded
/// acceleration turns positive.
pub const BRAKE_LOW_DECEL_MPS2: f64 = 0.6;

/// Deceleration beyond which the brake is on at any speed with the brake-hold model on,
/// m/s² ([`brake_hold_card`]); it replaces [`BRAKE_SWITCH_DECEL_MPS2`] in the model, and
/// there is no latch above [`BRAKE_LOW_SPEED_MPS`].
pub const BRAKE_HIGH_DECEL_MPS2: f64 = 1.0;

/// How long the brake held at a stop stays on after the commanded acceleration turns
/// positive, s ([`brake_hold_card`]): the pedal overlap of a driver pulling away.
pub const BRAKE_PULL_AWAY_OVERLAP_S: f64 = 0.6;

/// How long the brake put on while slowing stays on after the commanded acceleration
/// turns positive, s ([`brake_hold_card`]): the pedal overlap of a driver who slowed
/// without stopping.
pub const BRAKE_RELEASE_OVERLAP_S: f64 = 0.2;

/// Commanded acceleration beyond which the brake is off at once, m/s² ([`brake_hold_card`]):
/// a driver whose car accelerates this hard has the foot on the throttle, not the brake.
/// The pedal overlaps ([`BRAKE_PULL_AWAY_OVERLAP_S`], [`BRAKE_RELEASE_OVERLAP_S`]) run only
/// while the acceleration is positive but under it. Without it the overlaps kept the brake
/// on at clearly positive accelerations, so SM-MBR `BrakeAcceleration` fired with a far
/// above zero (a 0.5 m/s² margin cleared 5 % of the engine's triggers, 72 % of Orem's;
/// K3 Addendum 3). **`todo-calibrate`**: 0.15 m/s², inside the 0.1-0.2 m/s² of a
/// level-road coast-to-throttle transition, not fitted.
pub const BRAKE_RELEASE_ACCEL_MPS2: f64 = 0.15;

/// The keyed stream the per-stop brake-hold draw comes from ([`brake_hold_card`]).
pub const BRAKE_HOLD_STREAM: &str = "node/vehicle/brake-hold";

/// The model id of the brake-hold model ([`brake_hold_card`]).
pub const BRAKE_HOLD_ID: &str = "node/vehicle/brake-hold";

/// The model card of the opt-in brake-hold model (`messages.bsm.brake_hold`): the driver's
/// foot on the service brake at low speed and at stops, released on pulling away.
pub fn brake_hold_card() -> v2xw_core::card::ModelCard {
    use v2xw_core::card::{Family, ModelCard, Parameter, Source, SourceKind};
    let mut card = ModelCard::new(
        BRAKE_HOLD_ID,
        Family::Generator,
        "0.3.0",
        "The brake switch comes on below a low speed when the driver decelerates beyond a \
         small threshold and stays on until the commanded acceleration turns positive, plus \
         a short pedal overlap, and at any speed beyond a larger threshold; at each stop one \
         keyed draw decides whether the driver holds the brake while standing, released a \
         pull-away overlap after the acceleration turns positive; a commanded acceleration \
         beyond a small release threshold takes the brake off at once.",
    );
    let fit = || Source::new(SourceKind::Dataset, BRAKE_HOLD_FIT_SOURCE);
    let mut push = |name: &str, unit: &str, v: f64, lo: f64, hi: f64| {
        let mut p = Parameter::new(name, unit, serde_json::json!(v), fit());
        p.range = Some(vec![serde_json::json!(lo), serde_json::json!(hi)]);
        card.parameters.push(p);
    };
    push("stop_hold_p", "1", BRAKE_HOLD_STOP_P, 0.0, 1.0);
    push("low_speed_mps", "m/s", BRAKE_LOW_SPEED_MPS, 0.0, 20.0);
    push(
        "low_speed_decel_mps2",
        "m/s^2",
        BRAKE_LOW_DECEL_MPS2,
        0.0,
        0.7,
    );
    push(
        "high_speed_decel_mps2",
        "m/s^2",
        BRAKE_HIGH_DECEL_MPS2,
        0.5,
        3.0,
    );
    push(
        "pull_away_overlap_s",
        "s",
        BRAKE_PULL_AWAY_OVERLAP_S,
        0.0,
        2.0,
    );
    push("release_overlap_s", "s", BRAKE_RELEASE_OVERLAP_S, 0.0, 2.0);
    let mut r = Parameter::new(
        "release_accel_mps2",
        "m/s^2",
        serde_json::json!(BRAKE_RELEASE_ACCEL_MPS2),
        Source::new(
            SourceKind::TodoCalibrate,
            "design value: a driver accelerating beyond it is on the throttle; Orem \
             ev64-jn8c BSMs with the brake on have a near-zero accelerometer",
        ),
    );
    r.range = Some(vec![serde_json::json!(0.0), serde_json::json!(0.5)]);
    r.calibration = Some(
        "Fit to the Orem ev64-jn8c distribution of accel_long while the brake is on, by \
         speed band (P(a >= x | brake on)), jointly with the two overlaps so that \
         P(brake on | band) keeps its fitted values; the SM-MBR BrakeAcceleration rate \
         with and without a 0.5 m/s^2 margin is the held-out check."
            .to_string(),
    );
    card.parameters.push(r);
    let mut s = Parameter::new(
        "standstill_mps",
        "m/s",
        serde_json::json!(STANDSTILL_MPS),
        Source::new(
            SourceKind::Standard,
            "SAE J2735 2024-09 DE_Speed: LSB 0.02 m/s",
        ),
    );
    s.range = Some(vec![serde_json::json!(0.0), serde_json::json!(0.02)]);
    card.parameters.push(s);
    card.sources.push(Source::new(
        SourceKind::Dataset,
        "USDOT ITS DataHub Utah (Orem) connected-vehicle BSMs, data.transportation.gov \
         ev64-jn8c, 2025-02-02..08: P(brake on) and P(accel >= 0 | brake on) by speed band",
    ));
    card
}

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
    /// The speed limit of the road it is on, m/s, from its navigation map; zero when the
    /// map has none.
    pub speed_limit_mps: f64,
    /// Whether its navigation map puts the road it is on outside the urban environment
    /// (a motorway or trunk road); `None` when the map cannot say (a junction's interior,
    /// a slip road, an unclassified road). The C2C-CC traffic-condition preconditions read
    /// it ([`crate::events`]).
    pub non_urban: Option<bool>,
    /// Whether the anti-lock braking system is regulating: the vehicle is braking at the
    /// tyre-road adhesion limit of a slippery surface (its own wheel-speed sensors; the
    /// C2C-CC traction-loss trigger reads it, [`crate::events`]).
    pub abs_active: bool,
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
    /// The identity the messages go out under.
    identity: Option<[u8; 8]>,
    /// What a change of `identity` does to the path history.
    on_change: PathHistoryOnChange,
    /// When the vehicle last stood still, for the disabled-vehicle flag.
    standing_since: Option<SimTime>,
    /// The vehicle's role, for the CAM.
    role: v2xw_msg::cam::VehicleRole,
    /// Whether the brake-hold model is on ([`brake_hold_card`]).
    brake_hold: bool,
    /// Whether the model has the brake on now.
    held: bool,
    /// When the brake comes off: set once the commanded acceleration turns positive.
    release_at: Option<SimTime>,
    /// Whether the current stop's hold draw has been taken.
    stop_decided: bool,
    /// Whether the brake now on is the one held at a stop (its release takes the
    /// pull-away overlap).
    from_stop: bool,
    /// The keyed uniform draw for the next stop ([`BRAKE_HOLD_STREAM`]), supplied by the
    /// runtime ([`OwnVehicle::supply_stop_draw`]).
    stop_draw: Option<f64>,
}

impl Default for OwnVehicle {
    fn default() -> Self {
        Self {
            bus: None,
            history: PathHistoryBuilder::new(PathHistoryParams::J2945_1),
            predictor: PathPredictor::new(PathPredictionParams::J2945_1),
            lights: None,
            identity: None,
            on_change: PathHistoryOnChange::Clear,
            standing_since: None,
            role: v2xw_msg::cam::VehicleRole::Default,
            brake_hold: false,
            held: false,
            release_at: None,
            stop_decided: false,
            from_stop: false,
            stop_draw: None,
        }
    }
}

impl OwnVehicle {
    /// This vehicle with its messages' path history capped at `max_points`.
    #[must_use]
    pub fn with_path_history_points(mut self, max_points: usize) -> Self {
        let params = PathHistoryParams {
            max_points,
            ..*self.history.params()
        };
        self.history = PathHistoryBuilder::new(params);
        self
    }

    /// This vehicle with a pseudonym change doing `on_change` to its path history.
    ///
    /// [`OwnVehicle::default`] clears, the behaviour before J2945/1 conformance work,
    /// so a station whose profile does not choose (an ETSI station's CAM low-frequency
    /// container and DENM trace) is unchanged. A J2945/1 BSM sender is built with
    /// `messages.bsm.path_history_on_change`, whose default is
    /// [`PathHistoryOnChange::Keep`].
    #[must_use]
    pub fn with_path_history_on_change(mut self, on_change: PathHistoryOnChange) -> Self {
        self.on_change = on_change;
        self
    }

    /// This vehicle with the brake-hold model on or off (`messages.bsm.brake_hold`; off in
    /// [`OwnVehicle::default`]).
    #[must_use]
    pub fn with_brake_hold(mut self, on: bool) -> Self {
        self.brake_hold = on;
        self
    }

    /// Whether the brake-hold model waits for its next per-stop draw
    /// ([`OwnVehicle::supply_stop_draw`]); never with the model off.
    pub fn needs_stop_draw(&self) -> bool {
        self.brake_hold && self.stop_draw.is_none()
    }

    /// Hands the brake-hold model the uniform `[0, 1)` draw its next stop is decided by,
    /// from the node's keyed stream [`BRAKE_HOLD_STREAM`].
    pub fn supply_stop_draw(&mut self, u: f64) {
        self.stop_draw = Some(u);
    }

    /// The brake-hold model's step ([`brake_hold_card`]).
    fn step_brake_hold(&mut self, bus: &VehicleBus) {
        let (v, a) = (bus.speed_mps, bus.a_long_mps2);
        if v < STANDSTILL_MPS {
            if !self.stop_decided {
                if let Some(u) = self.stop_draw.take() {
                    self.stop_decided = true;
                    self.held = u < BRAKE_HOLD_STOP_P;
                    self.from_stop = self.held;
                    self.release_at = None;
                }
            }
        } else {
            self.stop_decided = false;
            if v < BRAKE_LOW_SPEED_MPS && a <= -BRAKE_LOW_DECEL_MPS2 {
                self.held = true;
                self.from_stop = false;
                self.release_at = None;
            }
        }
        if self.held {
            if a > BRAKE_RELEASE_ACCEL_MPS2 {
                self.held = false;
                self.release_at = None;
            } else if a > 0.0 {
                let overlap_s = if self.from_stop {
                    BRAKE_PULL_AWAY_OVERLAP_S
                } else {
                    BRAKE_RELEASE_OVERLAP_S
                };
                let overlap = (overlap_s * 1e9).round() as SimTime;
                let at = *self.release_at.get_or_insert(bus.t.saturating_add(overlap));
                if bus.t >= at {
                    self.held = false;
                    self.release_at = None;
                }
            } else {
                self.release_at = None;
            }
        }
    }

    /// What a pseudonym change does to the path history.
    pub fn path_history_on_change(&self) -> PathHistoryOnChange {
        self.on_change
    }

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
        // The first reading is the lights' state, not a change of it: `at` 0 marks "never
        // changed", so `eventLightsChanged` is not raised by the unit starting up.
        match self.lights {
            Some((old, _)) if old == bits => {}
            None => self.lights = Some((bits, 0)),
            Some(_) => self.lights = Some((bits, bus.t)),
        }
        if bus.speed_mps < 0.1 {
            self.standing_since.get_or_insert(bus.t);
        } else {
            self.standing_since = None;
        }
        if self.brake_hold {
            self.step_brake_hold(&bus);
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

    /// The pseudonym the messages go out under. A new one keeps the path history under
    /// [`PathHistoryOnChange::Keep`] (J2945/1 has no clearing rule, and §6.3.6.16
    /// `DATAACC-039` asks for 200 m of history except initially), and clears it under
    /// [`PathHistoryOnChange::Clear`], so the new identity's first messages do not carry
    /// the old identity's track.
    pub fn set_identity(&mut self, id: [u8; 8]) {
        if self.identity != Some(id) {
            if self.identity.is_some() && self.on_change == PathHistoryOnChange::Clear {
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

    /// Whether the brake switch is on: past [`BRAKE_SWITCH_DECEL_MPS2`], or with the
    /// brake-hold model on, held or past [`BRAKE_HIGH_DECEL_MPS2`].
    fn brake_switch(&self, bus: &VehicleBus) -> bool {
        if self.brake_hold {
            self.held || bus.a_long_mps2 <= -BRAKE_HIGH_DECEL_MPS2
        } else {
            bus.brake_applied()
        }
    }

    /// The J2735 brake status: every wheel braked when the brake switch is on.
    pub fn brakes(&self) -> BrakeSystemStatus {
        let Some(bus) = self.bus else {
            return BrakeSystemStatus::UNAVAILABLE;
        };
        BrakeSystemStatus {
            wheel_brakes: if self.brake_switch(&bus) {
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
        if bus.abs_active {
            f |= VehicleEventFlags::ABS_ACTIVATED.0;
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

    /// Whether a J2945/1 critical event flag is set at `now`: hard braking, ABS, traction
    /// control or stability control (§3.1.2). The vehicle bus reports no traction-control
    /// or stability-control activity, so those two bits are never set today.
    pub fn critical_event(&self, now: SimTime) -> bool {
        const CRITICAL: u16 = VehicleEventFlags::HARD_BRAKING.0
            | VehicleEventFlags::ABS_ACTIVATED.0
            | VehicleEventFlags::TRACTION_CONTROL_LOSS.0
            | VehicleEventFlags::STABILITY_CONTROL_ACTIVATED.0;
        self.events(now).is_some_and(|f| f.0 & CRITICAL != 0)
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

    /// The path-history points a BSM anchored at `belief` carries, newest first: never
    /// none ([`PathHistoryBuilder::bsm_points`], J2945/1 `BSMCONT-003/004`).
    pub fn bsm_history_points(&self, belief: &PositionEstimate) -> Vec<Breadcrumb> {
        self.history.bsm_points(belief.pos, belief.time_ns)
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
        let points = self.bsm_history_points(belief);
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
            speed_limit_mps: 13.4,
            non_urban: None,
            abs_active: false,
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

    fn moving(t_s: f64, v: f64, a: f64) -> VehicleBus {
        VehicleBus {
            speed_mps: v,
            ..bus(t_s, a)
        }
    }

    /// Brake hold on, a stop whose draw holds the brake: on while standing with the
    /// accelerometer at zero, still on during the pedal overlap after the commanded
    /// acceleration turns positive, and released after it. Fault: drop `self.held ||` in
    /// `brake_switch` -> the standstill assertion is red; drop the `a > 0.0` release -> the
    /// released assertion is red.
    #[test]
    fn brake_hold_is_released_when_accelerating() {
        let mut v = OwnVehicle::default().with_brake_hold(true);
        assert!(v.needs_stop_draw());
        v.supply_stop_draw(0.0);
        assert!(!v.needs_stop_draw());
        v.set_bus(moving(1.0, 8.0, 0.3));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::NONE);
        v.set_bus(moving(2.0, 0.0, 0.0));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::ALL_WHEELS);
        assert!(v.needs_stop_draw(), "the stop took its draw");
        v.set_bus(moving(30.0, 0.0, 0.0));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::ALL_WHEELS);
        v.set_bus(moving(30.1, 0.1, 0.1));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::ALL_WHEELS);
        v.set_bus(moving(30.1 + BRAKE_PULL_AWAY_OVERLAP_S, 0.9, 0.1));
        assert_eq!(
            v.brakes().wheel_brakes,
            BrakeAppliedStatus::NONE,
            "released"
        );
        // Slowing gently below the low speed puts it on again, and it stays on while
        // coasting until the acceleration turns positive.
        v.set_bus(moving(40.0, 3.0, -BRAKE_LOW_DECEL_MPS2));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::ALL_WHEELS);
        v.set_bus(moving(40.5, 2.9, -0.05));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::ALL_WHEELS);
        v.set_bus(moving(41.0, 2.9, 0.4));
        v.set_bus(moving(42.0, 3.2, 0.4));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::NONE);
        // Above the low speed the same gentle slowing is engine drag, not the brake.
        v.set_bus(moving(
            50.0,
            BRAKE_LOW_SPEED_MPS + 1.0,
            -BRAKE_LOW_DECEL_MPS2,
        ));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::NONE);
        let card = brake_hold_card();
        assert!(card.validate().is_ok());
        assert_eq!(card.todo_calibrate().count(), 1);
        assert!(card.parameters.iter().any(|p| p.name == "stop_hold_p"));
    }

    /// A commanded acceleration beyond [`BRAKE_RELEASE_ACCEL_MPS2`] takes the brake off at
    /// once, inside either pedal overlap: no brake on with the car clearly accelerating.
    /// Fault: drop the `a > BRAKE_RELEASE_ACCEL_MPS2` branch in `step_brake_hold` -> both
    /// asserts are red (the overlap keeps the brake on).
    #[test]
    fn brake_hold_is_off_at_once_when_clearly_accelerating() {
        let mut v = OwnVehicle::default().with_brake_hold(true);
        v.supply_stop_draw(0.0);
        v.set_bus(moving(2.0, 0.0, 0.0));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::ALL_WHEELS);
        v.set_bus(moving(10.0, 0.05, BRAKE_RELEASE_ACCEL_MPS2 + 0.3));
        assert_eq!(
            v.brakes().wheel_brakes,
            BrakeAppliedStatus::NONE,
            "pull-away overlap cut"
        );
        v.set_bus(moving(20.0, 3.0, -BRAKE_LOW_DECEL_MPS2));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::ALL_WHEELS);
        v.set_bus(moving(20.1, 3.0, BRAKE_RELEASE_ACCEL_MPS2 + 0.3));
        assert_eq!(
            v.brakes().wheel_brakes,
            BrakeAppliedStatus::NONE,
            "release overlap cut"
        );
    }

    /// The per-stop draw decides the standstill hold: a draw at or above
    /// [`BRAKE_HOLD_STOP_P`] leaves the brake off for the whole stop, one below it holds
    /// it, and a stop takes one draw only. Fault: compare with `>` in `step_brake_hold` ->
    /// both assertions are red.
    #[test]
    fn brake_hold_at_a_standstill_follows_the_per_stop_draw() {
        let mut v = OwnVehicle::default().with_brake_hold(true);
        v.supply_stop_draw(BRAKE_HOLD_STOP_P);
        v.set_bus(moving(1.0, 0.0, 0.0));
        v.supply_stop_draw(0.0);
        v.set_bus(moving(2.0, 0.0, 0.0));
        assert_eq!(
            v.brakes().wheel_brakes,
            BrakeAppliedStatus::NONE,
            "one draw per stop"
        );
        v.set_bus(moving(3.0, 6.0, 1.0));
        v.set_bus(moving(4.0, 0.0, 0.0));
        assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::ALL_WHEELS);
    }

    /// The model's brake sequence over many stops is a function of the node's keyed
    /// stream: two runs give the same sequence, and it holds at some stops and not at
    /// others. Fault: draw from a stream keyed by a varying entity -> the first assertion
    /// is red (with overwhelming probability).
    #[test]
    fn brake_hold_is_deterministic_across_runs() {
        use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
        let run = || {
            let mut rng = RngStream::derive(
                7,
                RngDomain::plugin(BRAKE_HOLD_STREAM),
                EntityRef::Node(v2xw_core::ids::NodeId::new(3)),
            );
            let mut v = OwnVehicle::default().with_brake_hold(true);
            let mut out = Vec::new();
            for k in 0..64 {
                if v.needs_stop_draw() {
                    v.supply_stop_draw(rng.f64());
                }
                let t = f64::from(k) * 10.0;
                v.set_bus(moving(t, 0.0, 0.0));
                out.push(v.brakes().wheel_brakes != BrakeAppliedStatus::NONE);
                v.set_bus(moving(t + 5.0, 8.0, 1.0));
            }
            out
        };
        let (a, b) = (run(), run());
        assert_eq!(a, b);
        assert!(a.iter().any(|&h| h) && a.iter().any(|&h| !h));
    }

    /// Brake hold off (the default): standing and slow driving leave the switch off.
    #[test]
    fn brake_hold_off_leaves_the_switch_to_deceleration() {
        let mut v = OwnVehicle::default();
        for (t, s) in [(1.0, 0.0), (2.0, 1.0), (3.0, 0.0)] {
            v.set_bus(moving(t, s, 0.2));
            assert_eq!(v.brakes().wheel_brakes, BrakeAppliedStatus::NONE);
        }
    }

    fn fix(t_s: f64, x: f64) -> PositionEstimate {
        let mut p = PositionEstimate::no_fix((t_s * 1e9) as SimTime);
        p.pos = v2xw_core::geom::Vec3::new(x, 0.0, 0.0);
        p.vel = v2xw_core::geom::Vec3::new(10.0, 0.0, 0.0);
        p.fix = v2xw_core::belief::FixQuality::ThreeD;
        p
    }

    /// A 400 m drive, then a pseudonym change: J2945/1's vehicle keeps its history.
    fn drive_and_change(on_change: PathHistoryOnChange) -> (Vec<Breadcrumb>, Vec<Breadcrumb>) {
        let mut v = OwnVehicle::default().with_path_history_on_change(on_change);
        v.set_identity([1; 8]);
        for i in 0..=400 {
            v.on_fix(&fix(f64::from(i) * 0.1, f64::from(i)));
        }
        let at = fix(40.1, 401.0);
        let before = v.history_points(&at);
        v.set_identity([2; 8]);
        (before, v.history_points(&at))
    }

    #[test]
    fn a_pseudonym_change_keeps_the_path_history() {
        assert_eq!(PathHistoryOnChange::default(), PathHistoryOnChange::Keep);
        let (before, after) = drive_and_change(PathHistoryOnChange::default());
        let covered = 401.0 - before.last().expect("a history").pos.x;
        assert!(covered >= 200.0, "covers {covered} m before the change");
        assert_eq!(after, before, "the change dropped the history");
        // The research option starts again.
        let (before, after) = drive_and_change(PathHistoryOnChange::Clear);
        assert!(!before.is_empty());
        assert!(
            after.is_empty(),
            "{} points after a clearing change",
            after.len()
        );
    }

    /// The first BSM, at the first fix, carries a `PathHistory` (J2945/1 `BSMCONT-003/004`).
    #[test]
    fn the_first_bsm_carries_a_path_history() {
        let origin = GeoOrigin::new(40.75, -73.99, 0.0);
        let mut v = OwnVehicle::default();
        v.set_bus(bus(1.0, 0.0));
        let b = fix(1.0, 0.0);
        v.on_fix(&b);
        let ext = v
            .safety_extensions(&b, b.time_ns, origin)
            .expect("extensions");
        let ph = ext.path_history.expect("the first BSM has no PathHistory");
        assert_eq!(ph.crumb_data.len(), 1);
    }

    #[test]
    fn abs_activity_sets_the_abs_flag() {
        let mut v = OwnVehicle::default();
        let mut b = bus(1.0, -2.0);
        b.abs_active = true;
        v.set_bus(b);
        let e = v.events(1_000_000_000).expect("an event");
        assert_ne!(e.0 & VehicleEventFlags::ABS_ACTIVATED.0, 0, "{e:?}");
        assert_eq!(e.0 & VehicleEventFlags::HARD_BRAKING.0, 0);
        assert!(v.critical_event(1_000_000_000), "ABS is a critical event");
        b.abs_active = false;
        b.t = 2_000_000_000;
        v.set_bus(b);
        assert!(v.events(2_000_000_000).is_none());
        assert!(!v.critical_event(2_000_000_000));
        // Hard braking alone is critical too; the hazard lights are not.
        v.set_bus(bus(3.0, -4.5));
        assert!(v.critical_event(3_000_000_000));
        let mut h = bus(4.0, 0.0);
        h.hazard_lights = true;
        v.set_bus(h);
        assert!(!v.critical_event(4_000_000_000));
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
