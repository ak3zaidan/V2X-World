//! GeoNetworking on the air: `net.geonet.forwarding: on-air`.
//!
//! The `overlay` forwarding ([`super::geonet`]) runs the routers at the hazard's instant
//! over a disk of `range_m` with no medium access: every forward reaches the air the
//! moment its router asks, nothing collides, and none of it loads the channel. That makes
//! multi-hop reach optimistic, and the gap grows exactly where a study cares — in dense
//! traffic, where the contention timers of many CBF candidates expire within a few slots.
//!
//! Here every equipped station carries an EN 302 636-4-1 [`Router`] inside the run:
//!
//! * **Origination.** A DENM a node's own DEN service hands down is a GeoBroadcast to a
//!   circle of `area_radius_m` around the sender; every other message (CAM, BSM, CPM, ...)
//!   is a single-hop broadcast, whose source position vector refreshes each receiver's
//!   location table. A station that sends CAMs never beacons: every SHB resets
//!   `itsGnBeaconServiceRetransmitTimer` (EN 302 636-4-1 §10.3.6.2), so no beacons are
//!   generated.
//! * **Reception.** Every frame a receiver decodes — through the run's own PHY, link
//!   budget, interference and collisions — goes to its router.
//! * **Forwarding.** A forward is a **new frame** at the forwarder: a copy of the received
//!   frame (same SPDU: GeoNetworking forwarding does not re-sign the payload) with the
//!   router's packet, queued in the forwarder's EDCA MAC. A CBF contender's copy waits in
//!   the MAC's pending list until its contention timer; when it comes due, the router is
//!   asked whether it still forwards (a copy heard meanwhile cancels it), and only then
//!   is the frame enqueued. From there it is an ordinary frame: it passes the DCC gate,
//!   waits for AIFS and backoff, can collide, is counted in every station's CBR and in
//!   `node.tx`, and its receptions are decided by the PHY.
//!
//! What is not modelled: GeoUnicast's link-layer acknowledgement and retransmission (a
//! greedy next hop is addressed, but the frame is sent once, as a broadcast frame would
//! be); the GN security header's verification at forwarders (the frame's own
//! verification happens at the node, as for any frame); a location service.
//!
//! # Determinism
//!
//! Routers live in a `BTreeMap` by [`NodeId`]; receptions reach them in the PHY's
//! receiver order, which is [`NodeId`] order; the router draws no randomness.

use std::collections::{BTreeMap, BTreeSet};

use v2xw_core::ids::{FrameSeq, NodeId};
use v2xw_core::time::SimTime;
use v2xw_net::BtpPort;
use v2xw_net::stack::router::LinkDest;
use v2xw_net::stack::{
    Area, AreaForwarding, CbfVariant, ForwardingConfig, GnAddr, GnMib, GnPacket, NonAreaForwarding,
    PacketKey, Router, RouterAction,
};
use v2xw_radio::types::AccessCategory;

use super::{Engine, FrameState, GeonetWarning};
use crate::scenario::schema::{Geonet, Scenario};

/// The GeoNetworking state a frame carries when the run forwards on the air.
#[derive(Debug, Clone)]
pub(super) struct GnFrame {
    /// The packet as this transmission carries it.
    pub(super) packet: GnPacket,
    /// Broadcast, or the greedy next hop.
    pub(super) to: LinkDest,
    /// A CBF candidate whose contention timer has not been asked yet.
    pub(super) cbf: bool,
}

/// What the on-air routers did over the run, for [`super::RunReport::gn_air`].
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct GnAirStats {
    /// GeoBroadcast packets the stations originated (each DENM transmission).
    pub gbc_originated: u64,
    /// Forwarded copies handed to a MAC.
    pub forwards_sent: u64,
    /// CBF contention timers armed.
    pub cbf_armed: u64,
    /// CBF candidates that stood down when their timer came due (a copy was heard).
    pub cbf_cancelled: u64,
    /// Forwarded frames' air time, µs: their share of the channel load.
    pub forward_airtime_us: u64,
    /// Packets the routers stopped, by reason (duplicate, hop limit, out of area, ...).
    pub drops: BTreeMap<String, u64>,
}

/// One hazard warning followed on the air.
#[derive(Debug, Clone)]
struct Tracked {
    /// When the hazard fired.
    at: SimTime,
    /// When the origin's first DENM for it went down: the latency's zero.
    first: Option<SimTime>,
    /// The end of the follow window, `first + window_s` (open until the first DENM).
    end: SimTime,
    origin: NodeId,
    addr: GnAddr,
    targets: BTreeSet<NodeId>,
    first_rx: BTreeMap<NodeId, (SimTime, u8)>,
    vehicles: u64,
    originated: u64,
    forwarded: u64,
}

/// The run's on-air GeoNetworking layer.
#[derive(Debug, Clone)]
pub(super) struct GnOnAir {
    fwd: ForwardingConfig,
    area_radius_m: f64,
    window_ns: u64,
    routers: BTreeMap<NodeId, Router>,
    /// The instant each `(node, packet)` CBF timer is armed for. A candidate frame whose
    /// ready instant is not this one was superseded.
    armed: BTreeMap<(NodeId, PacketKey), SimTime>,
    tracked: Vec<Tracked>,
    stats: GnAirStats,
}

/// `net.geonet`'s CBF timers and `DIST_MAX` over a MIB (network audit). The loader has
/// refused a minimum above the maximum and a non-positive range.
pub(super) fn tuned_mib(mut mib: GnMib, g: &Geonet) -> GnMib {
    if let Some(ms) = g.cbf_min_time_ms {
        mib.cbf_min_ms = ms;
    }
    if let Some(ms) = g.cbf_max_time_ms {
        mib.cbf_max_ms = ms;
    }
    if let Some(m) = g.max_comm_range_m {
        mib.max_comm_range_m = m;
    }
    mib
}

/// The router configuration a scenario's `net.geonet` names (shared with the overlay).
pub(super) fn forwarding_config(g: &Geonet) -> ForwardingConfig {
    let mib = GnMib {
        default_hop_limit: g.hop_limit.clamp(1, 255) as u8,
        area_forwarding: if g.area_forwarding == "simple" {
            AreaForwarding::Simple
        } else {
            AreaForwarding::Cbf
        },
        non_area_forwarding: if g.non_area_forwarding == "cbf" {
            NonAreaForwarding::Cbf
        } else {
            NonAreaForwarding::Greedy
        },
        ..GnMib::default()
    };
    // network audit: `net.geonet` CBF timers and DIST_MAX over Annex H.
    let mib = tuned_mib(mib, g);
    let variant = match g.cbf_variant.as_str() {
        "etsi" => CbfVariant::Etsi,
        "dpd" => CbfVariant::Dpd,
        _ => CbfVariant::Gpc,
    };
    ForwardingConfig {
        mib,
        cbf_variant: variant,
        out_of_area_broadcast_forward: variant == CbfVariant::Etsi,
    }
}

/// The originator's DENM repetition on the air (`net.geonet.repetition_*`), as
/// (interval, duration). The on-air breakdown DENM is the node's own DEN service, which
/// before this setting reached it always repeated every 1 s for 15 s (the C2C-CC
/// stationary-vehicle profile, `v2xw_node::events::STATIONARY_REPETITION_DURATION`). An
/// unset duration therefore keeps that 15 s, so existing runs do not change; any value
/// given (5 000 ms included) is used as given.
pub(super) fn on_air_repetition(
    g: &Geonet,
) -> (v2xw_core::time::Duration, v2xw_core::time::Duration) {
    use v2xw_core::time::Duration;
    let duration_ms = g
        .repetition_duration_ms
        .unwrap_or(v2xw_node::events::STATIONARY_REPETITION_DURATION.as_nanos() / 1_000_000);
    (
        Duration::from_millis(g.repetition_interval_ms),
        Duration::from_millis(duration_ms),
    )
}

impl GnOnAir {
    /// The layer, when the scenario forwards on the air and the run has a MAC to queue in.
    pub(super) fn for_scenario(s: &Scenario, has_mac: bool) -> Option<Self> {
        let g = s.net.geonet.as_ref()?;
        if s.net.layer != "gn-btp" || g.forwarding != "on-air" || !has_mac {
            return None;
        }
        Some(GnOnAir {
            fwd: forwarding_config(g),
            area_radius_m: g.area_radius_m,
            window_ns: (g.window_s * 1e9).round().max(1.0) as u64,
            routers: BTreeMap::new(),
            armed: BTreeMap::new(),
            tracked: Vec::new(),
            stats: GnAirStats::default(),
        })
    }

    fn router(&mut self, node: NodeId) -> &mut Router {
        let fwd = self.fwd;
        self.routers
            .entry(node)
            .or_insert_with(|| Router::new(addr(node), fwd, u64::MAX))
    }
}

/// A station's GeoNetworking address: its node index plus one (0 is reserved).
pub(super) fn addr(node: NodeId) -> GnAddr {
    GnAddr(u64::from(node.index()) + 1)
}

/// The BTP port a message is demultiplexed on (EN 302 636-5-1 / TS 103 248).
fn port_of(msg: v2xw_msg::MsgType) -> BtpPort {
    match msg {
        v2xw_msg::MsgType::Denm => BtpPort::DENM,
        v2xw_msg::MsgType::Spat => BtpPort(2004),
        v2xw_msg::MsgType::Map => BtpPort::MAPEM,
        _ => BtpPort::CAM,
    }
}

impl Engine {
    /// The EDCA queue a frame waits in: `net.access_categories: etsi-profiles`, or else
    /// the queues `radio.mac.periodic_access_category` and `event_access_category` name
    /// (both AC_VO by default, SAE J2945/1's single queue). A forwarded copy is event
    /// traffic.
    ///
    /// `etsi-profiles` maps the DCC profiles of ETSI TS 102 724 V1.1.1 §5 (DP0: high
    /// priority DENMs; DP1: DENMs; DP2: CAMs; DP3: forwarded packets and other low-priority
    /// traffic) onto EDCA as ETSI EN 302 663 V1.3.1 Annex B does (DP0 -> AC_VO, DP1 ->
    /// AC_VI, DP2 -> AC_BE, DP3 -> AC_BK). A hazard DENM is DP0. The table is cited from
    /// the published profiles; CPM, VAM, SPaT and MAP are given DP3 here because the 2012
    /// profile table predates them (this build's choice).
    pub(super) fn access_category(
        &self,
        msg: v2xw_msg::MsgType,
        forwarded: bool,
    ) -> AccessCategory {
        if self.scenario.net.access_categories.as_deref() != Some("etsi-profiles") {
            return if forwarded || matches!(msg, v2xw_msg::MsgType::Denm) {
                self.event_ac
            } else {
                self.periodic_ac
            };
        }
        if forwarded {
            return AccessCategory::Bk;
        }
        match msg {
            v2xw_msg::MsgType::Denm => AccessCategory::Vo,
            v2xw_msg::MsgType::Cam | v2xw_msg::MsgType::Bsm => AccessCategory::Be,
            _ => AccessCategory::Bk,
        }
    }

    /// Whether `node` carries a GeoNetworking router: roadside units and the vehicles'
    /// on-board units. A pedestrian's or cyclist's device sends its own messages but does
    /// not forward (TS 103 300-3 VRU devices are single-hop).
    fn gn_station(&self, node: NodeId) -> bool {
        self.rsus.contains_key(&node) || self.actor_of(node).is_some_and(|a| !a.class.is_vru())
    }

    /// Positions `node`'s router where the run has the station at `now`.
    fn gn_place(&mut self, node: NodeId, now: SimTime) {
        let (x, y, speed, heading) = if let Some(p) = self.rsus.get(&node) {
            (p.x, p.y, 0.0, 0.0)
        } else if let Some(a) = self.actor_of(node) {
            let k = a.last.extrapolate(now);
            let s = v2xw_core::math::hypot(k.vel.x, k.vel.y);
            let h = v2xw_core::math::atan2(k.vel.x, k.vel.y)
                .to_degrees()
                .rem_euclid(360.0);
            (k.pos.x, k.pos.y, s, h)
        } else {
            return;
        };
        if let Some(gn) = self.gn_air.as_mut() {
            gn.router(node).set_ego(x, y, speed, heading, now);
        }
    }

    /// The GeoNetworking packet a frame `node` originates carries, when the run forwards on
    /// the air: a DENM is GeoBroadcast to the warning area around the sender, anything
    /// else is a single-hop broadcast.
    pub(super) fn gn_originate(
        &mut self,
        node: NodeId,
        msg: v2xw_msg::MsgType,
        bytes: u32,
        now: SimTime,
    ) -> Option<GnFrame> {
        if self.gn_air.is_none() || !self.gn_station(node) {
            return None;
        }
        self.gn_place(node, now);
        let gn = self.gn_air.as_mut()?;
        let port = port_of(msg);
        let radius = gn.area_radius_m;
        let r = gn.router(node);
        let acts = if msg == v2xw_msg::MsgType::Denm {
            let e = r.ego();
            r.originate_gbc(Area::circle(e.x, e.y, radius), port, bytes, 0, now)
        } else {
            r.originate_shb(port, bytes, 0, now)
        };
        let (packet, to) = acts.into_iter().find_map(|a| match a {
            RouterAction::Transmit { packet, to, .. } => Some((packet, to)),
            _ => None,
        })?;
        if msg == v2xw_msg::MsgType::Denm {
            gn.stats.gbc_originated += 1;
            let window = gn.window_ns;
            for t in &mut gn.tracked {
                if t.origin == node && now >= t.at && now <= t.end {
                    if t.first.is_none() {
                        t.first = Some(now);
                        t.end = now.saturating_add(window);
                    }
                    t.originated += 1;
                }
            }
        }
        Some(GnFrame {
            packet,
            to,
            cbf: false,
        })
    }

    /// A receiver decoded `state`: its router takes the packet, and any forward it asks
    /// for becomes a frame in its MAC.
    pub(super) fn gn_on_receive(&mut self, rx: NodeId, state: &FrameState, now: SimTime) {
        let Some(g) = state.gn.as_ref() else {
            return;
        };
        if self.gn_air.is_none() || !self.gn_station(rx) {
            return;
        }
        self.gn_place(rx, now);
        let sender = addr(state.tx);
        let Some(gn) = self.gn_air.as_mut() else {
            return;
        };
        let acts = gn.router(rx).receive(&g.packet, sender, g.to, now);
        for a in acts {
            match a {
                RouterAction::Transmit { packet, to, .. } => {
                    self.gn_forward(rx, state, packet, to, now, false);
                }
                RouterAction::ArmCbfTimer { key, at_ns } => {
                    let at = at_ns.max(now);
                    if let Some(gn) = self.gn_air.as_mut() {
                        gn.stats.cbf_armed += 1;
                        gn.armed.insert((rx, key), at);
                    }
                    self.gn_forward(rx, state, g.packet.clone(), g.to, at, true);
                }
                RouterAction::Deliver { packet } => {
                    let Some(gn) = self.gn_air.as_mut() else {
                        continue;
                    };
                    for t in &mut gn.tracked {
                        if packet.source.addr == t.addr
                            && packet.btp_port == BtpPort::DENM
                            && t.first.is_some_and(|f| now >= f)
                            && now <= t.end
                            && rx != t.origin
                        {
                            t.first_rx.entry(rx).or_insert((now, packet.hops()));
                        }
                    }
                }
                RouterAction::Drop { reason, .. } => {
                    if let Some(gn) = self.gn_air.as_mut() {
                        *gn.stats.drops.entry(format!("{reason:?}")).or_default() += 1;
                    }
                }
            }
        }
    }

    /// Puts a forwarded copy of `template` in `node`'s MAC pending list, ready at `at`.
    /// A CBF candidate is asked again at `at` ([`Engine::gn_cbf_fires`]).
    fn gn_forward(
        &mut self,
        node: NodeId,
        template: &FrameState,
        packet: GnPacket,
        to: LinkDest,
        at: SimTime,
        cbf: bool,
    ) {
        let horizon = self.scenario.time.horizon_ns();
        if at > horizon || !self.nodes.contains_key(&node) {
            return;
        }
        let seq = FrameSeq::new(self.next_frame);
        self.next_frame += 1;
        let mut state = template.clone();
        state.tx = node;
        state.tx_pos = v2xw_core::geom::Vec3::ZERO;
        state.dcc_event = None;
        state.ready_at = at;
        state.start = at;
        state.end = state.air.after(at);
        state.descriptor.tx_power_dbm = self.tx_power_dbm(node);
        state.descriptor.ac = self.access_category(template.msg_type, true);
        state.descriptor.sdu_ref =
            v2xw_radio::types::SduRef::new(template.descriptor.sdu_ref.sdu, seq);
        state.tx_handle = None;
        state.arrivals.clear();
        state.faint.clear();
        state.tx_heading = None;
        state.app = None;
        state.sl_resource = None;
        state.sl_interferers.clear();
        state.focus_high.clear();
        state.focus_placement.clear();
        state.sl = super::sidelink::SlFrame::default();
        state.mac_aifs_ns = 0;
        state.mac_backoff_ns = 0;
        state.census.clear();
        state.frag = None;
        state.cert_frag = None;
        state.gn = Some(GnFrame { packet, to, cbf });
        if !cbf {
            self.gn_count_forward(node, &state);
        }
        // The radio the frame goes out on: its own channel (WP-13 keeps a forwarded
        // packet on the channel it arrived for).
        let channel = state.descriptor.channel.0;
        self.frames.insert(seq, state);
        self.pending_tx.entry(node).or_default().push((at, seq));
        self.schedule_mac_timer(at, node, channel);
    }

    fn gn_count_forward(&mut self, node: NodeId, state: &FrameState) {
        let Some(gn) = self.gn_air.as_mut() else {
            return;
        };
        let Some(g) = state.gn.as_ref() else {
            return;
        };
        gn.stats.forwards_sent += 1;
        gn.stats.forward_airtime_us += state.air.as_nanos() / 1_000;
        let now = state.ready_at;
        for t in &mut gn.tracked {
            if g.packet.source.addr == t.addr
                && t.first.is_some_and(|f| now >= f)
                && now <= t.end
                && node != t.origin
            {
                t.forwarded += 1;
            }
        }
    }

    /// At the MAC timer that takes `frame` from the pending list: a CBF candidate's
    /// contention timer has come due, and its router decides. `false` when the frame stood
    /// down (it is then gone).
    pub(super) fn gn_cbf_fires(&mut self, node: NodeId, frame: FrameSeq, now: SimTime) -> bool {
        let Some((key, ready)) = self.frames.get(&frame).and_then(|s| {
            s.gn.as_ref()
                .filter(|g| g.cbf)
                .map(|g| (g.packet.key(), s.ready_at))
        }) else {
            return true;
        };
        let Some(gn) = self.gn_air.as_mut() else {
            return true;
        };
        if gn.armed.get(&(node, key)) != Some(&ready) {
            // Re-armed for a later instant, or already decided by an earlier candidate.
            self.frames.remove(&frame);
            return false;
        }
        gn.armed.remove(&(node, key));
        self.gn_place(node, now);
        let Some(gn) = self.gn_air.as_mut() else {
            return true;
        };
        let out = gn
            .router(node)
            .on_cbf_timer(key, now)
            .into_iter()
            .find_map(|a| match a {
                RouterAction::Transmit { packet, to, .. } => Some((packet, to)),
                _ => None,
            });
        match out {
            Some((packet, to)) => {
                let Some(mut state) = self.frames.remove(&frame) else {
                    return false;
                };
                if let Some(g) = state.gn.as_mut() {
                    g.packet = packet;
                    g.to = to;
                    g.cbf = false;
                }
                self.gn_count_forward(node, &state);
                self.frames.insert(frame, state);
                true
            }
            None => {
                gn.stats.cbf_cancelled += 1;
                self.frames.remove(&frame);
                false
            }
        }
    }

    /// Starts following a hazard warning raised by `origin` at `now` on the air, and says
    /// so in words for the event's note.
    pub(super) fn gn_track_warning(&mut self, origin: NodeId, now: SimTime) -> Option<String> {
        let pos = self.node_pos(origin, now)?;
        let radius = self.gn_air.as_ref()?.area_radius_m;
        let mut targets = BTreeSet::new();
        let mut vehicles = 0;
        for a in self.actors.values() {
            let Some(n) = a.node else { continue };
            if a.class.is_vru() {
                continue;
            }
            vehicles += 1;
            let p = a.last.extrapolate(now).pos;
            if n != origin && v2xw_core::math::hypot(p.x - pos.x, p.y - pos.y) <= radius {
                targets.insert(n);
            }
        }
        let gn = self.gn_air.as_mut()?;
        let n = targets.len();
        gn.tracked.push(Tracked {
            at: now,
            first: None,
            end: SimTime::MAX,
            origin,
            addr: addr(origin),
            targets,
            first_rx: BTreeMap::new(),
            vehicles,
            originated: 0,
            forwarded: 0,
        });
        Some(format!(
            "; its warning is GeoBroadcast on the air to the {n} equipped vehicles within {radius:.0} m, forwarded through each relay's own radio queue (reach and latency in the run summary)"
        ))
    }

    /// The warnings followed on the air, as the run report's `geonet` rows, and the
    /// routers' totals.
    pub(super) fn gn_finish(&mut self) {
        let Some(gn) = self.gn_air.as_mut() else {
            return;
        };
        let tracked = std::mem::take(&mut gn.tracked);
        let mut rows = Vec::new();
        for t in &tracked {
            let mut lat: Vec<u64> = Vec::new();
            let mut max_hops = 0u8;
            for n in &t.targets {
                if let Some(&(at, h)) = t.first_rx.get(n) {
                    lat.push(at.saturating_sub(t.first.unwrap_or(t.at)));
                    max_hops = max_hops.max(h);
                }
            }
            lat.sort_unstable();
            let q = |p: f64| -> u64 {
                if lat.is_empty() {
                    return 0;
                }
                let i = ((lat.len() - 1) as f64 * p).round() as usize;
                lat[i.min(lat.len() - 1)] / 1_000
            };
            let in_area = t.targets.len() as u64;
            let reached = lat.len() as u64;
            rows.push(GeonetWarning {
                at: t.at,
                node: u64::from(t.origin.index()),
                vehicles: t.vehicles,
                in_area,
                reached,
                reach_permille: if in_area == 0 {
                    1_000
                } else {
                    reached * 1_000 / in_area
                },
                transmissions: t.originated + t.forwarded,
                forwarded: t.forwarded,
                kept_alive: 0,
                latency_p50_us: q(0.5),
                latency_p95_us: q(0.95),
                max_hops,
            });
        }
        let stats = gn.stats.clone();
        self.report.geonet.extend(rows);
        self.report.gn_air = Some(stats);
    }
}

#[cfg(test)]
mod repetition_tests {
    use super::on_air_repetition;
    use crate::scenario::schema::Geonet;

    #[test]
    fn an_explicit_5000_ms_duration_holds_on_the_air() {
        let mut g = Geonet::default();
        assert_eq!(on_air_repetition(&g).1.as_nanos() / 1_000_000, 15_000);
        g.repetition_duration_ms = Some(5_000);
        assert_eq!(on_air_repetition(&g).1.as_nanos() / 1_000_000, 5_000);
        let parsed: Geonet =
            serde_json::from_str(r#"{"repetition_duration_ms": 5000}"#).expect("parses");
        assert_eq!(parsed.repetition_duration_ms, Some(5_000));
    }
}
