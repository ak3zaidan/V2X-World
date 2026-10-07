//! The demand of a day on the civil clock: a morning peak, an evening plateau and a
//! night lull, from counted traffic (04-models.md §2.4).
//!
//! [`crate::demand::DemandProfile`] shapes demand over the *fraction of the run*, so a
//! `rush` profile puts its peaks a quarter and three quarters of the way through whatever
//! the run lasts. A run of days needs the demand of the hour on the clock: 08:00 is busy
//! on every day of a three-day run, and Saturday is not Tuesday. [`DailyProfile`] is that:
//! a factor for each hour of the local day, interpolated linearly between hour centres,
//! with a weekday and a weekend curve chosen by the local date.
//!
//! # The curve, and where it comes from
//!
//! **NYC DOT, Automated Traffic Volume Counts** (NYC Open Data dataset `7ym2-wayt`,
//! accessed 2026-10-07): every automated traffic recorder count in the borough of
//! Manhattan from 2015 to 2026, in 15-minute intervals — 20,102 (date, hour) aggregates,
//! 627 weekdays and 239 weekend days, 184,683 count records. Each hour's factor is the
//! mean volume per 15-minute count that hour, normalised so the 24 weekday factors average
//! 1; the weekend curve is normalised the same way and then scaled by the weekend's mean
//! volume relative to the weekday's (0.819). So a scenario's `rate_veh_per_h` is the
//! **mean weekday rate**, and a weekday offers 24 × that rate over the day.
//!
//! The query and the reduction are `docs/calibration/nyc-dot-hourly-profile.py`; rerunning
//! it reproduces the table to three decimals.
//!
//! What the curve shows, and what a day-long run therefore shows: the night lull at 03:00
//! (0.26 of the mean), the morning peak at 08:00 (1.30), a slight midday dip, and the
//! evening plateau from 15:00 to 18:00 (1.33) — Manhattan's afternoon is as busy as its
//! morning, unlike a commuter freeway's two sharp peaks.
//!
//! What it is not: one borough's average over a decade of counting sites, not the counts
//! at the streets a world contains; and the civil clock is standard time from a fixed UTC
//! offset — daylight saving time moves no hour here.

use serde::{Deserialize, Serialize};
use v2xw_core::time::SimTime;

/// Manhattan's weekday factors, hour 0 (00:00–01:00) to hour 23, mean 1 (see the module
/// note for the source).
pub const NYC_DOT_MANHATTAN_WEEKDAY: [f64; 24] = [
    0.610, 0.409, 0.300, 0.264, 0.346, 0.607, 1.005, 1.280, 1.299, 1.207, 1.149, 1.140, 1.156,
    1.189, 1.268, 1.336, 1.329, 1.334, 1.332, 1.277, 1.177, 1.095, 1.009, 0.880,
];

/// Manhattan's weekend shape, hour 0 to 23, mean 1 before [`WEEKEND_VOLUME_RATIO`].
pub const NYC_DOT_MANHATTAN_WEEKEND: [f64; 24] = [
    0.932, 0.747, 0.603, 0.498, 0.454, 0.420, 0.533, 0.691, 0.821, 0.952, 1.093, 1.186, 1.266,
    1.309, 1.362, 1.405, 1.395, 1.368, 1.348, 1.303, 1.222, 1.121, 1.052, 0.921,
];

/// A Manhattan weekend day's mean volume relative to a weekday's, from the same counts.
pub const WEEKEND_VOLUME_RATIO: f64 = 0.819;

/// Which day curve applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DailyCurve {
    /// Manhattan, weekday or weekend by the local date.
    #[default]
    NycDotManhattan,
    /// Manhattan's weekday curve on every day.
    NycDotManhattanWeekday,
    /// Manhattan's weekend curve on every day.
    NycDotManhattanWeekend,
}

/// Demand by the hour of the local day.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyProfile {
    /// The curve.
    #[serde(default)]
    pub curve: DailyCurve,
    /// The local clock's offset from UTC, hours. Left out, the engine takes it from the
    /// world's longitude (15° an hour, rounded): −5 for New York, which is its standard
    /// time.
    #[serde(default)]
    pub utc_offset_h: Option<f64>,
    /// Unix time of `t = 0`, seconds: the scenario's `time.t0`. **Set by the engine**;
    /// a value written in a scenario is overwritten.
    #[serde(default)]
    pub t0_unix_s: f64,
}

impl DailyProfile {
    /// The largest factor the profile reaches: what the candidate rate is boosted by so
    /// the thinning stays exact.
    #[must_use]
    pub fn supremum(&self) -> f64 {
        let max = |c: &[f64; 24]| c.iter().copied().fold(0.0, f64::max);
        match self.curve {
            DailyCurve::NycDotManhattan => {
                max(&NYC_DOT_MANHATTAN_WEEKDAY).max(max(&NYC_DOT_MANHATTAN_WEEKEND) * WEEKEND_VOLUME_RATIO)
            }
            DailyCurve::NycDotManhattanWeekday => max(&NYC_DOT_MANHATTAN_WEEKDAY),
            DailyCurve::NycDotManhattanWeekend => {
                max(&NYC_DOT_MANHATTAN_WEEKEND) * WEEKEND_VOLUME_RATIO
            }
        }
    }

    /// The factor of local hour `h` (0–23) on local day `day` (days since 1970-01-01).
    fn hourly(&self, day: i64, h: usize) -> f64 {
        // 1970-01-01 was a Thursday: (day + 3) mod 7 is 0 on a Monday.
        let weekend = (day + 3).rem_euclid(7) >= 5;
        let weekend = match self.curve {
            DailyCurve::NycDotManhattan => weekend,
            DailyCurve::NycDotManhattanWeekday => false,
            DailyCurve::NycDotManhattanWeekend => true,
        };
        if weekend {
            NYC_DOT_MANHATTAN_WEEKEND[h] * WEEKEND_VOLUME_RATIO
        } else {
            NYC_DOT_MANHATTAN_WEEKDAY[h]
        }
    }

    /// The local clock at `t`: (day since 1970-01-01, seconds into that day).
    #[must_use]
    pub fn local(&self, t: SimTime) -> (i64, f64) {
        let local = self.t0_unix_s
            + v2xw_core::time::ns_to_secs(t)
            + self.utc_offset_h.unwrap_or(0.0) * 3600.0;
        let day = (local / 86_400.0).floor();
        (day as i64, local - day * 86_400.0)
    }

    /// The factor at `t`: the hourly factors interpolated linearly between hour centres,
    /// across midnight into the neighbouring day's curve.
    #[must_use]
    pub fn factor(&self, t: SimTime) -> f64 {
        let (day, secs) = self.local(t);
        // Position in hours relative to hour centres: hour h's centre is h + 0.5.
        let x = secs / 3600.0 - 0.5;
        let lo = x.floor();
        let w = x - lo;
        let at = |k: i64| {
            let d = day + k.div_euclid(24);
            self.hourly(d, k.rem_euclid(24) as usize)
        };
        let k = lo as i64;
        at(k) * (1.0 - w) + at(k + 1) * w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(curve: DailyCurve) -> DailyProfile {
        // 2027-03-04T00:00:00 local (EST), a Thursday.
        DailyProfile {
            curve,
            utc_offset_h: Some(-5.0),
            t0_unix_s: 1_804_118_400.0 + 5.0 * 3600.0,
        }
    }

    fn at_hours(p: &DailyProfile, h: f64) -> f64 {
        p.factor((h * 3600.0 * 1e9) as u64)
    }

    /// The shape a day must have: a night lull, a morning peak, an evening plateau, and a
    /// weekday mean of 1 — and Saturday lighter than Thursday.
    #[test]
    fn a_day_has_its_peaks_and_its_lull() {
        let p = profile(DailyCurve::NycDotManhattan);
        assert_eq!(p.local(0).1, 0.0, "t0 is local midnight");
        let night = at_hours(&p, 3.5);
        let morning = at_hours(&p, 8.5);
        let midday = at_hours(&p, 11.5);
        let evening = at_hours(&p, 15.5);
        assert!(night < 0.3, "03:30 {night}");
        assert!(morning > 1.25 && morning > midday, "08:30 {morning} vs 11:30 {midday}");
        assert!(evening > morning, "15:30 {evening}");
        assert!(morning / night > 4.0);
        // The mean over the day, by sampling every minute, is the weekday mean of 1.
        let mean: f64 = (0..1440).map(|m| at_hours(&p, f64::from(m) / 60.0)).sum::<f64>() / 1440.0;
        assert!((mean - 1.0).abs() < 0.01, "weekday mean {mean}");
        // Saturday (two days on) is the weekend curve at 0.819 of the volume.
        let sat: f64 = (0..1440)
            .map(|m| at_hours(&p, 48.0 + f64::from(m) / 60.0))
            .sum::<f64>()
            / 1440.0;
        assert!((sat - WEEKEND_VOLUME_RATIO).abs() < 0.01, "saturday mean {sat}");
        assert!(at_hours(&p, 48.0 + 8.5) < morning * 0.7, "saturday morning is quiet");
        // Continuous across midnight.
        let before = at_hours(&p, 23.999);
        let after = at_hours(&p, 24.001);
        assert!((before - after).abs() < 0.01);
        assert!(p.supremum() >= evening);
    }
}
