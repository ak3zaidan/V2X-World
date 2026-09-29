//! The pure half of a link budget: where the two antennas are, which law prices the link,
//! and what stands on its path.
//!
//! [`Engine::link_budget`](super::Engine) is split in two because its halves have opposite
//! natures. The **geometry** — the antennas, the focus-region placement, the building
//! classification, the street corner a blocked path turns round and the vehicles on a
//! clear one — is a pure function of the world and of the state the last mobility step
//! published; it was two thirds of a dense Manhattan run's time. The **budget** — the path
//! loss, the shadowing process correlated along a trajectory, the fading draw — carries
//! state and runs sequentially in receiver order, as it always has.
//!
//! So a frame's geometries are computed in parallel over its receivers (ADR 0004
//! decision 5, a pure map) and handed, in receiver order, to the sequential budget. Nothing
//! a geometry reads is written during the map, every model it calls takes `&self`, and the
//! result vector is indexed by the candidate list, so the answer is the same at any thread
//! count and in any schedule — which is what the content digest at 1, 2 and 4 threads
//! checks.

use std::collections::BTreeMap;

use v2xw_core::geom::Vec3;
use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::time::SimTime;
use v2xw_mobility::ActorSnapshot;
use v2xw_radio::{LinkEvaluation, LinkPlacement, LosResult, RadioEndpoint};
use v2xw_world::World;

use super::{ActorRecord, EngineCtx, distance_to_segment_2d, jamming, radio_class};
use crate::scenario::Scenario;
use crate::wiring::{ObstacleStack, PropagationChoice};

/// Shared borrows of everything a link's geometry reads. `Sync`, so a parallel map can
/// hold it; the engine itself is not (its node runtimes are not `Send`).
pub(super) struct LinkView<'a> {
    pub scenario: &'a Scenario,
    pub world: &'a World,
    pub snapshot: &'a ActorSnapshot,
    pub actors: &'a BTreeMap<ActorId, ActorRecord>,
    pub rsus: &'a BTreeMap<NodeId, Vec3>,
    pub node_class: &'a BTreeMap<NodeId, v2xw_radio::ActorClass>,
    pub obstacles: &'a ObstacleStack,
    pub focus: Option<&'a v2xw_radio::FocusPlan>,
    pub main_law: PropagationChoice,
    pub focus_law: Option<PropagationChoice>,
}

/// One link's geometry: what [`LinkView::geometry`] computes and the budget consumes.
pub(super) struct LinkGeometry {
    pub tx_end: RadioEndpoint,
    pub rx_end: RadioEndpoint,
    pub evaluation: Option<LinkEvaluation>,
    pub law: PropagationChoice,
    pub los: LosResult,
}

impl LinkView<'_> {
    /// The geometry of the link `tx → rx` at `now`, from the two ground points and, for the
    /// corner tracer, each end's street direction.
    #[allow(clippy::too_many_arguments)]
    pub fn geometry(
        &self,
        tx: NodeId,
        tx_pos: Vec3,
        tx_heading: Option<f64>,
        rx: NodeId,
        rx_pos: Vec3,
        rx_heading: Option<f64>,
        now: SimTime,
    ) -> LinkGeometry {
        // The antennas, not the ground points: a vehicle's phase centre stands at its
        // class's antenna height above the road (1.5 m for a car, TR 36.885), and a
        // roadside unit's position already carries its mast height. The building test
        // below is 2.5-D and compares roof heights against these.
        let tx_end = self.endpoint(tx, tx_pos, now);
        let rx_end = self.endpoint(rx, rx_pos, now);

        let evaluation = self.focus.map(|plan| plan.evaluate(tx_end.pos, rx_end.pos));
        let inside_focus = matches!(
            evaluation.map(|e| e.placement),
            Some(LinkPlacement::Inside)
        );
        // The law that prices this link, and so who charges buildings and whether the
        // street geometry is needed.
        let law = match (inside_focus, self.focus_law) {
            (true, Some(l)) => l,
            _ => self.main_law,
        };

        // What obstructs the path (04-models.md §3.5): building footprints crossed
        // (Sommer 2011), and for the geometric law the corner a blocked path turns round
        // and the vehicles on a clear one; terrain knife edges (ITU-R P.526). Each only
        // when the scenario turned it on and the world has it. A clear link costs nothing
        // but the index query.
        let dir = |h: Option<f64>| {
            h.map(|h| {
                let (s, c) = v2xw_core::math::sin_cos(h);
                (c, s)
            })
        };
        let mut los = self.obstacles.classify_directed_shared(
            self.world,
            tx_end.pos,
            rx_end.pos,
            law.traces_geometry(),
            (dir(tx_heading), dir(rx_heading)),
        );
        // TR 37.885's NLOSv is a same-street state: the vehicles on the path are looked
        // for only when no building is, and only near the line.
        if law.traces_geometry()
            && !los.class.has_building()
            && let Some(vehicles) = self.obstacles.vehicles.as_ref()
        {
            let set = self.vehicles_between(tx, rx, tx_end.pos, rx_end.pos);
            if !set.as_slice().is_empty() {
                let blocked =
                    <v2xw_radio::obstacle::VehicleBlockage as v2xw_radio::ObstacleModel<
                        EngineCtx<'_>,
                    >>::los(
                        vehicles, self.world, tx_end.pos, rx_end.pos, Some(&set)
                    );
                if blocked.class.has_vehicle() {
                    los = v2xw_radio::merge_los(&[los, blocked]);
                }
            }
        }
        LinkGeometry {
            tx_end,
            rx_end,
            evaluation,
            law,
            los,
        }
    }

    /// The vehicles that may stand between two antennas: every actor whose body centre is
    /// within 8 m of the straight path (half a 13 m truck and a lane), less the two ends'
    /// own bodies, as obstacles with their actual dimensions.
    pub fn vehicles_between(
        &self,
        tx: NodeId,
        rx: NodeId,
        a: Vec3,
        b: Vec3,
    ) -> v2xw_radio::ActorSet {
        let mid = Vec3::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y), 0.0);
        let radius = 0.5 * a.distance_2d(b) + 10.0;
        let mut set = Vec::new();
        for actor in self.snapshot.actors_within(mid, radius) {
            let Some(rec) = self.actors.get(&actor) else {
                continue;
            };
            if rec.node == Some(tx) || rec.node == Some(rx) {
                continue;
            }
            let Some(entry) = self.snapshot.get(actor) else {
                continue;
            };
            let class = radio_class(entry.view.class);
            // The published reference is the rear bumper; the body is centred half a
            // length ahead of it along the heading.
            let k = &entry.kinematics;
            let half = 0.5 * entry.view.dims.length_m;
            let (s, c) = v2xw_core::math::sin_cos(k.heading_rad);
            let centre = Vec3::new(k.pos.x + half * c, k.pos.y + half * s, k.pos.z);
            if distance_to_segment_2d(centre, a, b) > 8.0 {
                continue;
            }
            set.push(v2xw_radio::ActorObstacle {
                actor,
                pos: centre,
                dims: entry.view.dims,
                heading_rad: k.heading_rad,
                class,
            });
        }
        v2xw_radio::ActorSet::from_iter_sorted(set)
    }

    /// One node's radio endpoint at an instant: its antenna position and class.
    pub fn endpoint(&self, node: NodeId, ground: Vec3, now: SimTime) -> RadioEndpoint {
        if node.index() >= jamming::JAMMER_ID_BASE {
            // A jammer's declared position is its antenna's.
            return RadioEndpoint::isotropic(node, ground, v2xw_radio::ActorClass::Car, now);
        }
        if self.rsus.contains_key(&node) {
            // A mast's position is its antenna's (`crate::phase2`'s mast height), unless
            // `radio.devices.rsu.antenna_height_m` puts every roadside antenna at one
            // height above the ground under it.
            let device = crate::wiring::device_for(self.scenario, v2xw_radio::ActorClass::Rsu);
            let pos = match device.antenna_height_m {
                Some(h) => Vec3::new(
                    ground.x,
                    ground.y,
                    self.world.ground_height_at(ground.x, ground.y) + h,
                ),
                None => ground,
            };
            let mut end = RadioEndpoint::isotropic(node, pos, v2xw_radio::ActorClass::Rsu, now);
            end.gain_dbi = device.net_gain_db();
            return end;
        }
        let class = self
            .node_class
            .get(&node)
            .copied()
            .unwrap_or(v2xw_radio::ActorClass::Car);
        // `radio.devices`: the class's antenna height, and its gain net of the cable
        // between radio and antenna, at both ends of every link.
        let device = crate::wiring::device_for(self.scenario, class);
        let pos = Vec3::new(
            ground.x,
            ground.y,
            ground.z
                + device
                    .antenna_height_m
                    .unwrap_or_else(|| class.default_antenna_height_m()),
        );
        let mut end = RadioEndpoint::isotropic(node, pos, class, now);
        end.gain_dbi = device.net_gain_db();
        end
    }
}
