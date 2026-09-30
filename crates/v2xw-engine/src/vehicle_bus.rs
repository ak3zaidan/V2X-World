//! What each equipped vehicle's on-board unit reads off its own vehicle bus, built from
//! the vehicle's own state each mobility step ([`v2xw_node::vehicle`]).
//!
//! Every field is the vehicle's own sensor or switch about itself: the wheel speed, the
//! stability control's accelerometers and yaw-rate sensor, the hazard switch, the
//! headlights and the navigation's next turn. None is a view of another road user.
//!
//! # Headlights
//!
//! Low beams are on between sunset and sunrise and whenever visibility is under 1,000 ft
//! (304.8 m) — the rule most US states write (e.g. California Vehicle Code §24400,
//! New York VTL §375(2)(a)(i)). Sunrise and sunset come from the NOAA "general solar
//! position" approximation (the declination from the day of the year, the hour angle at
//! which the sun's centre crosses the horizon), evaluated in local mean solar time at the
//! world's origin; it is good to a few minutes, which is well inside the range of real
//! drivers' switching times. The equation of time and refraction are left out, stated.

use v2xw_core::geo::GeoOrigin;
use v2xw_core::kinematics::Kinematics;
use v2xw_core::math;
use v2xw_core::time::{SimTime, WallClock};
use v2xw_mobility::VehicleClass;
use v2xw_node::vehicle::{HEADLIGHT_VISIBILITY_M, OwnIntent, TurnIntent, VehicleBus};
use v2xw_world::{TurnDirection, World};

/// Whether J2735's heavier hard-braking rule (0.2 g) applies: a Class 4 bus or a
/// Class 7–13 truck. Delivery vans (FHWA Class 5, two axles, six tyres) are not in it.
pub const fn is_heavy(class: VehicleClass) -> bool {
    matches!(
        class,
        VehicleClass::Truck | VehicleClass::Trailer | VehicleClass::Bus | VehicleClass::Coach
    )
}

/// Whether the sun is below the horizon at `t` at `origin`.
pub fn is_night(wall: WallClock, t: SimTime, origin: GeoOrigin) -> bool {
    let unix = wall.unix_seconds_at(t);
    let days = unix.div_euclid(86_400);
    let utc_s = unix.rem_euclid(86_400) as f64;
    // Day of the year (1-based), from the civil date.
    let civil = wall.civil_at(t);
    let jan1 = v2xw_core::time::CivilDateTime {
        year: civil.year,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
    };
    let n = (days - jan1.to_unix_seconds().div_euclid(86_400) + 1) as f64;
    let pi = core::f64::consts::PI;
    // Solar declination (Cooper 1969): δ = 23.44° · sin(2π (284 + n) / 365).
    let decl = (23.44_f64).to_radians() * math::sin(2.0 * pi * (284.0 + n) / 365.0);
    let lat = origin.lat_deg.to_radians();
    // Hour angle of sunrise: cos ω₀ = −tan φ · tan δ (no refraction).
    let x = -(math::sin(lat) / math::cos(lat)) * (math::sin(decl) / math::cos(decl));
    let x = x.clamp(-1.0, 1.0);
    let omega = math::atan2(math::sqrt(1.0 - x * x), x); // acos
    let half_day_h = omega.to_degrees() / 15.0;
    // Local mean solar time.
    let solar_h = (utc_s / 3_600.0 + origin.lon_deg / 15.0).rem_euclid(24.0);
    solar_h < 12.0 - half_day_h || solar_h >= 12.0 + half_day_h
}

/// The vehicle's bus reading from its own state.
#[allow(clippy::too_many_arguments)]
pub fn bus_of(
    world: &World,
    truth: &Kinematics,
    class: VehicleClass,
    intent: Option<v2xw_mobility::Intent>,
    hazard_lights: bool,
    night: bool,
    visibility_m: f64,
    t: SimTime,
) -> VehicleBus {
    let q = truth.quantized();
    let (s, c) = math::sin_cos(q.heading_rad);
    let a_long = math::q3(q.acc.x * c + q.acc.y * s);
    let a_lat = math::q3(-q.acc.x * s + q.acc.y * c);
    let intent = intent.and_then(|i| {
        let turn = match i.turn {
            TurnDirection::Straight => TurnIntent::Straight,
            TurnDirection::Left | TurnDirection::SlightLeft | TurnDirection::UTurn => {
                TurnIntent::Left
            }
            TurnDirection::Right | TurnDirection::SlightRight => TurnIntent::Right,
            #[allow(unreachable_patterns)]
            _ => TurnIntent::Straight,
        };
        let junction_pos = world.roads.try_junction(i.junction)?.position;
        Some(OwnIntent {
            turn,
            distance_m: i.distance_m,
            junction_pos,
        })
    });
    VehicleBus {
        t,
        speed_mps: math::hypot(q.vel.x, q.vel.y),
        a_long_mps2: a_long,
        a_lat_mps2: a_lat,
        yaw_rate_rad_s: q.yaw_rate_rad_s,
        hazard_lights,
        low_beam: night || visibility_m < HEADLIGHT_VISIBILITY_M,
        intent,
        heavy: is_heavy(class),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// New York at the June solstice: dark at 04:00 local (08:00 UTC), light at noon,
    /// dark at 22:00 local.
    #[test]
    fn night_follows_the_sun() {
        let nyc = GeoOrigin::new(40.75, -73.99, 0.0);
        let at = |rfc: &str| {
            let w = WallClock::parse_rfc3339(rfc).expect("t0");
            is_night(w, 0, nyc)
        };
        assert!(at("2026-06-21T08:00:00Z"));
        assert!(!at("2026-06-21T16:00:00Z"));
        assert!(at("2026-06-22T02:00:00Z"));
        // 06:00 local in December is still dark; in June it is light.
        assert!(at("2026-12-21T11:00:00Z"));
        assert!(!at("2026-06-21T10:00:00Z"));
    }
}
