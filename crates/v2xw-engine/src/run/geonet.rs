//! Multi-hop hazard warnings: `net.geonet`.
//!
//! When a `safety.*` event fires on an equipped vehicle and the scenario selects
//! `net.geonet` (on `gn-btp`), the warning's DENM is GeoBroadcast to a circle around the
//! vehicle and run through [`v2xw_net::stack::dissem`]: every equipped vehicle carries
//! a GeoAdhoc router and a DEN service, positioned where the mobility tier last put it
//! and moving at its last velocity. Routers beacon for one beacon period plus jitter
//! before the warning, so their location tables are filled as a running stack's would be.
//!
//! The result goes into the event's note (what a person reads in the timeline) and into
//! [`super::RunReport::geonet`]. This is the `overlay` forwarding: forwarded copies ride the protocol run, not the MAC (`on-air`, the default, is [`super::gn_air`]): the
//! frame-level radio carries each node's own messages; the overlay adds the forwarding.
//!
//! # Determinism
//!
//! Nodes enter the run in actor-id order; the run's random stream is derived from the
//! scenario seed and the event's timeline index, so a warning's outcome does not depend
//! on how many warnings came before it.

use serde::Serialize;
use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::time::SimTime;
use v2xw_net::stack::{
    Area, AreaForwarding, CbfVariant, DenParams, DissemConfig, DisseminationRun, ForwardingConfig,
    GnMib, Hazard, MotionNode, NonAreaForwarding, UnitDisk,
};

use super::Engine;

/// What one multi-hop warning did, on integer grids so the run report stays `Eq`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GeonetWarning {
    /// When the hazard fired, ns.
    pub at: SimTime,
    /// The originating node.
    pub node: u64,
    /// Equipped vehicles in the network at that instant.
    pub vehicles: u64,
    /// Of those, the ones inside the warning area.
    pub in_area: u64,
    /// Of those, the ones the warning reached within the window.
    pub reached: u64,
    /// `reached / in_area`, thousandths.
    pub reach_permille: u64,
    /// GeoNetworking transmissions of the warning: originated + forwarded + kept alive.
    pub transmissions: u64,
    /// Forwards.
    pub forwarded: u64,
    /// Keep-alive forwards.
    pub kept_alive: u64,
    /// Median first-reception latency over the area, µs.
    pub latency_p50_us: u64,
    /// 95th percentile, µs.
    pub latency_p95_us: u64,
    /// Most hops a first reception took.
    pub max_hops: u8,
}

/// Beacon warm-up before the warning: one `itsGnBeaconServiceRetransmitTimer` plus its
/// maximum jitter (EN 302 636-4-1 Annex H), so every router has beaconed once.
const WARMUP_MS: u64 = 3_000 + 750;

impl Engine {
    /// Run the warning of the timeline item at `index`, raised by `actor` at `now`.
    /// `None` when `net.geonet` is unset, the layer is not `gn-btp`, or the vehicle has
    /// no on-board unit.
    pub(super) fn geonet_warning(
        &mut self,
        index: usize,
        actor: ActorId,
        node: Option<NodeId>,
        now: SimTime,
    ) -> Option<String> {
        let g = self.scenario.net.geonet.clone()?;
        if self.scenario.net.layer != "gn-btp" {
            return None;
        }
        let node = node?;
        // On the air the warning is the node's own DENM, forwarded frame by frame
        // (`super::gn_air`); here it is only registered to be followed.
        if self.gn_air.is_some() {
            return self.gn_track_warning(node, now);
        }
        let mut nodes = Vec::new();
        let mut origin = None;
        for (id, a) in &self.actors {
            if a.node.is_none() || a.class.is_vru() {
                continue;
            }
            if *id == actor {
                origin = Some(nodes.len());
            }
            let k = &a.last;
            // Positions are taken at the warning; the run's clock starts WARMUP before it,
            // so wind each node back along its velocity.
            let back = WARMUP_MS as f64 / 1_000.0;
            nodes.push(MotionNode {
                x: k.pos.x - k.vel.x * back,
                y: k.pos.y - k.vel.y * back,
                vx: k.vel.x,
                vy: k.vel.y,
                wrap_x: 0.0,
                wrap_y: 0.0,
            });
        }
        let origin = origin?;
        let o = &self.actors.get(&actor)?.last;
        // The originator is stopping: hold it where it is.
        nodes[origin] = MotionNode::fixed(o.pos.x, o.pos.y);
        let area = Area::circle(o.pos.x, o.pos.y, g.area_radius_m);
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
        let mib = super::gn_air::tuned_mib(mib, &g);
        let variant = match g.cbf_variant.as_str() {
            "etsi" => CbfVariant::Etsi,
            "dpd" => CbfVariant::Dpd,
            _ => CbfVariant::Gpc,
        };
        let window_ms = (g.window_s * 1_000.0).round().max(1.0) as u64;
        let cfg = DissemConfig {
            forwarding: ForwardingConfig {
                mib,
                cbf_variant: variant,
                out_of_area_broadcast_forward: variant == CbfVariant::Etsi,
            },
            den: DenParams {
                repetition_interval_ms: g.repetition_interval_ms,
                repetition_duration_ms: g
                    .repetition_duration_ms
                    .unwrap_or(crate::scenario::schema::Geonet::OVERLAY_REPETITION_DURATION_MS),
                transmission_interval_ms: if g.keep_alive_forwarding {
                    g.repetition_interval_ms.max(1_000)
                } else {
                    0
                },
                kaf: g.keep_alive_forwarding,
                ..DenParams::default()
            },
            beacons: true,
            end_ms: WARMUP_MS + window_ms,
            processing_us: 0,
            seed: self.scenario.seed ^ ((index as u64 + 1) << 32),
            mac: None,
            // No MAC and a unit-disk medium: a unicast hop is never lost, so there is
            // nothing to retry.
            unicast_retry_limit: 0,
        };
        let medium = UnitDisk { range_m: g.range_m };
        let vehicles = nodes.len() as u64;
        let rep = DisseminationRun::new(
            cfg,
            &medium,
            nodes,
            vec![Hazard {
                at_ms: WARMUP_MS,
                node: origin,
                area,
            }],
        )
        .run();
        let a = rep.actions.first()?;
        let w = GeonetWarning {
            at: now,
            node: u64::from(node.index()),
            vehicles,
            in_area: a.targets,
            reached: a.reached,
            reach_permille: if a.targets == 0 {
                1_000
            } else {
                a.reached * 1_000 / a.targets
            },
            transmissions: a.transmissions,
            forwarded: a.forwarded,
            kept_alive: a.kaf,
            latency_p50_us: (a.latency_p50_ms * 1_000.0).round() as u64,
            latency_p95_us: (a.latency_p95_ms * 1_000.0).round() as u64,
            max_hops: a.max_hops,
        };
        let text = format!(
            "; its warning reached {} of {} equipped vehicles within {:.0} m in {:.0} ms (95 %: {:.0} ms) over up to {} hops, with {} transmissions",
            w.reached,
            w.in_area,
            g.area_radius_m,
            a.latency_p50_ms,
            a.latency_p95_ms,
            u32::from(w.max_hops) + 1,
            w.transmissions
        );
        self.report.geonet.push(w);
        Some(text)
    }
}
