//! The GeoNetworking packet as the router sees it: decoded fields, not octets.
//!
//! The octet count of each header is [`crate::GnTransport::header_bytes`]; this module
//! carries the fields the forwarding algorithms read (EN 302 636-4-1 §9.6-9.8).

use serde::{Deserialize, Serialize};

use super::area::Area;
use crate::netlayer::{BTP_HEADER_BYTES, BtpPort, GnTransport};

/// A GeoNetworking address [EN 302 636-4-1 §6]: the 48-bit MID is all the router uses,
/// so this is the station's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GnAddr(pub u64);

/// A long position vector [§9.5.2]: address, timestamp, position, speed, heading.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PositionVector {
    /// The station.
    pub addr: GnAddr,
    /// When the position was taken, ns of simulation time.
    pub t_ns: u64,
    /// x east, m.
    pub x: f64,
    /// y north, m.
    pub y: f64,
    /// Speed, m/s.
    pub speed: f64,
    /// Heading, degrees clockwise from north.
    pub heading_deg: f64,
}

impl PositionVector {
    /// Distance to another point, m.
    pub fn dist(&self, x: f64, y: f64) -> f64 {
        v2xw_core::math::hypot(self.x - x, self.y - y)
    }
}

/// Where a packet is going.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Destination {
    /// Single-hop broadcast or beacon: every neighbour, no forwarding.
    SingleHop,
    /// Topologically scoped broadcast: every node within the hop limit.
    Topological,
    /// GeoBroadcast to every node in the area.
    Area(Area),
    /// GeoUnicast to one node, last known at the destination position vector.
    Node(PositionVector),
}

/// The fields of one GeoNetworking packet that the router acts on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GnPacket {
    /// Header type.
    pub transport: GnTransport,
    /// The source's position vector (SO PV).
    pub source: PositionVector,
    /// Sequence number [§9.8.2]; beacons and SHB have none and carry 0.
    pub sn: u16,
    /// Remaining hop limit [§9.7.4].
    pub rhl: u8,
    /// Maximum hop limit as set by the source.
    pub mhl: u8,
    /// Lifetime, ms [§9.6.4].
    pub lifetime_ms: u64,
    /// Destination.
    pub dest: Destination,
    /// BTP destination port (BTP-B).
    pub btp_port: BtpPort,
    /// Payload size above BTP, octets (the SDU the facilities layer handed down).
    pub payload_bytes: u32,
    /// An opaque payload tag the facilities layer uses to match a packet to its message
    /// (for a DENM: the ActionID packed as `station << 16 | sequence`).
    pub payload_tag: u64,
}

impl GnPacket {
    /// The key duplicate detection and the CBF buffer use: `(source, sequence number)`
    /// [Annex A.2].
    pub fn key(&self) -> PacketKey {
        PacketKey { source: self.source.addr, sn: self.sn }
    }

    /// The frame's network-layer octets: GN header + BTP header + payload.
    pub fn bytes(&self) -> u32 {
        self.transport.header_bytes() + BTP_HEADER_BYTES + self.payload_bytes
    }

    /// Hops taken so far.
    pub fn hops(&self) -> u8 {
        self.mhl.saturating_sub(self.rhl)
    }
}

/// `(source address, sequence number)`: a packet's identity across hops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PacketKey {
    /// The originating station.
    pub source: GnAddr,
    /// Its sequence number.
    pub sn: u16,
}
