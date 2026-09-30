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

use v2xw_core::geom::{Dims, Vec3};
use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::time::SimTime;
use v2xw_mobility::ActorSnapshot;
use v2xw_radio::{LinkEvaluation, LinkPlacement, LosResult, RadioEndpoint};
use v2xw_world::World;

use super::{ActorRecord, EngineCtx, distance_to_segment_2d, jamming};
use crate::scenario::Scenario;
use crate::wiring::{ObstacleStack, PropagationChoice};

/// Shared borrows of everything a link's geometry reads. `Sync`, so a parallel map can
/// hold it; the engine itself is not (its node runtimes are not `Send`).
pub(super) struct LinkView<'a> {
    pub scenario: &'a Scenario,
    pub world: &'a World,
    pub actors: &'a BTreeMap<ActorId, ActorRecord>,
    pub rsus: &'a BTreeMap<NodeId, Vec3>,
    pub node_class: &'a BTreeMap<NodeId, v2xw_radio::ActorClass>,
    pub obstacles: &'a ObstacleStack,
    pub bodies: &'a BodyIndex,
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
        let mut tx_end = self.endpoint(tx, tx_pos, now);
        let mut rx_end = self.endpoint(rx, rx_pos, now);
        // Each vehicle's antenna pattern toward the other end (TR 37.885 §6.1.4): a
        // truck's front and rear panels are 6.75 dB down to its side. It changes the
        // ends' gains only, not their positions, so nothing below depends on it.
        tx_end.gain_dbi += self.pattern_gain_db(&tx_end, tx_heading, rx_end.pos);
        rx_end.gain_dbi += self.pattern_gain_db(&rx_end, rx_heading, tx_end.pos);

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
    ///
    /// The set is the one a disk query round the path's midpoint, `½·|ab| + 10 m` across,
    /// filtered by the 8 m test, has always given; it is found by walking the cells of
    /// [`BodyIndex`] along the path instead, so a link across Midtown looks at the
    /// vehicles on its own street and not at every vehicle within half a kilometre.
    pub fn vehicles_between(
        &self,
        tx: NodeId,
        rx: NodeId,
        a: Vec3,
        b: Vec3,
    ) -> v2xw_radio::ActorSet {
        let mid = Vec3::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y), 0.0);
        let radius = 0.5 * a.distance_2d(b) + 10.0;
        let mut near = self.bodies.near_segment(a, b, CORRIDOR_M);
        near.sort_unstable();
        let mut set = Vec::new();
        for i in near {
            let body = &self.bodies.bodies[i as usize];
            // The disk query's own test, on the published reference point.
            if body.pos.distance_2d(mid) > radius {
                continue;
            }
            let Some(rec) = self.actors.get(&body.actor) else {
                continue;
            };
            if rec.node == Some(tx) || rec.node == Some(rx) {
                continue;
            }
            if distance_to_segment_2d(body.centre, a, b) > CORRIDOR_M {
                continue;
            }
            set.push(v2xw_radio::ActorObstacle {
                actor: body.actor,
                pos: body.centre,
                dims: body.dims,
                heading_rad: body.heading_rad,
                class: body.class,
            });
        }
        v2xw_radio::ActorSet::from_iter_sorted(set)
    }

    /// The pattern of `end`'s antenna toward `toward`, dB relative to its scalar gain
    /// (`radio.devices.obu.antenna_pattern`, [`v2xw_radio::antenna`]). Zero for a
    /// roadside unit, a pedestrian's device, a jammer, and a vehicle whose heading is
    /// not known.
    pub fn pattern_gain_db(&self, end: &RadioEndpoint, heading: Option<f64>, toward: Vec3) -> f64 {
        use crate::scenario::schema::ObuAntennaPattern;
        use v2xw_radio::antenna::AntennaMount;
        if end.node.index() >= jamming::JAMMER_ID_BASE || self.rsus.contains_key(&end.node) {
            return 0.0;
        }
        let mount = match (self.scenario.radio.devices.obu.antenna_pattern, AntennaMount::for_class(end.class)) {
            (_, AntennaMount::Isotropic) | (ObuAntennaPattern::Isotropic, _) => return 0.0,
            (ObuAntennaPattern::Rooftop, _) => AntennaMount::Rooftop,
            (ObuAntennaPattern::Tr37885, m) => m,
        };
        let (dx, dy, dz) = (toward.x - end.pos.x, toward.y - end.pos.y, toward.z - end.pos.z);
        let horizontal = v2xw_core::math::hypot(dx, dy);
        let elevation = v2xw_core::math::atan2(dz, horizontal.max(1e-9));
        let azimuth = heading.map_or(0.0, |h| v2xw_core::math::atan2(dy, dx) - h);
        if heading.is_none() && mount == AntennaMount::FrontRear {
            return 0.0;
        }
        mount.relative_gain_db(azimuth, elevation)
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

/// How far a vehicle's body centre may be from a link's straight path and still stand on
/// it, metres: half a 13 m truck and a lane.
const CORRIDOR_M: f64 = 8.0;

/// One actor's body at the last published state: where the link budget's vehicle test
/// reads it.
pub(super) struct Body {
    actor: ActorId,
    /// The published reference point (the rear bumper).
    pos: Vec3,
    /// The body's centre, half a length ahead of the reference along the heading.
    centre: Vec3,
    dims: Dims,
    heading_rad: f64,
    class: v2xw_radio::ActorClass,
}

/// The actors' bodies at the last published state, bucketed by body centre on a fine
/// grid, rebuilt with the snapshot at every mobility step.
///
/// The vehicles on a link's path used to be found by the snapshot's disk query round the
/// path's midpoint. Its cells are a kilometre across (sized for the radio range, ADR 0004
/// decision 6), so on a dense map every line-of-sight link walked every actor — the
/// published reference, a map lookup, a sine and a cosine each — to keep the few within
/// 8 m of the line. This computes each body once per step and walks only the cells along
/// the path.
#[derive(Default)]
pub(super) struct BodyIndex {
    /// In actor-id order, so sorted indices are id order.
    bodies: Vec<Body>,
    origin: (f64, f64),
    cols: i64,
    rows: i64,
    /// Row-major cells: `start[c]..start[c + 1]` indexes `items`.
    start: Vec<u32>,
    items: Vec<u32>,
}

impl BodyIndex {
    /// The grid's cell size, metres.
    const CELL_M: f64 = 32.0;

    /// The index of a snapshot's actors.
    pub fn build(snapshot: &ActorSnapshot) -> Self {
        let bodies: Vec<Body> = snapshot
            .iter()
            .map(|entry| {
                let k = &entry.kinematics;
                let half = 0.5 * entry.view.dims.length_m;
                let (s, c) = v2xw_core::math::sin_cos(k.heading_rad);
                Body {
                    actor: entry.view.actor,
                    pos: k.pos,
                    centre: Vec3::new(k.pos.x + half * c, k.pos.y + half * s, k.pos.z),
                    dims: entry.view.dims,
                    heading_rad: k.heading_rad,
                    class: super::radio_class(entry.view.class),
                }
            })
            .collect();
        Self::from_bodies(bodies)
    }

    /// The index of bodies already computed, which must be in actor-id order.
    fn from_bodies(bodies: Vec<Body>) -> Self {
        if bodies.is_empty() {
            return Self::default();
        }
        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for b in &bodies {
            min_x = min_x.min(b.centre.x);
            min_y = min_y.min(b.centre.y);
            max_x = max_x.max(b.centre.x);
            max_y = max_y.max(b.centre.y);
        }
        let origin = (min_x, min_y);
        let cols = (((max_x - min_x) / Self::CELL_M).floor() as i64 + 1).max(1);
        let rows = (((max_y - min_y) / Self::CELL_M).floor() as i64 + 1).max(1);
        let cell_of = |p: Vec3| -> usize {
            let i = (((p.x - origin.0) / Self::CELL_M).floor() as i64).clamp(0, cols - 1);
            let j = (((p.y - origin.1) / Self::CELL_M).floor() as i64).clamp(0, rows - 1);
            (j * cols + i) as usize
        };
        let n = (cols * rows) as usize;
        let mut count = vec![0u32; n + 1];
        for b in &bodies {
            count[cell_of(b.centre) + 1] += 1;
        }
        for c in 0..n {
            count[c + 1] += count[c];
        }
        let start = count.clone();
        let mut fill = count;
        let mut items = vec![0u32; bodies.len()];
        for (i, b) in bodies.iter().enumerate() {
            let c = cell_of(b.centre);
            items[fill[c] as usize] = i as u32;
            fill[c] += 1;
        }
        Self {
            bodies,
            origin,
            cols,
            rows,
            start,
            items,
        }
    }

    /// The indices of every body whose centre may lie within `half_width_m` of the segment
    /// `a → b`: a superset, each at most once, in no particular order.
    ///
    /// Row by row, the cells the segment's band `half_width_m` (plus a metre of slack for
    /// rounding) either side can reach: a centre within the half-width of a point `Q` of
    /// the segment is within it in `x` and in `y`, so its row's band holds `Q` and its
    /// column is inside the band's `x` extent grown by the half-width.
    fn near_segment(&self, a: Vec3, b: Vec3, half_width_m: f64) -> Vec<u32> {
        let mut out = Vec::new();
        if self.bodies.is_empty() {
            return out;
        }
        let m = half_width_m + 1.0;
        let cell = Self::CELL_M;
        let (ox, oy) = self.origin;
        let col = |x: f64| ((x - ox) / cell).floor() as i64;
        let row = |y: f64| ((y - oy) / cell).floor() as i64;
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let j0 = row(a.y.min(b.y) - m).max(0);
        let j1 = row(a.y.max(b.y) + m).min(self.rows - 1);
        for j in j0..=j1 {
            let band_lo = oy + j as f64 * cell - m;
            let band_hi = oy + (j + 1) as f64 * cell + m;
            let (t0, t1) = if dy == 0.0 {
                if a.y < band_lo || a.y > band_hi {
                    continue;
                }
                (0.0, 1.0)
            } else {
                let (u, v) = ((band_lo - a.y) / dy, (band_hi - a.y) / dy);
                (u.min(v).max(0.0), u.max(v).min(1.0))
            };
            if t0 > t1 {
                continue;
            }
            let (x0, x1) = (a.x + t0 * dx, a.x + t1 * dx);
            let i0 = col(x0.min(x1) - m).max(0);
            let i1 = col(x0.max(x1) + m).min(self.cols - 1);
            for i in i0..=i1 {
                let c = (j * self.cols + i) as usize;
                out.extend_from_slice(
                    &self.items[self.start[c] as usize..self.start[c + 1] as usize],
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator (SplitMix64), so the test needs no RNG crate.
    struct Mix(u64);
    impl Mix {
        fn unit(&mut self) -> f64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        }
        fn point(&mut self, span: f64) -> Vec3 {
            Vec3::new(span * self.unit() - 200.0, span * self.unit() - 300.0, 0.0)
        }
    }

    #[test]
    fn the_corridor_walk_finds_every_body_within_the_half_width() {
        let mut g = Mix(7);
        let span = 2_000.0;
        let bodies: Vec<Body> = (0..3_000u32)
            .map(|i| {
                let centre = g.point(span);
                Body {
                    actor: ActorId::new(i),
                    pos: centre,
                    centre,
                    dims: Dims::new(4.5, 1.8, 1.5),
                    heading_rad: 0.0,
                    class: v2xw_radio::ActorClass::Car,
                }
            })
            .collect();
        let index = BodyIndex::from_bodies(bodies);
        let mut checked = 0usize;
        for k in 0..2_000 {
            let a = g.point(span);
            // Every shape of segment: long diagonals, short hops, axis-aligned, degenerate,
            // and ones running off the populated area.
            let b = match k % 5 {
                0 => g.point(span),
                1 => Vec3::new(a.x + 40.0 * g.unit(), a.y, 0.0),
                2 => Vec3::new(a.x, a.y - 900.0 * g.unit(), 0.0),
                3 => a,
                _ => Vec3::new(a.x + 3_000.0, a.y + 50.0, 0.0),
            };
            let mut near = index.near_segment(a, b, CORRIDOR_M);
            let found = near.len();
            near.sort_unstable();
            near.dedup();
            assert_eq!(near.len(), found, "a body was returned twice");
            for (i, body) in index.bodies.iter().enumerate() {
                if distance_to_segment_2d(body.centre, a, b) <= CORRIDOR_M {
                    checked += 1;
                    assert!(
                        near.binary_search(&(i as u32)).is_ok(),
                        "body {i} at {:?} is {} m from {a:?} -> {b:?} and was not found",
                        body.centre,
                        distance_to_segment_2d(body.centre, a, b)
                    );
                }
            }
        }
        assert!(checked > 1_000, "the segments must actually pass near bodies: {checked}");
    }
}
