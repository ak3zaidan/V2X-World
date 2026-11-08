//! A roadside unit away from the safety channel (`radio.devices.rsu.off_channel`, P1-E5).
//!
//! IEEE 1609.4-2016 §6.2.5 divides UTC time into 100 ms sync intervals, the first 50 ms
//! of each the control-channel interval. A one-radio unit that serves anything off the
//! safety channel leaves it for part of that first half and hears nothing on it while
//! away. The public Tampa (CV Pilot THEA) RSU log shows exactly this footprint: msgCnt
//! says the missing BSMs were sent (a gap of k intervals carries a msgCnt step of k in
//! 84-89 % of pairs), and free devices' BSMs generated 10-30 ms into the UTC 100 ms
//! interval are lost 50-57 % of the time against 18-24 % in its second half, at every
//! location cell, with no locked device within 300 m (so not synchronised collisions).
//!
//! The model: in each sync interval `k`, independently with `probability`, the unit is
//! away over `[k·100 ms + from, k·100 ms + from + D)`, `D` uniform in
//! `[min_duration, max_duration]`. A frame whose air time overlaps the absence is lost as
//! [`LossCause::OffChannel`](v2xw_radio::LossCause::OffChannel). The draw is keyed by
//! `(unit, interval)` on a single-use scope, so it depends on neither the frames the unit
//! heard nor the thread that asked. Scenario time 0 is a whole UTC second
//! (`time.t0`), so the intervals are UTC-aligned as 1609.4 requires.
//!
//! Not modelled: the unit's own safety-channel transmissions are not deferred while it
//! is away (its CRL and SPaT frames still leave on time).

use v2xw_core::ids::{LinkKey, NodeId};
use v2xw_core::rng::{EntityRef, RngDomain, RngRegistry};
use v2xw_core::time::SimTime;

/// IEEE 1609.4-2016 §6.2.5's sync interval, ns.
pub(crate) const SYNC_INTERVAL_NS: u64 = 100_000_000;

/// The draw scope's domain.
static DOMAIN: std::sync::LazyLock<RngDomain> =
    std::sync::LazyLock::new(|| RngDomain::plugin("radio/rsu-off-channel"));

/// The resolved absence schedule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OffChannel {
    probability: f64,
    from_ns: u64,
    min_ns: u64,
    max_ns: u64,
}

impl OffChannel {
    /// The scenario's schedule, or `None` when the units stay on the channel.
    pub(crate) fn from_scenario(s: &crate::scenario::Scenario) -> Option<Self> {
        let o = s.radio.devices.rsu.off_channel.as_ref()?;
        let ns = |ms: f64| (ms * 1e6).round().max(0.0) as u64;
        Some(Self {
            probability: o.probability,
            from_ns: ns(o.from_ms),
            min_ns: ns(o.min_duration_ms),
            max_ns: ns(o.max_duration_ms),
        })
    }

    /// The absence of `unit` in sync interval `k`, if it leaves in that interval.
    fn window(&self, rng: &RngRegistry, unit: NodeId, k: u64) -> Option<(SimTime, SimTime)> {
        let mut s = rng.checkout(
            *DOMAIN,
            EntityRef::LinkFrame {
                link: LinkKey::new(unit, unit),
                frame: k,
            },
        );
        if s.f64() >= self.probability {
            return None;
        }
        let span = self.max_ns - self.min_ns;
        let d = self.min_ns + (s.f64() * span as f64) as u64;
        let a = k * SYNC_INTERVAL_NS + self.from_ns;
        Some((a, a + d))
    }

    /// Whether `unit` is away at any instant of `[start, end]`.
    pub(crate) fn away(
        &self,
        rng: &RngRegistry,
        unit: NodeId,
        start: SimTime,
        end: SimTime,
    ) -> bool {
        (start / SYNC_INTERVAL_NS..=end / SYNC_INTERVAL_NS).any(|k| {
            self.window(rng, unit, k)
                .is_some_and(|(a, b)| start < b && end > a)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sched(p: f64, from: f64, lo: f64, hi: f64) -> OffChannel {
        let ns = |ms: f64| (ms * 1e6) as u64;
        OffChannel {
            probability: p,
            from_ns: ns(from),
            min_ns: ns(lo),
            max_ns: ns(hi),
        }
    }

    /// The share of 0.3 ms frames starting at `phase_ms` into an interval that are lost.
    fn lost_share(o: &OffChannel, rng: &RngRegistry, phase_ms: f64) -> f64 {
        let n = 4_000u64;
        let unit = NodeId::new(super::super::ROADSIDE_ID_BASE);
        let lost = (0..n)
            .filter(|k| {
                let t = k * SYNC_INTERVAL_NS + (phase_ms * 1e6) as u64;
                o.away(rng, unit, t, t + 300_000)
            })
            .count();
        lost as f64 / n as f64
    }

    #[test]
    fn the_unit_is_away_only_inside_its_window_and_at_its_rate() {
        let rng = RngRegistry::new(7);
        let o = sched(0.4, 5.0, 10.0, 30.0);
        // Before it leaves and after the longest absence: never away.
        assert_eq!(lost_share(&o, &rng, 2.0), 0.0);
        assert_eq!(lost_share(&o, &rng, 60.0), 0.0);
        // Inside the shortest absence: away in `probability` of the intervals.
        let mid = lost_share(&o, &rng, 10.0);
        assert!((mid - 0.4).abs() < 0.03, "{mid}");
        // Halfway through the duration range: half of those.
        let tail = lost_share(&o, &rng, 25.0);
        assert!((tail - 0.2).abs() < 0.03, "{tail}");
        // Deterministic: the same unit, interval and seed give the same answer.
        assert_eq!(lost_share(&o, &rng, 25.0), tail);
        // Nobody leaves at probability 0; everybody at 1.
        assert_eq!(lost_share(&sched(0.0, 0.0, 50.0, 50.0), &rng, 10.0), 0.0);
        assert_eq!(lost_share(&sched(1.0, 0.0, 50.0, 50.0), &rng, 10.0), 1.0);
    }
}
