//! A discrete-event dissemination run: GeoAdhoc routers, DEN services and beacons on
//! moving nodes over a [`Medium`], and the metrics a multi-hop study reports — reach,
//! forwarding overhead and dissemination latency.
//!
//! # What this run models, and what it does not
//!
//! * **Modelled:** every protocol rule of [`super::router`] and [`super::den`]; beaconing
//!   and the location table it fills (so greedy forwarding sees stale neighbours, as a
//!   real router does); each frame's 802.11p air time at 6 Mbit/s; per-link reception
//!   with the medium's probability; node motion during the run.
//! * **Optional ([`DissemConfig::mac`]):** CSMA/CA as 802.11p broadcast does it — carrier
//!   sense over the medium's range, AIFS and a contention-window backoff after a busy
//!   medium, no acknowledgement and no retry — with collisions (two overlapping frames
//!   at a receiver are both lost; no capture) and half-duplex loss. Off (`None`), every
//!   frame reaches the air when the router asks, which is the overlay the engine's
//!   `net.geonet.forwarding: overlay` runs.
//! * **Not modelled:** DCC. Amador et al. ran LIMERIC DCC, which is why [`super::eval`]
//!   compares *ratios and orderings* with their tables rather than absolute counts. The
//!   engine's on-air forwarding (`net.geonet.forwarding: on-air`) runs the full radio
//!   pipeline, DCC included.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};

use super::area::Area;
use super::den::{ActionId, DenAction, DenService, DenTimer, Denm, TriggerRequest};
use super::packet::{GnAddr, GnPacket, PacketKey};
use super::router::{ForwardingConfig, LinkDest, Router, RouterAction};
use crate::frame::{FCS_BYTES, MAC_HEADER_QOS_DATA_BYTES};
use crate::netlayer::{BtpPort, LLC_SNAP_BYTES};

/// The model id the run's random stream is derived under.
pub const DISSEM_RNG_MODEL: &str = "net/gn-stack/dissemination";

/// The channel between two points.
pub trait Medium {
    /// Beyond this distance nothing is received, m (the search radius).
    fn range_m(&self) -> f64;
    /// Probability that a frame from `tx` is received at `rx`.
    fn p_rx(&self, tx: (f64, f64), rx: (f64, f64)) -> f64;
}

/// Every frame within `range_m` is received, none beyond: the textbook unit disk, and the
/// shape of Amador et al.'s "maximum transmission range 778 m".
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UnitDisk {
    /// Range, m.
    pub range_m: f64,
}

impl Medium for UnitDisk {
    fn range_m(&self) -> f64 {
        self.range_m
    }
    fn p_rx(&self, tx: (f64, f64), rx: (f64, f64)) -> f64 {
        if v2xw_core::math::hypot(tx.0 - rx.0, tx.1 - rx.1) <= self.range_m {
            1.0
        } else {
            0.0
        }
    }
}

/// A packet-delivery-ratio curve against distance: a logistic falling through 50 % at
/// `d50_m` with width `width_m`, cut at `range_m`. The engine fits one to its link
/// budget; [`DistancePdr::from_samples`] interpolates a sampled curve instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DistancePdr {
    /// Cut-off, m.
    pub range_m: f64,
    /// `(distance m, pdr)` samples, ascending; linear interpolation between them.
    pub samples: Vec<(f64, f64)>,
}

impl DistancePdr {
    /// A logistic curve sampled every 10 m.
    pub fn logistic(d50_m: f64, width_m: f64, range_m: f64) -> Self {
        let w = width_m.max(1.0);
        let samples = (0..=((range_m / 10.0) as usize))
            .map(|i| {
                let d = i as f64 * 10.0;
                (d, 1.0 / (1.0 + v2xw_core::math::exp((d - d50_m) / w)))
            })
            .collect();
        DistancePdr { range_m, samples }
    }

    /// From caller-sampled points.
    pub fn from_samples(range_m: f64, mut samples: Vec<(f64, f64)>) -> Self {
        samples.sort_by(|a, b| a.0.total_cmp(&b.0));
        DistancePdr { range_m, samples }
    }

    /// The PDR at `d` m.
    pub fn at(&self, d: f64) -> f64 {
        if d > self.range_m || self.samples.is_empty() {
            return 0.0;
        }
        let i = self.samples.partition_point(|s| s.0 <= d);
        if i == 0 {
            return self.samples[0].1;
        }
        if i >= self.samples.len() {
            return self.samples[self.samples.len() - 1].1;
        }
        let (d0, p0) = self.samples[i - 1];
        let (d1, p1) = self.samples[i];
        if d1 <= d0 {
            p0
        } else {
            p0 + (p1 - p0) * (d - d0) / (d1 - d0)
        }
    }
}

impl Medium for DistancePdr {
    fn range_m(&self) -> f64 {
        self.range_m
    }
    fn p_rx(&self, tx: (f64, f64), rx: (f64, f64)) -> f64 {
        self.at(v2xw_core::math::hypot(tx.0 - rx.0, tx.1 - rx.1))
    }
}

/// A Manhattan grid: streets every `block_m` along x and y. Two points on the same
/// street (both within `street_half_m` of one street line) see each other up to
/// `los_range_m`; any other pair — buildings in between — up to `nlos_range_m`. The
/// shape of the urban LOS/NLOS split of 3GPP TR 37.885 §6.2.1, as a unit disk each.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UrbanGridMedium {
    /// Block pitch, m.
    pub block_m: f64,
    /// Half-width of a street, m.
    pub street_half_m: f64,
    /// Line-of-sight range, m.
    pub los_range_m: f64,
    /// Non-line-of-sight range, m.
    pub nlos_range_m: f64,
}

impl UrbanGridMedium {
    fn on_line(&self, v: f64) -> Option<i64> {
        let k = (v / self.block_m).round();
        ((v - k * self.block_m).abs() <= self.street_half_m).then_some(k as i64)
    }
}

impl Medium for UrbanGridMedium {
    fn range_m(&self) -> f64 {
        self.los_range_m.max(self.nlos_range_m)
    }
    fn p_rx(&self, tx: (f64, f64), rx: (f64, f64)) -> f64 {
        let d = v2xw_core::math::hypot(tx.0 - rx.0, tx.1 - rx.1);
        let same_ns =
            matches!((self.on_line(tx.0), self.on_line(rx.0)), (Some(a), Some(b)) if a == b);
        let same_ew =
            matches!((self.on_line(tx.1), self.on_line(rx.1)), (Some(a), Some(b)) if a == b);
        let r = if same_ns || same_ew {
            self.los_range_m
        } else {
            self.nlos_range_m
        };
        if d <= r { 1.0 } else { 0.0 }
    }
}

/// A node moving at constant velocity, optionally wrapping inside `[0, wrap)` on each
/// axis (a ring road or a torus of streets, so density stays constant).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionNode {
    /// x at t = 0, m.
    pub x: f64,
    /// y at t = 0, m.
    pub y: f64,
    /// Velocity east, m/s.
    pub vx: f64,
    /// Velocity north, m/s.
    pub vy: f64,
    /// Wrap extent on x, m (0: none).
    pub wrap_x: f64,
    /// Wrap extent on y, m (0: none).
    pub wrap_y: f64,
}

impl MotionNode {
    /// A parked node.
    pub fn fixed(x: f64, y: f64) -> Self {
        MotionNode {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            wrap_x: 0.0,
            wrap_y: 0.0,
        }
    }

    /// Position at `t_ns`.
    pub fn at(&self, t_ns: u64) -> (f64, f64) {
        let t = t_ns as f64 / 1e9;
        let mut x = self.x + self.vx * t;
        let mut y = self.y + self.vy * t;
        if self.wrap_x > 0.0 {
            x = x.rem_euclid(self.wrap_x);
        }
        if self.wrap_y > 0.0 {
            y = y.rem_euclid(self.wrap_y);
        }
        (x, y)
    }

    fn speed_heading(&self) -> (f64, f64) {
        let s = v2xw_core::math::hypot(self.vx, self.vy);
        let h = v2xw_core::math::atan2(self.vx, self.vy)
            .to_degrees()
            .rem_euclid(360.0);
        (s, h)
    }
}

/// A hazard: at `at_ms`, node `node` triggers a DENM to `area`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Hazard {
    /// When, ms.
    pub at_ms: u64,
    /// The originating node's index.
    pub node: usize,
    /// The relevance / destination area.
    pub area: Area,
}

/// The DENM parameters every hazard uses.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DenParams {
    /// `validityDuration`, ms.
    pub validity_ms: u64,
    /// `repetitionInterval`, ms (0: send once).
    pub repetition_interval_ms: u64,
    /// `repetitionDuration`, ms.
    pub repetition_duration_ms: u64,
    /// `transmissionInterval` for KAF, ms (0: not set).
    pub transmission_interval_ms: u64,
    /// Keep-alive forwarding at receivers.
    pub kaf: bool,
    /// KAF jitter upper bound, ms.
    pub kaf_jitter_ms: u64,
    /// DENM size above BTP, octets (secured).
    pub bytes: u32,
}

impl Default for DenParams {
    fn default() -> Self {
        DenParams {
            validity_ms: 600_000,
            repetition_interval_ms: 0,
            repetition_duration_ms: 0,
            transmission_interval_ms: 0,
            kaf: false,
            kaf_jitter_ms: 100,
            bytes: 301,
        }
    }
}

/// CSMA/CA for broadcast frames, IEEE 802.11-2020 §10.22.2 with the OCB EDCA parameters
/// of Table 9-155 and the 10 MHz OFDM timing of Table 17-21.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Csma {
    /// `aSIFSTime`, µs (32 at 10 MHz).
    pub sifs_us: u64,
    /// `aSlotTime`, µs (13 at 10 MHz).
    pub slot_us: u64,
    /// AIFSN of the access category (AC_VO: 2).
    pub aifsn: u64,
    /// CWmin of the access category (AC_VO: 3). Broadcast never doubles it.
    pub cw_min: u64,
}

impl Csma {
    /// AC_VO on a 10 MHz OCB channel, the category DENMs use.
    pub const AC_VO: Csma = Csma {
        sifs_us: 32,
        slot_us: 13,
        aifsn: 2,
        cw_min: 3,
    };

    /// `AIFS = SIFS + AIFSN × slot`, ns.
    pub fn aifs_ns(&self) -> u64 {
        (self.sifs_us + self.aifsn * self.slot_us) * 1_000
    }
}

/// A dissemination run's configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DissemConfig {
    /// The routers' configuration.
    pub forwarding: ForwardingConfig,
    /// DENM parameters.
    pub den: DenParams,
    /// Run beacons (off: location tables stay empty, CBF uses `TO_CBF_MAX`, greedy fails).
    pub beacons: bool,
    /// When the run stops, ms.
    pub end_ms: u64,
    /// Per-hop processing delay, µs.
    pub processing_us: u64,
    /// Master seed.
    pub seed: u64,
    /// Medium access: `None` puts every frame on the air at once (no contention).
    #[serde(default)]
    pub mac: Option<Csma>,
}

impl Default for DissemConfig {
    fn default() -> Self {
        DissemConfig {
            forwarding: ForwardingConfig::default(),
            den: DenParams::default(),
            beacons: true,
            end_ms: 30_000,
            processing_us: 0,
            seed: 1,
            mac: None,
        }
    }
}

/// 802.11p TXTIME at 10 MHz, 6 Mbit/s (QPSK ½, 48 data bits per 8 µs symbol): 32 µs
/// preamble + 8 µs SIGNAL + 8 µs × ⌈(16 + 8·L + 6) / 48⌉ [IEEE 802.11-2020 §17.4.3,
/// Eq. 17-29, Table 17-4 half-clocked], for an `L`-octet PSDU (LLC/SNAP, QoS-data MAC
/// header and FCS added here).
pub fn airtime_ns(network_bytes: u32) -> u64 {
    let l = u64::from(network_bytes + LLC_SNAP_BYTES + MAC_HEADER_QOS_DATA_BYTES + FCS_BYTES);
    let symbols = (16 + 8 * l + 6).div_ceil(48);
    (40 + 8 * symbols) * 1_000
}

/// Per-hazard results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionStats {
    /// Originator station.
    pub origin: u64,
    /// DENM sequence number.
    pub sn: u16,
    /// Generation time, ms.
    pub generated_ms: u64,
    /// Nodes inside the area at generation (the originator excluded).
    pub targets: u64,
    /// Of those, how many received the DENM at least once.
    pub reached: u64,
    /// `reached / targets` (1 when there are no targets).
    pub reach: f64,
    /// Nodes that received it at least once, inside the area or not.
    pub receivers: u64,
    /// GeoNetworking transmissions carrying it: all of them.
    pub transmissions: u64,
    /// ... of which the originator's own (first send and repetitions).
    pub originated: u64,
    /// ... of which GN forwards (CBF, greedy, simple).
    pub forwarded: u64,
    /// ... of which keep-alive forwards.
    pub kaf: u64,
    /// `transmissions / reached` (0 when nothing was reached).
    pub tx_per_reached: f64,
    /// Latency from generation to first reception over reached targets, ms: median.
    pub latency_p50_ms: f64,
    /// ... 95th percentile.
    pub latency_p95_ms: f64,
    /// ... maximum.
    pub latency_max_ms: f64,
    /// Most hops any first reception took.
    pub max_hops: u8,
    /// Reach by distance from the originator, 500 m bins: `(bin start m, targets, reached)`.
    pub reach_by_distance: Vec<(u32, u64, u64)>,
}

/// A run's results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DissemReport {
    /// Node count.
    pub nodes: u64,
    /// Per hazard.
    pub actions: Vec<ActionStats>,
    /// Mean over originated DENM packets of `reached / targets`.
    pub packet_pdr_mean: f64,
    /// Mean GN transmissions per originated DENM packet.
    pub tx_per_packet_mean: f64,
    /// Beacons sent.
    pub beacons: u64,
    /// All frames sent.
    pub frames: u64,
    /// All successful frame receptions.
    pub receptions: u64,
    /// Drops by router reason.
    pub drops: BTreeMap<String, u64>,
    /// With a MAC: receptions lost to an overlapping frame.
    #[serde(default)]
    pub collisions: u64,
    /// With a MAC: receptions lost because the receiver was transmitting.
    #[serde(default)]
    pub half_duplex: u64,
    /// With a MAC: mean channel-access delay, µs.
    #[serde(default)]
    pub access_delay_mean_us: f64,
}

#[derive(Debug, Clone)]
enum Ev {
    Beacon(usize),
    Rx { node: usize, tx: usize },
    Cbf { node: usize, key: PacketKey },
    Den { node: usize, timer: DenTimer },
    Hazard(usize),
}

struct Tx {
    packet: GnPacket,
    sender: GnAddr,
    to: LinkDest,
    from: usize,
    start: u64,
    end: u64,
    pos: (f64, f64),
}

#[derive(Default)]
struct ActionTrack {
    origin: usize,
    generated_ns: u64,
    targets: BTreeSet<usize>,
    first_rx: BTreeMap<usize, (u64, u8)>,
    originated: u64,
    forwarded: u64,
    kaf: u64,
}

/// A dissemination run.
pub struct DisseminationRun<'m> {
    cfg: DissemConfig,
    medium: &'m dyn Medium,
    nodes: Vec<MotionNode>,
    routers: Vec<Router>,
    dens: Vec<DenService>,
    hazards: Vec<Hazard>,
    queue: BTreeMap<(u64, u64), Ev>,
    seq: u64,
    rng: RngStream,
    txs: Vec<Tx>,
    denms: Vec<Denm>,
    actions: BTreeMap<ActionId, ActionTrack>,
    packets: BTreeMap<PacketKey, (BTreeSet<usize>, BTreeSet<usize>, u64)>,
    beacons: u64,
    receptions: u64,
    drops: BTreeMap<String, u64>,
    collisions: u64,
    half_duplex: u64,
    access_ns: u64,
    accesses: u64,
    /// Indices into `txs` of frames that may still overlap a reception, by start.
    recent: Vec<usize>,
}

impl<'m> DisseminationRun<'m> {
    /// A run over `nodes` (station `i` has GN address `i + 1`) with `hazards`.
    pub fn new(
        cfg: DissemConfig,
        medium: &'m dyn Medium,
        nodes: Vec<MotionNode>,
        hazards: Vec<Hazard>,
    ) -> Self {
        let mut rng = RngStream::derive(
            cfg.seed,
            RngDomain::plugin(DISSEM_RNG_MODEL),
            EntityRef::Global,
        );
        let period = cfg.forwarding.mib.beacon_retransmit_ms * 1_000_000;
        let routers = (0..nodes.len())
            .map(|i| {
                let first = if period > 0 { rng.below(period) } else { 0 };
                Router::new(GnAddr(i as u64 + 1), cfg.forwarding, first)
            })
            .collect::<Vec<_>>();
        let dens = (0..nodes.len())
            .map(|i| DenService::new(i as u32 + 1, cfg.den.kaf))
            .collect();
        let mut run = DisseminationRun {
            cfg,
            medium,
            nodes,
            routers,
            dens,
            hazards,
            queue: BTreeMap::new(),
            seq: 0,
            rng,
            txs: Vec::new(),
            denms: Vec::new(),
            actions: BTreeMap::new(),
            packets: BTreeMap::new(),
            beacons: 0,
            receptions: 0,
            drops: BTreeMap::new(),
            collisions: 0,
            half_duplex: 0,
            access_ns: 0,
            accesses: 0,
            recent: Vec::new(),
        };
        if run.cfg.beacons && period > 0 {
            for i in 0..run.routers.len() {
                let at = run.routers[i].next_beacon_ns();
                run.push(at, Ev::Beacon(i));
            }
        }
        for h in 0..run.hazards.len() {
            let at = run.hazards[h].at_ms * 1_000_000;
            run.push(at, Ev::Hazard(h));
        }
        run
    }

    fn push(&mut self, at: u64, ev: Ev) {
        self.seq += 1;
        self.queue.insert((at, self.seq), ev);
    }

    fn place(&mut self, i: usize, now: u64) -> (f64, f64) {
        let (x, y) = self.nodes[i].at(now);
        let (s, h) = self.nodes[i].speed_heading();
        self.routers[i].set_ego(x, y, s, h, now);
        (x, y)
    }

    /// Run to `end_ms` and report.
    pub fn run(mut self) -> DissemReport {
        let end = self.cfg.end_ms * 1_000_000;
        while let Some(((t, _), ev)) = self.queue.pop_first() {
            if t > end {
                break;
            }
            match ev {
                Ev::Beacon(i) => {
                    self.place(i, t);
                    let jmax = self.cfg.forwarding.mib.beacon_max_jitter_ms * 1_000_000;
                    let j = if jmax > 0 { self.rng.below(jmax) } else { 0 };
                    if let Some(b) = self.routers[i].beacon(t, j) {
                        self.beacons += 1;
                        self.transmit(i, b, LinkDest::Broadcast, t);
                    }
                    let next = self.routers[i].next_beacon_ns();
                    self.push(next, Ev::Beacon(i));
                }
                Ev::Rx { node, tx } => {
                    if self.cfg.mac.is_some() && !self.survives(node, tx) {
                        continue;
                    }
                    self.place(node, t);
                    let (packet, sender, to) = {
                        let x = &self.txs[tx];
                        (x.packet.clone(), x.sender, x.to)
                    };
                    let acts = self.routers[node].receive(&packet, sender, to, t);
                    self.apply(node, acts, t);
                }
                Ev::Cbf { node, key } => {
                    self.place(node, t);
                    let acts = self.routers[node].on_cbf_timer(key, t);
                    self.apply(node, acts, t);
                }
                Ev::Den { node, timer } => {
                    self.place(node, t);
                    let j = self.kaf_jitter();
                    let acts = self.dens[node].on_timer(timer, t / 1_000_000, j);
                    self.apply_den(node, acts, t);
                }
                Ev::Hazard(h) => {
                    let hz = self.hazards[h];
                    if hz.node >= self.nodes.len() {
                        continue;
                    }
                    self.place(hz.node, t);
                    let d = self.cfg.den;
                    let req = TriggerRequest {
                        area: hz.area,
                        validity_ms: d.validity_ms,
                        repetition_interval_ms: (d.repetition_interval_ms > 0)
                            .then_some(d.repetition_interval_ms),
                        repetition_duration_ms: (d.repetition_duration_ms > 0)
                            .then_some(d.repetition_duration_ms),
                        transmission_interval_ms: (d.transmission_interval_ms > 0)
                            .then_some(d.transmission_interval_ms),
                        bytes: d.bytes,
                    };
                    let (action, acts) = self.dens[hz.node].trigger(req, t / 1_000_000);
                    let targets = self.in_area(&hz.area, hz.node, t);
                    self.actions.insert(
                        action,
                        ActionTrack {
                            origin: hz.node,
                            generated_ns: t,
                            targets,
                            ..Default::default()
                        },
                    );
                    self.apply_den(hz.node, acts, t);
                }
            }
        }
        self.report()
    }

    fn kaf_jitter(&mut self) -> u64 {
        let j = self.cfg.den.kaf_jitter_ms;
        if j > 0 { self.rng.below(j) } else { 0 }
    }

    fn in_area(&self, area: &Area, except: usize, t: u64) -> BTreeSet<usize> {
        (0..self.nodes.len())
            .filter(|&i| i != except)
            .filter(|&i| {
                let (x, y) = self.nodes[i].at(t);
                area.contains(x, y)
            })
            .collect()
    }

    fn apply(&mut self, node: usize, acts: Vec<RouterAction>, t: u64) {
        for a in acts {
            match a {
                RouterAction::Transmit {
                    packet,
                    to,
                    originated,
                } => {
                    if !originated {
                        if let Some(tr) = self.track_of(&packet) {
                            tr.forwarded += 1;
                        }
                    }
                    self.transmit(node, packet, to, t);
                }
                RouterAction::ArmCbfTimer { key, at_ns } => self.push(at_ns, Ev::Cbf { node, key }),
                RouterAction::Deliver { packet } => self.deliver(node, packet, t),
                RouterAction::Drop { reason, .. } => {
                    *self.drops.entry(format!("{reason:?}")).or_default() += 1;
                }
            }
        }
    }

    fn track_of(&mut self, packet: &GnPacket) -> Option<&mut ActionTrack> {
        if packet.btp_port != BtpPort::DENM {
            return None;
        }
        let denm = self.denms.get(packet.payload_tag as usize)?;
        self.actions.get_mut(&denm.action)
    }

    fn deliver(&mut self, node: usize, packet: GnPacket, t: u64) {
        if packet.btp_port != BtpPort::DENM {
            return;
        }
        let Some(denm) = self.denms.get(packet.payload_tag as usize).copied() else {
            return;
        };
        if let Some(tr) = self.actions.get_mut(&denm.action) {
            if tr.origin != node {
                tr.first_rx.entry(node).or_insert((t, packet.hops()));
            }
        }
        if let Some(p) = self.packets.get_mut(&packet.key()) {
            p.1.insert(node);
        }
        let (x, y) = self.nodes[node].at(t);
        let inside = denm.area.contains(x, y);
        let j = self.kaf_jitter();
        let acts = self.dens[node].on_receive(denm, t / 1_000_000, inside, j);
        self.apply_den(node, acts, t);
    }

    fn apply_den(&mut self, node: usize, acts: Vec<DenAction>, t: u64) {
        for a in acts {
            match a {
                DenAction::Send { denm, kaf } => {
                    let tag = self.denms.len() as u64;
                    self.denms.push(denm);
                    if let Some(tr) = self.actions.get_mut(&denm.action) {
                        if kaf { tr.kaf += 1 } else { tr.originated += 1 }
                    }
                    let racts = self.routers[node].originate_gbc(
                        denm.area,
                        BtpPort::DENM,
                        denm.bytes,
                        tag,
                        t,
                    );
                    for ra in &racts {
                        if let RouterAction::Transmit { packet, .. } = ra {
                            let targets = self.in_area(&denm.area, node, t);
                            self.packets
                                .insert(packet.key(), (targets, BTreeSet::new(), 0));
                        }
                    }
                    self.apply(node, racts, t);
                }
                DenAction::ArmTimer { timer, at_ms } => {
                    self.push(at_ms * 1_000_000, Ev::Den { node, timer })
                }
                DenAction::Indicate { .. } => {}
            }
        }
    }

    /// Whether `node`'s reception of frame `tx` survives the frames that overlapped it.
    fn survives(&mut self, node: usize, tx: usize) -> bool {
        let (s, e) = (self.txs[tx].start, self.txs[tx].end);
        let rxp = self.nodes[node].at(s);
        for &k in &self.recent {
            if k == tx {
                continue;
            }
            let o = &self.txs[k];
            if o.start >= e || o.end <= s {
                continue;
            }
            if o.from == node {
                self.half_duplex += 1;
                return false;
            }
            if self.medium.p_rx(o.pos, rxp) > 0.0 {
                self.collisions += 1;
                return false;
            }
        }
        true
    }

    /// When `from`, asked to send at `t`, wins the medium: carrier sense over every frame
    /// already scheduled, AIFS, and a backoff after a busy medium.
    fn access(&mut self, from: usize, t: u64, air: u64) -> u64 {
        let Some(mac) = self.cfg.mac else {
            return t;
        };
        self.recent.retain(|&k| self.txs[k].end + 5_000_000 > t);
        let here = self.nodes[from].at(t);
        let sensed = |run: &Self, at: u64, until: u64| -> Option<u64> {
            run.recent
                .iter()
                .map(|&k| &run.txs[k])
                .filter(|o| o.start < until && o.end > at)
                .filter(|o| o.from == from || run.medium.p_rx(o.pos, here) > 0.0)
                .map(|o| o.end)
                .max()
        };
        let mut at = t;
        let mut deferred = false;
        // Busy now: wait for it to clear.
        while let Some(end) = sensed(self, at, at + 1) {
            at = end;
            deferred = true;
        }
        loop {
            let slots = if deferred {
                self.rng.below(mac.cw_min + 1)
            } else {
                0
            };
            let go = at + mac.aifs_ns() + slots * mac.slot_us * 1_000;
            // A frame that starts while this one counts down freezes it; resume after.
            match sensed(self, at, go) {
                Some(end) => {
                    at = end;
                    deferred = true;
                }
                None => {
                    at = go;
                    break;
                }
            }
        }
        let _ = air;
        self.access_ns += at - t;
        self.accesses += 1;
        at
    }

    fn transmit(&mut self, from: usize, packet: GnPacket, to: LinkDest, t: u64) {
        let air = airtime_ns(packet.bytes());
        let t = self.access(from, t, air);
        let arrive = t + air + self.cfg.processing_us * 1_000;
        if let Some(p) = self.packets.get_mut(&packet.key()) {
            if packet.btp_port == BtpPort::DENM {
                p.2 += 1;
            }
        }
        let txp = self.nodes[from].at(t);
        let sender = self.routers[from].addr();
        let idx = self.txs.len();
        self.txs.push(Tx {
            packet,
            sender,
            to,
            from,
            start: t,
            end: t + air,
            pos: txp,
        });
        if self.cfg.mac.is_some() {
            self.recent.push(idx);
        }
        let range = self.medium.range_m();
        let mut rx = Vec::new();
        for m in 0..self.nodes.len() {
            if m == from {
                continue;
            }
            if let LinkDest::Unicast(a) = to {
                if a.0 != m as u64 + 1 {
                    continue;
                }
            }
            let p = self.nodes[m].at(t);
            if (p.0 - txp.0).abs() > range || (p.1 - txp.1).abs() > range {
                continue;
            }
            let pr = self.medium.p_rx(txp, p);
            if pr >= 1.0 || (pr > 0.0 && self.rng.f64() < pr) {
                rx.push(m);
            }
        }
        self.receptions += rx.len() as u64;
        for m in rx {
            self.push(arrive, Ev::Rx { node: m, tx: idx });
        }
    }

    fn report(self) -> DissemReport {
        let mut actions = Vec::new();
        for (a, tr) in &self.actions {
            let origin_pos = self.nodes[tr.origin].at(tr.generated_ns);
            let mut lat: Vec<f64> = Vec::new();
            let mut bins: BTreeMap<u32, (u64, u64)> = BTreeMap::new();
            let mut max_hops = 0;
            for &n in &tr.targets {
                let p = self.nodes[n].at(tr.generated_ns);
                let d = v2xw_core::math::hypot(p.0 - origin_pos.0, p.1 - origin_pos.1);
                let bin = ((d / 500.0).floor() as u32) * 500;
                let e = bins.entry(bin).or_default();
                e.0 += 1;
                if let Some(&(t, h)) = tr.first_rx.get(&n) {
                    e.1 += 1;
                    lat.push((t - tr.generated_ns) as f64 / 1e6);
                    max_hops = max_hops.max(h);
                }
            }
            v2xw_core::math::sort_total_order(&mut lat);
            let q = |p: f64| {
                if lat.is_empty() {
                    0.0
                } else {
                    v2xw_core::math::q3(v2xw_core::math::quantile_sorted(&lat, p))
                }
            };
            let reached = lat.len() as u64;
            let targets = tr.targets.len() as u64;
            let transmissions = tr.originated + tr.forwarded + tr.kaf;
            actions.push(ActionStats {
                origin: tr.origin as u64 + 1,
                sn: a.sn,
                generated_ms: tr.generated_ns / 1_000_000,
                targets,
                reached,
                reach: if targets == 0 {
                    1.0
                } else {
                    v2xw_core::math::q3(reached as f64 / targets as f64)
                },
                receivers: tr.first_rx.len() as u64,
                transmissions,
                originated: tr.originated,
                forwarded: tr.forwarded,
                kaf: tr.kaf,
                tx_per_reached: if reached == 0 {
                    0.0
                } else {
                    v2xw_core::math::q3(transmissions as f64 / reached as f64)
                },
                latency_p50_ms: q(0.5),
                latency_p95_ms: q(0.95),
                latency_max_ms: lat.last().copied().map(v2xw_core::math::q3).unwrap_or(0.0),
                max_hops,
                reach_by_distance: bins.into_iter().map(|(b, (t, r))| (b, t, r)).collect(),
            });
        }
        let pk: Vec<&(BTreeSet<usize>, BTreeSet<usize>, u64)> = self.packets.values().collect();
        let pdr: Vec<f64> = pk
            .iter()
            .filter(|p| !p.0.is_empty())
            .map(|p| p.0.intersection(&p.1).count() as f64 / p.0.len() as f64)
            .collect();
        let mean = |v: &[f64]| {
            if v.is_empty() {
                0.0
            } else {
                v2xw_core::math::q3(
                    v2xw_core::math::sum_ordered(v.iter().copied()) / v.len() as f64,
                )
            }
        };
        let tpp: Vec<f64> = pk.iter().map(|p| p.2 as f64).collect();
        DissemReport {
            nodes: self.nodes.len() as u64,
            actions,
            packet_pdr_mean: mean(&pdr),
            tx_per_packet_mean: mean(&tpp),
            beacons: self.beacons,
            frames: self.txs.len() as u64,
            receptions: self.receptions,
            drops: self.drops,
            collisions: self.collisions,
            half_duplex: self.half_duplex,
            access_delay_mean_us: if self.accesses == 0 {
                0.0
            } else {
                v2xw_core::math::q3(self.access_ns as f64 / self.accesses as f64 / 1_000.0)
            },
        }
    }
}

/// A straight multi-lane highway: `lanes_per_dir` lanes each way, 3.5 m apart, vehicles
/// at `per_km_lane` per km per lane, evenly spaced with a random offset, at 25-33 m/s,
/// wrapping on `length_m` so the density holds.
pub fn highway(length_m: f64, lanes_per_dir: u32, per_km_lane: f64, seed: u64) -> Vec<MotionNode> {
    let mut rng = RngStream::derive(
        seed,
        RngDomain::plugin(DISSEM_RNG_MODEL),
        EntityRef::Custom { kind: 1, id: 0 },
    );
    let per_lane = ((length_m / 1_000.0) * per_km_lane).round().max(0.0) as usize;
    let mut out = Vec::new();
    for lane in 0..(2 * lanes_per_dir) {
        let dir = if lane < lanes_per_dir { 1.0 } else { -1.0 };
        let y = 3.5 * lane as f64 + 1.75;
        let gap = length_m / per_lane.max(1) as f64;
        let speed = rng.uniform(25.0, 33.0);
        for k in 0..per_lane {
            let x = (k as f64 + rng.uniform(0.0, 0.8)) * gap;
            out.push(MotionNode {
                x,
                y,
                vx: dir * speed,
                vy: 0.0,
                wrap_x: length_m,
                wrap_y: 0.0,
            });
        }
    }
    out
}

/// A Manhattan grid of `streets × streets` street lines `block_m` apart, `vehicles` cars
/// placed on random streets driving along them at 8-14 m/s, wrapping on the grid.
pub fn grid_city(streets: u32, block_m: f64, vehicles: usize, seed: u64) -> Vec<MotionNode> {
    let mut rng = RngStream::derive(
        seed,
        RngDomain::plugin(DISSEM_RNG_MODEL),
        EntityRef::Custom { kind: 2, id: 0 },
    );
    let extent = streets as f64 * block_m;
    (0..vehicles)
        .map(|_| {
            let line = rng.below(u64::from(streets)) as f64 * block_m;
            let along = rng.uniform(0.0, extent);
            let v = rng.uniform(8.0, 14.0) * if rng.bool(0.5) { 1.0 } else { -1.0 };
            if rng.bool(0.5) {
                MotionNode {
                    x: along,
                    y: line,
                    vx: v,
                    vy: 0.0,
                    wrap_x: extent,
                    wrap_y: extent,
                }
            } else {
                MotionNode {
                    x: line,
                    y: along,
                    vx: 0.0,
                    vy: v,
                    wrap_x: extent,
                    wrap_y: extent,
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::mib::AreaForwarding;

    #[test]
    fn airtime_of_a_300_octet_denm_is_about_half_a_millisecond() {
        // 300 + 8 + 26 + 4 = 338 octets -> ceil(2726/48) = 57 symbols -> 40 + 456 = 496 µs.
        assert_eq!(airtime_ns(300), 496_000);
    }

    #[test]
    fn a_chain_carries_a_denm_hop_by_hop_and_counts_it() {
        // Ten nodes 400 m apart, range 500 m: only multi-hop reaches the end.
        let nodes: Vec<_> = (0..10)
            .map(|i| MotionNode::fixed(400.0 * i as f64, 0.0))
            .collect();
        let area = Area::rect(-10.0, -10.0, 3_700.0, 10.0);
        for af in [AreaForwarding::Cbf, AreaForwarding::Simple] {
            let mut cfg = DissemConfig {
                end_ms: 8_000,
                ..Default::default()
            };
            cfg.forwarding.mib.area_forwarding = af;
            let m = UnitDisk { range_m: 500.0 };
            let r = DisseminationRun::new(
                cfg,
                &m,
                nodes.clone(),
                vec![Hazard {
                    at_ms: 4_000,
                    node: 0,
                    area,
                }],
            )
            .run();
            let a = &r.actions[0];
            assert_eq!(a.targets, 9, "{af:?}");
            assert_eq!(a.reached, 9, "{af:?}");
            assert_eq!(a.max_hops, 8, "{af:?}");
            // In a chain every hop is needed: 1 origination + 8 forwards (the last node's
            // forward has nobody new to reach but still goes out).
            assert_eq!(a.originated, 1);
            assert_eq!(a.forwarded, 9, "{af:?}");
            if af == AreaForwarding::Cbf {
                // Each CBF hop waits TO_CBF(400 m) = 100 - 99·0.4 = 60.4 ms.
                assert!(
                    a.latency_max_ms > 8.0 * 60.0 && a.latency_max_ms < 8.0 * 62.0,
                    "{}",
                    a.latency_max_ms
                );
            }
        }
    }

    #[test]
    fn kaf_reaches_a_node_that_enters_after_the_originator_went_quiet() {
        // A relay parked inside the area; a car drives in from far away at 30 m/s.
        let area = Area::rect(-10.0, -50.0, 1_000.0, 50.0);
        let nodes = vec![
            MotionNode::fixed(0.0, 0.0),
            MotionNode::fixed(400.0, 0.0),
            MotionNode {
                x: 3_000.0,
                y: 0.0,
                vx: -30.0,
                vy: 0.0,
                wrap_x: 0.0,
                wrap_y: 0.0,
            },
        ];
        let m = UnitDisk { range_m: 500.0 };
        let mut got = Vec::new();
        for kaf in [false, true] {
            let cfg = DissemConfig {
                end_ms: 90_000,
                den: DenParams {
                    kaf,
                    transmission_interval_ms: 1_000,
                    ..Default::default()
                },
                ..Default::default()
            };
            let r = DisseminationRun::new(
                cfg,
                &m,
                nodes.clone(),
                vec![Hazard {
                    at_ms: 4_000,
                    node: 0,
                    area,
                }],
            )
            .run();
            got.push(r.actions[0].receivers);
        }
        assert_eq!(got, vec![1, 2]);
    }
}
