//! The civil calendar the US SCMS operates on (WP-B4): US Eastern time with daylight
//! saving, US federal holidays, and business days.
//!
//! Three operational rules read it:
//!
//! * the end entity's misbehaviour-report queue multiplies every weight "every day, at
//!   5 am Eastern time" ([SM-MBR] §2.2; 03 §8.1) — [`next_eastern_time`];
//! * a provider publishes a CRL "within 1 business day of any change, and before
//!   nextCrl" ([SM-PROV] §3.4.9–3.4.10; 03 §10) — [`next_publication_slot`];
//! * business days are Monday to Friday except the US federal holidays of 5 U.S.C. § 6103
//!   with the observed-date rule of Executive Order 11582 (a Saturday holiday is observed
//!   the Friday before, a Sunday holiday the Monday after) — [`is_business_day`].
//!
//! Eastern time is UTC−5, and UTC−4 under daylight saving: from 2007, the second Sunday
//! of March at 02:00 local standard time to the first Sunday of November at 02:00 local
//! daylight time (15 U.S.C. § 260a as amended by the Energy Policy Act of 2005 § 110);
//! 1987–2006, the first Sunday of April to the last Sunday of October. Earlier years
//! are not needed (the SCMS calendar epoch is 2015-01-06, 01 §4.1).
//!
//! Every function works on Unix seconds; leap seconds are not civil-time events here.

/// Seconds per day.
const DAY: i64 = 86_400;

/// Days since 1970-01-01 of the civil date (proleptic Gregorian).
#[must_use]
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date `(year, month, day)` of a day number since 1970-01-01.
#[must_use]
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + i64::from(m <= 2)) as i32, m, d)
}

/// Day of the week of a day number, 0 = Sunday … 6 = Saturday.
#[must_use]
pub fn weekday(days: i64) -> u32 {
    (days + 4).rem_euclid(7) as u32
}

/// The day number of the `n`-th (1-based) `wd` weekday of a month; `n = 0` is the last.
fn nth_weekday(y: i32, m: u32, wd: u32, n: u32) -> i64 {
    if n == 0 {
        let next = if m == 12 {
            days_from_civil(y + 1, 1, 1)
        } else {
            days_from_civil(y, m + 1, 1)
        };
        let last = next - 1;
        let back = (i64::from(weekday(last)) - i64::from(wd)).rem_euclid(7);
        return last - back;
    }
    let first = days_from_civil(y, m, 1);
    let fwd = (i64::from(wd) - i64::from(weekday(first))).rem_euclid(7);
    first + fwd + 7 * i64::from(n - 1)
}

/// The UTC instants daylight saving starts and ends in year `y` (Unix seconds).
fn dst_bounds(y: i32) -> (i64, i64) {
    let (start, end) = if y >= 2007 {
        (nth_weekday(y, 3, 0, 2), nth_weekday(y, 11, 0, 1))
    } else {
        (nth_weekday(y, 4, 0, 1), nth_weekday(y, 10, 0, 0))
    };
    // 02:00 EST = 07:00 UTC; 02:00 EDT = 06:00 UTC.
    (start * DAY + 7 * 3600, end * DAY + 6 * 3600)
}

/// The Eastern-time offset from UTC at Unix second `unix`, in seconds (−18000 or −14400).
#[must_use]
pub fn eastern_offset(unix: i64) -> i64 {
    let (y, _, _) = civil_from_days(unix.div_euclid(DAY));
    let (s, e) = dst_bounds(y);
    if unix >= s && unix < e {
        -4 * 3600
    } else {
        -5 * 3600
    }
}

/// The Unix second of Eastern local `(y, m, d, hh:mm)`. A local time skipped by the
/// spring change resolves with the standard offset; an ambiguous autumn one with the
/// daylight offset (the first occurrence).
#[must_use]
pub fn eastern_to_unix(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> i64 {
    let local = days_from_civil(y, m, d) * DAY + i64::from(hh) * 3600 + i64::from(mm) * 60;
    let edt = local + 4 * 3600;
    if eastern_offset(edt) == -4 * 3600 {
        edt
    } else {
        local + 5 * 3600
    }
}

/// The Eastern civil date at Unix second `unix`.
#[must_use]
pub fn eastern_date(unix: i64) -> (i32, u32, u32) {
    civil_from_days((unix + eastern_offset(unix)).div_euclid(DAY))
}

/// The first instant strictly after `unix` at which Eastern local time is `hh:mm`.
#[must_use]
pub fn next_eastern_time(unix: i64, hh: u32, mm: u32) -> i64 {
    let (y, m, d) = eastern_date(unix);
    let mut day = days_from_civil(y, m, d);
    loop {
        let (y, m, d) = civil_from_days(day);
        let t = eastern_to_unix(y, m, d, hh, mm);
        if t > unix {
            return t;
        }
        day += 1;
    }
}

/// How many Eastern `hh:mm` instants lie in `(from, to]`.
#[must_use]
pub fn eastern_times_between(from: i64, to: i64, hh: u32, mm: u32) -> u32 {
    let mut n = 0u32;
    let mut t = next_eastern_time(from, hh, mm);
    while t <= to {
        n += 1;
        t = next_eastern_time(t, hh, mm);
    }
    n
}

/// The observed day numbers of the US federal holidays of year `y` (5 U.S.C. § 6103(a);
/// Juneteenth from 2021, Pub. L. 117-17). Inauguration Day (Washington DC only) is not
/// included. A Saturday holiday is observed on the Friday before and a Sunday holiday on
/// the Monday after (E.O. 11582; 5 U.S.C. § 6103(b)).
#[must_use]
pub fn federal_holidays(y: i32) -> Vec<i64> {
    let fixed = |m, d| {
        let day = days_from_civil(y, m, d);
        match weekday(day) {
            6 => day - 1,
            0 => day + 1,
            _ => day,
        }
    };
    let mut v = vec![
        fixed(1, 1),              // New Year's Day
        nth_weekday(y, 1, 1, 3),  // Birthday of Martin Luther King, Jr.
        nth_weekday(y, 2, 1, 3),  // Washington's Birthday
        nth_weekday(y, 5, 1, 0),  // Memorial Day
        fixed(7, 4),              // Independence Day
        nth_weekday(y, 9, 1, 1),  // Labor Day
        nth_weekday(y, 10, 1, 2), // Columbus Day
        fixed(11, 11),            // Veterans Day
        nth_weekday(y, 11, 4, 4), // Thanksgiving Day
        fixed(12, 25),            // Christmas Day
    ];
    if y >= 2021 {
        v.push(fixed(6, 19)); // Juneteenth National Independence Day
    }
    // New Year's Day of the following year observed on 31 December.
    if weekday(days_from_civil(y + 1, 1, 1)) == 6 {
        v.push(days_from_civil(y, 12, 31));
    }
    v.sort_unstable();
    v
}

/// Is the day number a US business day: Monday to Friday, not an observed federal
/// holiday?
#[must_use]
pub fn is_business_day(day: i64) -> bool {
    let wd = weekday(day);
    if wd == 0 || wd == 6 {
        return false;
    }
    let (y, _, _) = civil_from_days(day);
    !federal_holidays(y).contains(&day)
}

/// The publication slot a provider's CRL operator next uses: the first Eastern
/// business-day `hh:mm` strictly after `unix`. A change decided on a Friday evening, a
/// weekend or a federal holiday is published on the next business day, which keeps
/// the "within 1 business day" rule of [SM-PROV] §3.4.9.
///
/// The slot hour is an operating assumption (the provider rule fixes the deadline, not
/// the hour); [`crate::scms::params::CrlIssuance`] carries it as a setting.
#[must_use]
pub fn next_publication_slot(unix: i64, hh: u32, mm: u32) -> i64 {
    let mut t = next_eastern_time(unix, hh, mm);
    loop {
        let (y, m, d) = eastern_date(t);
        if is_business_day(days_from_civil(y, m, d)) {
            return t;
        }
        t = next_eastern_time(t, hh, mm);
    }
}

/// The latest instant a change decided at `unix` may be published under the "within 1
/// business day" rule: the end (24:00 Eastern) of the first business day after the
/// decision's Eastern date.
#[must_use]
pub fn publication_deadline(unix: i64) -> i64 {
    let (y, m, d) = eastern_date(unix);
    let mut day = days_from_civil(y, m, d) + 1;
    while !is_business_day(day) {
        day += 1;
    }
    let (y, m, d) = civil_from_days(day + 1);
    eastern_to_unix(y, m, d, 0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip_and_weekdays() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(
            civil_from_days(days_from_civil(2026, 10, 17)),
            (2026, 10, 17)
        );
        // 2015-01-06 was a Tuesday; 2026-10-16 is a Friday.
        assert_eq!(weekday(days_from_civil(2015, 1, 6)), 2);
        assert_eq!(weekday(days_from_civil(2026, 10, 16)), 5);
    }

    #[test]
    fn eastern_offsets_follow_the_2007_rule() {
        // 2026: DST from Sunday 8 March 07:00 UTC to Sunday 1 November 06:00 UTC.
        let s = days_from_civil(2026, 3, 8) * DAY + 7 * 3600;
        let e = days_from_civil(2026, 11, 1) * DAY + 6 * 3600;
        assert_eq!(eastern_offset(s - 1), -18_000);
        assert_eq!(eastern_offset(s), -14_400);
        assert_eq!(eastern_offset(e - 1), -14_400);
        assert_eq!(eastern_offset(e), -18_000);
        // The epoch: 2015-01-06 04:00 EST = 09:00 UTC.
        assert_eq!(
            eastern_to_unix(2015, 1, 6, 4, 0),
            days_from_civil(2015, 1, 6) * DAY + 9 * 3600
        );
    }

    #[test]
    fn five_am_eastern_moves_with_daylight_saving() {
        // 05:00 EDT = 09:00 UTC in July; 05:00 EST = 10:00 UTC in January.
        let jul = days_from_civil(2026, 7, 1) * DAY;
        assert_eq!(next_eastern_time(jul, 5, 0), jul + 9 * 3600);
        let jan = days_from_civil(2026, 1, 5) * DAY;
        assert_eq!(next_eastern_time(jan, 5, 0), jan + 10 * 3600);
        // Strictly after: at exactly 05:00 the next one is tomorrow's.
        assert_eq!(
            next_eastern_time(jan + 10 * 3600, 5, 0),
            jan + DAY + 10 * 3600
        );
        assert_eq!(eastern_times_between(jan, jan + 7 * DAY, 5, 0), 7);
    }

    #[test]
    fn federal_holidays_2026_observed() {
        let d = |m, dd| days_from_civil(2026, m, dd);
        let h = federal_holidays(2026);
        // 4 July 2026 is a Saturday: observed Friday 3 July.
        assert!(h.contains(&d(7, 3)) && !h.contains(&d(7, 4)));
        for (m, dd) in [
            (1, 1),
            (1, 19),
            (2, 16),
            (5, 25),
            (6, 19),
            (9, 7),
            (10, 12),
            (11, 11),
            (11, 26),
            (12, 25),
        ] {
            assert!(h.contains(&d(m, dd)), "2026-{m:02}-{dd:02}");
        }
        assert_eq!(h.len(), 11);
        // 2027-12-25 is a Saturday (observed Friday 24th); 2028-01-01 a Saturday,
        // observed Friday 2027-12-31.
        let h27 = federal_holidays(2027);
        assert!(h27.contains(&days_from_civil(2027, 12, 24)));
        assert!(h27.contains(&days_from_civil(2027, 12, 31)));
    }

    #[test]
    fn publication_slots_skip_weekends_and_holidays() {
        // Friday 2026-10-16 19:00 EDT → Monday 19 October 10:00 EDT.
        let fri = eastern_to_unix(2026, 10, 16, 19, 0);
        assert_eq!(
            next_publication_slot(fri, 10, 0),
            eastern_to_unix(2026, 10, 19, 10, 0)
        );
        // Thanksgiving, Thursday 2026-11-26 11:00 EST → Friday 27th 10:00.
        let tg = eastern_to_unix(2026, 11, 26, 11, 0);
        assert_eq!(
            next_publication_slot(tg, 10, 0),
            eastern_to_unix(2026, 11, 27, 10, 0)
        );
        // Friday 2026-07-02 18:00 → Monday 6 July (Friday 3rd is the observed 4th).
        let jul = eastern_to_unix(2026, 7, 2, 18, 0);
        assert_eq!(
            next_publication_slot(jul, 10, 0),
            eastern_to_unix(2026, 7, 6, 10, 0)
        );
        // A Tuesday 08:00 decision goes out the same morning.
        let tue = eastern_to_unix(2026, 10, 20, 8, 0);
        assert_eq!(
            next_publication_slot(tue, 10, 0),
            eastern_to_unix(2026, 10, 20, 10, 0)
        );
        // Every slot is within the one-business-day deadline.
        for t in [fri, tg, jul, tue] {
            assert!(next_publication_slot(t, 10, 0) <= publication_deadline(t));
        }
        assert_eq!(
            publication_deadline(fri),
            eastern_to_unix(2026, 10, 20, 0, 0)
        );
    }
}
