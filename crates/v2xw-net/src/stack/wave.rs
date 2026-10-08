//! The US stack's two protocol pieces beyond the WSMP header: the IEEE 1609.3 WAVE
//! Service Advertisement (WSA) and IEEE 1609.4 alternating channel access.
//!
//! * **1609.4 channel switching** [IEEE 1609.4-2016 §6.2.5]: a sync interval of
//!   100 ms, aligned to UTC seconds, split into a 50 ms CCH interval and a 50 ms SCH
//!   interval, each starting with a 4 ms guard interval in which nothing may be sent. A
//!   single-radio device in alternating mode can send on the CCH only in the CCH
//!   interval, so a BSM generated in the SCH interval waits — the latency penalty this
//!   module computes. Continuous mode (the US deployment profile, SAE J2945/1 §6.2:
//!   BSMs on channel 172 continuously) has no penalty.
//! * **1609.3 WSA** [IEEE 1609.3-2020 §8.2]: a provider periodically advertises its
//!   services (PSID, channel) on the CCH; a user that hears one may join the advertised
//!   service channel. This module models the advertisement schedule and the user's
//!   decision; the WSA's octets are not synthesised (see the crate docs).

use serde::{Deserialize, Serialize};

/// Sync interval, ms [1609.4-2016 §6.2.5].
pub const SYNC_INTERVAL_MS: u64 = 100;
/// CCH interval, ms [1609.4-2016 §6.2.5].
pub const CCH_INTERVAL_MS: u64 = 50;
/// Guard interval, ms [1609.4-2016 §6.2.5: SyncTolerance/2 + MaxChSwitchTime].
pub const GUARD_INTERVAL_MS: u64 = 4;
/// The control channel number, 178 [IEEE 1609.4-2016; clause number not re-checked].
pub const CCH_CHANNEL: u8 = 178;

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
        let ms_in = (t_ns / 1_000) % (SYNC_INTERVAL_MS * 1_000);
        let g = GUARD_INTERVAL_MS * 1_000;
        let c = CCH_INTERVAL_MS * 1_000;
        if ms_in < g || (ms_in >= c && ms_in < c + g) {
            Interval::Guard
        } else if ms_in < c {
            Interval::Cch
        } else {
            Interval::Sch
        }
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
        let sync = SYNC_INTERVAL_MS * 1_000_000;
        let base = t_ns - t_ns % sync;
        let offset = if cch { GUARD_INTERVAL_MS } else { CCH_INTERVAL_MS + GUARD_INTERVAL_MS } * 1_000_000;
        let cand = base + offset;
        if cand >= t_ns { cand } else { cand + sync }
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
            ChannelAccess::Alternating => (CCH_INTERVAL_MS - GUARD_INTERVAL_MS) as f64 / SYNC_INTERVAL_MS as f64,
        }
    }
}

/// One advertised service [1609.3-2020 §8.2.4, Provider Service Info].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// The PSID.
    pub psid: u32,
    /// The service channel it is offered on.
    pub channel: u8,
}

/// A WSA provider: advertises its services every `repeat_ms` [1609.3 §8.2.2.5,
/// RepeatRate: WSAs per 5 s].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsaProvider {
    /// The services.
    pub services: Vec<ServiceInfo>,
    /// WSAs per 5 s (the RepeatRate field).
    pub repeat_rate_per_5s: u8,
}

impl WsaProvider {
    /// Interval between WSAs, ms.
    pub fn interval_ms(&self) -> u64 {
        5_000 / u64::from(self.repeat_rate_per_5s.max(1))
    }
}

/// A WSA user: the PSIDs it is interested in and the channel it joined.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsaUser {
    /// PSIDs of interest.
    pub interests: Vec<u32>,
    /// The service channel joined, if any.
    pub joined: Option<u8>,
}

impl WsaUser {
    /// Process a received WSA: join the first advertised service of interest
    /// [1609.3 §8.2.3: the user decides on a match of PSID]. Returns the channel joined.
    pub fn on_wsa(&mut self, wsa: &WsaProvider) -> Option<u8> {
        if self.joined.is_some() {
            return self.joined;
        }
        let hit = wsa.services.iter().find(|s| self.interests.contains(&s.psid))?;
        self.joined = Some(hit.channel);
        self.joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternating_access_waits_for_the_right_interval() {
        let s = ChannelSchedule { access: ChannelAccess::Alternating };
        assert_eq!(s.interval(0), Interval::Guard);
        assert_eq!(s.interval(10_000_000), Interval::Cch);
        assert_eq!(s.interval(52_000_000), Interval::Guard);
        assert_eq!(s.interval(60_000_000), Interval::Sch);
        // A CCH frame at 60 ms waits for 104 ms.
        assert_eq!(s.next_opportunity_ns(60_000_000, true), 104_000_000);
        assert_eq!(s.access_delay_ns(10_000_000, true), 0);
        assert_eq!(s.next_opportunity_ns(10_000_000, false), 54_000_000);
        let c = ChannelSchedule { access: ChannelAccess::Continuous };
        assert_eq!(c.access_delay_ns(60_000_000, true), 0);
        let mut u = WsaUser { interests: vec![0x8002], joined: None };
        let p = WsaProvider { services: vec![ServiceInfo { psid: 0x8002, channel: 174 }], repeat_rate_per_5s: 50 };
        assert_eq!(p.interval_ms(), 100);
        assert_eq!(u.on_wsa(&p), Some(174));
    }
}
