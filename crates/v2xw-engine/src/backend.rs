//! How a vehicle reaches the credential and misbehaviour backend.
//!
//! V2V safety messages go over the sidelink. **Backend traffic does not.** A US SCMS end
//! entity reaches the Registration Authority through the Location Obscurer Proxy over
//! whatever IP connectivity it has — a cellular modem, or a roadside unit's IP service
//! relayed over the unit's backhaul (05-protocols.md §3.2: "[Uu or RSU backhaul]";
//! [BRECHT §II-B]; the connected-vehicle pilots provisioned through RSUs) — and an ETSI
//! C-ITS station reaches its Enrolment and Authorization Authorities over HTTP/IP,
//! "ITS-G5 via RSU, WLAN, cellular, EV charger, OBD at a garage" [TS 102 941 §6.2.2]. This
//! module is that access leg, per vehicle:
//!
//! | Access | When | What a byte costs |
//! |---|---|---|
//! | `cellular` | `net.uu` names a Uu model and the vehicle's keyed draw falls inside `net.uu.params.penetration` | the Uu model of `v2xw_radio::cellular` at the vehicle's position: latency, per-cell capacity and M/M/1 queue, handover interruption and loss, coverage |
//! | `rsu-relay` | no cellular modem, and a unit with the relaying role within `relay_range_m` | the sidelink hop to the unit, then the unit's backhaul (`actors.rsus[].backhaul`, else `net.backhaul`) |
//! | `offline` | neither | nothing leaves the vehicle; reports wait in its outbox |
//!
//! Every byte goes in its own bucket (invariant I-N1): the cellular uplink and downlink,
//! the backhaul, and the backend network between entities — never the sidelink broadcast
//! the safety messages share, except for the one hop a relayed message really takes to
//! its roadside unit.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_core::rng::{EntityRef, RngDomain};
use v2xw_core::time::{Duration, SimTime};
use v2xw_proto::{Link, Transport};
use v2xw_radio::cellular::{
    CellCapacityUu, CellPlan, CellularUu, Direction, FixedLatencyUu, HandoverOutageUu,
    MecPlacement, Qos, RadioLatencyClass, SendOutcome, UuLatencyPreset,
};
use v2xw_radio::cellular_packet::{AtClock, BaseStationLayout, PacketUu, Rat};
use v2xw_radio::ip_transport::{
    BACKHAUL_DSL, BACKHAUL_FIBRE, BACKHAUL_MICROWAVE, PacketPath, TcpConfig, TcpConn,
    TransferReport, WiredLink, handshake as tcp_handshake, stream as tcp_stream,
};

use crate::ctx::EngineCtx;
use crate::error::{EngineError, Result};
use crate::scenario::Scenario;

/// `net.uu` model ids this build accepts.
pub const UU_MODELS: [&str; 4] = [
    FixedLatencyUu::ID,
    CellCapacityUu::ID,
    HandoverOutageUu::ID,
    PacketUu::ID,
];

/// `actors.rsus[].backhaul` and `net.backhaul` ids this build accepts.
pub const BACKHAUL_MODELS: [&str; 6] = [
    "backhaul/fixed",
    BACKHAUL_FIBRE,
    BACKHAUL_DSL,
    BACKHAUL_MICROWAVE,
    "backhaul/cellular",
    "backhaul/none",
];

/// How a CRL reaches cellular vehicles (`net.uu.params.crl_delivery`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrlDelivery {
    /// Each vehicle polls the CRL Store over its own unicast TCP/TLS connection.
    Unicast,
    /// The operator broadcasts each new list once per MBSFN area (eMBMS, TS 23.246 /
    /// TS 36.300 §15): no uplink request, one copy on the air for every vehicle in
    /// coverage, repeated as a carousel for vehicles that were out of coverage.
    Mbms,
}

/// The eMBMS bearer a CRL is broadcast on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MbmsBearer {
    /// Its rate, bit/s.
    pub rate_bps: f64,
    /// The MCH scheduling period, ms: a new session's data waits for the next one.
    pub period_ms: f64,
    /// How often the carousel repeats the current list, s.
    pub carousel_s: f64,
}

/// MBSFN subframes per radio frame a CRL bearer is given (`mbms_subframes`), default 1:
/// TS 36.331 `MBSFN-SubframeConfig` allows up to six of the ten subframes of an FDD frame
/// (subframes 0, 4, 5 and 9 cannot be MBSFN, TS 36.211 §6.1).
pub const MBMS_SUBFRAMES_DEFAULT: f64 = 1.0;
/// Resource elements per PRB in an MBSFN subframe that carry PMCH data: extended cyclic
/// prefix, 12 symbols × 12 subcarriers [TS 36.211 §6.10.2, §6.12], less a 2-symbol
/// non-MBSFN control region [TS 36.211 §6.7] = 120 (the MBSFN reference signals are left
/// in, so the rate is an upper bound by their share).
pub const MBMS_RE_PER_PRB: f64 = 120.0;
/// The PMCH's spectral efficiency, bit/RE: QPSK at CQI 6, 1.1758 [TS 36.213 Table
/// 7.2.3-1], the robust end an operator runs a broadcast at so the cell edge decodes it.
pub const MBMS_SE: f64 = 1.1758;
/// The MCH scheduling period, ms: `rf32` = 320 ms, of the rf8 … rf1024 options of TS
/// 36.331 `PMCH-Config.mch-SchedulingPeriod`.
pub const MBMS_PERIOD_MS: f64 = 320.0;
/// The carousel's repetition, s (an implementer's choice; FLUTE carousels in TS 26.346
/// §7 repeat at the operator's interval).
pub const MBMS_CAROUSEL_S: f64 = 10.0;

impl MbmsBearer {
    /// The bearer on `prbs` PRBs with `subframes` MBSFN subframes per 10 ms frame.
    #[must_use]
    pub fn on(prbs: u32, subframes: f64) -> MbmsBearer {
        MbmsBearer {
            rate_bps: subframes * 100.0 * f64::from(prbs) * MBMS_RE_PER_PRB * MBMS_SE,
            period_ms: MBMS_PERIOD_MS,
            carousel_s: MBMS_CAROUSEL_S,
        }
    }

    /// When a list of `bytes` handed to the BM-SC at `at` has been received.
    #[must_use]
    pub fn delivered(&self, at: SimTime, core: Duration, bytes: u32) -> SimTime {
        let ready = core.after(at);
        let period = ((self.period_ms * 1e6) as u64).max(1);
        let start = ready.div_ceil(period) * period;
        let tx = (f64::from(bytes) * 8.0 / self.rate_bps.max(1.0) * 1e9) as u64;
        start + tx
    }
}

/// `net.backend_net` ids this build accepts.
pub const BACKEND_NET_MODELS: [&str; 1] = ["backend-net/fixed"];

/// The default one-way backhaul latency of a wired roadside unit, milliseconds.
///
/// **ASSUMPTION** (constants.md): no document gives an RSU's field-network latency to the
/// internet. It is the SCMS's own library backend-link figure
/// (`v2xw_proto::ScmsParams::backend_link_latency`), reused so a relayed message and a
/// backend hop are not given two invented numbers; the deployment's city-to-region leg
/// is added on top (WP-D1). `net.backhaul` (`backhaul/fibre`, `dsl`, `microwave`, with a
/// distance) or `params.latency_ms` replaces it.
pub const BACKHAUL_LATENCY_MS: f64 = 10.0;

/// The default wired backhaul capacity, Mbit/s. **ASSUMPTION**: a gigabit Ethernet
/// port. The USDOT RSU Specification v4.1 (2016) §2.1 requires a PoE (IEEE 802.3at)
/// Ethernet interface with IPv4 and IPv6 but no rate, and the field network behind it
/// is the deployment's.
pub const BACKHAUL_CAPACITY_MBPS: f64 = 1000.0;

/// How far a vehicle can be from a relaying roadside unit and still use its IP service,
/// metres.
///
/// **ASSUMPTION** (constants.md). The USDOT RSU Specification v4.1 §2.2 has vehicles
/// request certificates through the RSU's IPv6 service ("request … within range of one
/// RSU and receive them when in range of another") but prints no range; 300 m is the
/// nominal 5.9 GHz DSRC range ("300 m nominal, up to 1000 m", Roebuck, ISART 2005 slides
/// p.8; a vendor figure, secondary). The value that matters for a run is the sidelink's
/// own reach, which the radio decides for a relayed *report* (it is a real frame on the
/// air). This radius gates only the exchanges the engine does not put on the air frame
/// by frame — a certificate top-up's batch download.
pub const RELAY_RANGE_M: f64 = 300.0;

/// The internet leg between a mobile operator's core and a backend in the same region,
/// one way, milliseconds: `(58.0 − 29.2) / 2`, the 4G round trip to an east-coast server
/// less the 4G first-hop round trip, halved [Narayanan et al. WWW'20 Table 2 via
/// 04-models.md §10.1]. Added to the capacity and handover tiers, whose own latency chain
/// ends at the operator's edge (Coll-Perales 2022), because an SCMS is not at the edge.
pub const INTERNET_ONE_WAY_MS: f64 = (58.0 - 29.2) / 2.0;

/// Which access a vehicle has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccessKind {
    /// A cellular modem.
    Cellular,
    /// No modem; roadside units' IP service when in range.
    RsuRelay,
    /// No backend connectivity at all.
    Offline,
}

impl AccessKind {
    /// The label a record prints.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            AccessKind::Cellular => "cellular",
            AccessKind::RsuRelay => "rsu-relay",
            AccessKind::Offline => "offline",
        }
    }
}

/// The Uu model in force.
#[derive(Debug, Clone)]
enum Uu {
    Fixed(FixedLatencyUu),
    Capacity(CellCapacityUu),
    Handover(HandoverOutageUu),
    /// The packet-level model, held by the [`Carrier`]; every message rides a TCP/TLS
    /// connection per vehicle.
    Packet,
}

/// What a roadside unit's backhaul is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackhaulMedium {
    /// `backhaul/fixed`: a latency and a capacity.
    Fixed,
    /// `backhaul/fibre`.
    Fibre,
    /// `backhaul/dsl`.
    Dsl,
    /// `backhaul/microwave`.
    Microwave,
    /// `backhaul/cellular`: the unit's own modem on the Uu model.
    Cellular,
    /// `backhaul/none`.
    None,
}

/// One roadside unit's backhaul.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Backhaul {
    /// One-way latency (the fixed part: propagation and forwarding).
    pub latency: Duration,
    /// Capacity towards the backend, bit/s.
    pub bandwidth_bps: u64,
    /// Whether the unit has a backhaul at all.
    pub connected: bool,
    /// The medium.
    pub medium: BackhaulMedium,
    /// Capacity from the backend, bit/s (equal to `bandwidth_bps` on a symmetric medium).
    pub down_bps: u64,
    /// Per-packet loss.
    pub loss: f64,
}

impl Backhaul {
    /// The FIFO line this backhaul is, for the per-message carrier.
    fn line(&self, name: &str) -> WiredLink {
        let mut w = WiredLink::new(
            name,
            self.bandwidth_bps.max(1) as f64,
            0.0,
            1,
            self.latency.as_nanos() as f64 / 1e3,
        );
        w.down_bps = self.down_bps.max(1) as f64;
        w.loss = self.loss;
        w
    }
}

/// Linux's floor on a TCP retransmission timeout, 200 ms (`TCP_RTO_MIN`,
/// include/net/tcp.h): what a message lost on a wired backhaul waits before it is sent
/// again.
pub const WIRED_RTO_MS: f64 = 200.0;
/// The initial retransmission timeout of RFC 6298 §2.1, 1 s: how long a vehicle whose
/// TCP connection gave up waits before it opens a new one and tries again.
pub const RECONNECT_S: f64 = 1.0;

/// The longest an EE↔SCMS TLS session may live: "The max lifetime of a session shall be
/// 5 minutes", with session resumption not supported ([SMP] §3.2; 03 §2). A message sent
/// on a session older than this opens a new TCP connection and a full TLS handshake.
pub const TLS_SESSION_MAX: Duration = Duration::from_secs(5 * 60);

/// The EE↔SCMS handshake flights, bytes of TLS records: `(ClientHello, server flight,
/// client closing flight)` for TLS 1.3 and TLS 1.2.
///
/// **Measured** (constants.md, "TLS"): `tools/scms_constants/tls_handshake_sizes.py`
/// runs a real OpenSSL 3.0.15 handshake restricted to the US profile — server-only X.509
/// authentication, ECDHE-ECDSA suites, no session tickets ([SMP] §2.1.18, §3.2;
/// [SM-EESEC] §5.2.3; 03 §2) — and counts each flight. **ASSUMPTIONS** the measurement
/// rests on: a two-certificate ECDSA P-256 chain (leaf 367 B + issuing CA 323 B DER; no
/// provider publishes its TLS chain) and OpenSSL's ClientHello (an OBU's stack is not
/// published). TLS 1.3's 80 B client Finished rides with the first request and is not
/// charged separately; TLS 1.2's closing flight is `client_finish + 50` (the
/// ChangeCipherSpec and record headers, 93 B measured) and the server's 51 B.
pub const SCMS_TLS13_FLIGHTS: (u32, u32, u32) = (256, 1_066, 80);
/// As [`SCMS_TLS13_FLIGHTS`], TLS 1.2 (`ECDHE-ECDSA-AES128-GCM-SHA256`).
pub const SCMS_TLS12_FLIGHTS: (u32, u32, u32) = (173, 934, 43);

/// The TCP/TLS figures of an EE↔SCMS connection: the transport defaults with the
/// measured US-profile handshake flights of `tls`.
#[must_use]
pub fn scms_tcp_config(tls: v2xw_radio::ip_transport::TlsVersion) -> TcpConfig {
    use v2xw_radio::ip_transport::TlsVersion;
    let (hello, server, finish) = match tls {
        TlsVersion::Tls12 => SCMS_TLS12_FLIGHTS,
        _ => SCMS_TLS13_FLIGHTS,
    };
    TcpConfig {
        tls,
        client_hello: hello,
        server_flight: server,
        client_finish: finish,
        ..TcpConfig::default()
    }
}

/// Every message of a backend exchange, carried by the access model the run selected.
///
/// Shared between [`BackendAccess`] (the engine's own sends: a misbehaviour report, a CRL
/// poll, a roadside unit's forward) and the protocol kernels' [`PathOracle`] (every
/// request, response and poll of a top-up, a re-enrolment or a trust-list fetch), so both
/// queue on the same cells, the same TCP connections and the same backhaul lines.
#[derive(Debug)]
pub struct Carrier {
    packet: Option<PacketUu>,
    tcp: TcpConfig,
    conns: BTreeMap<NodeId, TcpConn>,
    /// When each open connection's TLS session began.
    sessions: BTreeMap<NodeId, SimTime>,
    /// [`TLS_SESSION_MAX`].
    session_max: Duration,
    rsu_lines: BTreeMap<NodeId, (Backhaul, WiredLink)>,
    relay_of: BTreeMap<NodeId, NodeId>,
    internet: Duration,
    air_latency: Duration,
    air_bps: u64,
    device_base: u32,
    /// Counters.
    pub stats: CarrierStats,
}

/// What the carrier did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct CarrierStats {
    /// Kernel messages carried segment by segment over the packet-level Uu.
    pub uu_messages: u64,
    /// Kernel and engine messages carried over a roadside unit's backhaul line.
    pub backhaul_messages: u64,
    /// Bytes those backhaul lines carried, both directions.
    pub backhaul_line_bytes: u64,
    /// Backhaul messages lost on the line and sent again after an RTO.
    pub backhaul_retx: u64,
    /// TCP connections opened.
    pub tcp_connections: u64,
    /// TCP retransmissions.
    pub tcp_retransmissions: u64,
    /// TCP timeouts.
    pub tcp_rtos: u64,
    /// Full TLS handshakes (server-authenticated, no resumption: every session pays
    /// the whole handshake, [SMP] §3.2).
    pub tls_handshakes: u64,
    /// Sessions closed because they reached [`TLS_SESSION_MAX`].
    pub tls_sessions_expired: u64,
    /// Bytes the TCP and TLS handshakes carried, both directions, headers included.
    pub tls_handshake_bytes: u64,
    /// Connections that gave up and were reopened.
    pub reconnects: u64,
    /// Messages that could not be carried after three attempts and were charged at the
    /// nominal link instead.
    pub fallbacks: u64,
}

impl Carrier {
    fn new(packet: Option<PacketUu>, internet: Duration) -> Carrier {
        let p = v2xw_proto::ScmsParams::default();
        Carrier {
            packet,
            tcp: scms_tcp_config(v2xw_radio::ip_transport::TlsVersion::Tls13),
            conns: BTreeMap::new(),
            sessions: BTreeMap::new(),
            session_max: TLS_SESSION_MAX,
            rsu_lines: BTreeMap::new(),
            relay_of: BTreeMap::new(),
            internet,
            air_latency: p.v2x_air_latency,
            air_bps: p.v2x_air_bandwidth_bps,
            device_base: crate::phase2::SCMS_DEVICE_BASE,
            stats: CarrierStats::default(),
        }
    }

    /// One message over vehicle `node`'s TCP/TLS connection on the packet-level Uu,
    /// opening the connection when needed and reopening it (after [`RECONNECT_S`]) when
    /// TCP gives up, three times at most. `None` when it never got through.
    fn tcp_carry(
        &mut self,
        node: NodeId,
        dir: Direction,
        bytes: u32,
        at: SimTime,
    ) -> Option<SimTime> {
        let Carrier {
            packet,
            tcp,
            conns,
            sessions,
            session_max,
            stats,
            ..
        } = self;
        let m = packet.as_mut()?;
        let mut t0 = at;
        // A session past its lifetime is closed; with no resumption the next message
        // pays a new connection and a full handshake.
        if let Some(&began) = sessions.get(&node)
            && at.saturating_sub(began) >= session_max.as_nanos()
        {
            conns.remove(&node);
            sessions.remove(&node);
            stats.tls_sessions_expired += 1;
        }
        for _ in 0..3 {
            let conn = conns.entry(node).or_insert_with(|| TcpConn::new(tcp));
            let mut r = TransferReport {
                start: t0,
                ..TransferReport::default()
            };
            let mut path = AtClock(m);
            let mut t = Some(t0);
            if !conn.established {
                stats.tcp_connections += 1;
                t = tcp_handshake(&mut (), &mut path, node, conn, tcp, t0, &mut r);
                if t.is_some() {
                    stats.tls_handshakes += 1;
                    stats.tls_handshake_bytes += r.bytes_ul + r.bytes_dl;
                    sessions.insert(node, t0);
                }
            }
            let done = t.and_then(|t| {
                tcp_stream(&mut (), &mut path, node, conn, tcp, dir, bytes, t, &mut r)
            });
            stats.tcp_retransmissions += r.retransmissions;
            stats.tcp_rtos += r.rtos;
            if let Some(d) = done {
                return Some(d);
            }
            conns.remove(&node);
            sessions.remove(&node);
            stats.reconnects += 1;
            t0 = ms(RECONNECT_S * 1e3).after(r.done_at.unwrap_or(t0).max(t0));
        }
        None
    }

    /// Sets the TLS version of every new session: `session-scmsAuth` is TLS 1.2 or 1.3
    /// ([SMP] §2.1.18; CAMP's EE suites are TLS 1.2, [CAMP122] §6.3.3.5).
    pub fn set_tls(&mut self, tls: v2xw_radio::ip_transport::TlsVersion) {
        self.tcp = scms_tcp_config(tls);
    }

    /// One message over roadside unit `rsu`'s backhaul, from `at`: its FIFO line (lost
    /// packets sent again after [`WIRED_RTO_MS`], three times at most), or for a cellular
    /// backhaul the unit's own TCP connection on the packet-level Uu.
    fn line_carry(
        &mut self,
        rsu: NodeId,
        dir: Direction,
        bytes: u32,
        at: SimTime,
    ) -> Option<SimTime> {
        let (bh, _) = self.rsu_lines.get(&rsu)?;
        if !bh.connected {
            return None;
        }
        self.stats.backhaul_messages += 1;
        self.stats.backhaul_line_bytes += u64::from(bytes);
        if bh.medium == BackhaulMedium::Cellular && self.packet.is_some() {
            let t = match dir {
                Direction::Uplink => self
                    .tcp_carry(rsu, dir, bytes, at)
                    .map(|t| self.internet.after(t)),
                Direction::Downlink => self.tcp_carry(rsu, dir, bytes, self.internet.after(at)),
            };
            if t.is_some() {
                return t;
            }
            self.stats.fallbacks += 1;
        }
        let (bh, line) = self.rsu_lines.get_mut(&rsu)?;
        let mut t = at;
        for _ in 0..4 {
            if let Some(a) = PacketPath::<()>::deliver(line, &mut (), rsu, dir, t, bytes) {
                return Some(a);
            }
            self.stats.backhaul_retx += 1;
            t = ms(WIRED_RTO_MS).after(t);
        }
        self.stats.fallbacks += 1;
        Some(bh.delay(bytes).after(t))
    }

    /// The sidelink hop between a relayed vehicle and its unit.
    fn air(&self, bytes: u32) -> Duration {
        Link {
            latency: self.air_latency,
            bandwidth_bps: self.air_bps,
            transport: Transport::V2xAir,
        }
        .delay(bytes)
    }

    fn carry(
        &mut self,
        from: NodeId,
        to: NodeId,
        link: &Link,
        bytes: u32,
        at: SimTime,
    ) -> Option<SimTime> {
        let base = self.device_base;
        let (device, dir) = if from.index() >= base {
            (from, Direction::Uplink)
        } else if to.index() >= base {
            (to, Direction::Downlink)
        } else {
            return None;
        };
        let node = NodeId::new(device.index() - base);
        match link.transport {
            Transport::CellularUu => {
                self.packet.as_ref()?;
                self.stats.uu_messages += 1;
                let t = match dir {
                    Direction::Uplink => self
                        .tcp_carry(node, dir, bytes, at)
                        .map(|t| self.internet.after(t)),
                    Direction::Downlink => {
                        self.tcp_carry(node, dir, bytes, self.internet.after(at))
                    }
                };
                t.or_else(|| {
                    self.stats.fallbacks += 1;
                    Some(
                        link.delay(bytes)
                            .after(ms(3.0 * RECONNECT_S * 1e3).after(at)),
                    )
                })
            }
            // A roadside unit's own SCMS traffic (its application-certificate cycle)
            // rides its backhaul line with no sidelink hop.
            Transport::RsuBackhaul if self.rsu_lines.contains_key(&node) => {
                self.line_carry(node, dir, bytes, at)
            }
            Transport::RsuBackhaul => {
                let rsu = *self.relay_of.get(&node)?;
                let air = self.air(bytes);
                match dir {
                    Direction::Uplink => self.line_carry(rsu, dir, bytes, air.after(at)),
                    Direction::Downlink => {
                        self.line_carry(rsu, dir, bytes, at).map(|t| air.after(t))
                    }
                }
            }
            _ => None,
        }
    }
}

/// The kernels' handle on the [`Carrier`].
#[derive(Debug)]
struct CarrierOracle(Arc<Mutex<Carrier>>);

impl v2xw_proto::net::PathOracle for CarrierOracle {
    fn carry(
        &mut self,
        from: NodeId,
        to: NodeId,
        link: &Link,
        bytes: u32,
        at: SimTime,
    ) -> Option<SimTime> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .carry(from, to, link, bytes, at)
    }
}

/// One backend exchange type's latency: count, sum and largest, ns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct ExchangeLatency {
    /// Exchanges completed.
    pub count: u64,
    /// Their latencies summed, ns.
    pub sum_ns: u64,
    /// The largest, ns.
    pub max_ns: u64,
}

impl ExchangeLatency {
    /// The mean, ms.
    #[must_use]
    pub fn mean_ms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum_ns as f64 / self.count as f64 / 1e6
        }
    }
}

impl Backhaul {
    /// The time to move `bytes` across it.
    #[must_use]
    pub fn delay(&self, bytes: u32) -> Duration {
        Link {
            latency: self.latency,
            bandwidth_bps: self.bandwidth_bps,
            transport: Transport::RsuBackhaul,
        }
        .delay(bytes)
    }
}

/// What one access-leg send did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccessOutcome {
    /// It arrives at the far end at this instant.
    Arrives(SimTime),
    /// Lost on the link (a handover interruption, the loss draw, an unstable queue).
    Lost,
    /// No coverage now; the sender keeps it and tries again.
    NoCoverage,
}

/// Counters for the run report and the page.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct AccessReport {
    /// Vehicles given a cellular modem.
    pub cellular_vehicles: u64,
    /// Vehicles with no modem.
    pub relay_only_vehicles: u64,
    /// Vehicles with no backend access at all.
    pub offline_vehicles: u64,
    /// Cellular uplink bytes.
    pub uu_ul_bytes: u64,
    /// Cellular downlink bytes.
    pub uu_dl_bytes: u64,
    /// Roadside backhaul bytes.
    pub backhaul_bytes: u64,
    /// Backend-network bytes (between entities).
    pub backend_bytes: u64,
    /// Sends the Uu model lost.
    pub uu_lost: u64,
    /// Sends that found no coverage and were held.
    pub uu_no_coverage: u64,
    /// `cellular/uu/packet-pf` only: HARQ retransmissions.
    pub uu_harq_retx: u64,
    /// `cellular/uu/packet-pf` only: RLC AM retransmissions.
    pub uu_rlc_retx: u64,
    /// `cellular/uu/packet-pf` only: handovers and the measured mean gap, µs.
    pub uu_handovers: u64,
    /// The measured mean handover gap, µs.
    pub uu_ho_gap_mean_us: u64,
    /// `cellular/uu/packet-pf` only: TCP connections opened, segments retransmitted, RTOs.
    pub tcp_connections: u64,
    /// TCP retransmissions.
    pub tcp_retransmissions: u64,
    /// TCP retransmission timeouts.
    pub tcp_rtos: u64,
    /// True when the packet-level results differ from the simple model's.
    pub uu_differs_from_simple: bool,
    /// What the comparison found, in a sentence (empty for the other Uu models).
    pub uu_vs_simple: String,
    /// The per-message carrier's counters: kernel messages carried over the packet-level
    /// Uu and over roadside backhaul lines.
    pub carrier: CarrierStats,
    /// Backend latency per exchange type (`topup`, `reenrol`, `trust-fetch`,
    /// `crl-fetch`, `crl-mbms`, `report`, `rsu-report`), request to completion.
    pub exchange_latency: BTreeMap<String, ExchangeLatency>,
    /// `crl_delivery: mbms` only: CRL copies broadcast (one per MBSFN area and carousel
    /// turn), the bytes they put on the bearer, and the vehicles that received one.
    pub mbms_broadcasts: u64,
    /// See [`AccessReport::mbms_broadcasts`].
    pub mbms_bytes: u64,
    /// See [`AccessReport::mbms_broadcasts`].
    pub mbms_receptions: u64,
}

/// The access legs of a run.
#[derive(Debug, Clone)]
pub struct BackendAccess {
    uu: Option<Uu>,
    uu_id: Option<String>,
    penetration: f64,
    internet: Duration,
    /// The nominal one-way latency a flow the backend kernel carries sees on this access.
    nominal_uu: Duration,
    nominal_uu_bps: u64,
    relay_range_m: f64,
    default_backhaul: Backhaul,
    default_backhaul_id: Option<String>,
    default_backhaul_params: serde_json::Value,
    kinds: BTreeMap<NodeId, AccessKind>,
    carrier: Arc<Mutex<Carrier>>,
    crl_delivery: CrlDelivery,
    mbms: MbmsBearer,
    /// The counters.
    pub report: AccessReport,
}

fn num(params: &serde_json::Value, key: &str) -> Option<f64> {
    params.get(key).and_then(serde_json::Value::as_f64)
}

fn conflict(field: &str, message: impl Into<String>) -> EngineError {
    EngineError::Scenario(crate::ScenarioError::conflict(field, message.into()))
}

fn ms(v: f64) -> Duration {
    Duration::from_nanos((v * 1e6).round().max(0.0) as u64)
}

/// The Uu latency preset a scenario names, by its label.
fn preset_named(label: &str) -> Option<UuLatencyPreset> {
    UuLatencyPreset::ALL
        .into_iter()
        .find(|p| p.label() == label)
}

impl BackendAccess {
    /// The access legs the scenario describes.
    ///
    /// # Errors
    /// [`EngineError::Scenario`] for a model id or a preset this build does not have.
    pub fn from_scenario(scenario: &Scenario) -> Result<BackendAccess> {
        let mut uu = None;
        let mut uu_id = None;
        let mut penetration = 1.0;
        let mut internet = ms(INTERNET_ONE_WAY_MS);
        let mut nominal_uu = ms(29.0);
        let mut nominal_uu_bps = 10_000_000;
        let mut packet = None;
        let mut prbs = 100;
        let mut crl_delivery = CrlDelivery::Unicast;
        let mut mbms_subframes = MBMS_SUBFRAMES_DEFAULT;
        let mut mbms_period = MBMS_PERIOD_MS;
        let mut mbms_carousel = MBMS_CAROUSEL_S;
        if let Some(choice) = &scenario.net.uu {
            let p = &choice.params;
            penetration = num(p, "penetration").unwrap_or(1.0).clamp(0.0, 1.0);
            if let Some(v) = num(p, "internet_ms") {
                internet = ms(v);
            }
            let plan = {
                let mut plan = CellPlan::urban_macro();
                if let Some(isd) = num(p, "isd_m") {
                    plan.isd_m = isd.max(1.0);
                }
                plan
            };
            let capacity = |plan: CellPlan| -> CellCapacityUu {
                let mut m = CellCapacityUu::new(plan).with_chain(
                    RadioLatencyClass::LowAutomationLowLoad,
                    MecPlacement::AtCoreNetwork,
                );
                if let (Some(ul), Some(dl)) = (num(p, "uplink_mbps"), num(p, "downlink_mbps")) {
                    m = m.with_capacity_bps(ul * 1e6, dl * 1e6);
                }
                m
            };
            let model = match choice.id.as_str() {
                FixedLatencyUu::ID => {
                    let label = p
                        .get("preset")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(UuLatencyPreset::Lte4gEastCoast.label());
                    let preset = preset_named(label).ok_or_else(|| {
                        conflict(
                            "net.uu.params.preset",
                            format!(
                                "'{label}' is not a Uu latency preset; allowed: {}",
                                UuLatencyPreset::ALL
                                    .iter()
                                    .map(|p| p.label())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        )
                    })?;
                    let spec = preset.one_way_ms(Direction::Uplink);
                    nominal_uu = ms(spec.mean_ms);
                    // The fixed-latency preset is a measured end-to-end latency to a
                    // server, so no internet leg is added to it.
                    internet = Duration::ZERO;
                    Uu::Fixed(FixedLatencyUu::new(preset))
                }
                CellCapacityUu::ID => {
                    let m = capacity(plan);
                    nominal_uu = ms(m.fixed_latency_ms()) + internet;
                    nominal_uu_bps = (CellCapacityUu::MOSAIC_UPLINK_BPS) as u64;
                    Uu::Capacity(m)
                }
                HandoverOutageUu::ID => {
                    let inner = capacity(plan);
                    nominal_uu = ms(inner.fixed_latency_ms()) + internet;
                    nominal_uu_bps = (CellCapacityUu::MOSAIC_UPLINK_BPS) as u64;
                    let mut m = HandoverOutageUu::new(inner);
                    if let Some(pct) = num(p, "loss_percentile") {
                        m = m.at_loss_percentile(pct);
                    }
                    Uu::Handover(m)
                }
                PacketUu::ID => {
                    let rat_label = p
                        .get("rat")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("lte");
                    let rat = Rat::named(rat_label).ok_or_else(|| {
                        conflict(
                            "net.uu.params.rat",
                            format!("'{rat_label}' is not a RAT; allowed: lte, nr"),
                        )
                    })?;
                    let layout = match (
                        p.get("sites").and_then(serde_json::Value::as_array),
                        p.get("city_bbox").and_then(serde_json::Value::as_array),
                    ) {
                        (Some(sites), _) => BaseStationLayout::from_sites(
                            plan.clone(),
                            sites
                                .iter()
                                .filter_map(|v| Some((v.get(0)?.as_f64()?, v.get(1)?.as_f64()?)))
                                .collect(),
                        ),
                        (None, Some(b)) if b.len() == 4 => {
                            let f = |i: usize| b[i].as_f64().unwrap_or(0.0);
                            BaseStationLayout::for_city(plan.clone(), (f(0), f(1)), (f(2), f(3)))
                        }
                        _ => BaseStationLayout::lattice(plan.clone()),
                    };
                    let bg = num(p, "background_load")
                        .unwrap_or(v2xw_radio::cellular_packet::DEFAULT_BACKGROUND_LOAD);
                    let mut m = PacketUu::new(rat, layout)
                        .with_background_load(bg, num(p, "neighbour_load").unwrap_or(bg))
                        .with_core(MecPlacement::AtCoreNetwork);
                    if let (Some(ul), Some(dl)) = (num(p, "uplink_mbps"), num(p, "downlink_mbps")) {
                        m = m.with_simple_capacity_bps(ul * 1e6, dl * 1e6);
                    }
                    nominal_uu = ms(capacity(plan).fixed_latency_ms()) + internet;
                    nominal_uu_bps = (CellCapacityUu::MOSAIC_UPLINK_BPS) as u64;
                    prbs = m.rat_config().prbs;
                    packet = Some(m);
                    Uu::Packet
                }
                other => {
                    return Err(conflict(
                        "net.uu",
                        format!(
                            "'{other}' is not a Uu model; allowed: {}",
                            UU_MODELS.join(", ")
                        ),
                    ));
                }
            };
            uu = Some(model);
            uu_id = Some(choice.id.clone());
            match p.get("crl_delivery").and_then(serde_json::Value::as_str) {
                None | Some("unicast") => {}
                Some("mbms") => crl_delivery = CrlDelivery::Mbms,
                Some(other) => {
                    return Err(conflict(
                        "net.uu.params.crl_delivery",
                        format!("'{other}' is not a CRL delivery; allowed: unicast, mbms"),
                    ));
                }
            }
            if let Some(v) = num(p, "mbms_subframes") {
                mbms_subframes = v.clamp(1.0, 6.0);
            }
            if let Some(v) = num(p, "mbms_period_ms") {
                mbms_period = v.clamp(80.0, 10_240.0);
            }
            if let Some(v) = num(p, "mbms_carousel_s") {
                mbms_carousel = v.clamp(0.5, 3600.0);
            }
        }
        let mut mbms = MbmsBearer::on(prbs, mbms_subframes);
        mbms.period_ms = mbms_period;
        mbms.carousel_s = mbms_carousel;
        let default_backhaul = match &scenario.net.backhaul {
            None => backhaul_from("backhaul/fixed", &serde_json::Value::Null, nominal_uu)?,
            Some(choice) => backhaul_from(&choice.id, &choice.params, nominal_uu)?,
        };
        // `security.scms.tls` (SCMS settings audit): TLS 1.3 unless the scenario states 1.2.
        let mut carrier = Carrier::new(packet, internet);
        if scenario
            .security
            .scms
            .as_ref()
            .and_then(|s| s.tls.as_deref())
            == Some("1.2")
        {
            carrier.set_tls(v2xw_radio::ip_transport::TlsVersion::Tls12);
        }
        Ok(BackendAccess {
            uu,
            uu_id,
            penetration,
            internet,
            nominal_uu,
            nominal_uu_bps,
            relay_range_m: RELAY_RANGE_M,
            default_backhaul,
            default_backhaul_id: scenario.net.backhaul.as_ref().map(|c| c.id.clone()),
            default_backhaul_params: scenario
                .net
                .backhaul
                .as_ref()
                .map_or(serde_json::Value::Null, |c| c.params.clone()),
            kinds: BTreeMap::new(),
            carrier: Arc::new(Mutex::new(carrier)),
            crl_delivery,
            mbms,
            report: AccessReport::default(),
        })
    }

    fn carrier(&self) -> std::sync::MutexGuard<'_, Carrier> {
        self.carrier.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The per-message oracle to install on the protocol kernels, so every message of a
    /// backend flow is carried by this run's access model.
    #[must_use]
    pub fn oracle(&self) -> v2xw_proto::net::SharedOracle {
        Arc::new(Mutex::new(CarrierOracle(Arc::clone(&self.carrier))))
    }

    /// How a CRL reaches cellular vehicles.
    #[must_use]
    pub const fn crl_delivery(&self) -> CrlDelivery {
        self.crl_delivery
    }

    /// The eMBMS bearer (`crl_delivery: mbms`).
    #[must_use]
    pub const fn mbms(&self) -> MbmsBearer {
        self.mbms
    }

    /// The core-network and internet leg a broadcast list crosses to the BM-SC.
    #[must_use]
    pub fn mbms_core(&self) -> Duration {
        let core = self
            .carrier()
            .packet
            .as_ref()
            .map_or(Duration::ZERO, |m| ms(m.core_ms()));
        core + self.internet
    }

    /// The MBSFN area (cell) a position is served by, for counting broadcast copies; a
    /// non-packet Uu model has one area.
    #[must_use]
    pub fn mbms_area(&self, pos: Vec3) -> u32 {
        self.carrier()
            .packet
            .as_ref()
            .and_then(|m| m.cell_at(pos))
            .unwrap_or(0)
    }

    /// Records one exchange of `kind` that took from `start` to `end`.
    pub fn note_exchange(&mut self, kind: &str, start: SimTime, end: SimTime) {
        let d = end.saturating_sub(start);
        let e = self
            .report
            .exchange_latency
            .entry(kind.to_string())
            .or_default();
        e.count += 1;
        e.sum_ns += d;
        e.max_ns = e.max_ns.max(d);
    }

    /// Gives roadside unit `node` its backhaul line, at `pos` (a cellular backhaul's modem
    /// is placed there on the packet-level Uu).
    pub fn register_rsu(&mut self, node: NodeId, backhaul: Backhaul, pos: Vec3) {
        let mut c = self.carrier();
        if backhaul.medium == BackhaulMedium::Cellular
            && let Some(m) = c.packet.as_mut()
        {
            m.place(node, pos);
        }
        let line = backhaul.line(&format!("rsu-{}", node.index()));
        c.rsu_lines.insert(node, (backhaul, line));
    }

    /// Notes that vehicle `node`'s next relayed flow goes through unit `rsu`.
    pub fn bind_relay(&mut self, node: NodeId, rsu: NodeId) {
        self.carrier().relay_of.insert(node, rsu);
    }

    /// One message over unit `rsu`'s backhaul offered at `at`: its arrival, or `None`
    /// for a unit with no backhaul.
    pub fn rsu_send(
        &mut self,
        rsu: NodeId,
        dir: Direction,
        bytes: u32,
        at: SimTime,
    ) -> Option<SimTime> {
        self.carrier().line_carry(rsu, dir, bytes, at)
    }

    /// The counters, with the carrier's folded in.
    #[must_use]
    pub fn report_now(&self) -> AccessReport {
        let mut r = self.report.clone();
        let c = self.carrier();
        r.carrier = c.stats;
        r.tcp_connections += c.stats.tcp_connections;
        r.tcp_retransmissions += c.stats.tcp_retransmissions;
        r.tcp_rtos += c.stats.tcp_rtos;
        if let Some(m) = c.packet.as_ref() {
            refresh_packet_report(&mut r, m);
        }
        r
    }

    /// The Uu model's id, when there is one.
    #[must_use]
    pub fn uu_id(&self) -> Option<&str> {
        self.uu_id.as_deref()
    }

    /// The backhaul a unit declaring `id` has.
    ///
    /// # Errors
    /// [`EngineError::Scenario`] for an id this build does not have.
    pub fn backhaul_of(&self, id: Option<&str>) -> Result<Backhaul> {
        self.backhaul_with(id, None)
    }

    /// The backhaul a unit declaring `id` and `params` (`actors.rsus[].backhaul_params`)
    /// has: the unit's own parameters over `net.backhaul.params` when it names the same
    /// medium (or none), over the medium's card defaults otherwise.
    ///
    /// # Errors
    /// [`EngineError::Scenario`] for an id this build does not have.
    pub fn backhaul_with(
        &self,
        id: Option<&str>,
        params: Option<&serde_json::Value>,
    ) -> Result<Backhaul> {
        let same = id.is_none() || id == self.default_backhaul_id.as_deref();
        if same && params.is_none() {
            return Ok(self.default_backhaul);
        }
        let id = id
            .or(self.default_backhaul_id.as_deref())
            .unwrap_or("backhaul/fixed");
        let mut merged = if same {
            self.default_backhaul_params.clone()
        } else {
            serde_json::Value::Null
        };
        if let Some(serde_json::Value::Object(own)) = params {
            if !merged.is_object() {
                merged = serde_json::json!({});
            }
            if let Some(m) = merged.as_object_mut() {
                for (k, v) in own {
                    m.insert(k.clone(), v.clone());
                }
            }
        }
        backhaul_from(id, &merged, self.nominal_uu)
    }

    /// The relay radius.
    #[must_use]
    pub const fn relay_range_m(&self) -> f64 {
        self.relay_range_m
    }

    /// Gives a new vehicle its access, from a draw keyed by the node.
    pub fn assign(
        &mut self,
        rng: &v2xw_core::rng::RngRegistry,
        node: NodeId,
        relay: bool,
    ) -> AccessKind {
        let kind = if self.uu.is_some()
            && rng
                .checkout(RngDomain::Backend, EntityRef::Node(node))
                .bool(self.penetration)
        {
            self.report.cellular_vehicles += 1;
            AccessKind::Cellular
        } else if relay {
            self.report.relay_only_vehicles += 1;
            AccessKind::RsuRelay
        } else {
            self.report.offline_vehicles += 1;
            AccessKind::Offline
        };
        self.kinds.insert(node, kind);
        kind
    }

    /// A vehicle's access.
    #[must_use]
    pub fn kind(&self, node: NodeId) -> AccessKind {
        self.kinds
            .get(&node)
            .copied()
            .unwrap_or(AccessKind::Offline)
    }

    /// Books bytes a backend transfer moved, by bucket — called wherever a `net.bytes`
    /// record is written for one, so the counters and the recording agree.
    pub fn note_bytes(&mut self, bucket: v2xw_metrics::channels::ByteBucket, bytes: u64) {
        use v2xw_metrics::channels::ByteBucket as B;
        match bucket {
            B::CellularUl => self.report.uu_ul_bytes += bytes,
            B::CellularDl => self.report.uu_dl_bytes += bytes,
            B::Backhaul => self.report.backhaul_bytes += bytes,
            B::Backend => self.report.backend_bytes += bytes,
            _ => {}
        }
    }

    /// Forgets a retired vehicle.
    pub fn retire(&mut self, node: NodeId) {
        self.kinds.remove(&node);
    }

    /// Whether the vehicle's modem has a serving cell at `pos`.
    #[must_use]
    pub fn uu_coverage(&self, pos: Vec3, t: SimTime) -> bool {
        match &self.uu {
            None => false,
            Some(Uu::Fixed(m)) => CellularUu::<EngineCtx<'_>>::coverage(m, pos, t).is_some(),
            Some(Uu::Capacity(m)) => CellularUu::<EngineCtx<'_>>::coverage(m, pos, t).is_some(),
            Some(Uu::Handover(m)) => CellularUu::<EngineCtx<'_>>::coverage(m, pos, t).is_some(),
            Some(Uu::Packet) => self
                .carrier()
                .packet
                .as_ref()
                .is_some_and(|m| CellularUu::<EngineCtx<'_>>::coverage(m, pos, t).is_some()),
        }
    }

    /// One packet over the vehicle's cellular link, from the Uu model.
    ///
    /// The handover tier first observes the position, which is how a cell change starts an
    /// interruption. Delivery is at the model's instant plus the internet leg.
    pub fn uu_send(
        &mut self,
        ctx: &mut EngineCtx<'_>,
        node: NodeId,
        pos: Vec3,
        dir: Direction,
        bytes: u32,
    ) -> AccessOutcome {
        let now = v2xw_core::ctx::Ctx::now(ctx);
        if !self.uu_coverage(pos, now) {
            self.report.uu_no_coverage += 1;
            return AccessOutcome::NoCoverage;
        }
        let outcome = match self.uu.as_mut() {
            None => return AccessOutcome::NoCoverage,
            Some(Uu::Fixed(m)) => m.send(ctx, node, dir, bytes, Qos::Reliable),
            Some(Uu::Capacity(m)) => m.send(ctx, node, dir, bytes, Qos::Reliable),
            Some(Uu::Handover(m)) => {
                let _ = m.observe_position(ctx, node, pos);
                m.send(ctx, node, dir, bytes, Qos::Reliable)
            }
            Some(Uu::Packet) => {
                let mut c = self.carrier.lock().unwrap_or_else(PoisonError::into_inner);
                if let Some(m) = c.packet.as_mut() {
                    // Through the context: a cell change starts a handover, and a UE seen
                    // for the first time draws its salt from the keyed stream.
                    let _ = m.observe_position(ctx, node, pos);
                }
                let done = c.tcp_carry(node, dir, bytes, now);
                if let Some(m) = c.packet.as_ref() {
                    refresh_packet_report(&mut self.report, m);
                }
                match done {
                    Some(at) => SendOutcome::Scheduled {
                        deliver_at: at,
                        bytes_on_wire: bytes,
                    },
                    None => SendOutcome::Dropped(v2xw_radio::types::DropCause::QueueFull {
                        ac: "tcp",
                        depth: 0,
                    }),
                }
            }
        };
        match outcome {
            SendOutcome::Scheduled { deliver_at, .. } => {
                AccessOutcome::Arrives(self.internet.after(deliver_at))
            }
            SendOutcome::Dropped(_) => {
                self.report.uu_lost += 1;
                AccessOutcome::Lost
            }
            SendOutcome::NoCoverage => {
                self.report.uu_no_coverage += 1;
                AccessOutcome::NoCoverage
            }
        }
    }

    /// For `cellular/uu/packet-pf` only: prepares vehicle `node`'s modem for a
    /// multi-message flow (a top-up, a re-enrolment, a trust-list fetch) and returns the
    /// link to give the kernel.
    ///
    /// The link's numbers are only the fallback: the kernel's [`PathOracle`] carries every
    /// message of the flow — each request, response and poll — segment by segment over
    /// the vehicle's own keep-alive TCP/TLS connection through the PF scheduler
    /// ([`Carrier`]). What this does is observe the vehicle's position (a cell change
    /// starts a handover) and refuse a vehicle with no serving cell.
    pub fn packet_link(
        &mut self,
        ctx: &mut EngineCtx<'_>,
        node: NodeId,
        pos: Vec3,
    ) -> Option<Link> {
        if !matches!(self.uu, Some(Uu::Packet)) {
            return None;
        }
        let now = v2xw_core::ctx::Ctx::now(ctx);
        {
            let mut c = self.carrier();
            let m = c.packet.as_mut()?;
            let _ = m.observe_position(ctx, node, pos);
            CellularUu::<EngineCtx<'_>>::coverage(&*m, pos, now)?;
        }
        Some(Link {
            latency: self.nominal_uu,
            bandwidth_bps: self.nominal_uu_bps,
            transport: Transport::CellularUu,
        })
    }

    /// The link a flow the backend kernel carries end to end sees on this access — used
    /// for the multi-message exchanges (a top-up's request, acknowledgement and batch
    /// polls) that the engine does not relay message by message.
    #[must_use]
    pub fn nominal_link(&self, kind: AccessKind, backhaul: Option<Backhaul>) -> Option<Link> {
        match kind {
            AccessKind::Cellular => Some(Link {
                latency: self.nominal_uu,
                bandwidth_bps: self.nominal_uu_bps,
                transport: Transport::CellularUu,
            }),
            AccessKind::RsuRelay => {
                // The oracle carries each message over the unit's line (bound with
                // `bind_relay`); these numbers are the fallback.
                let b = backhaul.filter(|b| b.connected)?;
                // The sidelink hop's channel access, then the unit's backhaul.
                Some(Link {
                    latency: v2xw_proto::ScmsParams::default().v2x_air_latency + b.latency,
                    bandwidth_bps: b
                        .bandwidth_bps
                        .min(v2xw_proto::ScmsParams::default().v2x_air_bandwidth_bps),
                    transport: Transport::RsuBackhaul,
                })
            }
            AccessKind::Offline => None,
        }
    }
}

/// The cards of the models this module and the authority path select, for the page's
/// catalogue: the three Uu tiers, the fixed backhaul and the legacy authority pipeline.
#[must_use]
pub fn catalogue_cards() -> Vec<v2xw_core::card::ModelCard> {
    use v2xw_core::model::Model;
    vec![
        FixedLatencyUu::new(UuLatencyPreset::Lte4gEastCoast)
            .card()
            .clone(),
        CellCapacityUu::new(CellPlan::urban_macro()).card().clone(),
        HandoverOutageUu::new(CellCapacityUu::new(CellPlan::urban_macro()))
            .card()
            .clone(),
        PacketUu::new(
            Rat::Lte,
            BaseStationLayout::lattice(CellPlan::urban_macro()),
        )
        .card()
        .clone(),
        v2xw_node::rsu::Backhaul::fibre().card().clone(),
        v2xw_threat::LegacyWindow::legacy_defaults().card().clone(),
    ]
    .into_iter()
    .chain(v2xw_radio::ip_transport::wired_cards())
    .collect()
}

/// Registers the cards of the backend models this run uses — the vehicles' Uu model, the
/// authority pipeline and the credential protocol — so the manifest pins them like every
/// other model of the run.
///
/// # Errors
/// [`EngineError::Registry`] if a card does not validate.
pub fn register_used(
    scenario: &Scenario,
    registry: &mut v2xw_core::registry::Registry,
) -> Result<()> {
    use v2xw_core::model::Model;
    let security = scenario.actors.backend.protocol.is_some()
        || scenario.security.protocol.is_some()
        || !scenario.actors.rsus.is_empty()
        || !scenario.threats.attackers.is_empty()
        || !scenario.detection.local.is_empty();
    if !security {
        return Ok(());
    }
    let mut cards = vec![v2xw_threat::LegacyWindow::legacy_defaults().card().clone()];
    if let Some(uu) = &scenario.net.uu {
        cards.extend(catalogue_cards().into_iter().filter(|c| c.id == uu.id));
    }
    for card in cards {
        if !registry.contains(&card.id) {
            registry.register(card)?;
        }
    }
    let mut protocols = v2xw_core::registry::Registry::new();
    let _ = v2xw_proto::register_all(&mut protocols);
    let wanted = scenario
        .actors
        .backend
        .protocol
        .clone()
        .or_else(|| scenario.security.protocol.as_ref().map(|c| c.id.clone()));
    for (_, m) in protocols.iter_by_id() {
        if wanted.as_deref() == Some(m.card.id.as_str()) && !registry.contains(&m.card.id) {
            registry.register(m.card.clone())?;
        }
    }
    Ok(())
}

/// Copies the packet-level model's counters and its comparison with the simple model
/// into the run report.
fn refresh_packet_report(report: &mut AccessReport, m: &PacketUu) {
    let s = m.stats();
    report.uu_harq_retx = s.harq_retx;
    report.uu_rlc_retx = s.rlc_retx;
    report.uu_handovers = s.handovers;
    report.uu_ho_gap_mean_us = s.ho_gap_sum_us.checked_div(s.ho_gaps).unwrap_or(0);
    let c = m.comparison();
    report.uu_differs_from_simple = c.differs;
    report.uu_vs_simple = c.note;
}

/// A backhaul from its id and parameters.
fn backhaul_from(id: &str, params: &serde_json::Value, uu_one_way: Duration) -> Result<Backhaul> {
    let loss = num(params, "loss").unwrap_or(0.0).clamp(0.0, 0.5);
    let bps = |key: &str, default_mbps: f64| {
        (num(params, key).unwrap_or(default_mbps).max(1e-3) * 1e6) as u64
    };
    match id {
        "backhaul/fixed" => {
            let up = bps("capacity_mbps", BACKHAUL_CAPACITY_MBPS);
            Ok(Backhaul {
                latency: ms(num(params, "latency_ms").unwrap_or(BACKHAUL_LATENCY_MS)),
                bandwidth_bps: up,
                connected: true,
                medium: BackhaulMedium::Fixed,
                down_bps: num(params, "down_mbps").map_or(up, |v| (v.max(1e-3) * 1e6) as u64),
                loss,
            })
        }
        BACKHAUL_FIBRE | BACKHAUL_DSL | BACKHAUL_MICROWAVE => {
            let mut w = WiredLink::medium(id).unwrap_or_else(WiredLink::metro_fibre);
            if let Some(v) = num(params, "distance_km") {
                w.distance_km = v.max(0.0);
            }
            if let Some(v) = num(params, "hops") {
                w.hops = v.clamp(0.0, 64.0) as u32;
            }
            let up = bps("rate_mbps", w.rate_bps / 1e6);
            let down = bps("down_mbps", w.rate(Direction::Downlink) / 1e6);
            let latency = num(params, "latency_ms").map_or(Duration::from_nanos(w.fixed_ns()), ms);
            Ok(Backhaul {
                latency,
                bandwidth_bps: up,
                connected: true,
                medium: match id {
                    BACKHAUL_FIBRE => BackhaulMedium::Fibre,
                    BACKHAUL_DSL => BackhaulMedium::Dsl,
                    _ => BackhaulMedium::Microwave,
                },
                down_bps: down,
                loss: num(params, "loss").map_or(w.loss, |v| v.clamp(0.0, 0.5)),
            })
        }
        // A unit whose backhaul is itself a cellular modem: the Uu one-way latency, and
        // the MOSAIC uplink cap as its capacity. Under `cellular/uu/packet-pf` the
        // carrier puts its messages on the unit's own TCP connection through the PF
        // scheduler instead, and these numbers are the fallback.
        "backhaul/cellular" => {
            let up = bps("capacity_mbps", CellCapacityUu::MOSAIC_UPLINK_BPS / 1e6);
            Ok(Backhaul {
                latency: num(params, "latency_ms").map_or(uu_one_way, ms),
                bandwidth_bps: up,
                connected: true,
                medium: BackhaulMedium::Cellular,
                down_bps: up,
                loss,
            })
        }
        "backhaul/none" => Ok(Backhaul {
            latency: Duration::ZERO,
            bandwidth_bps: 0,
            connected: false,
            medium: BackhaulMedium::None,
            down_bps: 0,
            loss: 0.0,
        }),
        other => Err(conflict(
            "net.backhaul",
            format!(
                "'{other}' is not a backhaul model; allowed: {}",
                BACKHAUL_MODELS.join(", ")
            ),
        )),
    }
}

#[cfg(test)]
mod carrier_tests {
    use super::*;
    use v2xw_radio::ip_transport::TlsVersion;

    fn lte() -> Carrier {
        Carrier::new(
            Some(PacketUu::new(
                Rat::Lte,
                BaseStationLayout::lattice(CellPlan::urban_macro()),
            )),
            Duration::ZERO,
        )
    }

    const MIN: u64 = 60_000_000_000;

    #[test]
    fn a_tls_session_lives_at_most_five_minutes_and_is_never_resumed() {
        let ue = NodeId::new(5);
        let mut c = lte();
        c.tcp_carry(ue, Direction::Uplink, 600, 0).expect("carried");
        let first = c.stats.tls_handshake_bytes;
        assert_eq!(c.stats.tls_handshakes, 1);
        // Within the session: the same connection, no handshake.
        c.tcp_carry(ue, Direction::Downlink, 600, 4 * MIN)
            .expect("carried");
        assert_eq!(c.stats.tls_handshakes, 1);
        assert_eq!(c.stats.tls_sessions_expired, 0);
        // Five minutes after it began: a new session, a full handshake of the same bytes.
        c.tcp_carry(ue, Direction::Uplink, 600, 5 * MIN)
            .expect("carried");
        assert_eq!(c.stats.tls_handshakes, 2);
        assert_eq!(c.stats.tls_sessions_expired, 1);
        assert_eq!(
            c.stats.tls_handshake_bytes,
            2 * first,
            "no abbreviated handshake"
        );
        assert_eq!(c.stats.tcp_connections, 2);
        // The new session's clock starts at its own handshake.
        c.tcp_carry(ue, Direction::Uplink, 600, 9 * MIN)
            .expect("carried");
        assert_eq!(c.stats.tls_handshakes, 2);
        // Another vehicle has its own session.
        c.tcp_carry(NodeId::new(6), Direction::Uplink, 600, 9 * MIN)
            .expect("carried");
        assert_eq!(c.stats.tls_handshakes, 3);
        assert_eq!(TLS_SESSION_MAX, Duration::from_secs(300));
    }

    #[test]
    fn a_tls_handshake_costs_its_flights_tls12_one_round_trip_more() {
        // TCP: SYN and SYN-ACK, 60 B each. TLS 1.3 (RFC 8446 §2): ClientHello, then the
        // server flight; TLS 1.2 (RFC 5246 §7.3): ClientHello, server flight, client key
        // exchange + Finished (+50 B ChangeCipherSpec and record headers), server
        // ChangeCipherSpec + Finished (51 B). Each segment adds its 52 B of IP/TCP headers.
        // The flight sizes are the measured US-profile handshake (WP-D3,
        // tools/scms_constants/tls_handshake_sizes.py): TLS 1.3 256 / 1,066 B, TLS 1.2
        // 173 / 934 / 93 / 51 B, written out here so a changed constant fails.
        let cfg = TcpConfig::default();
        let seg = |b: u32| u64::from(b + cfg.header * b.div_ceil(cfg.mss));
        let ue = NodeId::new(5);
        let mut c13 = lte();
        let t13 = c13.tcp_carry(ue, Direction::Uplink, 1, 0).unwrap();
        let hs13 = 120 + seg(256) + seg(1_066);
        let mut c12 = lte();
        c12.set_tls(TlsVersion::Tls12);
        let t12 = c12.tcp_carry(ue, Direction::Uplink, 1, 0).unwrap();
        let hs12 = 120 + seg(173) + seg(934) + seg(93) + seg(51);
        let got13 = c13.stats.tls_handshake_bytes;
        let got12 = c12.stats.tls_handshake_bytes;
        // The flights' bytes are exact (pure ACKs are not counted as handshake bytes).
        assert_eq!(got13, hs13);
        assert_eq!(got12, hs12);
        // Switching versions switches every flight, not only the round-trip count.
        assert_eq!(scms_tcp_config(TlsVersion::Tls12).client_hello, 173);
        assert_eq!(scms_tcp_config(TlsVersion::Tls13).server_flight, 1_066);
        assert!(
            t12 > t13,
            "TLS 1.2 takes one more round trip: {t12} vs {t13}"
        );
    }

    #[test]
    fn mbms_waits_for_the_scheduling_period_and_serialises() {
        let b = MbmsBearer::on(100, 1.0);
        // 100 PRB × 120 RE × 1.1758 bit × 100 subframes/s ≈ 1.41 Mbit/s.
        assert!((b.rate_bps - 1.411e6).abs() < 1e4, "{}", b.rate_bps);
        let t = b.delivered(0, Duration::from_millis(10), 14_110);
        // Next 320 ms boundary after 10 ms, then 80 ms on the bearer.
        assert!((t as f64 / 1e6 - 400.0).abs() < 1.0, "{t}");
    }

    #[test]
    fn a_backhaul_line_queues_and_media_differ() {
        let fibre =
            backhaul_from(BACKHAUL_FIBRE, &serde_json::Value::Null, Duration::ZERO).unwrap();
        let dsl = backhaul_from(BACKHAUL_DSL, &serde_json::Value::Null, Duration::ZERO).unwrap();
        assert!(dsl.latency > fibre.latency * 10);
        assert!(dsl.down_bps > dsl.bandwidth_bps);
        let slow = backhaul_from(
            BACKHAUL_FIBRE,
            &serde_json::json!({"rate_mbps": 1.0, "latency_ms": 5.0}),
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(slow.latency, Duration::from_millis(5));
        let mut c = Carrier::new(None, Duration::ZERO);
        let rsu = NodeId::new(7);
        c.rsu_lines.insert(rsu, (slow, slow.line("t")));
        let a = c.line_carry(rsu, Direction::Uplink, 125_000, 0).unwrap();
        let b = c.line_carry(rsu, Direction::Uplink, 125_000, 0).unwrap();
        // 1 s on the line each; the second waits for the first.
        assert!(a >= 1_005_000_000 && b >= a + 1_000_000_000, "{a} {b}");
        assert_eq!(c.stats.backhaul_line_bytes, 250_000);
        // A kernel message from a relayed vehicle's device id rides that line.
        c.relay_of.insert(NodeId::new(3), rsu);
        let link = Link {
            latency: Duration::ZERO,
            bandwidth_bps: 0,
            transport: Transport::RsuBackhaul,
        };
        let dev = NodeId::new(crate::phase2::SCMS_DEVICE_BASE + 3);
        let t = c.carry(dev, NodeId::new(1), &link, 1000, 0).unwrap();
        assert!(t > b, "queued behind the earlier messages: {t}");
        // Two backend entities: not the carrier's link.
        assert!(
            c.carry(NodeId::new(1), NodeId::new(2), &link, 10, 0)
                .is_none()
        );
    }
}
