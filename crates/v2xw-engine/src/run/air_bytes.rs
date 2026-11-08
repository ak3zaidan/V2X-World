//! Where the bytes on the air go: every transmitted frame's octets by layer, the PHY's
//! own time, and the share multi-hop forwarding adds — the run summary's ledger.
//!
//! The per-message-type ratios are the `overhead` metric family's
//! (`v2xw_metrics::overhead`); this is the whole-run total a researcher reads first.
//! Each frame is counted once, when it leaves the air, from the layers it was built with
//! (`v2xw_net::FrameLayers`), so the layer columns sum to the PSDU octets exactly.
//!
//! The PHY time per frame is the 802.11p preamble and SIGNAL field at 10 MHz: 32 µs of
//! training symbols plus one 8 µs SIGNAL symbol (IEEE 802.11-2020 §17.4.3, Table 17-21,
//! half-clocked), carried by every frame whatever its length.

use super::FrameState;

/// The 802.11p PLCP preamble and SIGNAL at 10 MHz, µs (IEEE 802.11-2020 Table 17-21).
pub const PHY_PREAMBLE_SIGNAL_US: u64 = 40;

/// One message type's frames on the air ([`super::RunReport::air_by_type`]): what each
/// message family costs the channel. Its `airtime_us` over the run's airtime is the type's
/// share of the channel busy time, so of the CBR (ETSI TS 102 687 4.2: CBR is the busy
/// share of the channel; every frame's air time counts once at each node that senses it).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct TypeAir {
    /// Frames of this type that went on the air.
    pub frames: u64,
    /// Their PSDU octets.
    pub psdu: u64,
    /// The mean PSDU of one frame, octets (the size distribution's centre).
    pub psdu_mean: u64,
    /// Their air time, µs.
    pub airtime_us: u64,
}

impl TypeAir {
    /// Counts one frame of this type.
    pub(super) fn add(&mut self, psdu: u64, airtime_us: u64) {
        self.frames += 1;
        self.psdu += psdu;
        self.airtime_us += airtime_us;
        self.psdu_mean = self.psdu / self.frames;
    }
}

/// Octets on the air by layer over the run, for [`super::RunReport::air_bytes`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct AirBytes {
    /// Frames that went on the air.
    pub frames: u64,
    /// Application (facilities-layer) payload.
    pub payload: u64,
    /// Security envelope: headers, signer identifier or certificate, signature.
    pub security: u64,
    /// SPDUs whose payload/security split is not known (sized from a table).
    pub spdu_unsplit: u64,
    /// Fragmentation headers.
    pub fragmentation: u64,
    /// Network and transport headers (WSMP, or GeoNetworking + BTP).
    pub network: u64,
    /// LLC/SNAP.
    pub llc_snap: u64,
    /// 802.11 MAC header.
    pub mac_header: u64,
    /// Frame check sequence.
    pub fcs: u64,
    /// Every octet above, the PSDUs.
    pub psdu: u64,
    /// Air time of all frames, µs.
    pub airtime_us: u64,
    /// Of it, preamble and SIGNAL, µs.
    pub phy_preamble_us: u64,
    /// PSDU octets of forwarded GeoNetworking copies (`net.geonet.forwarding: on-air`).
    pub gn_forwarded: u64,
    /// The application payload's share of the PSDU octets, ‰.
    pub payload_permille: u64,
}

impl AirBytes {
    /// Counts one frame that left the air.
    pub(super) fn add(&mut self, s: &FrameState) {
        let l = &s.layers;
        self.frames += 1;
        self.payload += u64::from(l.payload);
        self.security += u64::from(l.security);
        self.spdu_unsplit += u64::from(l.spdu_unsplit);
        self.fragmentation += u64::from(l.fragmentation);
        self.network += u64::from(l.network);
        self.llc_snap += u64::from(l.llc_snap);
        self.mac_header += u64::from(l.mac_header);
        self.fcs += u64::from(l.fcs);
        let psdu = u64::from(l.psdu_bytes());
        self.psdu += psdu;
        self.airtime_us += s.air.as_nanos() / 1_000;
        self.phy_preamble_us += PHY_PREAMBLE_SIGNAL_US;
        if s.gn
            .as_ref()
            .is_some_and(|g| g.packet.source.addr != super::gn_air::addr(s.tx))
        {
            self.gn_forwarded += psdu;
        }
        self.payload_permille = (self.payload * 1_000).checked_div(self.psdu).unwrap_or(0);
    }
}
