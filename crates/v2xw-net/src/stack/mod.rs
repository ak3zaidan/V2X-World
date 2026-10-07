//! `stack` — the V2X networking stacks as **protocols**, not just header sizes.
//!
//! The rest of this crate counts octets (see the crate docs). This module runs the
//! protocols' state machines: who forwards a packet, when, and why not. Artery and Vanetza
//! are the reference implementations; where this module's behaviour differs from theirs,
//! the module that differs says so.
//!
//! | Concern | Module | Specification |
//! |---|---|---|
//! | GeoNetworking MIB constants (Annex H) | [`mib`] | EN 302 636-4-1 V1.4.1 Annex H |
//! | Geographical areas and the geometric function F | [`area`] | EN 302 931 V1.1.1 §5 |
//! | Addresses, position vectors, the GN packet and its BTP header | [`packet`] | EN 302 636-4-1 §8-9, EN 302 636-5-1 §7 |
//! | Location table and duplicate packet list | [`loct`] | EN 302 636-4-1 §8.1, Annex A.2 |
//! | The GeoAdhoc router: SHB, TSB, GBC, GUC, beacons, greedy and CBF forwarding | [`router`] | EN 302 636-4-1 §10, Annexes D-F |
//! | BTP demultiplexing | [`btp`] | EN 302 636-5-1 V2.2.1 §7 |
//! | CA basic service generation rules | [`ca`] | EN 302 637-2 V1.4.1 §6.1.3 |
//! | DEN basic service: trigger, update, terminate, repetition, keep-alive forwarding | [`den`] | EN 302 637-3 V1.3.1 §6, §8 |
//! | IEEE 1609.3 WSA and 1609.4 channel switching | [`wave`] | IEEE 1609.3-2020 §8, 1609.4-2016 §6.2.5 |
//! | A discrete-event dissemination run over moving nodes, and its metrics | [`dissem`] | — |
//! | The comparison with a published GeoNetworking evaluation | [`eval`] | Amador et al., Veh. Commun. 34 (2022) |
//!
//! # Determinism
//!
//! Every map is a `BTreeMap`; the event queue orders by `(time, sequence)`; the only
//! randomness (reception draws, beacon and keep-alive jitter) is drawn from a
//! [`v2xw_core::rng::RngStream`] the caller derives, so a dissemination run is a pure
//! function of its inputs and seed.

pub mod area;
pub mod btp;
pub mod ca;
pub mod den;
pub mod dissem;
pub mod eval;
pub mod loct;
pub mod mib;
pub mod packet;
pub mod router;
pub mod wave;

pub use area::{Area, AreaShape};
pub use dissem::{
    ActionStats, DenParams, DissemConfig, DissemReport, DisseminationRun, DistancePdr, Hazard,
    Medium, MotionNode, UnitDisk, UrbanGridMedium,
};
pub use mib::{AreaForwarding, GnMib, NonAreaForwarding};
pub use packet::{GnAddr, GnPacket, PacketKey, PositionVector};
pub use router::{CbfVariant, ForwardingConfig, Router, RouterAction};

/// Which networking stack a scenario runs over the radio: the selector a scenario's
/// `net.stack` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StackKind {
    /// IEEE 1609.3 WSMP, single hop, optionally with 1609.4 channel switching.
    Wave,
    /// ETSI GeoNetworking + BTP, multi-hop.
    GeoNetworking,
}

impl StackKind {
    /// The name this stack serialises and reports as.
    pub const fn as_str(self) -> &'static str {
        match self {
            StackKind::Wave => "wave",
            StackKind::GeoNetworking => "geonetworking",
        }
    }
}
