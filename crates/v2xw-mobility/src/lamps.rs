//! Exterior lamps: what a vehicle's stop lamps, direction indicators and warning beacons show,
//! decided from the state the mobility model already holds (vwp-v1 §3.3.5).
//!
//! The viewer used to draw every car dark. A queue at a red light with no brake lamps, a car
//! swinging left across a junction with no indicator, and a fire engine with its beacons off
//! are three things nobody who has stood at a Manhattan corner would believe, and each of them
//! follows from facts this crate has — the longitudinal acceleration, the route's next
//! movement, the lane change in progress, the vehicle class. This module turns those facts into
//! the `lamps` byte the stream carries; the headlamps are the engine's, because they depend on
//! the time of day and the weather (`v2xw_engine::daylight`).
//!
//! # The rules, and where each number comes from
//!
//! * **Stop lamps** light when the driver is on the service brake. A car in gear that lifts
//!   off the throttle decelerates without it — rolling resistance, aerodynamic drag and engine
//!   braking, up to about 1 m/s² at urban speeds — so a deceleration is only a brake
//!   application beyond [`LampParams::brake_on_decel_mps2`], with hysteresis down to
//!   [`LampParams::brake_off_decel_mps2`] so a car hovering at the threshold does not flash
//!   its lamps. The coast-down figure is a physical reading, not a measurement from one
//!   source, so both thresholds are parameters and the defaults are stated as a choice.
//!   A vehicle standing still is held on the brake — an automatic transmission creeps
//!   otherwise, and automatics are nearly the whole US fleet — so its lamps are lit at a
//!   standstill ([`LampParams::hold_speed_mps`]).
//! * **Direction indicators** operate "continuously during not less than the last one hundred
//!   feet traveled by the vehicle before turning" (New York Vehicle and Traffic Law §1163(b);
//!   California Vehicle Code §22108 says the same), and through the turn itself; and during a
//!   lane change (NY VTL §1163(a), §1128). Not every driver does it: in the largest field
//!   study, 12,000 turning vehicles observed in Ohio, 25 % of turns and 48 % of lane changes
//!   were made without signalling (Ponziani, R., "Turn Signal Usage Rate Results: A
//!   Comprehensive Field Study of 12,000 Observed Turning Vehicles", SAE Technical Paper
//!   2012-01-0261). Each driver's habit is drawn once, from its own keyed stream: whether they
//!   signal their turns, and whether they signal their lane changes. A slight left or right —
//!   bearing onto the branch of a fork — is not a turn in the statutes' sense and is not
//!   signalled.
//! * **Warning beacons** (J2735 `LightbarInUse`): an emergency-class vehicle is taken to be
//!   responding, because a scenario that puts one on the road does so to exercise the
//!   emergency-vehicle applications; [`LampParams::emergency_beacons`] turns that off.
//! * People on foot, bicycles and e-scooters carry no lamps the stream reports.
//!
//! Hazard warning and reversing lamps have bits in the byte (J2735 `hazardSignalOn`, and the
//! reversing lamp) but nothing in this build produces them: no vehicle breaks down, double-parks
//! or reverses. The bit is clear because the condition cannot arise, which is said here rather
//! than left for a reader to wonder.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_world::TurnDirection;

use crate::classes::VehicleClass;

/// Stop lamps lit.
pub const LAMP_BRAKE: u8 = 0x01;
/// Left direction indicator operating.
pub const LAMP_TURN_LEFT: u8 = 0x02;
/// Right direction indicator operating.
pub const LAMP_TURN_RIGHT: u8 = 0x04;
/// Hazard warning (both indicators). Not produced by this build.
pub const LAMP_HAZARD: u8 = 0x08;
/// Dipped headlamps. Set by the engine from the time of day and the weather.
pub const LAMP_LOW_BEAM: u8 = 0x10;
/// Reversing lamps. Not produced by this build.
pub const LAMP_REVERSE: u8 = 0x20;
/// An emergency vehicle's warning beacons.
pub const LAMP_EMERGENCY: u8 = 0x40;

/// The lamp model's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LampParams {
    /// Deceleration from which the driver is on the service brake, m/s² (a magnitude).
    pub brake_on_decel_mps2: f64,
    /// Deceleration below which a braking driver is off the brake again, m/s².
    pub brake_off_decel_mps2: f64,
    /// At or below this speed a stopped vehicle is held on the brake, m/s.
    pub hold_speed_mps: f64,
    /// How far before a turn the indicator is on, metres (100 ft, NY VTL §1163(b)).
    pub indicator_distance_m: f64,
    /// Share of drivers who signal their turns (Ponziani 2012: 75 %).
    pub turn_signal_use: f64,
    /// Share of drivers who signal their lane changes (Ponziani 2012: 52 %).
    pub lane_change_signal_use: f64,
    /// Whether an emergency-class vehicle runs its beacons.
    pub emergency_beacons: bool,
}

impl Default for LampParams {
    fn default() -> Self {
        LampParams {
            brake_on_decel_mps2: 1.0,
            brake_off_decel_mps2: 0.6,
            hold_speed_mps: 0.3,
            indicator_distance_m: 30.48,
            turn_signal_use: 0.75,
            lane_change_signal_use: 0.52,
            emergency_beacons: true,
        }
    }
}

/// One driver's signalling habit, drawn once per vehicle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalHabit {
    /// Signals turns.
    pub turns: bool,
    /// Signals lane changes.
    pub lane_changes: bool,
}

impl SignalHabit {
    /// A driver who always signals, for tests and for a model with no heterogeneity.
    pub const ALWAYS: SignalHabit = SignalHabit {
        turns: true,
        lane_changes: true,
    };
}

/// What the lamp rules need to know about one vehicle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LampInput {
    /// The vehicle class.
    pub class: VehicleClass,
    /// Speed along the lane, m/s.
    pub speed_mps: f64,
    /// Longitudinal acceleration, m/s² (negative when slowing).
    pub accel_mps2: f64,
    /// A lane change in progress: `Some(true)` to the left, `Some(false)` to the right.
    pub lane_change_left: Option<bool>,
    /// The next turn on the route and how far ahead it starts, metres.
    pub next_turn: Option<(TurnDirection, f64)>,
    /// The turn the vehicle is making now (it is on a junction connector).
    pub turning: Option<TurnDirection>,
}

/// The indicator a movement calls for: `Some(true)` left, `Some(false)` right, `None` for
/// straight on or a slight bear.
pub fn indicator_for(direction: TurnDirection) -> Option<bool> {
    match direction {
        TurnDirection::Left | TurnDirection::UTurn => Some(true),
        TurnDirection::Right => Some(false),
        TurnDirection::Straight | TurnDirection::SlightLeft | TurnDirection::SlightRight => None,
    }
}

/// Whether a class carries the lamps this module reports.
pub fn has_lamps(class: VehicleClass) -> bool {
    !matches!(
        class,
        VehicleClass::Pedestrian | VehicleClass::Bicycle | VehicleClass::Scooter
    )
}

/// The lamps one vehicle shows this step, given what it showed last step.
///
/// Headlamps ([`LAMP_LOW_BEAM`]) are not decided here; the engine ORs them in.
pub fn lamps(input: &LampInput, previous: u8, habit: SignalHabit, p: &LampParams) -> u8 {
    if !has_lamps(input.class) {
        return 0;
    }
    let mut bits = 0u8;

    // Stop lamps.
    let decel = -input.accel_mps2;
    let standing = input.speed_mps <= p.hold_speed_mps && input.accel_mps2 <= 0.25;
    let was_braking = previous & LAMP_BRAKE != 0;
    let braking = if was_braking {
        decel > p.brake_off_decel_mps2
    } else {
        decel >= p.brake_on_decel_mps2
    };
    if standing || braking {
        bits |= LAMP_BRAKE;
    }

    // Direction indicators: the manoeuvre under way first, then the turn ahead.
    let side = if let Some(left) = input.lane_change_left {
        habit.lane_changes.then_some(left)
    } else if let Some(d) = input.turning.and_then(indicator_for) {
        habit.turns.then_some(d)
    } else if let Some((d, dist)) = input.next_turn {
        indicator_for(d).filter(|_| habit.turns && dist <= p.indicator_distance_m)
    } else {
        None
    };
    match side {
        Some(true) => bits |= LAMP_TURN_LEFT,
        Some(false) => bits |= LAMP_TURN_RIGHT,
        None => {}
    }

    if input.class == VehicleClass::Emergency && p.emergency_beacons {
        bits |= LAMP_EMERGENCY;
    }
    bits
}

/// The lamp model's card.
pub fn card(p: &LampParams) -> ModelCard {
    let statute = |r: &str| Source {
        kind: SourceKind::Standard,
        reference: r.to_string(),
        accessed: Some("2026-09-30".to_string()),
        note: None,
    };
    let ponziani = Source {
        kind: SourceKind::Paper,
        reference: "Ponziani, R., Turn Signal Usage Rate Results: A Comprehensive Field Study \
                    of 12,000 Observed Turning Vehicles, SAE Technical Paper 2012-01-0261"
            .to_string(),
        accessed: Some("2026-09-30".to_string()),
        note: Some("25 % of turns and 48 % of lane changes made without signalling".to_string()),
    };
    let choice = |why: &str| Source {
        kind: SourceKind::TodoCalibrate,
        reference: "implementer's choice".to_string(),
        accessed: None,
        note: Some(why.to_string()),
    };
    let calibrate = |mut prm: Parameter, plan: &str| {
        prm.calibration = Some(plan.to_string());
        prm
    };
    let mut card = ModelCard::new(
        "mobility/lamps/exterior",
        Family::Mobility,
        "1.0.0",
        "The exterior lamps a vehicle shows (vwp-v1 §3.3.5): stop lamps while the driver is \
         on the service brake or holding the vehicle at a standstill, direction indicators \
         for the last 100 ft before a turn, through it and during a lane change — for the \
         drivers who signal — and an emergency vehicle's beacons.",
    );
    card.tier = vec![Tier::Medium];
    card.parameters = vec![
        calibrate(
            Parameter::new(
                "brake_on_decel_mps2",
                "m/s²",
                serde_json::json!(p.brake_on_decel_mps2),
                choice(
                    "coast-down in gear (rolling resistance, drag, engine braking) reaches \
                     about 1 m/s² at urban speeds; harder needs the service brake",
                ),
            ),
            "compare with brake-pedal traces from a naturalistic driving study (SHRP 2)",
        ),
        calibrate(
            Parameter::new(
                "brake_off_decel_mps2",
                "m/s²",
                serde_json::json!(p.brake_off_decel_mps2),
                choice("hysteresis below the on threshold, so a hovering car's lamp does not flash"),
            ),
            "as brake_on_decel_mps2",
        ),
        Parameter::new(
            "hold_speed_mps",
            "m/s",
            serde_json::json!(p.hold_speed_mps),
            statute("FMVSS 108 stop lamps; a standing automatic is held on the brake"),
        ),
        Parameter::new(
            "indicator_distance_m",
            "m",
            serde_json::json!(p.indicator_distance_m),
            statute(
                "NY Vehicle and Traffic Law §1163(b); California Vehicle Code §22108: the \
                 last 100 ft before turning",
            ),
        ),
        Parameter::new(
            "turn_signal_use",
            "-",
            serde_json::json!(p.turn_signal_use),
            ponziani.clone(),
        ),
        Parameter::new(
            "lane_change_signal_use",
            "-",
            serde_json::json!(p.lane_change_signal_use),
            ponziani.clone(),
        ),
        Parameter::new(
            "emergency_beacons",
            "-",
            serde_json::json!(p.emergency_beacons),
            statute("SAE J2735 (2016) DE_LightbarInUse"),
        ),
    ];
    card.assumptions = vec![
        "A driver's signalling habit is fixed for the trip: one draw for turns, one for \
         lane changes."
            .to_string(),
        "An emergency-class vehicle on the road is responding.".to_string(),
    ];
    card.ignores = vec![
        "Hazard warning and reversing lamps: nothing in this build breaks down, \
         double-parks or reverses."
            .to_string(),
        "Headlamps are the engine's (time of day and weather), not this model's.".to_string(),
    ];
    card.sources = vec![
        statute("NY Vehicle and Traffic Law §1163 Turning movements and required signals"),
        statute("SAE J2735 (2016) DE_ExteriorLights, DF_BrakeSystemStatus"),
        ponziani,
    ];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec!["lamps::tests".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(speed: f64, accel: f64) -> LampInput {
        LampInput {
            class: VehicleClass::Passenger,
            speed_mps: speed,
            accel_mps2: accel,
            lane_change_left: None,
            next_turn: None,
            turning: None,
        }
    }

    #[test]
    fn a_car_standing_at_a_light_shows_its_stop_lamps() {
        let p = LampParams::default();
        assert_eq!(lamps(&car(0.0, 0.0), 0, SignalHabit::ALWAYS, &p) & LAMP_BRAKE, LAMP_BRAKE);
        // Moving off: accelerating from a standstill is off the brake.
        assert_eq!(lamps(&car(0.1, 1.5), LAMP_BRAKE, SignalHabit::ALWAYS, &p) & LAMP_BRAKE, 0);
    }

    #[test]
    fn coasting_is_not_braking_and_the_threshold_has_hysteresis() {
        let p = LampParams::default();
        assert_eq!(lamps(&car(12.0, -0.5), 0, SignalHabit::ALWAYS, &p), 0);
        assert_eq!(lamps(&car(12.0, -1.2), 0, SignalHabit::ALWAYS, &p), LAMP_BRAKE);
        // Between the two thresholds the lamp keeps what it showed.
        assert_eq!(lamps(&car(12.0, -0.8), LAMP_BRAKE, SignalHabit::ALWAYS, &p), LAMP_BRAKE);
        assert_eq!(lamps(&car(12.0, -0.8), 0, SignalHabit::ALWAYS, &p), 0);
        assert_eq!(lamps(&car(12.0, -0.4), LAMP_BRAKE, SignalHabit::ALWAYS, &p), 0);
    }

    #[test]
    fn the_indicator_runs_for_the_last_hundred_feet_and_through_the_turn() {
        let p = LampParams::default();
        let mut c = car(8.0, 0.0);
        c.next_turn = Some((TurnDirection::Left, 45.0));
        assert_eq!(lamps(&c, 0, SignalHabit::ALWAYS, &p), 0);
        c.next_turn = Some((TurnDirection::Left, 30.0));
        assert_eq!(lamps(&c, 0, SignalHabit::ALWAYS, &p), LAMP_TURN_LEFT);
        c.next_turn = None;
        c.turning = Some(TurnDirection::Right);
        assert_eq!(lamps(&c, 0, SignalHabit::ALWAYS, &p), LAMP_TURN_RIGHT);
        // A bear onto a fork is not a turn.
        c.turning = Some(TurnDirection::SlightLeft);
        assert_eq!(lamps(&c, 0, SignalHabit::ALWAYS, &p), 0);
    }

    #[test]
    fn a_driver_who_does_not_signal_does_not() {
        let p = LampParams::default();
        let mut c = car(8.0, 0.0);
        c.lane_change_left = Some(false);
        let lazy = SignalHabit {
            turns: true,
            lane_changes: false,
        };
        assert_eq!(lamps(&c, 0, lazy, &p), 0);
        assert_eq!(lamps(&c, 0, SignalHabit::ALWAYS, &p), LAMP_TURN_RIGHT);
    }

    #[test]
    fn people_and_bicycles_carry_no_lamps_and_a_fire_engine_its_beacons() {
        let p = LampParams::default();
        let mut c = car(0.0, 0.0);
        c.class = VehicleClass::Pedestrian;
        assert_eq!(lamps(&c, 0, SignalHabit::ALWAYS, &p), 0);
        c.class = VehicleClass::Bicycle;
        assert_eq!(lamps(&c, 0, SignalHabit::ALWAYS, &p), 0);
        c.class = VehicleClass::Emergency;
        c.speed_mps = 10.0;
        assert_eq!(lamps(&c, 0, SignalHabit::ALWAYS, &p), LAMP_EMERGENCY);
    }
}
