//! What an equipped vehicle's own sensors perceive of the road users around it: the input
//! to its Collective Perception Messages (ETSI TS 103 324).
//!
//! Like the GNSS model, this is the one place that holds both the truth and a vehicle's
//! view of it: it reads the true positions and hands the vehicle a list of **perceived**
//! objects — detected with a probability, placed with an error, hidden when something is
//! in the way — which is all the vehicle's facilities layer ever sees.
//!
//! # Sensors
//!
//! Every vehicle that sends CPMs carries the same default suite, representative of a
//! current production front sensor set, stated rather than invented:
//!
//! | Sensor | Range | Field of view | Source |
//! |---|---|---|---|
//! | long-range radar | 250 m | ±9° | Continental ARS 408-21 datasheet, far range |
//! | short-range radar | 70 m | ±45° | the same sensor's near range |
//! | front camera | 80 m | ±26° | a 52° mono camera (Mobileye EyeQ class); the range is this build's choice |
//!
//! # Detection, error, occlusion, tracking
//!
//! * **Detection probability** falls linearly from 0.99 at the sensor to 0.90 at its range
//!   limit — this build's choice, in the range the ETSI TR 103 562 simulation study
//!   assumes (it uses a single detection probability per sensor).
//! * **Position error** is Gaussian, σ = 0.10 m + 0.5 % of range for radar and
//!   0.20 m + 2 % of range for the camera — this build's choice, the order of the
//!   datasheet accuracies (radar ±0.4 m at 250 m; camera distance error of a few percent).
//! * **Occlusion**: a road user is not seen when the line from the sensor to it crosses a
//!   building footprint, or passes through the body of another vehicle nearer the sensor.
//! * **Tracking**: detections are associated to tracks (by the object itself — a perfect
//!   association, stated); a track is *confirmed* after 3 hits in its last 5 scans (the
//!   common M-of-N rule) and dropped after 1 s unseen. Only confirmed tracks are
//!   reported, each with an object id local to the perceiving vehicle.

use std::collections::BTreeMap;

use v2xw_core::geom::{Dims, Vec3};
use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::kinematics::Kinematics;
use v2xw_core::math;
use v2xw_core::rng::{EntityRef, RngDomain, RngRegistry};
use v2xw_core::time::SimTime;
use v2xw_world::World;

/// A sensor's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorKind {
    /// A radar.
    Radar,
    /// A mono camera.
    Camera,
}

/// One sensor of a vehicle's suite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SensorSpec {
    /// Its id within the vehicle (the CPM's `sensorId`).
    pub id: u8,
    /// What it is.
    pub kind: SensorKind,
    /// How far it sees, metres.
    pub range_m: f64,
    /// Half its horizontal field of view, radians, about the vehicle's heading.
    pub half_fov_rad: f64,
    /// Detection probability at the sensor.
    pub pd_near: f64,
    /// Detection probability at the range limit.
    pub pd_far: f64,
    /// Position error σ at the sensor, metres.
    pub sigma_base_m: f64,
    /// Position error σ growth per metre of range.
    pub sigma_per_m: f64,
}

/// The default suite (see the module table).
pub fn default_sensors() -> Vec<SensorSpec> {
    vec![
        SensorSpec {
            id: 1,
            kind: SensorKind::Radar,
            range_m: 250.0,
            half_fov_rad: 9f64.to_radians(),
            pd_near: 0.99,
            pd_far: 0.90,
            sigma_base_m: 0.10,
            sigma_per_m: 0.005,
        },
        SensorSpec {
            id: 2,
            kind: SensorKind::Radar,
            range_m: 70.0,
            half_fov_rad: 45f64.to_radians(),
            pd_near: 0.99,
            pd_far: 0.90,
            sigma_base_m: 0.10,
            sigma_per_m: 0.005,
        },
        SensorSpec {
            id: 3,
            kind: SensorKind::Camera,
            range_m: 80.0,
            half_fov_rad: 26f64.to_radians(),
            pd_near: 0.99,
            pd_far: 0.90,
            sigma_base_m: 0.20,
            sigma_per_m: 0.02,
        },
    ]
}

/// What kind of road user a perceived object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectClass {
    /// A vehicle.
    Vehicle,
    /// A pedestrian.
    Pedestrian,
    /// A cyclist.
    Cyclist,
}

/// One road user as a vehicle perceives it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Perceived {
    /// The object id, local to the perceiving vehicle.
    pub id: u16,
    /// Where it is perceived to be, world ENU metres.
    pub pos: Vec3,
    /// Its perceived velocity, m/s.
    pub vel: Vec3,
    /// Its perceived heading, ENU radians.
    pub heading_rad: f64,
    /// Its body.
    pub dims: Dims,
    /// Its class.
    pub class: ObjectClass,
    /// The position error σ, metres.
    pub sigma_m: f64,
    /// When it was last measured.
    pub measured_at: SimTime,
    /// When the track began.
    pub first_seen: SimTime,
    /// Which sensors saw it this scan, as a bit per sensor id.
    pub sensors: u8,
}

#[derive(Debug, Clone, Copy)]
struct TrackState {
    id: u16,
    /// Hit history of the last scans, newest in bit 0.
    hits: u8,
    last_seen: SimTime,
    last: Option<Perceived>,
}

/// One observer's scan input: who it is, where it truly is.
#[derive(Debug, Clone, Copy)]
pub struct Observer {
    /// Its node.
    pub node: NodeId,
    /// Its actor.
    pub actor: ActorId,
    /// Its true state.
    pub k: Kinematics,
}

/// A road user that may be perceived.
#[derive(Debug, Clone, Copy)]
pub struct Subject {
    /// The actor.
    pub actor: ActorId,
    /// Its true state.
    pub k: Kinematics,
    /// Its class.
    pub class: ObjectClass,
}

/// The perception model for every vehicle that runs it.
#[derive(Debug, Default)]
pub struct Perception {
    sensors: Vec<SensorSpec>,
    tracks: BTreeMap<(NodeId, ActorId), TrackState>,
    next_id: BTreeMap<NodeId, u16>,
}

/// A track is dropped this long after its last detection, nanoseconds.
const DROP_AFTER_NS: u64 = 1_000_000_000;

impl Perception {
    /// The model with the default suite.
    pub fn new() -> Self {
        Self {
            sensors: default_sensors(),
            ..Self::default()
        }
    }

    /// The suite.
    pub fn sensors(&self) -> &[SensorSpec] {
        &self.sensors
    }

    /// One scan at `now` by `observer` of `candidates` (the road users within the longest
    /// sensor range, in actor order): the confirmed tracks it reports, in object-id order.
    pub fn scan(
        &mut self,
        world: &World,
        rng: &mut RngRegistry,
        now: SimTime,
        observer: &Observer,
        candidates: &[Subject],
    ) -> Vec<Perceived> {
        let o = observer.k;
        let (s, c) = math::sin_cos(o.heading_rad);
        // The sensors sit at the front of the vehicle; the reference point is the rear
        // axle, so they are a body length (less the overhang) ahead of it.
        let mount = Vec3::new(
            o.pos.x + c * (o.dims.length_m - 1.0).max(0.0),
            o.pos.y + s * (o.dims.length_m - 1.0).max(0.0),
            o.pos.z + 0.6,
        );
        let mut seen: Vec<(ActorId, Perceived)> = Vec::new();
        for subject in candidates {
            if subject.actor == observer.actor {
                continue;
            }
            let p = subject.k.pos;
            let (dx, dy) = (p.x - mount.x, p.y - mount.y);
            let range = math::hypot(dx, dy);
            let bearing = v2xw_msg::j2945::wrap_pi(math::atan2(dy, dx) - o.heading_rad);
            let mut sensors = 0u8;
            let mut sigma = f64::INFINITY;
            for sensor in &self.sensors {
                if range > sensor.range_m || bearing.abs() > sensor.half_fov_rad {
                    continue;
                }
                let pd = sensor.pd_near
                    + (sensor.pd_far - sensor.pd_near) * (range / sensor.range_m).clamp(0.0, 1.0);
                let draw = rng
                    .stream(RngDomain::Perception, EntityRef::Node(observer.node))
                    .bool(pd);
                if draw {
                    sensors |= 1 << sensor.id.min(7);
                    sigma = sigma.min(sensor.sigma_base_m + sensor.sigma_per_m * range);
                }
            }
            if sensors == 0 {
                continue;
            }
            if occluded(world, mount, p, subject.actor, observer.actor, candidates) {
                continue;
            }
            let stream = rng.stream(RngDomain::Perception, EntityRef::Node(observer.node));
            let ex = stream.normal(0.0, sigma);
            let ey = stream.normal(0.0, sigma);
            seen.push((
                subject.actor,
                Perceived {
                    id: 0,
                    pos: Vec3::new(p.x + ex, p.y + ey, p.z),
                    vel: subject.k.vel,
                    heading_rad: subject.k.heading_rad,
                    dims: subject.k.dims,
                    class: subject.class,
                    sigma_m: sigma,
                    measured_at: now,
                    first_seen: now,
                    sensors,
                },
            ));
        }
        // Tracking: update every track of this observer, then report the confirmed ones.
        let node = observer.node;
        let mut hit: BTreeMap<ActorId, Perceived> = seen.into_iter().collect();
        let keys: Vec<(NodeId, ActorId)> = self
            .tracks
            .range((node, ActorId::new(0))..=(node, ActorId::new(u32::MAX)))
            .map(|(k, _)| *k)
            .collect();
        for key in keys {
            if let Some(t) = self.tracks.get_mut(&key) {
                t.hits <<= 1;
                if let Some(p) = hit.remove(&key.1) {
                    t.hits |= 1;
                    t.last_seen = now;
                    let first_seen = t.last.map_or(now, |l| l.first_seen);
                    t.last = Some(Perceived {
                        id: t.id,
                        first_seen,
                        ..p
                    });
                }
                if now.saturating_sub(t.last_seen) > DROP_AFTER_NS {
                    self.tracks.remove(&key);
                }
            }
        }
        for (actor, p) in hit {
            let next = self.next_id.entry(node).or_insert(0);
            *next = next.wrapping_add(1).max(1);
            let id = *next;
            self.tracks.insert(
                (node, actor),
                TrackState {
                    id,
                    hits: 1,
                    last_seen: now,
                    last: Some(Perceived { id, ..p }),
                },
            );
        }
        let mut out: Vec<Perceived> = self
            .tracks
            .range((node, ActorId::new(0))..=(node, ActorId::new(u32::MAX)))
            .filter(|(_, t)| (t.hits & 0b1_1111).count_ones() >= 3)
            .filter_map(|(_, t)| t.last)
            .collect();
        out.sort_by_key(|p| p.id);
        out
    }

    /// The actor behind `(observer, object id)`, for labelling a warning a CPM object
    /// caused. Ground truth: the engine's, never a node's.
    pub fn actor_of(&self, observer: NodeId, id: u16) -> Option<ActorId> {
        self.tracks
            .range((observer, ActorId::new(0))..=(observer, ActorId::new(u32::MAX)))
            .find(|(_, t)| t.id == id)
            .map(|(k, _)| k.1)
    }

    /// Forgets an observer that left the run.
    pub fn forget(&mut self, observer: NodeId) {
        self.tracks
            .retain(|(n, _), _| *n != observer);
        self.next_id.remove(&observer);
    }
}

/// Whether the line from `a` to the subject at `b` is blocked by a building or by the body
/// of another road user nearer the sensor.
fn occluded(
    world: &World,
    a: Vec3,
    b: Vec3,
    subject: ActorId,
    observer: ActorId,
    others: &[Subject],
) -> bool {
    if !world.buildings_on_segment(a, b).is_empty() {
        return true;
    }
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-6 {
        return false;
    }
    others.iter().any(|o| {
        if o.actor == subject || o.actor == observer || o.class != ObjectClass::Vehicle {
            return false;
        }
        let q = o.k.pos;
        let u = ((q.x - a.x) * dx + (q.y - a.y) * dy) / len2;
        if !(0.05..0.95).contains(&u) {
            return false;
        }
        let (cx, cy) = (a.x + u * dx, a.y + u * dy);
        // A vehicle body blocks the line when the line passes within half its width of
        // its centre line — a disc of that radius, conservative for a long body seen end
        // on and generous for one seen side on.
        math::hypot(q.x - cx, q.y - cy) < 0.5 * o.k.dims.width_m.max(0.5)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_suite_covers_the_front() {
        let s = default_sensors();
        assert_eq!(s.len(), 3);
        assert!(s.iter().any(|x| x.range_m >= 250.0));
        assert!(s.iter().all(|x| x.pd_far < x.pd_near));
    }
}
