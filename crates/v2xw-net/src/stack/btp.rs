//! BTP [EN 302 636-5-1 V2.2.1 §7]: the transport header over GeoNetworking and the port
//! demultiplexer that hands a delivered packet to its facility.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::netlayer::{BtpKind, BtpPort};

/// A decoded BTP header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtpHeader {
    /// BTP-A (interactive) or BTP-B (non-interactive) [§7.2-7.3].
    pub kind: BtpKind,
    /// Destination port.
    pub dest_port: BtpPort,
    /// BTP-A: the source port; BTP-B: the destination port info (0 when unused).
    pub second: u16,
}

impl BtpHeader {
    /// A BTP-B header to `port`, the form every broadcast facility uses.
    pub const fn b(port: BtpPort) -> Self {
        BtpHeader {
            kind: BtpKind::B,
            dest_port: port,
            second: 0,
        }
    }

    /// The four header octets as on the wire: two big-endian 16-bit fields [§7.2-7.3].
    pub fn encode(&self) -> [u8; 4] {
        let d = self.dest_port.0.to_be_bytes();
        let s = self.second.to_be_bytes();
        [d[0], d[1], s[0], s[1]]
    }

    /// Decode four octets as a header of `kind` (the GN common header's next-header field
    /// says which).
    pub fn decode(kind: BtpKind, b: [u8; 4]) -> Self {
        BtpHeader {
            kind,
            dest_port: BtpPort(u16::from_be_bytes([b[0], b[1]])),
            second: u16::from_be_bytes([b[2], b[3]]),
        }
    }
}

/// Which facility a port is bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Facility {
    /// CA basic service.
    Ca,
    /// DEN basic service.
    Den,
    /// Any other application, by a caller-chosen number.
    Other(u16),
}

/// The port table [§8: the BTP-Data.indication goes to the entity bound to the port].
#[derive(Debug, Clone, Default)]
pub struct PortTable {
    bound: BTreeMap<u16, Facility>,
    /// Packets that arrived for an unbound port (counted, then discarded).
    pub unbound: u64,
}

impl PortTable {
    /// The standard binding: CAM and DENM well-known ports.
    pub fn standard() -> Self {
        let mut t = PortTable::default();
        t.bind(BtpPort::CAM, Facility::Ca);
        t.bind(BtpPort::DENM, Facility::Den);
        t
    }

    /// Bind `port` to `facility`.
    pub fn bind(&mut self, port: BtpPort, facility: Facility) {
        self.bound.insert(port.0, facility);
    }

    /// Demultiplex a delivered packet.
    pub fn demux(&mut self, port: BtpPort) -> Option<Facility> {
        let f = self.bound.get(&port.0).copied();
        if f.is_none() {
            self.unbound += 1;
        }
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trips_and_ports_demux() {
        let h = BtpHeader::b(BtpPort::DENM);
        assert_eq!(h.encode(), [0x07, 0xD2, 0, 0]);
        assert_eq!(BtpHeader::decode(BtpKind::B, h.encode()), h);
        let mut t = PortTable::standard();
        assert_eq!(t.demux(BtpPort::CAM), Some(Facility::Ca));
        assert_eq!(t.demux(BtpPort(9)), None);
        assert_eq!(t.unbound, 1);
    }
}
