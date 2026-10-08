//! Driving styles, and how heavy vehicles differ from cars where the car-following
//! parameters alone do not say it.
//!
//! # Driving styles
//!
//! The engine's drivers already differ in the speed they choose on a free road
//! ([`crate::engine::DriverTraits::speed_factor`], SUMO's `speedFactor`) and in how quickly
//! they react. What they did not differ in was how they *follow*: every car kept the same
//! time gap and accelerated at the same rate, so a queue moved off like a train and every
//! platoon had the same spacing. Field studies of following behaviour find the opposite —
//! drivers who choose higher speeds also follow closer and accelerate harder, and the
//! drivers with more violations and crashes are the ones who keep the shortest headways
//! (Evans & Wasielewski, *Accident Analysis & Prevention* 14(3), 1982, and 15(2), 1983;
//! **secondary**: the papers' direction of effect is cited, not their numbers).
//!
//! So the style is not a second, independent draw: it is read from the same latent the
//! free speed came from. A driver whose speed factor is above the 85th percentile of its
//! class's distribution is [`DriverStyle::Assertive`], below the 15th
//! [`DriverStyle::Cautious`], and everyone between [`DriverStyle::Normal`] — 15 / 70 / 15
//! by construction. Those cut points are the traffic engineer's own convention (the 85th
//! percentile speed is the one a speed limit is set from, MUTCD 2009 §2B.13), and **the
//! proportions are that convention, not a measured share of aggressive drivers**: no
//! population survey that sorts drivers into following styles was found to cite.
//!
//! | Style | time gap `T` | acceleration `a` | comfortable deceleration `b` | standstill gap `s0` |
//! |---|---|---|---|---|
//! | assertive | × 0.8 | × 1.15 | × 1.15 | × 0.85 |
//! | normal | × 1 | × 1 | × 1 | × 1 |
//! | cautious | × 1.25 | × 0.85 | × 0.85 | × 1.15 |
//!
//! The multipliers are **choices**: they keep the fleet's mean time gap and acceleration at
//! the calibrated values (0.15 × 0.8 + 0.7 + 0.15 × 1.25 = 1.0075) so the HCM saturation
//! flow the [`crate::carfollowing::idm::IdmPreset::UrbanHcm`] preset was calibrated to is
//! kept. Their spread (±20-25 %) is of the order by which IDM parameters fitted to
//! different drivers' trajectories differ (Kesting & Treiber 2008, *TRR* 2088; **secondary**,
//! the order of magnitude only — the fitted values were not transcribed).
//!
//! # Heavy vehicles at a turn
//!
//! The AASHTO design vehicles' minimum turning paths, which the [`VehicleClass::design_turn`]
//! radii come from, are drawn for turns made at **10 mph (16 km/h) or less** (AASHTO Green
//! Book 2018, §2.1.3: "the minimum turning paths ... at speeds of 10 mph or less";
//! **secondary**: as the Green Book is quoted). A bus or a truck turning at a Manhattan
//! corner uses all of its swept path at that speed and no more, while the car turn speed
//! the engine derives from the connector's curvature (SUMO's 5.5 m/s² lateral
//! acceleration) let a 12 m bus take a 9 m-radius corner at 7 m/s. [`turn_speed_cap`]
//! holds a heavy vehicle on a turning connector to [`HEAVY_TURN_SPEED_MPS`].

//!
//! # Heavy-vehicle dimensions against the US design vehicles
//!
//! The bodies come from the SUMO vType table ([`crate::classes`]); set beside the AASHTO
//! design vehicles a US street is laid out for (Green Book 2018, Table 2-1b; **secondary**:
//! as the table is widely reproduced), they are the vehicles a Midtown street sees:
//!
//! | Class | SUMO body | AASHTO design vehicle | its length × width |
//! |---|---|---|---|
//! | `bus` | 12.0 × 2.5 m | CITY-BUS | 40 ft × 8.5 ft (12.2 × 2.6 m) |
//! | `coach` | 14.0 × 2.6 m | BUS-45 (BUS-14) | 45 ft × 8.5 ft (13.7 × 2.6 m) |
//! | `trailer` | 16.5 × 2.55 m | WB-50 (WB-15) | 55 ft × 8.5 ft (16.8 × 2.6 m) |
//! | `truck` | 7.1 × 2.4 m | (SU-30 is 30 ft, 9.1 m) | a 24-ft box truck, the common Manhattan delivery truck, shorter than the SU-30 the turning radii are taken from |
//! | `delivery` | 6.5 × 2.16 m | (none) | a large van |
//!
//! [`DESIGN_VEHICLE_LENGTH_M`] holds the design lengths, and the crate's test holds the bus,
//! coach and trailer bodies within 5 % of them.

use serde::{Deserialize, Serialize};

use crate::classes::VehicleClass;

/// The AASHTO design vehicle length a heavy class stands for, metres (see the module
/// table): CITY-BUS 40 ft, BUS-45 45 ft, WB-50 55 ft, SU-30 30 ft.
pub const fn design_vehicle_length_m(class: VehicleClass) -> Option<f64> {
    match class {
        VehicleClass::Bus => Some(40.0 * 0.3048),
        VehicleClass::Coach => Some(45.0 * 0.3048),
        VehicleClass::Trailer => Some(55.0 * 0.3048),
        VehicleClass::Truck => Some(30.0 * 0.3048),
        _ => None,
    }
}

/// [`design_vehicle_length_m`] for the classes the module table checks.
pub const DESIGN_VEHICLE_LENGTH_M: [(VehicleClass, f64); 3] = [
    (VehicleClass::Bus, 40.0 * 0.3048),
    (VehicleClass::Coach, 45.0 * 0.3048),
    (VehicleClass::Trailer, 55.0 * 0.3048),
];
use crate::views::DriverProfile;

/// The 85th percentile of the standard normal, the cut between normal and assertive
/// drivers (and its negative between cautious and normal).
pub const STYLE_CUT_Z: f64 = 1.036_433;

/// How a driver follows, read from the latent their free speed was drawn from.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum DriverStyle {
    /// Below the class's 15th percentile of free speed: a longer time gap, gentler
    /// acceleration and braking.
    Cautious,
    /// Between the 15th and 85th percentiles.
    #[default]
    Normal,
    /// Above the 85th percentile: a shorter time gap, harder acceleration and braking.
    Assertive,
}

impl DriverStyle {
    /// Every style.
    pub const ALL: [DriverStyle; 3] = [
        DriverStyle::Cautious,
        DriverStyle::Normal,
        DriverStyle::Assertive,
    ];

    /// A stable label.
    pub const fn label(self) -> &'static str {
        match self {
            DriverStyle::Cautious => "cautious",
            DriverStyle::Normal => "normal",
            DriverStyle::Assertive => "assertive",
        }
    }

    /// The style of a driver whose speed factor is `speed_factor` in a class whose
    /// speed factor has standard deviation `speed_dev` around 1.
    pub fn of_speed_factor(speed_factor: f64, speed_dev: f64) -> DriverStyle {
        if speed_dev <= 0.0 || !speed_factor.is_finite() {
            return DriverStyle::Normal;
        }
        let z = (speed_factor - 1.0) / speed_dev;
        if z > STYLE_CUT_Z {
            DriverStyle::Assertive
        } else if z < -STYLE_CUT_Z {
            DriverStyle::Cautious
        } else {
            DriverStyle::Normal
        }
    }

    /// `(T, a, b, s0)` multipliers (see the module table).
    pub const fn multipliers(self) -> (f64, f64, f64, f64) {
        match self {
            DriverStyle::Cautious => (1.25, 0.85, 0.85, 1.15),
            DriverStyle::Normal => (1.0, 1.0, 1.0, 1.0),
            DriverStyle::Assertive => (0.8, 1.15, 1.15, 0.85),
        }
    }

    /// `profile` driven in this style.
    pub fn apply(self, profile: DriverProfile) -> DriverProfile {
        let (t, a, b, s0) = self.multipliers();
        DriverProfile {
            time_headway_s: profile.time_headway_s * t,
            max_accel_mps2: profile.max_accel_mps2 * a,
            comfort_decel_mps2: profile.comfort_decel_mps2 * b,
            min_gap_m: profile.min_gap_m * s0,
            ..profile
        }
    }
}

/// The speed the AASHTO minimum turning paths are drawn for, m/s: 10 mph.
pub const HEAVY_TURN_SPEED_MPS: f64 = 10.0 * 0.447_04;

/// True for the classes that turn on a truck's or a bus's swept path.
pub const fn is_heavy(class: VehicleClass) -> bool {
    matches!(
        class,
        VehicleClass::Truck | VehicleClass::Trailer | VehicleClass::Bus | VehicleClass::Coach
    )
}

/// The speed a vehicle of `class` takes a turning connector at, given the car turn speed
/// `car_turn_mps`: no more than [`HEAVY_TURN_SPEED_MPS`] for a heavy vehicle.
pub fn turn_speed_cap(class: VehicleClass, car_turn_mps: f64) -> f64 {
    if is_heavy(class) {
        car_turn_mps.min(HEAVY_TURN_SPEED_MPS)
    } else {
        car_turn_mps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_style_cut_points_are_the_15th_and_85th_percentiles() {
        assert_eq!(DriverStyle::of_speed_factor(1.0, 0.1), DriverStyle::Normal);
        assert_eq!(
            DriverStyle::of_speed_factor(1.11, 0.1),
            DriverStyle::Assertive
        );
        assert_eq!(
            DriverStyle::of_speed_factor(0.89, 0.1),
            DriverStyle::Cautious
        );
        assert_eq!(DriverStyle::of_speed_factor(1.5, 0.0), DriverStyle::Normal);
        // The fleet's mean time gap and acceleration are kept within 1 %.
        let mean = |k: usize| {
            let m = |s: DriverStyle| {
                let (t, a, b, s0) = s.multipliers();
                [t, a, b, s0][k]
            };
            0.15 * m(DriverStyle::Cautious)
                + 0.7 * m(DriverStyle::Normal)
                + 0.15 * m(DriverStyle::Assertive)
        };
        for k in 0..4 {
            assert!((mean(k) - 1.0).abs() < 0.01, "{k}: {}", mean(k));
        }
    }

    #[test]
    fn buses_coaches_and_trailers_are_the_length_of_their_design_vehicles() {
        for (class, design) in DESIGN_VEHICLE_LENGTH_M {
            let body = class.spec().length_m;
            assert!(
                (body - design).abs() / design < 0.05,
                "{class}: body {body} m against design {design} m"
            );
            assert_eq!(design_vehicle_length_m(class), Some(design));
        }
        assert!(
            VehicleClass::Truck.spec().length_m
                < design_vehicle_length_m(VehicleClass::Truck).unwrap()
        );
    }

    #[test]
    fn a_heavy_vehicle_turns_no_faster_than_the_design_turning_speed() {
        assert_eq!(turn_speed_cap(VehicleClass::Bus, 7.0), HEAVY_TURN_SPEED_MPS);
        assert_eq!(turn_speed_cap(VehicleClass::Passenger, 7.0), 7.0);
        assert_eq!(turn_speed_cap(VehicleClass::Truck, 3.0), 3.0);
    }
}
