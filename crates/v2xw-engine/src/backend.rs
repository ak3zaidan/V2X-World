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

use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_core::rng::{EntityRef, RngDomain};
use v2xw_core::time::{Duration, SimTime};
use v2xw_proto::{Link, Transport};
use v2xw_radio::cellular::{
    CellCapacityUu, CellPlan, CellularUu, Direction, FixedLatencyUu, HandoverOutageUu,
    MecPlacement, Qos, RadioLatencyClass, SendOutcome, UuLatencyPreset,
};
use v2xw_radio::cellular_packet::{BaseStationLayout, PacketUu, Rat};
use v2xw_radio::ip_transport::{
    TcpConfig, TcpConn, TransferReport, handshake as tcp_handshake, stream as tcp_stream,
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
pub const BACKHAUL_MODELS: [&str; 3] = ["backhaul/fixed", "backhaul/cellular", "backhaul/none"];

/// `net.backend_net` ids this build accepts.
pub const BACKEND_NET_MODELS: [&str; 1] = ["backend-net/fixed"];

/// The default one-way backhaul latency of a wired roadside unit, milliseconds.
///
/// **Uncited.** It is the SCMS deployment's own backend-link figure
/// (`v2xw_proto::ScmsParams::backend_link_latency`, a `todo-calibrate` parameter on that
/// card), reused so a relayed message and a backend hop are not given two invented
/// numbers. A deployment's measured RSU backhaul latency replaces it through
/// `net.backhaul.params.latency_ms`.
pub const BACKHAUL_LATENCY_MS: f64 = 10.0;

/// The request a packet-level flow's first exchange carries, bytes: a signed and
/// encrypted SCMS request is of this order (`todo-calibrate` against the protocol crate's
/// encoded request sizes).
pub const PROBE_REQUEST_BYTES: u32 = 512;
/// The response of the same exchange, bytes (`todo-calibrate`, as above).
pub const PROBE_RESPONSE_BYTES: u32 = 4096;

/// The default wired backhaul capacity, Mbit/s. Uncited, as above.
pub const BACKHAUL_CAPACITY_MBPS: f64 = 1000.0;

/// How far a vehicle can be from a relaying roadside unit and still use its IP service,
/// metres.
///
/// **Uncited, `todo-calibrate`.** 300 m is the order of the DSRC roadside coverage radius
/// the US deployment guidance plans around; the value that matters for a run is the
/// sidelink's own reach, which the radio decides for a relayed *report* (it is a real
/// frame on the air). This radius gates only the exchanges the engine does not put on the
/// air frame by frame — a certificate top-up's batch download — and is on the model card
/// with that caveat.
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
    /// The packet-level model; every message rides a TCP/TLS connection per vehicle.
    Packet(Box<PacketUu>),
}

/// One roadside unit's backhaul.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Backhaul {
    /// One-way latency.
    pub latency: Duration,
    /// Capacity, bit/s.
    pub bandwidth_bps: u64,
    /// Whether the unit has a backhaul at all.
    pub connected: bool,
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
    kinds: BTreeMap<NodeId, AccessKind>,
    tcp: TcpConfig,
    conns: BTreeMap<NodeId, TcpConn>,
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
                    Uu::Packet(Box::new(m))
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
        }
        let default_backhaul = match &scenario.net.backhaul {
            None => Backhaul {
                latency: ms(BACKHAUL_LATENCY_MS),
                bandwidth_bps: (BACKHAUL_CAPACITY_MBPS * 1e6) as u64,
                connected: true,
            },
            Some(choice) => backhaul_from(&choice.id, &choice.params, nominal_uu)?,
        };
        Ok(BackendAccess {
            uu,
            uu_id,
            penetration,
            internet,
            nominal_uu,
            nominal_uu_bps,
            relay_range_m: RELAY_RANGE_M,
            default_backhaul,
            kinds: BTreeMap::new(),
            tcp: TcpConfig::default(),
            conns: BTreeMap::new(),
            report: AccessReport::default(),
        })
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
        match id {
            None => Ok(self.default_backhaul),
            Some(id) => backhaul_from(id, &serde_json::Value::Null, self.nominal_uu),
        }
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
            Some(Uu::Packet(m)) => CellularUu::<EngineCtx<'_>>::coverage(&**m, pos, t).is_some(),
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
            Some(Uu::Packet(m)) => {
                let _ = m.observe_position(ctx, node, pos);
                let conn = self
                    .conns
                    .entry(node)
                    .or_insert_with(|| TcpConn::new(&self.tcp));
                let mut r = TransferReport {
                    start: now,
                    ..TransferReport::default()
                };
                let mut t = Some(now);
                if !conn.established {
                    self.report.tcp_connections += 1;
                    t = tcp_handshake(ctx, &mut **m, node, conn, &self.tcp, now, &mut r);
                }
                let done = t.and_then(|t| {
                    tcp_stream(ctx, &mut **m, node, conn, &self.tcp, dir, bytes, t, &mut r)
                });
                if done.is_none() {
                    // A connection that gave up is closed; the next send opens a new one.
                    self.conns.remove(&node);
                }
                self.report.tcp_retransmissions += r.retransmissions;
                self.report.tcp_rtos += r.rtos;
                refresh_packet_report(&mut self.report, m);
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

    /// For `cellular/uu/packet-pf` only: the link a multi-message flow (a top-up, a
    /// re-enrolment, a trust-list fetch) sees *now*, measured rather than nominal.
    ///
    /// The flow's first request and response — [`PROBE_REQUEST_BYTES`] up and
    /// [`PROBE_RESPONSE_BYTES`] down, over the vehicle's own keep-alive TCP/TLS connection,
    /// opening it if needed — are carried at packet level through the PF scheduler; the
    /// link the backend kernel then charges for the whole flow has half that exchange's
    /// time as its one-way latency (so the handshakes are felt) and the response's measured
    /// goodput as its rate. `None` for the other Uu models, or when the exchange failed.
    pub fn packet_link(
        &mut self,
        ctx: &mut EngineCtx<'_>,
        node: NodeId,
        pos: Vec3,
    ) -> Option<Link> {
        let Some(Uu::Packet(m)) = self.uu.as_mut() else {
            return None;
        };
        let now = v2xw_core::ctx::Ctx::now(ctx);
        let _ = m.observe_position(ctx, node, pos);
        let conn = self
            .conns
            .entry(node)
            .or_insert_with(|| TcpConn::new(&self.tcp));
        let mut r = TransferReport {
            start: now,
            ..TransferReport::default()
        };
        let mut t = Some(now);
        if !conn.established {
            self.report.tcp_connections += 1;
            t = tcp_handshake(ctx, &mut **m, node, conn, &self.tcp, now, &mut r);
        }
        let up = t.and_then(|t| {
            tcp_stream(
                ctx,
                &mut **m,
                node,
                conn,
                &self.tcp,
                Direction::Uplink,
                PROBE_REQUEST_BYTES,
                t,
                &mut r,
            )
        });
        let done = up.and_then(|u| {
            tcp_stream(
                ctx,
                &mut **m,
                node,
                conn,
                &self.tcp,
                Direction::Downlink,
                PROBE_RESPONSE_BYTES,
                u,
                &mut r,
            )
        });
        self.report.tcp_retransmissions += r.retransmissions;
        self.report.tcp_rtos += r.rtos;
        refresh_packet_report(&mut self.report, m);
        let (Some(up), Some(done)) = (up, done) else {
            self.conns.remove(&node);
            self.report.uu_lost += 1;
            return None;
        };
        let dl_s = ((done - up) as f64 * 1e-9).max(1e-3);
        Some(Link {
            latency: Duration::from_nanos((done - now) / 2) + self.internet,
            bandwidth_bps: (f64::from(PROBE_RESPONSE_BYTES) * 8.0 / dl_s) as u64,
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
    match id {
        "backhaul/fixed" => Ok(Backhaul {
            latency: ms(num(params, "latency_ms").unwrap_or(BACKHAUL_LATENCY_MS)),
            bandwidth_bps: (num(params, "capacity_mbps").unwrap_or(BACKHAUL_CAPACITY_MBPS) * 1e6)
                as u64,
            connected: true,
        }),
        // A unit whose backhaul is itself a cellular modem: the Uu one-way latency, and
        // the MOSAIC uplink cap as its capacity.
        "backhaul/cellular" => Ok(Backhaul {
            latency: num(params, "latency_ms").map_or(uu_one_way, ms),
            bandwidth_bps: (num(params, "capacity_mbps")
                .unwrap_or(CellCapacityUu::MOSAIC_UPLINK_BPS / 1e6)
                * 1e6) as u64,
            connected: true,
        }),
        "backhaul/none" => Ok(Backhaul {
            latency: Duration::ZERO,
            bandwidth_bps: 0,
            connected: false,
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
