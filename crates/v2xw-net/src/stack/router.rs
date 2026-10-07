//! The GeoAdhoc router [EN 302 636-4-1 V1.4.1 §10]: packet handling for beacons, SHB,
//! TSB, GeoBroadcast and GeoUnicast, with greedy forwarding [Annex E.2], non-area CBF
//! [Annex E.3], simple area forwarding [Annex F.2] and area CBF [Annex F.3].
//!
//! The router is a pure state machine. It never reads a clock or a random number: every
//! call takes `now`, and every jitter it needs is passed in. It answers each event with a
//! list of [`RouterAction`]s the caller carries out (put a frame on the air, deliver up,
//! arm a timer). A cancelled CBF timer is not "disarmed" — the caller may still fire it,
//! and [`Router::on_cbf_timer`] then finds nothing buffered and does nothing.
//!
//! # The three CBF variants
//!
//! Amador, Urueña, Calderon and Soto, *Evaluation and improvement of ETSI ITS
//! Contention-Based Forwarding (CBF) of warning messages in highway scenarios*, Vehicular
//! Communications 34 (2022), arXiv:2403.05994, evaluate CBF in Artery/Vanetza and find:
//!
//! * [`CbfVariant::Etsi`] — CBF as their baseline ran it: a copy that arrives *after* a
//!   station already forwarded the packet finds an empty CBF buffer and is buffered
//!   again, because no duplicate packet detection runs in front of CBF. Forwarding
//!   overhead explodes (their Table 2).
//! * [`CbfVariant::Dpd`] — duplicate packet detection [Annex A.2] in front of CBF: a
//!   station forwards each packet at most once.
//! * [`CbfVariant::Gpc`] — DPD plus *geographically-aware packet cancellation*: a
//!   duplicate cancels the contention only if its sender is farther from the source than
//!   this station (it has already carried the packet past us); a duplicate from behind
//!   does not, so the wavefront does not stall. This is this module's reading of their
//!   GPC; their exact rule is in §5 of the paper.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::area::Area;
use super::loct::LocationTable;
use super::mib::{AreaForwarding, GnMib, NonAreaForwarding};
use super::packet::{Destination, GnAddr, GnPacket, PacketKey, PositionVector};
use crate::netlayer::{BtpPort, GnTransport};

/// Which CBF duplicate handling to run (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CbfVariant {
    /// No duplicate detection in front of CBF (Amador et al.'s ETSI baseline).
    Etsi,
    /// Duplicate packet detection in front of CBF.
    Dpd,
    /// DPD plus geographically-aware cancellation.
    Gpc,
}

/// The router's configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ForwardingConfig {
    /// The MIB.
    pub mib: GnMib,
    /// CBF duplicate handling.
    pub cbf_variant: CbfVariant,
    /// Whether a station *outside* the destination area forwards a GBC it received as a
    /// link-layer broadcast (the standard: yes, by line forwarding towards the area;
    /// Amador et al.'s improvement: no).
    pub out_of_area_broadcast_forward: bool,
}

impl Default for ForwardingConfig {
    fn default() -> Self {
        ForwardingConfig {
            mib: GnMib::default(),
            cbf_variant: CbfVariant::Dpd,
            out_of_area_broadcast_forward: true,
        }
    }
}

/// The link-layer destination of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkDest {
    /// Broadcast: every station in range processes the frame.
    Broadcast,
    /// Unicast: only this station processes it (others discard at the MAC).
    Unicast(GnAddr),
}

/// Why a router did not deliver or forward a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DropReason {
    /// Seen before [Annex A.2].
    Duplicate,
    /// A duplicate arrived while the packet was in the CBF buffer: contention lost.
    CbfCancelled,
    /// The remaining hop limit ran out.
    HopLimit,
    /// The packet outlived its lifetime.
    LifetimeExpired,
    /// Greedy forwarding found no neighbour closer to the destination [Annex E.2].
    LocalOptimum,
    /// Outside the area and configured not to forward link-layer broadcasts.
    OutOfArea,
    /// A unicast frame addressed to another station.
    NotAddressed,
    /// A GeoUnicast to a station whose position is unknown (no location service here).
    UnknownDestination,
    /// This station is the packet's source.
    OwnPacket,
}

/// What the caller must do.
#[derive(Debug, Clone, PartialEq)]
pub enum RouterAction {
    /// Put `packet` on the air to `to` now.
    Transmit {
        /// The packet as forwarded (hop limit already decremented).
        packet: GnPacket,
        /// Link-layer destination.
        to: LinkDest,
        /// True for the source's own transmission; false for a forward.
        originated: bool,
    },
    /// Hand `packet` up to BTP.
    Deliver {
        /// The packet.
        packet: GnPacket,
    },
    /// Call [`Router::on_cbf_timer`] with `key` at `at_ns`.
    ArmCbfTimer {
        /// The buffered packet.
        key: PacketKey,
        /// When the contention timer expires, ns.
        at_ns: u64,
    },
    /// The packet stops here.
    Drop {
        /// The packet.
        key: PacketKey,
        /// Why.
        reason: DropReason,
    },
}

/// One packet waiting in the CBF buffer.
#[derive(Debug, Clone)]
struct CbfEntry {
    packet: GnPacket,
    to: LinkDest,
}

/// Counters a router keeps for the metric layer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouterCounters {
    /// Packets originated.
    pub originated: u64,
    /// Packets forwarded.
    pub forwarded: u64,
    /// Beacons sent.
    pub beacons: u64,
    /// Packets delivered up.
    pub delivered: u64,
    /// Duplicates discarded.
    pub duplicates: u64,
    /// CBF contentions cancelled by a duplicate.
    pub cbf_cancelled: u64,
    /// Greedy local optima.
    pub local_optimum: u64,
}

/// One station's GeoAdhoc router.
#[derive(Debug, Clone)]
pub struct Router {
    addr: GnAddr,
    cfg: ForwardingConfig,
    ego: PositionVector,
    loct: LocationTable,
    sn: u16,
    cbf: BTreeMap<PacketKey, CbfEntry>,
    next_beacon_ns: u64,
    counters: RouterCounters,
}

impl Router {
    /// A router for station `addr`, first beacon due at `first_beacon_ns`.
    pub fn new(addr: GnAddr, cfg: ForwardingConfig, first_beacon_ns: u64) -> Self {
        Router {
            addr,
            cfg,
            ego: PositionVector { addr, t_ns: 0, x: 0.0, y: 0.0, speed: 0.0, heading_deg: 0.0 },
            loct: LocationTable::new(cfg.mib.lifetime_loc_te_ms, cfg.mib.dpl_length),
            sn: 0,
            cbf: BTreeMap::new(),
            next_beacon_ns: first_beacon_ns,
            counters: RouterCounters::default(),
        }
    }

    /// This station's address.
    pub fn addr(&self) -> GnAddr {
        self.addr
    }

    /// The configuration.
    pub fn config(&self) -> &ForwardingConfig {
        &self.cfg
    }

    /// The counters so far.
    pub fn counters(&self) -> RouterCounters {
        self.counters
    }

    /// The location table.
    pub fn location_table(&self) -> &LocationTable {
        &self.loct
    }

    /// Update the ego position vector (the caller's positioning, before each event).
    pub fn set_ego(&mut self, x: f64, y: f64, speed: f64, heading_deg: f64, now_ns: u64) {
        self.ego = PositionVector { addr: self.addr, t_ns: now_ns.max(1), x, y, speed, heading_deg };
    }

    /// The ego position vector.
    pub fn ego(&self) -> PositionVector {
        self.ego
    }

    /// When the next beacon is due, ns.
    pub fn next_beacon_ns(&self) -> u64 {
        self.next_beacon_ns
    }

    /// The beacon service [§10.3.6]: if a beacon is due, build it and schedule the next one
    /// `itsGnBeaconServiceRetransmitTimer + jitter_ns` later.
    pub fn beacon(&mut self, now_ns: u64, jitter_ns: u64) -> Option<GnPacket> {
        if self.cfg.mib.beacon_retransmit_ms == 0 || now_ns < self.next_beacon_ns {
            return None;
        }
        self.next_beacon_ns = now_ns + self.cfg.mib.beacon_retransmit_ms * 1_000_000 + jitter_ns;
        self.counters.beacons += 1;
        Some(GnPacket {
            transport: GnTransport::Beacon,
            source: self.ego,
            sn: 0,
            rhl: 1,
            mhl: 1,
            lifetime_ms: self.cfg.mib.default_packet_lifetime_ms,
            dest: Destination::SingleHop,
            btp_port: BtpPort(0),
            payload_bytes: 0,
            payload_tag: 0,
        })
    }

    fn next_sn(&mut self) -> u16 {
        self.sn = self.sn.wrapping_add(1);
        self.sn
    }

    fn new_packet(&mut self, transport: GnTransport, dest: Destination, port: BtpPort, bytes: u32, tag: u64, hops: u8) -> GnPacket {
        let sn = if matches!(transport, GnTransport::Shb | GnTransport::Beacon) { 0 } else { self.next_sn() };
        GnPacket {
            transport,
            source: self.ego,
            sn,
            rhl: hops,
            mhl: hops,
            lifetime_ms: self.cfg.mib.default_packet_lifetime_ms,
            dest,
            btp_port: port,
            payload_bytes: bytes,
            payload_tag: tag,
        }
    }

    /// Single-hop broadcast [§10.3.10.2]; also postpones the next beacon by a full period
    /// (an SHB carries the same position vector a beacon would).
    pub fn originate_shb(&mut self, port: BtpPort, bytes: u32, tag: u64, now_ns: u64) -> Vec<RouterAction> {
        let p = self.new_packet(GnTransport::Shb, Destination::SingleHop, port, bytes, tag, 1);
        self.next_beacon_ns = now_ns + self.cfg.mib.beacon_retransmit_ms * 1_000_000;
        self.counters.originated += 1;
        vec![RouterAction::Transmit { packet: p, to: LinkDest::Broadcast, originated: true }]
    }

    /// Topologically scoped broadcast [§10.3.9.2].
    pub fn originate_tsb(&mut self, port: BtpPort, bytes: u32, tag: u64, hops: u8, now_ns: u64) -> Vec<RouterAction> {
        let p = self.new_packet(GnTransport::Tsb, Destination::Topological, port, bytes, tag, hops);
        self.loct.check_and_record(self.addr, p.sn, now_ns);
        self.counters.originated += 1;
        vec![RouterAction::Transmit { packet: p, to: LinkDest::Broadcast, originated: true }]
    }

    /// GeoBroadcast [§10.3.11.2]: inside the area the source broadcasts; outside, it
    /// forwards towards the area with the non-area algorithm.
    pub fn originate_gbc(&mut self, area: Area, port: BtpPort, bytes: u32, tag: u64, now_ns: u64) -> Vec<RouterAction> {
        let hops = self.cfg.mib.default_hop_limit;
        let p = self.new_packet(GnTransport::Gbc, Destination::Area(area), port, bytes, tag, hops);
        self.loct.check_and_record(self.addr, p.sn, now_ns);
        self.counters.originated += 1;
        if area.contains(self.ego.x, self.ego.y) {
            return vec![RouterAction::Transmit { packet: p, to: LinkDest::Broadcast, originated: true }];
        }
        match self.greedy_next_hop(area.cx, area.cy, now_ns) {
            Some(nh) => vec![RouterAction::Transmit { packet: p, to: LinkDest::Unicast(nh), originated: true }],
            // At a local optimum the source still broadcasts: every neighbour then decides.
            None => vec![RouterAction::Transmit { packet: p, to: LinkDest::Broadcast, originated: true }],
        }
    }

    /// GeoUnicast [§10.3.8.2] to `dest`, whose position must be in the location table
    /// (this module has no location service).
    pub fn originate_guc(&mut self, dest: GnAddr, port: BtpPort, bytes: u32, tag: u64, now_ns: u64) -> Vec<RouterAction> {
        let Some(de) = self.loct.get(dest, now_ns).map(|e| e.pv) else {
            return vec![RouterAction::Drop { key: PacketKey { source: self.addr, sn: self.sn }, reason: DropReason::UnknownDestination }];
        };
        let hops = self.cfg.mib.default_hop_limit;
        let p = self.new_packet(GnTransport::Guc, Destination::Node(de), port, bytes, tag, hops);
        self.loct.check_and_record(self.addr, p.sn, now_ns);
        self.counters.originated += 1;
        self.forward_non_area(p, de.x, de.y, None, now_ns, true)
    }

    /// The neighbour closest to `(tx, ty)` that is closer than this station
    /// [Annex E.2, most-forward-within-radius].
    fn greedy_next_hop(&self, tx: f64, ty: f64, now_ns: u64) -> Option<GnAddr> {
        let mine = self.ego.dist(tx, ty);
        let mut best: Option<(f64, GnAddr)> = None;
        for e in self.loct.neighbours(now_ns) {
            if e.pv.addr == self.addr {
                continue;
            }
            let d = e.pv.dist(tx, ty);
            if d < mine && best.is_none_or(|(bd, ba)| d < bd || (d == bd && e.pv.addr < ba)) {
                best = Some((d, e.pv.addr));
            }
        }
        best.map(|(_, a)| a)
    }

    fn forward_non_area(&mut self, packet: GnPacket, tx: f64, ty: f64, sender: Option<GnAddr>, now_ns: u64, originated: bool) -> Vec<RouterAction> {
        let key = packet.key();
        match self.cfg.mib.non_area_forwarding {
            NonAreaForwarding::Greedy => match self.greedy_next_hop(tx, ty, now_ns) {
                Some(nh) => {
                    if !originated {
                        self.counters.forwarded += 1;
                    }
                    vec![RouterAction::Transmit { packet, to: LinkDest::Unicast(nh), originated }]
                }
                None => {
                    self.counters.local_optimum += 1;
                    vec![RouterAction::Drop { key, reason: DropReason::LocalOptimum }]
                }
            },
            NonAreaForwarding::Cbf => {
                if originated {
                    return vec![RouterAction::Transmit { packet, to: LinkDest::Broadcast, originated }];
                }
                let sender_pv = sender.and_then(|s| self.loct.get(s, now_ns)).map(|e| e.pv);
                let progress = match sender_pv {
                    Some(pv) => pv.dist(tx, ty) - self.ego.dist(tx, ty),
                    None => 0.0,
                };
                if progress <= 0.0 {
                    self.counters.local_optimum += 1;
                    return vec![RouterAction::Drop { key, reason: DropReason::LocalOptimum }];
                }
                let at = now_ns + self.cfg.mib.cbf_timeout_ns(progress);
                self.cbf.insert(key, CbfEntry { packet, to: LinkDest::Broadcast });
                vec![RouterAction::ArmCbfTimer { key, at_ns: at }]
            }
        }
    }

    /// A frame arrived from `sender` (the link-layer source) addressed to `link_dest`.
    pub fn receive(&mut self, packet: &GnPacket, sender: GnAddr, link_dest: LinkDest, now_ns: u64) -> Vec<RouterAction> {
        let key = packet.key();
        if let LinkDest::Unicast(a) = link_dest {
            if a != self.addr {
                return vec![RouterAction::Drop { key, reason: DropReason::NotAddressed }];
            }
        }
        if packet.source.addr == self.addr {
            return vec![RouterAction::Drop { key, reason: DropReason::OwnPacket }];
        }
        if packet.source.t_ns + packet.lifetime_ms * 1_000_000 < now_ns {
            return vec![RouterAction::Drop { key, reason: DropReason::LifetimeExpired }];
        }
        // The source's position vector updates the location table; it marks a neighbour
        // only when the source is the sender (a first hop, a beacon or an SHB).
        self.loct.update(packet.source, packet.source.addr == sender, now_ns);
        match packet.transport {
            GnTransport::Beacon => vec![],
            GnTransport::Shb => {
                self.counters.delivered += 1;
                vec![RouterAction::Deliver { packet: packet.clone() }]
            }
            GnTransport::Tsb => {
                if self.loct.check_and_record(key.source, key.sn, now_ns) {
                    self.counters.duplicates += 1;
                    return vec![RouterAction::Drop { key, reason: DropReason::Duplicate }];
                }
                self.counters.delivered += 1;
                let mut out = vec![RouterAction::Deliver { packet: packet.clone() }];
                out.push(self.decrement_and_broadcast(packet));
                out
            }
            GnTransport::Gbc | GnTransport::Gac => self.receive_gbc(packet, sender, link_dest, now_ns),
            GnTransport::Guc => self.receive_guc(packet, sender, now_ns),
        }
    }

    fn decrement_and_broadcast(&mut self, packet: &GnPacket) -> RouterAction {
        if packet.rhl <= 1 {
            return RouterAction::Drop { key: packet.key(), reason: DropReason::HopLimit };
        }
        let mut p = packet.clone();
        p.rhl -= 1;
        self.counters.forwarded += 1;
        RouterAction::Transmit { packet: p, to: LinkDest::Broadcast, originated: false }
    }

    fn receive_guc(&mut self, packet: &GnPacket, sender: GnAddr, now_ns: u64) -> Vec<RouterAction> {
        let key = packet.key();
        let cbf = self.cfg.mib.non_area_forwarding == NonAreaForwarding::Cbf;
        if cbf && self.cbf.remove(&key).is_some() {
            self.counters.cbf_cancelled += 1;
            return vec![RouterAction::Drop { key, reason: DropReason::CbfCancelled }];
        }
        if self.loct.check_and_record(key.source, key.sn, now_ns) {
            self.counters.duplicates += 1;
            return vec![RouterAction::Drop { key, reason: DropReason::Duplicate }];
        }
        let Destination::Node(mut de) = packet.dest else {
            return vec![];
        };
        if de.addr == self.addr {
            self.counters.delivered += 1;
            return vec![RouterAction::Deliver { packet: packet.clone() }];
        }
        if packet.rhl <= 1 {
            return vec![RouterAction::Drop { key, reason: DropReason::HopLimit }];
        }
        // A fresher DE PV from our own table replaces the header's [§10.3.8.3].
        if let Some(e) = self.loct.get(de.addr, now_ns) {
            if e.pv.t_ns > de.t_ns {
                de = e.pv;
            }
        }
        let mut p = packet.clone();
        p.rhl -= 1;
        p.dest = Destination::Node(de);
        self.forward_non_area(p, de.x, de.y, Some(sender), now_ns, false)
    }

    fn receive_gbc(&mut self, packet: &GnPacket, sender: GnAddr, link_dest: LinkDest, now_ns: u64) -> Vec<RouterAction> {
        let key = packet.key();
        let Destination::Area(area) = packet.dest else {
            return vec![];
        };
        let cbf_in_use = self.cfg.mib.area_forwarding == AreaForwarding::Cbf
            || self.cfg.mib.non_area_forwarding == NonAreaForwarding::Cbf;
        // A copy while contending: the CBF cancellation rule.
        if cbf_in_use && self.cbf.contains_key(&key) {
            let cancel = match self.cfg.cbf_variant {
                CbfVariant::Etsi | CbfVariant::Dpd => true,
                CbfVariant::Gpc => {
                    // Cancel only if the sender is farther from the source than we are.
                    match self.loct.get(sender, now_ns) {
                        Some(e) => {
                            e.pv.dist(packet.source.x, packet.source.y)
                                >= self.ego.dist(packet.source.x, packet.source.y)
                        }
                        None => true,
                    }
                }
            };
            if cancel {
                self.cbf.remove(&key);
                self.counters.cbf_cancelled += 1;
                return vec![RouterAction::Drop { key, reason: DropReason::CbfCancelled }];
            }
            self.counters.duplicates += 1;
            return vec![RouterAction::Drop { key, reason: DropReason::Duplicate }];
        }
        // Duplicate packet detection. The ETSI-baseline CBF runs without it, so a copy
        // after our own forward is taken as new; simple forwarding always needs it.
        let run_dpd = !(cbf_in_use && self.cfg.cbf_variant == CbfVariant::Etsi);
        let seen = self.loct.check_and_record(key.source, key.sn, now_ns);
        if run_dpd && seen {
            self.counters.duplicates += 1;
            return vec![RouterAction::Drop { key, reason: DropReason::Duplicate }];
        }
        let inside = area.contains(self.ego.x, self.ego.y);
        let mut out = Vec::new();
        if inside {
            self.counters.delivered += 1;
            out.push(RouterAction::Deliver { packet: packet.clone() });
        }
        if packet.rhl <= 1 {
            out.push(RouterAction::Drop { key, reason: DropReason::HopLimit });
            return out;
        }
        let mut p = packet.clone();
        p.rhl -= 1;
        if inside {
            match self.cfg.mib.area_forwarding {
                AreaForwarding::Simple => {
                    self.counters.forwarded += 1;
                    out.push(RouterAction::Transmit { packet: p, to: LinkDest::Broadcast, originated: false });
                }
                AreaForwarding::Cbf => {
                    let dist = self
                        .loct
                        .get(sender, now_ns)
                        .map(|e| e.pv.dist(self.ego.x, self.ego.y));
                    let to = match dist {
                        Some(d) => self.cfg.mib.cbf_timeout_ns(d),
                        None => self.cfg.mib.cbf_max_ms * 1_000_000,
                    };
                    self.cbf.insert(key, CbfEntry { packet: p, to: LinkDest::Broadcast });
                    out.push(RouterAction::ArmCbfTimer { key, at_ns: now_ns + to });
                }
            }
            return out;
        }
        if link_dest == LinkDest::Broadcast && !self.cfg.out_of_area_broadcast_forward {
            out.push(RouterAction::Drop { key, reason: DropReason::OutOfArea });
            return out;
        }
        out.extend(self.forward_non_area(p, area.cx, area.cy, Some(sender), now_ns, false));
        out
    }

    /// The contention timer for `key` expired: rebroadcast, unless it was cancelled.
    pub fn on_cbf_timer(&mut self, key: PacketKey, _now_ns: u64) -> Vec<RouterAction> {
        match self.cbf.remove(&key) {
            Some(e) => {
                self.counters.forwarded += 1;
                vec![RouterAction::Transmit { packet: e.packet, to: e.to, originated: false }]
            }
            None => vec![],
        }
    }

    /// Packets currently contending.
    pub fn cbf_buffered(&self) -> usize {
        self.cbf.len()
    }

    /// Purge expired location table entries.
    pub fn purge(&mut self, now_ns: u64) {
        self.loct.purge(now_ns);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn router(addr: u64, x: f64, cfg: ForwardingConfig) -> Router {
        let mut r = Router::new(GnAddr(addr), cfg, 0);
        r.set_ego(x, 0.0, 0.0, 0.0, 1);
        r
    }

    fn hear(r: &mut Router, from: &Router, now: u64) {
        let pv = from.ego();
        r.loct.update(pv, true, now);
    }

    #[test]
    fn far_receiver_contends_shorter_and_a_duplicate_cancels() {
        let cfg = ForwardingConfig::default();
        let mut src = router(1, 0.0, cfg);
        let mut near = router(2, 100.0, cfg);
        let mut far = router(3, 600.0, cfg);
        hear(&mut near, &src, 1);
        hear(&mut far, &src, 1);
        hear(&mut near, &far, 1);
        let area = Area::rect(-10.0, -10.0, 2_000.0, 10.0);
        let RouterAction::Transmit { packet, .. } = src.originate_gbc(area, BtpPort::DENM, 300, 0, 10).remove(0) else { panic!() };
        let an = near.receive(&packet, GnAddr(1), LinkDest::Broadcast, 10);
        let af = far.receive(&packet, GnAddr(1), LinkDest::Broadcast, 10);
        let t = |a: &[RouterAction]| a.iter().find_map(|x| match x { RouterAction::ArmCbfTimer { at_ns, .. } => Some(*at_ns), _ => None }).unwrap();
        assert!(t(&af) < t(&an));
        let fwd = far.on_cbf_timer(packet.key(), t(&af));
        let RouterAction::Transmit { packet: p2, .. } = &fwd[0] else { panic!() };
        assert_eq!(p2.rhl, packet.rhl - 1);
        let c = near.receive(p2, GnAddr(3), LinkDest::Broadcast, t(&af));
        assert!(matches!(c[0], RouterAction::Drop { reason: DropReason::CbfCancelled, .. }));
        assert!(near.on_cbf_timer(packet.key(), t(&an)).is_empty());
    }

    #[test]
    fn etsi_variant_rebuffers_a_late_copy_dpd_does_not() {
        for (variant, rebuffers) in [(CbfVariant::Etsi, true), (CbfVariant::Dpd, false)] {
            let cfg = ForwardingConfig { cbf_variant: variant, ..Default::default() };
            let mut src = router(1, 0.0, cfg);
            let mut r = router(2, 300.0, cfg);
            let area = Area::rect(-10.0, -10.0, 2_000.0, 10.0);
            let RouterAction::Transmit { packet, .. } = src.originate_gbc(area, BtpPort::DENM, 300, 0, 10).remove(0) else { panic!() };
            r.receive(&packet, GnAddr(1), LinkDest::Broadcast, 10);
            assert_eq!(r.on_cbf_timer(packet.key(), 200_000_000).len(), 1);
            let again = r.receive(&packet, GnAddr(9), LinkDest::Broadcast, 300_000_000);
            let armed = again.iter().any(|a| matches!(a, RouterAction::ArmCbfTimer { .. }));
            assert_eq!(armed, rebuffers, "{variant:?}");
        }
    }

    #[test]
    fn greedy_picks_most_progress_and_reports_local_optimum() {
        let cfg = ForwardingConfig::default();
        let mut s = router(1, 0.0, cfg);
        let a = router(2, 200.0, cfg);
        let b = router(3, 400.0, cfg);
        hear(&mut s, &a, 1);
        hear(&mut s, &b, 1);
        let far_area = Area::circle(5_000.0, 0.0, 100.0);
        let acts = s.originate_gbc(far_area, BtpPort::DENM, 100, 0, 5);
        assert!(matches!(acts[0], RouterAction::Transmit { to: LinkDest::Unicast(GnAddr(3)), .. }));
        let behind = Area::circle(-5_000.0, 0.0, 100.0);
        let mut lone = router(4, 0.0, cfg);
        hear(&mut lone, &a, 1);
        let RouterAction::Transmit { packet, .. } = s.originate_gbc(behind, BtpPort::DENM, 100, 0, 6).remove(0) else { panic!() };
        let out = lone.receive(&packet, GnAddr(1), LinkDest::Unicast(GnAddr(4)), 6);
        assert!(out.iter().any(|x| matches!(x, RouterAction::Drop { reason: DropReason::LocalOptimum, .. })));
    }
}
