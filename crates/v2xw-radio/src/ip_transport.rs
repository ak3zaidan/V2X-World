//! TCP/IP for backend traffic: `transport/tcp-cubic-tls` — TCP with slow start, CUBIC
//! congestion avoidance and retransmission, a TLS handshake and HTTP framing, carried
//! segment by segment over any [`PacketPath`]: the packet-level Uu of
//! [`crate::cellular_packet`], an RSU's wired backhaul ([`WiredLink`]), or the constant
//! link of the simple model ([`ConstantPath`]) for comparison.
//!
//! SCMS provisioning, pseudonym-certificate downloads and CRL fetches are HTTPS exchanges
//! (IEEE 1609.2.1 §6.3 specifies the SCMS REST interfaces over HTTPS), so their time is
//! the handshakes plus the windows TCP opens, not `bytes / bandwidth`.
//!
//! # The round model
//!
//! A stream is sent in rounds: each round puts `cwnd` segments on the path at once (the
//! path's own queue serialises them), the receiver acknowledges every second segment
//! (delayed ACK, RFC 1122 §4.2.3.2), and the next round starts when the last ACK of this
//! one arrives. A round with losses and at least three later deliveries is a fast
//! retransmit (RFC 5681 §3.2); otherwise the sender waits out the RTO (RFC 6298).
//! CUBIC follows RFC 8312 (C = 0.4, β = 0.7, the TCP-friendly region and the 1.5×
//! per-round growth cap).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use v2xw_core::ctx::Ctx;
use v2xw_core::ids::NodeId;
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime};

use crate::cellular::Direction;
use crate::cellular_packet::PacketUu;

/// A path packets cross one at a time: `Some(arrival)` or `None` for lost.
///
/// `Uplink` is client → server (vehicle or RSU → backend), `Downlink` the reverse.
pub trait PacketPath<C: ?Sized> {
    /// Offers one IP packet of `bytes` at `at`.
    fn deliver(
        &mut self,
        ctx: &mut C,
        ue: NodeId,
        dir: Direction,
        at: SimTime,
        bytes: u32,
    ) -> Option<SimTime>;
}

impl<C: Ctx + ?Sized> PacketPath<C> for PacketUu {
    fn deliver(
        &mut self,
        ctx: &mut C,
        ue: NodeId,
        dir: Direction,
        at: SimTime,
        bytes: u32,
    ) -> Option<SimTime> {
        self.send_at(ctx, ue, dir, bytes, at).deliver_at()
    }
}

/// The simple model: a fixed one-way latency and a serialisation time, no queue, no loss.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConstantPath {
    /// One-way latency.
    pub one_way: Duration,
    /// Rate, bit/s.
    pub bps: f64,
}

impl<C: ?Sized> PacketPath<C> for ConstantPath {
    fn deliver(
        &mut self,
        _ctx: &mut C,
        _ue: NodeId,
        _dir: Direction,
        at: SimTime,
        bytes: u32,
    ) -> Option<SimTime> {
        let ser = Duration::from_secs_f64(f64::from(bytes) * 8.0 / self.bps.max(1.0));
        Some(ser.after(self.one_way.after(at)))
    }
}

/// A wired link — an RSU's fibre or copper backhaul to the backend — as a FIFO per
/// direction: serialisation at the line rate, then propagation and per-hop forwarding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WiredLink {
    /// A label for reports.
    pub name: String,
    /// Line rate towards the backend (upstream), bit/s.
    pub rate_bps: f64,
    /// Line rate from the backend (downstream), bit/s; equal to `rate_bps` on a
    /// symmetric medium.
    #[serde(default)]
    pub down_bps: f64,
    /// Propagation per km of route, µs: [`FIBRE_US_PER_KM`] in glass, [`AIR_US_PER_KM`]
    /// over a radio hop.
    #[serde(default)]
    pub us_per_km: f64,
    /// Route length, km.
    pub distance_km: f64,
    /// Router hops.
    pub hops: u32,
    /// Forwarding delay per hop, µs.
    pub per_hop_us: f64,
    /// Per-packet loss probability (a hash draw, so it is a function of the schedule).
    pub loss: f64,
    busy_until: [SimTime; 2],
    sent: u64,
    /// Bytes carried per direction.
    pub bytes: [u64; 2],
}

/// Light in fibre: about 5 µs per km (group index ≈ 1.47) [ITU-T G.652 fibre; the figure
/// is the standard engineering rule].
pub const FIBRE_US_PER_KM: f64 = 4.9;

/// A radio hop: the speed of light, 1/299,792.458 km/s = 3.336 µs per km.
pub const AIR_US_PER_KM: f64 = 3.336;

/// The interleaving delay a VDSL2 line adds, ms (`todo-calibrate`, see
/// [`WiredLink::copper`]).
pub const DSL_INTERLEAVE_MS: f64 = 8.0;

/// The share of time a planned microwave hop is in outage (`todo-calibrate`, see
/// [`WiredLink::microwave`]).
pub const MICROWAVE_OUTAGE: f64 = 1e-5;

/// `backhaul/fibre`: a metro fibre backhaul, [`WiredLink::metro_fibre`].
pub const BACKHAUL_FIBRE: &str = "backhaul/fibre";
/// `backhaul/dsl`: a VDSL2 copper backhaul, [`WiredLink::copper`].
pub const BACKHAUL_DSL: &str = "backhaul/dsl";
/// `backhaul/microwave`: a point-to-point microwave backhaul, [`WiredLink::microwave`].
pub const BACKHAUL_MICROWAVE: &str = "backhaul/microwave";

/// The cards of the three wired media, for the catalogue and the settings page: each
/// parameter is one `actors.rsus[].backhaul_params` / `net.backhaul.params` key.
#[must_use]
pub fn wired_cards() -> Vec<v2xw_core::card::ModelCard> {
    use v2xw_core::card::{
        Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
        ValidationStatus,
    };
    let ranged = |name: &str, unit: &str, v: f64, src: Source, lo: f64, hi: f64| {
        let mut p = Parameter::new(name, unit, serde_json::json!(v), src);
        p.range = Some(vec![serde_json::json!(lo), serde_json::json!(hi)]);
        if p.source.kind == SourceKind::TodoCalibrate {
            p.calibration = Some(
                "Replace with the deployment's measured figure (an RSU vendor's backhaul \
                 test or the operator's SLA)."
                    .to_string(),
            );
        }
        p
    };
    let todo = Source::todo_calibrate;
    let mut out = Vec::new();
    for (id, link, what) in [
        (
            BACKHAUL_FIBRE,
            WiredLink::metro_fibre(),
            "Metro fibre from the roadside cabinet to the backend: high rate, sub-millisecond.",
        ),
        (
            BACKHAUL_DSL,
            WiredLink::copper(),
            "A VDSL2 telephone line: asymmetric rate and an interleaving delay of milliseconds.",
        ),
        (
            BACKHAUL_MICROWAVE,
            WiredLink::microwave(),
            "A point-to-point microwave hop: fibre-like rate, rare outages.",
        ),
    ] {
        let dsl = id == BACKHAUL_DSL;
        let mut card = ModelCard::new(id, Family::Backhaul, "1.0.0", what);
        card.tier = vec![Tier::High];
        card.equations = vec![Equation::new(
            "FIFO per direction",
            "arrival = max(offer, line free) + bytes·8 / rate(dir) + distance·us_per_km + hops·per_hop",
        )];
        let fixed_ms = link.fixed_ns() as f64 / 1e6;
        card.parameters = vec![
            ranged(
                "rate_mbps",
                "Mbit/s",
                link.rate_bps / 1e6,
                if dsl {
                    Source::new(
                        SourceKind::Datasheet,
                        "Openreach FTTC 80/20 tier: 20 Mbit/s upstream",
                    )
                } else {
                    todo("an implementer's line rate for this medium")
                },
                0.1,
                100_000.0,
            ),
            ranged(
                "down_mbps",
                "Mbit/s",
                link.rate(Direction::Downlink) / 1e6,
                if dsl {
                    Source::new(
                        SourceKind::Datasheet,
                        "Openreach FTTC 80/20 tier: 80 Mbit/s downstream",
                    )
                } else {
                    todo("symmetric: the upstream rate")
                },
                0.1,
                100_000.0,
            ),
            ranged(
                "distance_km",
                "km",
                link.distance_km,
                todo("a metro-scale route from the roadside to the backend"),
                0.0,
                5000.0,
            ),
            ranged(
                "hops",
                "-",
                f64::from(link.hops),
                todo("router hops of a metro access network"),
                0.0,
                64.0,
            ),
            ranged(
                "loss",
                "-",
                link.loss,
                todo("per-packet loss of the medium"),
                0.0,
                0.5,
            ),
            ranged(
                "latency_ms",
                "ms",
                fixed_ms,
                Source::new(
                    SourceKind::Standard,
                    "derived: distance × propagation (ITU-T G.652 fibre 4.9 µs/km; c in air) + hops × per-hop delay",
                ),
                0.0,
                2000.0,
            ),
        ];
        card.assumptions = vec![
            "One FIFO per direction per roadside unit: the messages of every vehicle it relays \
             queue behind each other on the line."
                .to_string(),
            "latency_ms, when given, replaces the distance and hop delay as the fixed one-way part."
                .to_string(),
        ];
        card.validation = Validation {
            status: ValidationStatus::UnitTested,
            references: vec![],
            tests: vec!["wired_media_differ_and_queue".to_string()],
        };
        out.push(card);
    }
    out
}

impl WiredLink {
    /// A link from its figures.
    #[must_use]
    pub fn new(name: &str, rate_bps: f64, distance_km: f64, hops: u32, per_hop_us: f64) -> Self {
        Self {
            name: name.to_string(),
            rate_bps,
            down_bps: rate_bps,
            us_per_km: FIBRE_US_PER_KM,
            distance_km,
            hops,
            per_hop_us,
            loss: 0.0,
            busy_until: [0; 2],
            sent: 0,
            bytes: [0; 2],
        }
    }

    /// A metro fibre backhaul: 1 Gbit/s, 20 km, 4 hops of 50 µs. The total one-way fixed
    /// part, about 0.3 ms, is of the order of the Coll-Perales transport-network figure to
    /// an M1 aggregation point (0.835 ms mean, via 04-models.md §10.1); the hop count and
    /// per-hop delay are `todo-calibrate`.
    #[must_use]
    pub fn metro_fibre() -> Self {
        Self::new("metro-fibre", 1e9, 20.0, 4, 50.0)
    }

    /// A copper (VDSL2-class) backhaul: 20 Mbit/s up and 80 Mbit/s down [the FTTC 80/20
    /// tier, Openreach SIN 498], 5 km, 6 hops of 50 µs, plus the interleaving the line adds
    /// as one hop of 8 ms [ITU-T G.993.2 lets the operator cap the interleaving delay
    /// (delay_max); 8 ms is `todo-calibrate`].
    #[must_use]
    pub fn copper() -> Self {
        let mut l = Self::new("copper", 20e6, 5.0, 7, 50.0);
        l.down_bps = 80e6;
        l.per_hop_us = (6.0 * 50.0 + DSL_INTERLEAVE_MS * 1e3) / 7.0;
        l
    }

    /// A licensed point-to-point microwave backhaul: 400 Mbit/s each way, 10 km of air
    /// at the speed of light ([`AIR_US_PER_KM`]), two radio terminals and two routers of
    /// 50 µs, and an outage share of 1e-5 (the "five nines" a link is planned to under
    /// ITU-R P.530; rate, terminal delay and outage are `todo-calibrate`).
    #[must_use]
    pub fn microwave() -> Self {
        let mut l = Self::new("microwave", 400e6, 10.0, 4, 50.0);
        l.us_per_km = AIR_US_PER_KM;
        l.loss = MICROWAVE_OUTAGE;
        l
    }

    /// The medium a `backhaul/*` id names, with its defaults.
    #[must_use]
    pub fn medium(id: &str) -> Option<Self> {
        match id {
            BACKHAUL_FIBRE => Some(Self::metro_fibre()),
            BACKHAUL_DSL => Some(Self::copper()),
            BACKHAUL_MICROWAVE => Some(Self::microwave()),
            _ => None,
        }
    }

    /// The line rate in direction `dir`, bit/s.
    #[must_use]
    pub fn rate(&self, dir: Direction) -> f64 {
        match dir {
            Direction::Uplink => self.rate_bps,
            Direction::Downlink if self.down_bps > 0.0 => self.down_bps,
            Direction::Downlink => self.rate_bps,
        }
    }

    /// The fixed one-way part, ns.
    #[must_use]
    pub fn fixed_ns(&self) -> u64 {
        let per_km = if self.us_per_km > 0.0 {
            self.us_per_km
        } else {
            FIBRE_US_PER_KM
        };
        ((self.distance_km * per_km + f64::from(self.hops) * self.per_hop_us) * 1e3) as u64
    }

    /// Bytes carried so far, both directions.
    #[must_use]
    pub fn bytes_total(&self) -> u64 {
        self.bytes[0] + self.bytes[1]
    }
}

impl<C: ?Sized> PacketPath<C> for WiredLink {
    fn deliver(
        &mut self,
        _ctx: &mut C,
        ue: NodeId,
        dir: Direction,
        at: SimTime,
        bytes: u32,
    ) -> Option<SimTime> {
        let d = dir as usize;
        self.sent += 1;
        if self.loss > 0.0 {
            let h = mix(self.sent ^ (u64::from(ue.index()) << 32));
            if (h >> 11) as f64 / (1u64 << 53) as f64 <= self.loss {
                return None;
            }
        }
        let start = at.max(self.busy_until[d]);
        let ser = (f64::from(bytes) * 8.0 / self.rate(dir).max(1.0) * 1e9) as u64;
        self.busy_until[d] = start + ser;
        self.bytes[d] += u64::from(bytes);
        Some(self.busy_until[d] + self.fixed_ns())
    }
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

// =========================================================================================
// TCP, TLS, HTTP
// =========================================================================================

/// Which TLS handshake opens the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TlsVersion {
    /// Plain TCP.
    None,
    /// TLS 1.2: two round trips (RFC 5246 §7.3).
    Tls12,
    /// TLS 1.3: one round trip (RFC 8446 §2).
    Tls13,
}

/// The TCP, TLS and HTTP figures.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TcpConfig {
    /// Maximum segment size, bytes: 1,500 B Ethernet MTU less 40 B of IPv4 and TCP
    /// headers and 12 B of timestamp option (RFC 7323).
    pub mss: u32,
    /// IP + TCP header bytes per segment, timestamps included.
    pub header: u32,
    /// Initial window, segments (RFC 6928).
    pub init_cwnd: f64,
    /// CUBIC `C` (RFC 8312 §5).
    pub cubic_c: f64,
    /// CUBIC `β` (RFC 8312 §4.5).
    pub cubic_beta: f64,
    /// Minimum RTO, s (Linux `TCP_RTO_MIN` = HZ/5 = 200 ms; RFC 6298 §2.4 recommends 1 s).
    pub rto_min_s: f64,
    /// Initial RTO, s (RFC 6298 §2.1).
    pub rto_init_s: f64,
    /// Consecutive RTOs before the connection is abandoned (Linux `tcp_retries2` is 15;
    /// a backend client gives up far sooner, `todo-calibrate`).
    pub max_rtos: u32,
    /// The handshake.
    pub tls: TlsVersion,
    /// ClientHello bytes (`todo-calibrate`: a typical TLS 1.3 ClientHello is 250–520 B).
    pub client_hello: u32,
    /// The server's handshake flight with its certificate chain (`todo-calibrate`: 3–5 kB
    /// for a two-certificate RSA chain).
    pub server_flight: u32,
    /// The client's closing flight (Finished, and for TLS 1.2 the key exchange).
    pub client_finish: u32,
    /// HTTP request header bytes (`todo-calibrate`).
    pub http_request_header: u32,
    /// HTTP response header bytes (`todo-calibrate`).
    pub http_response_header: u32,
    /// TLS record overhead per 16 KiB record: 5 B header + 16 B AEAD tag + 1 B content
    /// type (RFC 8446 §5.2).
    pub tls_record_overhead: u32,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            mss: 1448,
            header: 52,
            init_cwnd: 10.0,
            cubic_c: 0.4,
            cubic_beta: 0.7,
            rto_min_s: 0.2,
            rto_init_s: 1.0,
            max_rtos: 6,
            tls: TlsVersion::Tls13,
            client_hello: 512,
            server_flight: 4000,
            client_finish: 80,
            http_request_header: 350,
            http_response_header: 250,
            tls_record_overhead: 22,
        }
    }
}

/// One TCP connection's congestion state, kept between exchanges (HTTP keep-alive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TcpConn {
    /// Congestion window, segments.
    pub cwnd: f64,
    /// Slow-start threshold, segments.
    pub ssthresh: f64,
    w_max: f64,
    epoch: Option<SimTime>,
    srtt: Option<f64>,
    rttvar: f64,
    rto: f64,
    /// Whether the TCP and TLS handshakes are done.
    pub established: bool,
    last_active: SimTime,
}

impl TcpConn {
    /// A fresh connection.
    #[must_use]
    pub fn new(cfg: &TcpConfig) -> Self {
        Self {
            cwnd: cfg.init_cwnd,
            ssthresh: f64::INFINITY,
            w_max: 0.0,
            epoch: None,
            srtt: None,
            rttvar: 0.0,
            rto: cfg.rto_init_s,
            established: false,
            last_active: 0,
        }
    }

    /// The smoothed RTT, s, once there is a sample.
    #[must_use]
    pub const fn srtt(&self) -> Option<f64> {
        self.srtt
    }

    fn sample(&mut self, r: f64, cfg: &TcpConfig) {
        // RFC 6298 §2.2–2.3.
        match self.srtt {
            None => {
                self.srtt = Some(r);
                self.rttvar = r / 2.0;
            }
            Some(s) => {
                self.rttvar = 0.75 * self.rttvar + 0.25 * (s - r).abs();
                self.srtt = Some(0.875 * s + 0.125 * r);
            }
        }
        self.rto = (self.srtt.unwrap_or(r) + 4.0 * self.rttvar).max(cfg.rto_min_s);
    }

    fn on_loss(&mut self, cfg: &TcpConfig) {
        // RFC 8312 §4.5–4.6, with fast convergence.
        self.w_max = if self.cwnd < self.w_max {
            self.cwnd * (1.0 + cfg.cubic_beta) / 2.0
        } else {
            self.cwnd
        };
        self.cwnd = (self.cwnd * cfg.cubic_beta).max(2.0);
        self.ssthresh = self.cwnd;
        self.epoch = None;
    }

    fn on_rto(&mut self, cfg: &TcpConfig) {
        self.on_loss(cfg);
        self.cwnd = 1.0;
        self.rto = (self.rto * 2.0).min(60.0);
    }

    fn grow(&mut self, acked: f64, now: SimTime, cfg: &TcpConfig) {
        if self.cwnd < self.ssthresh {
            // Slow start, RFC 5681 §3.1.
            self.cwnd = (self.cwnd + acked).min(self.ssthresh.max(self.cwnd + 1.0));
            return;
        }
        let rtt = self.srtt.unwrap_or(0.1).max(1e-4);
        let epoch = *self.epoch.get_or_insert(now);
        let t = (now - epoch) as f64 * 1e-9;
        let origin = self.w_max.max(self.cwnd);
        let k = math::pow((origin - self.cwnd).max(0.0) / cfg.cubic_c, 1.0 / 3.0);
        let target = cfg.cubic_c * math::pow(t + rtt - k, 3.0) + origin;
        let w_est = self.w_max * cfg.cubic_beta
            + 3.0 * (1.0 - cfg.cubic_beta) / (1.0 + cfg.cubic_beta) * t / rtt;
        let next = target.max(w_est).min(1.5 * self.cwnd).max(self.cwnd);
        self.cwnd = next.min(10_000.0);
    }
}

/// What one exchange did.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct TransferReport {
    /// When the exchange started.
    pub start: SimTime,
    /// When the response's last byte reached the client, or `None` if it was abandoned.
    pub done_at: Option<SimTime>,
    /// Time spent on the TCP and TLS handshakes, s.
    pub handshake_s: f64,
    /// Segments sent, client to server.
    pub segments_ul: u64,
    /// Segments sent, server to client.
    pub segments_dl: u64,
    /// Segments retransmitted.
    pub retransmissions: u64,
    /// Retransmission timeouts.
    pub rtos: u64,
    /// Bytes on the wire, client to server.
    pub bytes_ul: u64,
    /// Bytes on the wire, server to client.
    pub bytes_dl: u64,
}

impl TransferReport {
    /// The duration, s, when it completed.
    #[must_use]
    pub fn seconds(&self) -> Option<f64> {
        self.done_at.map(|d| (d - self.start) as f64 * 1e-9)
    }
}

/// One HTTPS exchange: `request_body` up, `response_body` down, opening the connection
/// first when it is not established. Sending resumes from the idle window rule of
/// RFC 5681 §4.1 when the connection sat idle longer than its RTO.
pub fn https_exchange<C: ?Sized, P: PacketPath<C> + ?Sized>(
    ctx: &mut C,
    path: &mut P,
    ue: NodeId,
    conn: &mut TcpConn,
    cfg: &TcpConfig,
    start: SimTime,
    request_body: u32,
    response_body: u32,
) -> TransferReport {
    let mut r = TransferReport {
        start,
        ..TransferReport::default()
    };
    let mut t = start;
    if conn.established && (t - conn.last_active.min(t)) as f64 * 1e-9 > conn.rto {
        conn.cwnd = conn.cwnd.min(cfg.init_cwnd);
    }
    if !conn.established {
        match handshake(ctx, path, ue, conn, cfg, t, &mut r) {
            Some(a) => t = a,
            None => return r,
        }
    }
    let tls = |b: u32| match cfg.tls {
        TlsVersion::None => b,
        _ => b + cfg.tls_record_overhead * b.div_ceil(16_384).max(1),
    };
    let finish = if cfg.tls == TlsVersion::Tls13 && r.handshake_s > 0.0 {
        cfg.client_finish
    } else {
        0
    };
    let req = tls(cfg.http_request_header + request_body) + finish;
    let Some(at_server) = stream(ctx, path, ue, conn, cfg, Direction::Uplink, req, t, &mut r)
    else {
        return r;
    };
    let resp = tls(cfg.http_response_header + response_body);
    let done = stream(
        ctx,
        path,
        ue,
        conn,
        cfg,
        Direction::Downlink,
        resp,
        at_server,
        &mut r,
    );
    if let Some(d) = done {
        conn.last_active = d;
    }
    r.done_at = done;
    r
}

/// Opens a connection: the TCP handshake, then the TLS one. Returns when the client may
/// send its first request.
pub fn handshake<C: ?Sized, P: PacketPath<C> + ?Sized>(
    ctx: &mut C,
    path: &mut P,
    ue: NodeId,
    conn: &mut TcpConn,
    cfg: &TcpConfig,
    start: SimTime,
    r: &mut TransferReport,
) -> Option<SimTime> {
    let mut t = start;
    // SYN / SYN-ACK; the ACK rides with the first data.
    let mut rto = cfg.rto_init_s;
    let mut tries = 0;
    loop {
        let syn = path.deliver(ctx, ue, Direction::Uplink, t, 60);
        r.segments_ul += 1;
        r.bytes_ul += 60;
        let ack = syn.and_then(|a| {
            r.segments_dl += 1;
            r.bytes_dl += 60;
            path.deliver(ctx, ue, Direction::Downlink, a, 60)
        });
        match ack {
            Some(a) => {
                conn.sample((a - t) as f64 * 1e-9, cfg);
                t = a;
                break;
            }
            None => {
                tries += 1;
                r.rtos += 1;
                if tries > cfg.max_rtos {
                    return None;
                }
                t += (rto * 1e9) as u64;
                rto *= 2.0;
            }
        }
    }
    let flights: &[(Direction, u32)] = match cfg.tls {
        TlsVersion::None => &[],
        TlsVersion::Tls13 => &[
            (Direction::Uplink, cfg.client_hello),
            (Direction::Downlink, cfg.server_flight),
        ],
        TlsVersion::Tls12 => &[
            (Direction::Uplink, cfg.client_hello),
            (Direction::Downlink, cfg.server_flight),
            (Direction::Uplink, cfg.client_finish + 50),
            (Direction::Downlink, 51),
        ],
    };
    for &(dir, bytes) in flights {
        match stream(ctx, path, ue, conn, cfg, dir, bytes, t, r) {
            Some(a) => t = a,
            None => return None,
        }
    }
    conn.established = true;
    r.handshake_s += (t - start) as f64 * 1e-9;
    Some(t)
}

/// Sends `bytes` in one direction over an open connection; returns when the last byte
/// reached the receiver, or `None` when the connection gave up.
#[allow(clippy::too_many_arguments)]
pub fn stream<C: ?Sized, P: PacketPath<C> + ?Sized>(
    ctx: &mut C,
    path: &mut P,
    ue: NodeId,
    conn: &mut TcpConn,
    cfg: &TcpConfig,
    dir: Direction,
    bytes: u32,
    start: SimTime,
    r: &mut TransferReport,
) -> Option<SimTime> {
    let rev = match dir {
        Direction::Uplink => Direction::Downlink,
        Direction::Downlink => Direction::Uplink,
    };
    let mut fresh: VecDeque<u32> = VecDeque::new();
    let mut left = bytes.max(1);
    while left > 0 {
        let s = left.min(cfg.mss);
        fresh.push_back(s);
        left -= s;
    }
    let mut retx: VecDeque<u32> = VecDeque::new();
    let mut t = start;
    let mut last_arrival = start;
    let mut rtos_in_row = 0;
    while !fresh.is_empty() || !retx.is_empty() {
        let w = conn.cwnd.floor().max(1.0) as usize;
        let mut batch: Vec<(u32, bool)> = Vec::with_capacity(w);
        while batch.len() < w {
            if let Some(s) = retx.pop_front() {
                batch.push((s, true));
            } else if let Some(s) = fresh.pop_front() {
                batch.push((s, false));
            } else {
                break;
            }
        }
        let mut arrivals: Vec<SimTime> = Vec::new();
        let mut lost: Vec<u32> = Vec::new();
        let any_retx = batch.iter().any(|b| b.1);
        for &(s, is_retx) in &batch {
            let wire = s + cfg.header;
            match dir {
                Direction::Uplink => {
                    r.segments_ul += 1;
                    r.bytes_ul += u64::from(wire);
                }
                Direction::Downlink => {
                    r.segments_dl += 1;
                    r.bytes_dl += u64::from(wire);
                }
            }
            if is_retx {
                r.retransmissions += 1;
            }
            match path.deliver(ctx, ue, dir, t, wire) {
                Some(a) => arrivals.push(a),
                None => lost.push(s),
            }
        }
        arrivals.sort_unstable();
        let mut acks: Vec<SimTime> = Vec::new();
        for (i, &a) in arrivals.iter().enumerate() {
            if i % 2 == 1 || i + 1 == arrivals.len() {
                if let Some(ack) = path.deliver(ctx, ue, rev, a, cfg.header) {
                    acks.push(ack);
                }
            }
        }
        if let Some(&a) = arrivals.last() {
            last_arrival = last_arrival.max(a);
        }
        if acks.is_empty() {
            // Nothing came back: a timeout.
            retx.extend(lost);
            r.rtos += 1;
            rtos_in_row += 1;
            if rtos_in_row > cfg.max_rtos {
                return None;
            }
            t += (conn.rto * 1e9) as u64;
            conn.on_rto(cfg);
            continue;
        }
        rtos_in_row = 0;
        let round_end = *acks.iter().max().unwrap_or(&t);
        if !any_retx {
            // Karn's rule: only unambiguous samples.
            conn.sample((acks[0] - t) as f64 * 1e-9, cfg);
        }
        if lost.is_empty() {
            conn.grow(arrivals.len() as f64, round_end, cfg);
            t = round_end;
        } else if arrivals.len() >= 3 {
            retx.extend(lost);
            conn.on_loss(cfg);
            t = round_end;
        } else {
            retx.extend(lost);
            r.rtos += 1;
            t = round_end.max(t + (conn.rto * 1e9) as u64);
            conn.on_rto(cfg);
        }
    }
    Some(last_arrival)
}

/// An SCMS exchange's request and response bodies.
///
/// The sizes are the caller's: a certificate's encoded length comes from the protocol
/// crate's encoder, not from a constant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScmsExchange {
    /// Enrolment or authorization provisioning: a signed, encrypted request and a
    /// certificate response.
    Provisioning {
        /// Request body, bytes.
        request: u32,
        /// Response body, bytes.
        response: u32,
    },
    /// A pseudonym batch download: one encrypted response per certificate.
    CertificateDownload {
        /// Certificates in the batch.
        certs: u32,
        /// Encrypted bytes per certificate.
        per_cert: u32,
        /// Request body, bytes.
        request: u32,
    },
    /// A CRL fetch.
    CrlFetch {
        /// CRL bytes.
        crl: u32,
    },
}

impl ScmsExchange {
    /// `(request, response)` bodies, bytes.
    #[must_use]
    pub fn bodies(&self) -> (u32, u32) {
        match *self {
            ScmsExchange::Provisioning { request, response } => (request, response),
            ScmsExchange::CertificateDownload {
                certs,
                per_cert,
                request,
            } => (request, certs.saturating_mul(per_cert)),
            // An HTTP GET carries no body.
            ScmsExchange::CrlFetch { crl } => (0, crl),
        }
    }
}

/// The simple model's answer for the same exchange: one request and one response over a
/// link of fixed latency and rate, `2·L + (req + resp)·8 / rate` — what the backend kernel
/// charges today.
#[must_use]
pub fn simple_exchange_s(one_way: Duration, bps: f64, request: u32, response: u32) -> f64 {
    2.0 * one_way.as_secs_f64() + f64::from(request + response) * 8.0 / bps.max(1.0)
}

/// The TCP exchange against the simple one, and a sentence when they differ by more than
/// 25 % and 50 ms.
#[must_use]
pub fn compare_exchange(tcp: &TransferReport, simple_s: f64) -> (bool, String) {
    match tcp.seconds() {
        None => (
            true,
            format!(
                "the TCP exchange was abandoned after {} timeouts; the simple model gives {:.0} ms",
                tcp.rtos,
                simple_s * 1e3
            ),
        ),
        Some(s) => {
            let d = s - simple_s;
            let differs = d.abs() > 0.05 && d.abs() > 0.25 * simple_s;
            (
                differs,
                format!(
                    "TCP/TLS/HTTP takes {:.0} ms ({:.0} ms of handshakes, {} retransmissions) against {:.0} ms for the simple model",
                    s * 1e3,
                    tcp.handshake_s * 1e3,
                    tcp.retransmissions,
                    simple_s * 1e3
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cellular::CellPlan;
    use crate::cellular_packet::{BaseStationLayout, Rat};
    use crate::testctx::TestCtx;
    use v2xw_core::geom::Vec3;

    #[test]
    fn wired_media_differ_and_queue() {
        let ue = NodeId::new(1);
        let up = |l: &mut WiredLink, at: SimTime, bytes: u32, dir: Direction| {
            PacketPath::<()>::deliver(l, &mut (), ue, dir, at, bytes).unwrap()
        };
        let (mut fibre, mut dsl, mut mw) = (
            WiredLink::metro_fibre(),
            WiredLink::copper(),
            WiredLink::microwave(),
        );
        let f = up(&mut fibre, 0, 1500, Direction::Uplink);
        let d = up(&mut dsl, 0, 1500, Direction::Uplink);
        let m = up(&mut mw, 0, 1500, Direction::Uplink);
        assert!(f < 1_000_000, "fibre under 1 ms: {f}");
        assert!(d > 8_000_000, "DSL pays its interleaving: {d}");
        assert!(
            m < 2_000_000 && m != f,
            "microwave sub-2 ms and not fibre: {m}"
        );
        // A second packet offered at once queues behind the first on the line.
        let d2 = up(&mut dsl, 0, 1500, Direction::Uplink);
        assert!(d2 > d, "FIFO: {d2} after {d}");
        // Asymmetric DSL: the downstream serialises four times faster.
        let u = up(&mut WiredLink::copper(), 0, 1_000_000, Direction::Uplink);
        let dn = up(&mut WiredLink::copper(), 0, 1_000_000, Direction::Downlink);
        assert!(u > dn * 3, "up {u} vs down {dn}");
        assert_eq!(wired_cards().len(), 3);
    }

    #[test]
    fn handshakes_cost_round_trips() {
        let mut ctx = TestCtx::new(1);
        let mut path = ConstantPath {
            one_way: Duration::from_millis(20),
            bps: 1e9,
        };
        let ue = NodeId::new(1);
        let mut rtts = Vec::new();
        for tls in [TlsVersion::None, TlsVersion::Tls13, TlsVersion::Tls12] {
            let cfg = TcpConfig {
                tls,
                ..TcpConfig::default()
            };
            let mut conn = TcpConn::new(&cfg);
            let r = https_exchange(&mut ctx, &mut path, ue, &mut conn, &cfg, 0, 200, 300);
            rtts.push(r.seconds().unwrap() / 0.04);
            // Keep-alive: the second exchange skips the handshakes.
            let r2 = https_exchange(
                &mut ctx,
                &mut path,
                ue,
                &mut conn,
                &cfg,
                10_000_000_000,
                200,
                300,
            );
            assert!((r2.seconds().unwrap() / 0.04 - 1.0).abs() < 0.05);
        }
        // TCP 1 RTT + request; TLS 1.3 one more; TLS 1.2 two more.
        assert!((rtts[0] - 2.0).abs() < 0.05, "{rtts:?}");
        assert!((rtts[1] - 3.0).abs() < 0.05, "{rtts:?}");
        assert!((rtts[2] - 4.0).abs() < 0.05, "{rtts:?}");
    }

    #[test]
    fn slow_start_needs_rounds_and_loss_backs_off() {
        let mut ctx = TestCtx::new(1);
        let cfg = TcpConfig {
            tls: TlsVersion::None,
            ..TcpConfig::default()
        };
        let ue = NodeId::new(1);
        // 1 MB over a 100 Mbit/s, 25 ms path: slow start needs several RTTs where the
        // simple model charges one.
        let mut path = ConstantPath {
            one_way: Duration::from_millis(25),
            bps: 100e6,
        };
        let mut conn = TcpConn::new(&cfg);
        let r = https_exchange(&mut ctx, &mut path, ue, &mut conn, &cfg, 0, 0, 1_000_000);
        let simple = simple_exchange_s(Duration::from_millis(25), 100e6, 350, 1_000_250);
        let (differs, note) = compare_exchange(&r, simple);
        assert!(differs, "{note}");
        assert!(r.seconds().unwrap() > simple + 0.1, "{note}");
        // A lossy wired link retransmits and still completes.
        let mut wire = WiredLink::metro_fibre();
        wire.loss = 0.02;
        let mut conn = TcpConn::new(&cfg);
        let r = https_exchange(&mut ctx, &mut wire, ue, &mut conn, &cfg, 0, 0, 500_000);
        assert!(r.done_at.is_some());
        assert!(r.retransmissions > 0);
    }

    #[test]
    fn a_certificate_download_over_packet_lte() {
        let mut ctx = TestCtx::new(9);
        let mut uu = PacketUu::new(
            Rat::Lte,
            BaseStationLayout::lattice(CellPlan::urban_macro()),
        );
        let ue = NodeId::new(3);
        uu.observe_position(&mut ctx, ue, Vec3::new(100.0, 50.0, 0.0));
        let cfg = TcpConfig::default();
        let mut conn = TcpConn::new(&cfg);
        let (req, resp) = ScmsExchange::CertificateDownload {
            certs: 20,
            per_cert: 200,
            request: 300,
        }
        .bodies();
        let r = https_exchange(&mut ctx, &mut uu, ue, &mut conn, &cfg, 0, req, resp);
        let s = r.seconds().expect("completes");
        // Three handshake/request round trips of an LTE first hop (≈ 30 ms each) plus
        // the data: on the order of 0.1 s, well under a second.
        assert!(s > 0.04 && s < 1.0, "{s:.3} s");
        assert!(r.handshake_s > 0.0);
    }
}
