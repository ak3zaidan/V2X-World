//! When drivers have their headlamps on: the sun's elevation at the world's origin at the
//! run's civil time, and the weather (vwp-v1 §3.3.5 `LAMP_LOW_BEAM`).
//!
//! The mobility model decides the lamps a driver works — brake, indicators, beacons
//! (`v2xw_mobility::lamps`). Headlamps are different: they follow the light and the weather,
//! which the engine holds and the mobility model does not. New York's statute is typical:
//! headlamps "during the period from one-half hour after sunset to one-half hour before
//! sunrise", "at any time when … insufficient light or unfavorable atmospheric conditions"
//! leave persons and vehicles not clearly discernible at 1,000 feet, and "whenever the
//! windshield wipers are in continuous use because of rain, sleet, snow, hail or other
//! unfavorable atmospheric conditions" (NY Vehicle and Traffic Law §375(2)(a)).
//!
//! "Half an hour after sunset" is taken as the sun 6° below the horizon (the end of civil
//! twilight): at New York's latitude the sun sinks about 0.19° a minute near the horizon,
//! so the statute's half hour is 5.7° past the −0.83° of sunset, −6.5°, and the twilight
//! convention is within a few minutes of it. It is a parameter.
//!
//! The sun's position is the Astronomical Almanac's low-precision formula (The Astronomical
//! Almanac, Section C, "Low precision formulas for the Sun": about 0.01° between 1950 and
//! 2050), through `v2xw_core::math` so it is the same on every platform. The run has no
//! wall clock: the civil time is the scenario's `time.t0` plus sim time, which is data.

use v2xw_core::math;
use v2xw_core::weather::{WeatherKind, WeatherState};

/// The sun's elevation above the horizon, degrees, at Unix time `unix_s` seen from
/// `lat_deg`, `lon_deg` (WGS-84; refraction ignored).
pub fn solar_elevation_deg(unix_s: f64, lat_deg: f64, lon_deg: f64) -> f64 {
    let rad = core::f64::consts::PI / 180.0;
    // Days since J2000.0 (2000-01-01 12:00 TT; UT is close enough at this precision).
    let n = unix_s / 86_400.0 + 2_440_587.5 - 2_451_545.0;
    let l = (280.460 + 0.985_647_4 * n).rem_euclid(360.0);
    let g = ((357.528 + 0.985_600_3 * n).rem_euclid(360.0)) * rad;
    let lambda = (l + 1.915 * math::sin(g) + 0.020 * math::sin(2.0 * g)) * rad;
    let eps = (23.439 - 0.000_000_4 * n) * rad;
    let (sin_l, cos_l) = math::sin_cos(lambda);
    let decl = math::asin(math::sin(eps) * sin_l);
    let ra = math::atan2(math::cos(eps) * sin_l, cos_l);
    // Greenwich mean sidereal time, degrees.
    let gmst = (280.460_618_37 + 360.985_647_366_29 * n).rem_euclid(360.0);
    let hour_angle = (gmst + lon_deg) * rad - ra;
    let lat = lat_deg * rad;
    let s = math::sin(lat) * math::sin(decl)
        + math::cos(lat) * math::cos(decl) * math::cos(hour_angle);
    math::asin(s.clamp(-1.0, 1.0)) / rad
}

/// The headlamp rule's parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadlampParams {
    /// Headlamps on with the sun below this elevation, degrees.
    pub sun_elevation_deg: f64,
    /// Headlamps on with visibility below this, metres (1,000 ft).
    pub visibility_m: f64,
}

impl Default for HeadlampParams {
    fn default() -> Self {
        HeadlampParams {
            sun_elevation_deg: -6.0,
            visibility_m: 304.8,
        }
    }
}

/// Whether drivers have their dipped headlamps on.
pub fn headlamps_on(sun_elevation_deg: f64, weather: &WeatherState, p: &HeadlampParams) -> bool {
    if sun_elevation_deg < p.sun_elevation_deg {
        return true;
    }
    if weather.visibility_m < p.visibility_m {
        return true;
    }
    // Wipers in continuous use: any precipitation that is falling.
    matches!(
        weather.kind,
        WeatherKind::Rain | WeatherKind::Snow | WeatherKind::Sleet
    ) && weather.intensity > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Midtown on 2027-03-20, the equinox: high at 13:00 EDT, set at about 19:08 EDT
    /// (NOAA's calculator gives 19:09 for the day), and well down at 01:00.
    #[test]
    fn the_sun_is_up_at_noon_and_down_at_midnight_in_manhattan() {
        // 2027-03-20 17:00Z is 13:00 EDT: the sun near its highest, ~49° up (90 − 40.75 + ~0).
        let noon = 1_805_562_000.0;
        let e = solar_elevation_deg(noon, 40.75, -73.98);
        assert!((45.0..53.0).contains(&e), "noon elevation {e}");
        // 2027-03-21 05:00Z is 01:00 EDT.
        let night = noon + 12.0 * 3600.0;
        assert!(solar_elevation_deg(night, 40.75, -73.98) < -30.0);
        // 23:08Z: the upper limb on the horizon, -0.83 deg with refraction.
        let sunset = noon + 6.0 * 3600.0 + 8.0 * 60.0;
        let s = solar_elevation_deg(sunset, 40.75, -73.98);
        assert!((-1.3..-0.3).contains(&s), "sunset elevation {s}");
    }

    #[test]
    fn the_scenarios_t0_is_before_dawn_in_new_york() {
        // `2027-03-04T08:00:00Z` — every NYC scenario's t0 — is 03:00 EST.
        let t0 = 1_804_147_200.0;
        let e = solar_elevation_deg(t0, 40.744, -73.99);
        assert!(e < -20.0, "{e}");
        assert!(headlamps_on(e, &WeatherState::CLEAR, &HeadlampParams::default()));
    }

    #[test]
    fn rain_and_fog_put_the_headlamps_on_in_daylight() {
        let p = HeadlampParams::default();
        assert!(!headlamps_on(30.0, &WeatherState::CLEAR, &p));
        let mut rain = WeatherState::CLEAR;
        rain.kind = WeatherKind::Rain;
        rain.intensity = 0.3;
        assert!(headlamps_on(30.0, &rain, &p));
        let mut fog = WeatherState::CLEAR;
        fog.kind = WeatherKind::Fog;
        fog.visibility_m = 150.0;
        assert!(headlamps_on(30.0, &fog, &p));
    }
}
