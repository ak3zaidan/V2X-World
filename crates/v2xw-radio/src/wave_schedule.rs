//! The IEEE 1609.4 channel-switching schedule [IEEE 1609.4-2016 §6.2.5].
//!
//! A sync interval of 100 ms, aligned to UTC seconds, is split into a 50 ms CCH interval
//! and a 50 ms SCH interval, each starting with a 4 ms guard interval in which nothing
//! may be sent. A single-radio device in alternating mode can send on the CCH only in the
//! CCH interval and on its service channel only in the SCH interval. Continuous mode (the
//! US deployment profile, SAE J2945/1 §6.2: BSMs on channel 172 continuously) has no gate.
//!
//! The schedule lives here, in the radio crate, because the EDCA MAC gates its grants on
//! it (WP-16); `v2xw_net::stack::wave` re-exports it under the same names.
//!
//! Status: the 100/50/4 ms constants are UNVERIFIED at the primary level (the standard
//! is not cached; `04-models.md`, `11-open-questions.md`). They are the values every
//! secondary source and Veins' `Mac1609_4` use.

use serde::{Deserialize, Serialize};

/// Sync interval, ms [1609.4-2016 §6.2.5].
pub const SYNC_INTERVAL_MS: u64 = 100;
/// CCH interval, ms [1609.4-2016 §6.2.5].
pub const CCH_INTERVAL_MS: u64 = 50;
/// Guard interval, ms [1609.4-2016 §6.2.5: SyncTolerance/2 + MaxChSwitchTime].
pub const GUARD_INTERVAL_MS: u64 = 4;
/// The control channel number, 178 [IEEE 1609.4-2016; clause number not re-checked].
pub const CCH_CHANNEL: u8 = 178;

const MS: u64 = 1_000_000;

/// How a single-radio device uses its channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChannelAccess {
    /// One channel all the time (the deployed J2945/1 profile).
    Continuous,
    /// Alternate CCH and SCH every 50 ms [1609.4].
    Alternating,
}

/// Which interval a time falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Interval {
    /// The CCH interval (after its guard).
    Cch,
    /// The SCH interval (after its guard).
    Sch,
    /// A guard interval: no transmission.
    Guard,
}

/// The 1609.4 schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSchedule {
    /// The access mode.
    pub access: ChannelAccess,
}

impl ChannelSchedule {
    /// Which interval `t_ns` is in (the sync interval starts at multiples of 100 ms).
    pub fn interval(&self, t_ns: u64) -> Interval {
        let us_in = (t_ns / 1_000) % (SYNC_INTERVAL_MS * 1_000);
        let g = GUARD_INTERVAL_MS * 1_000;
        let c = CCH_INTERVAL_MS * 1_000;
        if us_in < g || (us_in >= c && us_in < c + g) {
            Interval::Guard
        } else if us_in < c {
            Interval::Cch
        } else {
            Interval::Sch
        }
    }

    /// The end of the interval (guard, CCH or SCH) that `t_ns` is in, ns: the first
    /// instant after `t_ns` that belongs to the next interval. A frame in the CCH or SCH
    /// interval must end at or before it.
    pub fn interval_end_ns(&self, t_ns: u64) -> u64 {
        let sync = SYNC_INTERVAL_MS * MS;
        let base = t_ns - t_ns % sync;
        let off = t_ns - base;
        let bounds = [
            GUARD_INTERVAL_MS * MS,
            CCH_INTERVAL_MS * MS,
            (CCH_INTERVAL_MS + GUARD_INTERVAL_MS) * MS,
            sync,
        ];
        let end = bounds.into_iter().find(|b| off < *b).unwrap_or(sync);
        base + end
    }

    /// The start of the next CCH (`cch = true`) or SCH interval strictly after the one
    /// `t_ns` is in, or the start of the coming one when `t_ns` is outside it, ns.
    fn next_start_ns(&self, t_ns: u64, cch: bool) -> u64 {
        let sync = SYNC_INTERVAL_MS * MS;
        let base = t_ns - t_ns % sync;
        let offset = if cch {
            GUARD_INTERVAL_MS
        } else {
            CCH_INTERVAL_MS + GUARD_INTERVAL_MS
        } * MS;
        let cand = base + offset;
        if cand > t_ns { cand } else { cand + sync }
    }

    /// The earliest time `>= t_ns` at which a frame for the CCH (`cch = true`) or the SCH
    /// may start, ns.
    pub fn next_opportunity_ns(&self, t_ns: u64, cch: bool) -> u64 {
        if self.access == ChannelAccess::Continuous {
            return t_ns;
        }
        let want = if cch { Interval::Cch } else { Interval::Sch };
        if self.interval(t_ns) == want {
            return t_ns;
        }
        self.next_start_ns(t_ns, cch)
    }

    /// Whether a frame of `air_ns` starting at `t_ns` on the CCH (`cch = true`) or the
    /// SCH fits the schedule: it must start in that channel's interval (never in a guard)
    /// and end at or before the interval's end. This is the rule Veins' `Mac1609_4`
    /// applies as `timeLeftInSlot` (behaviour reference only). `Err` carries the start
    /// of the next interval of that channel, where the MAC re-runs AIFS.
    ///
    /// A frame longer than a whole interval (46 ms, far above any 1609.3 WSM: 1 400
    /// bytes at 3 Mb/s is under 4 ms) can never fit; it is let through at the start of
    /// its interval rather than held for ever. Continuous access always fits.
    pub fn fits(&self, t_ns: u64, air_ns: u64, cch: bool) -> Result<(), u64> {
        if self.access == ChannelAccess::Continuous {
            return Ok(());
        }
        let want = if cch { Interval::Cch } else { Interval::Sch };
        if self.interval(t_ns) == want {
            let end = self.interval_end_ns(t_ns);
            if t_ns.saturating_add(air_ns) <= end {
                return Ok(());
            }
            let window = (CCH_INTERVAL_MS - GUARD_INTERVAL_MS) * MS;
            let start = end - window;
            if air_ns > window && t_ns == start {
                return Ok(());
            }
        }
        Err(self.next_start_ns(t_ns, cch))
    }

    /// The access delay a frame generated at `t_ns` suffers, ns.
    pub fn access_delay_ns(&self, t_ns: u64, cch: bool) -> u64 {
        self.next_opportunity_ns(t_ns, cch) - t_ns
    }

    /// The fraction of time one channel can carry frames: 0.46 alternating (46 of every
    /// 100 ms), 1.0 continuous.
    pub fn usable_fraction(&self) -> f64 {
        match self.access {
            ChannelAccess::Continuous => 1.0,
            ChannelAccess::Alternating => {
                (CCH_INTERVAL_MS - GUARD_INTERVAL_MS) as f64 / SYNC_INTERVAL_MS as f64
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALT: ChannelSchedule = ChannelSchedule {
        access: ChannelAccess::Alternating,
    };

    #[test]
    fn the_interval_end_is_the_next_boundary() {
        assert_eq!(ALT.interval_end_ns(0), 4 * MS);
        assert_eq!(ALT.interval_end_ns(4 * MS), 50 * MS);
        assert_eq!(ALT.interval_end_ns(49_999_999), 50 * MS);
        assert_eq!(ALT.interval_end_ns(50 * MS), 54 * MS);
        assert_eq!(ALT.interval_end_ns(60 * MS), 100 * MS);
        assert_eq!(ALT.interval_end_ns(1_234 * MS), 1_250 * MS);
        // Every instant's interval end is a boundary of a different interval.
        for t in (0..300 * MS).step_by(777_777) {
            let e = ALT.interval_end_ns(t);
            assert!(e > t && e - t <= 46 * MS);
            assert_eq!(ALT.interval(e - 1), ALT.interval(t));
            assert_ne!(ALT.interval(e), ALT.interval(t));
        }
    }

    #[test]
    fn a_frame_fits_only_inside_its_interval() {
        let air = 500_000; // 0.5 ms
        assert_eq!(ALT.fits(10 * MS, air, true), Ok(()));
        // Ends exactly at the interval end: fits.
        assert_eq!(ALT.fits(50 * MS - air, air, true), Ok(()));
        // Would cross into the guard: waits for the next CCH interval.
        assert_eq!(ALT.fits(50 * MS - air + 1, air, true), Err(104 * MS));
        // In the guard or the SCH interval: the next CCH start.
        assert_eq!(ALT.fits(2 * MS, air, true), Err(4 * MS));
        assert_eq!(ALT.fits(60 * MS, air, true), Err(104 * MS));
        // SCH frames.
        assert_eq!(ALT.fits(10 * MS, air, false), Err(54 * MS));
        assert_eq!(ALT.fits(52 * MS, air, false), Err(54 * MS));
        assert_eq!(ALT.fits(54 * MS, air, false), Ok(()));
        assert_eq!(ALT.fits(100 * MS - air + 1, air, false), Err(154 * MS));
        // Continuous: always.
        let c = ChannelSchedule {
            access: ChannelAccess::Continuous,
        };
        assert_eq!(c.fits(2 * MS, air, true), Ok(()));
        assert_eq!(c.fits(50 * MS - 1, air, false), Ok(()));
        // A frame longer than an interval goes at the interval start, never holds.
        assert_eq!(ALT.fits(4 * MS, 47 * MS, true), Ok(()));
        assert_eq!(ALT.fits(5 * MS, 47 * MS, true), Err(104 * MS));
    }

    #[test]
    fn fits_agrees_with_the_interval_for_random_frames() {
        // A keyed sweep: every Ok starts in the right interval and ends by its end; every
        // Err is the start of that channel's interval and later than t.
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..10_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let t = x % (1_000 * MS);
            let air = 50_000 + (x >> 40) % (4 * MS);
            let cch = x & 1 == 0;
            let want = if cch { Interval::Cch } else { Interval::Sch };
            match ALT.fits(t, air, cch) {
                Ok(()) => {
                    assert_eq!(ALT.interval(t), want);
                    assert!(t + air <= ALT.interval_end_ns(t));
                }
                Err(n) => {
                    assert!(n > t);
                    assert_eq!(ALT.interval(n), want);
                    assert_eq!(ALT.interval(n - 1), Interval::Guard);
                    assert_eq!(ALT.fits(n, air, cch), Ok(()));
                }
            }
        }
    }

    #[test]
    fn the_next_opportunity_is_unchanged() {
        assert_eq!(ALT.interval(0), Interval::Guard);
        assert_eq!(ALT.interval(10 * MS), Interval::Cch);
        assert_eq!(ALT.interval(52 * MS), Interval::Guard);
        assert_eq!(ALT.interval(60 * MS), Interval::Sch);
        assert_eq!(ALT.next_opportunity_ns(60 * MS, true), 104 * MS);
        assert_eq!(ALT.next_opportunity_ns(10 * MS, false), 54 * MS);
        assert_eq!(ALT.next_opportunity_ns(2 * MS, true), 4 * MS);
        assert_eq!(ALT.access_delay_ns(10 * MS, true), 0);
        assert!((ALT.usable_fraction() - 0.46).abs() < 1e-12);
    }
}
