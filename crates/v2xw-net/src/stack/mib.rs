//! The GeoNetworking MIB: the protocol constants of EN 302 636-4-1 V1.4.1 Annex H
//! (Table H.1), with Vanetza's `geonet/mib.hpp` as the cross-check.
//!
//! Every default below is the standard's default. Where Vanetza's default differs, the
//! field says which one this simulator uses and why.

use serde::{Deserialize, Serialize};

/// `itsGnDefaultHopLimit`: 10 [EN 302 636-4-1 Annex H].
pub const DEFAULT_HOP_LIMIT: u8 = 10;
/// `itsGnLifetimeLocTE`: 20 s [Annex H].
pub const LIFETIME_LOC_TE_MS: u64 = 20_000;
/// `itsGnBeaconServiceRetransmitTimer`: 3 000 ms [Annex H].
pub const BEACON_RETRANSMIT_MS: u64 = 3_000;
/// `itsGnBeaconServiceMaxJitter`: 750 ms, a quarter of the retransmit timer [Annex H].
pub const BEACON_MAX_JITTER_MS: u64 = 750;
/// `itsGnDefaultPacketLifetime`: 60 s [Annex H].
pub const DEFAULT_PACKET_LIFETIME_MS: u64 = 60_000;
/// `itsGnMaxPacketLifetime`: 600 s [Annex H].
pub const MAX_PACKET_LIFETIME_MS: u64 = 600_000;
/// `itsGnDefaultMaxCommunicationRange`: 1 000 m, the CBF timer's `DIST_MAX` [Annex H].
pub const DEFAULT_MAX_COMM_RANGE_M: f64 = 1_000.0;
/// `itsGnGeoBroadcastCbfMinTime` / `itsGnGeoUnicastCbfMinTime`: 1 ms [Annex H].
pub const CBF_MIN_TIME_MS: u64 = 1;
/// `itsGnGeoBroadcastCbfMaxTime` / `itsGnGeoUnicastCbfMaxTime`: 100 ms [Annex H].
pub const CBF_MAX_TIME_MS: u64 = 100;
/// `itsGnDPLLength`: 8 entries of the duplicate packet list per source [Annex H, Annex A.2].
pub const DPL_LENGTH: usize = 8;
/// `itsGnMaxGeoAreaSize`: 10 km² [Annex H].
pub const MAX_GEO_AREA_SIZE_KM2: f64 = 10.0;

/// `itsGnNonAreaForwardingAlgorithm` [Annex H]: how a packet travels *towards* its
/// destination area or destination node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NonAreaForwarding {
    /// Greedy forwarding [Annex E.2]: unicast to the neighbour with the most progress.
    /// The standard's default.
    Greedy,
    /// Non-area contention-based forwarding [Annex E.3]: a link-layer broadcast; every
    /// receiver with positive progress contends with a timer that shrinks with progress.
    Cbf,
}

/// `itsGnAreaForwardingAlgorithm` [Annex H]: how a packet spreads *inside* its area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AreaForwarding {
    /// Simple GeoBroadcast forwarding with line forwarding [Annex F.2]: every first
    /// reception is rebroadcast at once (flooding bounded by duplicate detection).
    Simple,
    /// Contention-based forwarding [Annex F.3]: buffer, wait `TO_CBF(dist)`, rebroadcast
    /// unless a duplicate was heard first. Vanetza's default, and the one Amador et al.
    /// (2022) evaluate.
    Cbf,
}

/// The MIB a router runs with.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GnMib {
    /// `itsGnDefaultHopLimit`.
    pub default_hop_limit: u8,
    /// `itsGnLifetimeLocTE`, ms.
    pub lifetime_loc_te_ms: u64,
    /// `itsGnBeaconServiceRetransmitTimer`, ms. Zero disables beaconing.
    pub beacon_retransmit_ms: u64,
    /// `itsGnBeaconServiceMaxJitter`, ms.
    pub beacon_max_jitter_ms: u64,
    /// `itsGnDefaultPacketLifetime`, ms.
    pub default_packet_lifetime_ms: u64,
    /// `itsGnDefaultMaxCommunicationRange`, m (`DIST_MAX`).
    pub max_comm_range_m: f64,
    /// CBF minimum timeout, ms (`TO_CBF_MIN`).
    pub cbf_min_ms: u64,
    /// CBF maximum timeout, ms (`TO_CBF_MAX`).
    pub cbf_max_ms: u64,
    /// `itsGnNonAreaForwardingAlgorithm`.
    pub non_area_forwarding: NonAreaForwarding,
    /// `itsGnAreaForwardingAlgorithm`.
    pub area_forwarding: AreaForwarding,
    /// `itsGnDPLLength`.
    pub dpl_length: usize,
}

impl Default for GnMib {
    fn default() -> Self {
        GnMib {
            default_hop_limit: DEFAULT_HOP_LIMIT,
            lifetime_loc_te_ms: LIFETIME_LOC_TE_MS,
            beacon_retransmit_ms: BEACON_RETRANSMIT_MS,
            beacon_max_jitter_ms: BEACON_MAX_JITTER_MS,
            default_packet_lifetime_ms: DEFAULT_PACKET_LIFETIME_MS,
            max_comm_range_m: DEFAULT_MAX_COMM_RANGE_M,
            cbf_min_ms: CBF_MIN_TIME_MS,
            cbf_max_ms: CBF_MAX_TIME_MS,
            non_area_forwarding: NonAreaForwarding::Greedy,
            area_forwarding: AreaForwarding::Cbf,
            dpl_length: DPL_LENGTH,
        }
    }
}

impl GnMib {
    /// The CBF contention timeout for a receiver `dist_m` from the sender (area CBF,
    /// Annex F.3) or with `dist_m` of progress (non-area CBF, Annex E.3), in ns:
    ///
    /// `TO_CBF = TO_CBF_MAX + (TO_CBF_MIN - TO_CBF_MAX) / DIST_MAX × DIST` for
    /// `DIST <= DIST_MAX`, else `TO_CBF_MIN`.
    ///
    /// The farther the receiver, the sooner it fires — so the packet makes the most
    /// progress per hop, and the nearer contenders hear it and cancel.
    pub fn cbf_timeout_ns(&self, dist_m: f64) -> u64 {
        let min = (self.cbf_min_ms * 1_000_000) as f64;
        let max = (self.cbf_max_ms * 1_000_000) as f64;
        let d = dist_m.max(0.0);
        if d >= self.max_comm_range_m || self.max_comm_range_m <= 0.0 {
            return min as u64;
        }
        (max + (min - max) / self.max_comm_range_m * d).round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbf_timer_runs_from_max_to_min_over_dist_max() {
        let m = GnMib::default();
        assert_eq!(m.cbf_timeout_ns(0.0), 100_000_000);
        assert_eq!(m.cbf_timeout_ns(1_000.0), 1_000_000);
        assert_eq!(m.cbf_timeout_ns(5_000.0), 1_000_000);
        // Halfway: 100 + (1 - 100)/1000 × 500 = 50.5 ms.
        assert_eq!(m.cbf_timeout_ns(500.0), 50_500_000);
    }
}
