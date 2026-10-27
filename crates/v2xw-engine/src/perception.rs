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
//! * **Occlusion** ([`Occlusion`]): by default (`centre-line`, the model before
//!   2026-10-26) a road user is not seen when the line from the sensor to its reference
//!   point crosses a building footprint, or passes within half a body width of another
//!   vehicle's reference point (a disc). With `perception.occlusion: outline` a sensor
//!   sees the target when **any corner of its outline** is in that sensor's range and
//!   field of view and the line to that corner crosses no building, no other vehicle's
//!   body (an oriented rectangle) and not the target's own body (only the corners on its
//!   silhouette count) — the visible-corner rule of the Artery environment model
//!   (Günther, Riebl et al., "Realizing Collective Perception in a Vehicle", IEEE VNC
//!   2016), as segment / oriented-rectangle intersection in this crate's own geometry.
//!   Re-implemented from Günther et al. 2016 and TS 103 324 V2.1.1 §7.1.4 (the sensor
//!   information container). Artery was read for behaviour only and no code was copied.
//! * **Tracking**: detections are associated to tracks (by the object itself — a perfect
//!   association, stated); a track is *confirmed* after 3 hits in its last 5 scans (the
//!   common M-of-N rule) and dropped after 1 s unseen. Only confirmed tracks are
//!   reported, each with an object id local to the perceiving vehicle.
//!
//! # Configurable suites and roadside sensors (WP-14)
//!
//! Each [`SensorSpec`] has a mount (an anchor on the body — front bumper, rear bumper or
//! body centre — plus an offset ahead, left and up) and a boresight azimuth about the
//! vehicle's heading, so a suite can carry rear and corner radars. A roadside unit with
//! the `cpm` role carries a fixed suite whose offsets and azimuths are in the
//! east-north-up frame ([`Frame::Fixed`]); its tracks feed an infrastructure CPM
//! (TS 103 324 V2.1.1 §7.1.3, the `originatingRsuContainer`). The default suite, the
//! default occlusion and the absence of a `perception` block reproduce the detections of
//! the model before 2026-10-26 draw for draw.

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
    /// A lidar.
    Lidar,
}

impl SensorKind {
    /// The CPM's `SensorType` (TS 103 324 / CDD): radar 1, lidar 2, mono-video 3.
    pub const fn cpm_type(self) -> u8 {
        match self {
            SensorKind::Radar => 1,
            SensorKind::Lidar => 2,
            SensorKind::Camera => 3,
        }
    }
}

/// Where on a vehicle's body a sensor's offset is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountAnchor {
    /// The front bumper.
    Front,
    /// The rear bumper (the vehicle's reference point).
    Rear,
    /// The body's centre.
    Centre,
}

/// The frame a suite's mounts and azimuths are stated in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Frame {
    /// A vehicle's body: x ahead along the heading, y to the left.
    #[default]
    Vehicle,
    /// The world's east-north-up frame: a roadside unit's mast.
    Fixed,
}

/// How a road user is hidden from a sensor (the module notes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Occlusion {
    /// The line to the reference point; other vehicles are discs (the model before
    /// 2026-10-26, the default).
    #[default]
    CentreLine,
    /// Any visible outline corner; other vehicles are oriented rectangles.
    Outline,
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
    /// The boresight, radians counter-clockwise from the frame's x axis (the heading, or
    /// east at a roadside unit).
    pub azimuth_rad: f64,
    /// What the offset is measured from on a vehicle (ignored at a roadside unit).
    pub anchor: MountAnchor,
    /// The offset: ahead, left and up, metres (east, north and up at a roadside unit).
    pub offset_m: [f64; 3],
}

impl SensorSpec {
    /// Where the sensor sits in its frame, metres from the reference point: ahead, left,
    /// up. A vehicle sensor stays on or ahead of the rear bumper.
    pub fn mount_in_frame(&self, frame: Frame, dims: Dims) -> [f64; 3] {
        let [dx, dy, dz] = self.offset_m;
        match frame {
            Frame::Fixed => [dx, dy, dz],
            Frame::Vehicle => {
                let anchor = match self.anchor {
                    MountAnchor::Front => dims.length_m,
                    MountAnchor::Rear => 0.0,
                    MountAnchor::Centre => 0.5 * dims.length_m,
                };
                [(anchor + dx).max(0.0), dy, dz]
            }
        }
    }
}

/// The default suite's mount: a body length less 1 m of overhang ahead of the rear
/// bumper, 0.6 m up, looking ahead (the model before 2026-10-26).
pub const FRONT: SensorSpec = SensorSpec {
    id: 0,
    kind: SensorKind::Radar,
    range_m: 0.0,
    half_fov_rad: 0.0,
    pd_near: 0.0,
    pd_far: 0.0,
    sigma_base_m: 0.0,
    sigma_per_m: 0.0,
    azimuth_rad: 0.0,
    anchor: MountAnchor::Front,
    offset_m: [-1.0, 0.0, 0.6],
};

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
            ..FRONT
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
            ..FRONT
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
            ..FRONT
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

/// The roadside units' default suite: four cameras with a 90° opening facing east, north,
/// west and south from the mast, 80 m each — together all round the junction. This
/// build's choice, stated: an intersection camera of the mono-video class the vehicle
/// suite uses, one per approach of a four-leg junction.
pub fn default_rsu_sensors() -> Vec<SensorSpec> {
    (0..4u8)
        .map(|i| SensorSpec {
            id: i + 1,
            kind: SensorKind::Camera,
            range_m: 80.0,
            half_fov_rad: 45f64.to_radians(),
            pd_near: 0.99,
            pd_far: 0.90,
            sigma_base_m: 0.20,
            sigma_per_m: 0.02,
            azimuth_rad: (f64::from(i) * 90.0).to_radians(),
            anchor: MountAnchor::Centre,
            offset_m: [0.0, 0.0, 0.0],
        })
        .collect()
}

/// One configured sensor ([`crate::scenario::schema::SensorConf`]).
pub fn sensor_of(c: &crate::scenario::schema::SensorConf) -> SensorSpec {
    use crate::scenario::schema::{MountSpec, SensorKindSpec};
    let kind = match c.kind {
        SensorKindSpec::Radar => SensorKind::Radar,
        SensorKindSpec::Camera => SensorKind::Camera,
        SensorKindSpec::Lidar => SensorKind::Lidar,
    };
    // The kind's error defaults: the module table for radar and camera; for lidar a
    // ±3 cm range accuracy (the Velodyne VLP-16 datasheet's "typical accuracy") and
    // 0.1 % of range, this build's choice.
    let (base, per) = match kind {
        SensorKind::Radar => (0.10, 0.005),
        SensorKind::Camera => (0.20, 0.02),
        SensorKind::Lidar => (0.03, 0.001),
    };
    SensorSpec {
        id: c.id,
        kind,
        range_m: c.range_m,
        half_fov_rad: (0.5 * c.fov_deg).to_radians(),
        pd_near: c.pd_near,
        pd_far: c.pd_far,
        sigma_base_m: c.sigma_base_m.unwrap_or(base),
        sigma_per_m: c.sigma_range_fraction.unwrap_or(per),
        azimuth_rad: c.azimuth_deg.to_radians(),
        anchor: match c.mount {
            MountSpec::Front => MountAnchor::Front,
            MountSpec::Rear => MountAnchor::Rear,
            MountSpec::Centre => MountAnchor::Centre,
        },
        offset_m: [c.ahead_m, c.left_m, c.height_m],
    }
}

impl Perception {
    /// The model a scenario's `perception` block describes.
    pub fn from_spec(spec: &crate::scenario::schema::PerceptionSpec) -> Self {
        let suite = |v: &[crate::scenario::schema::SensorConf]| -> Vec<SensorSpec> {
            v.iter().map(sensor_of).collect()
        };
        Perception::configured(
            spec.vehicle.as_deref().map_or_else(default_sensors, suite),
            spec.classes
                .iter()
                .map(|(k, v)| (k.clone(), suite(v)))
                .collect(),
            spec.rsu.as_deref().map_or_else(default_rsu_sensors, suite),
            match spec.occlusion {
                crate::scenario::schema::OcclusionSpec::CentreLine => Occlusion::CentreLine,
                crate::scenario::schema::OcclusionSpec::Outline => Occlusion::Outline,
            },
        )
    }
}

/// One observer's scan input: who it is, where it truly is.
#[derive(Debug, Clone, Copy)]
pub struct Observer {
    /// Its node.
    pub node: NodeId,
    /// Its actor; `None` for a roadside unit.
    pub actor: Option<ActorId>,
    /// Its true state (a roadside unit: its mast position, heading 0, no body).
    pub k: Kinematics,
    /// The frame its suite is stated in.
    pub frame: Frame,
    /// Which of the model's suites it carries ([`Perception::suite_for`]).
    pub suite: usize,
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

/// The perception model for every vehicle and roadside unit that runs it.
#[derive(Debug, Default)]
pub struct Perception {
    /// Suite 0 is the default vehicle suite; the others are per-class suites and the
    /// roadside units'.
    suites: Vec<Vec<SensorSpec>>,
    /// Vehicle class name → suite.
    by_class: BTreeMap<String, usize>,
    /// The roadside units' suite, when one is configured.
    rsu_suite: Option<usize>,
    occlusion: Occlusion,
    /// A `perception` block configured the model: the CPM states each sensor's mount and
    /// the candidate search reaches past the body.
    configured: bool,
    tracks: BTreeMap<(NodeId, ActorId), TrackState>,
    next_id: BTreeMap<NodeId, u16>,
}

/// A track is dropped this long after its last detection, nanoseconds.
const DROP_AFTER_NS: u64 = 1_000_000_000;

impl Perception {
    /// The model with the default suites: the vehicles' front suite and the roadside
    /// units' cameras.
    pub fn new() -> Self {
        Self {
            suites: vec![default_sensors(), default_rsu_sensors()],
            rsu_suite: Some(1),
            ..Self::default()
        }
    }

    /// The model a scenario's `perception` block configures: the default vehicle suite
    /// (`vehicle`), per-class suites, the roadside units' suite and the occlusion rule.
    pub fn configured(
        vehicle: Vec<SensorSpec>,
        by_class: Vec<(String, Vec<SensorSpec>)>,
        rsu: Vec<SensorSpec>,
        occlusion: Occlusion,
    ) -> Self {
        let mut suites = vec![vehicle];
        let mut map = BTreeMap::new();
        for (class, suite) in by_class {
            map.insert(class, suites.len());
            suites.push(suite);
        }
        let rsu_suite = Some(suites.len());
        suites.push(rsu);
        Self {
            suites,
            by_class: map,
            rsu_suite,
            occlusion,
            configured: true,
            ..Self::default()
        }
    }

    /// The default vehicle suite.
    pub fn sensors(&self) -> &[SensorSpec] {
        &self.suites[0]
    }

    /// Suite `i` (0 when `i` names none).
    pub fn suite(&self, i: usize) -> &[SensorSpec] {
        self.suites.get(i).unwrap_or(&self.suites[0])
    }

    /// The suite a vehicle of class `class` carries.
    pub fn suite_for(&self, class: &str) -> usize {
        self.by_class.get(class).copied().unwrap_or(0)
    }

    /// The suite a roadside unit with the `cpm` role carries, when the model has one.
    pub fn rsu_suite(&self) -> Option<usize> {
        self.rsu_suite
    }

    /// Whether a `perception` block configured the model.
    pub fn is_configured(&self) -> bool {
        self.configured
    }

    /// The occlusion rule.
    pub fn occlusion(&self) -> Occlusion {
        self.occlusion
    }

    /// How far around an observer of `suite` with body `dims` candidates are gathered,
    /// metres: the longest range, and under a configured model also the farthest mount
    /// and a body length, so a sensor mounted ahead of the reference point is not cut
    /// short (the model before 2026-10-26 gathered at the longest range alone).
    pub fn reach(&self, suite: usize, frame: Frame, dims: Dims) -> f64 {
        let sensors = self.suite(suite);
        let range = sensors.iter().map(|s| s.range_m).fold(0.0_f64, f64::max);
        if !self.configured {
            return range;
        }
        let mount = sensors
            .iter()
            .map(|s| {
                let [x, y, _] = s.mount_in_frame(frame, dims);
                math::hypot(x, y)
            })
            .fold(0.0_f64, f64::max);
        range + mount + 15.0
    }

    /// The CPM's description of suite `suite` (the sensor information container): each
    /// sensor's id, type, range and opening, and under a configured model its mount.
    pub fn sensor_info(&self, suite: usize, frame: Frame, dims: Dims) -> Vec<v2xw_node::cpm::SensorIn> {
        self.suite(suite)
            .iter()
            .map(|s| v2xw_node::cpm::SensorIn {
                id: s.id,
                sensor_type: s.kind.cpm_type(),
                range_m: s.range_m,
                half_fov_rad: s.half_fov_rad,
                azimuth_rad: s.azimuth_rad,
                mount_m: self.configured.then(|| s.mount_in_frame(frame, dims)),
            })
            .collect()
    }

    /// One scan at `now` by `observer` of `candidates` (the road users within
    /// [`Perception::reach`], in actor order): the confirmed tracks it reports, in
    /// object-id order.
    pub fn scan(
        &mut self,
        world: &World,
        rng: &mut RngRegistry,
        now: SimTime,
        observer: &Observer,
        candidates: &[Subject],
    ) -> Vec<Perceived> {
        let seen = detect(
            self.suite(observer.suite),
            self.occlusion,
            world,
            rng,
            now,
            observer,
            candidates,
        );
        self.track(now, observer.node, seen)
    }

    /// Tracking: updates every track of `node` with this scan's detections, then reports
    /// the confirmed ones.
    fn track(
        &mut self,
        now: SimTime,
        node: NodeId,
        seen: Vec<(ActorId, Perceived)>,
    ) -> Vec<Perceived> {
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

    /// The actor behind the confirmed track of `observer` nearest `pos`, within `gate_m`.
    ///
    /// A CPM's object ids are re-keyed by the sender's pseudonym on the air, so the
    /// measurement layer associates a reported object with its track by position (the
    /// CPM carries the track's own estimate, to the centimetre). Ground truth, as above.
    pub fn actor_near(&self, observer: NodeId, pos: Vec3, gate_m: f64) -> Option<ActorId> {
        let mut best: Option<(f64, ActorId)> = None;
        for ((_, actor), t) in self
            .tracks
            .range((observer, ActorId::new(0))..=(observer, ActorId::new(u32::MAX)))
        {
            let Some(p) = t.last else { continue };
            let d = p.pos.distance_2d(pos);
            if d <= gate_m && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, *actor));
            }
        }
        best.map(|(_, a)| a)
    }

    /// Forgets an observer that left the run.
    pub fn forget(&mut self, observer: NodeId) {
        self.tracks.retain(|(n, _), _| *n != observer);
        self.next_id.remove(&observer);
    }
}

/// One observer's detections this scan, in candidate order, before tracking.
///
/// Under [`Occlusion::CentreLine`] the draws are those of the model before 2026-10-26: per
/// candidate, one detection draw per sensor whose range and field of view hold the
/// reference point, then two error draws when a sensor that drew a detection has a clear
/// line. With the default suite every sensor shares one mount, so this is draw for draw
/// the old single-mount scan.
fn detect(
    sensors: &[SensorSpec],
    occlusion: Occlusion,
    world: &World,
    rng: &mut RngRegistry,
    now: SimTime,
    observer: &Observer,
    candidates: &[Subject],
) -> Vec<(ActorId, Perceived)> {
    let o = observer.k;
    let heading = match observer.frame {
        Frame::Vehicle => o.heading_rad,
        Frame::Fixed => 0.0,
    };
    let (s, c) = math::sin_cos(heading);
    let mounts: Vec<Vec3> = sensors
        .iter()
        .map(|sensor| {
            let [ahead, left, up] = sensor.mount_in_frame(observer.frame, o.dims);
            Vec3::new(
                o.pos.x + c * ahead - s * left,
                o.pos.y + s * ahead + c * left,
                o.pos.z + up,
            )
        })
        .collect();
    // The other vehicles' bodies, for the outline rule.
    let bodies: Vec<(ActorId, Rect)> = match occlusion {
        Occlusion::CentreLine => Vec::new(),
        Occlusion::Outline => candidates
            .iter()
            .filter(|x| x.class == ObjectClass::Vehicle && Some(x.actor) != observer.actor)
            .map(|x| (x.actor, Rect::of(x)))
            .collect(),
    };
    let mut seen: Vec<(ActorId, Perceived)> = Vec::new();
    for subject in candidates {
        if Some(subject.actor) == observer.actor {
            continue;
        }
        let p = subject.k.pos;
        let mut bits = 0u8;
        let mut sigma = f64::INFINITY;
        match occlusion {
            Occlusion::CentreLine => {
                // Whether the line from mount `i` is blocked, computed once per distinct
                // mount (the default suite has one).
                let mut blocked: Vec<Option<bool>> = vec![None; sensors.len()];
                for (i, sensor) in sensors.iter().enumerate() {
                    let m = mounts[i];
                    let (dx, dy) = (p.x - m.x, p.y - m.y);
                    let range = math::hypot(dx, dy);
                    let bearing =
                        v2xw_msg::j2945::wrap_pi(math::atan2(dy, dx) - heading - sensor.azimuth_rad);
                    if range > sensor.range_m || bearing.abs() > sensor.half_fov_rad {
                        continue;
                    }
                    let pd = sensor.pd_near
                        + (sensor.pd_far - sensor.pd_near)
                            * (range / sensor.range_m).clamp(0.0, 1.0);
                    let draw = rng
                        .stream(RngDomain::Perception, EntityRef::Node(observer.node))
                        .bool(pd);
                    if !draw {
                        continue;
                    }
                    let hidden = match blocked[i] {
                        Some(b) => b,
                        None => {
                            let b = occluded(world, m, p, subject.actor, observer.actor, candidates);
                            for (j, mj) in mounts.iter().enumerate() {
                                if *mj == m {
                                    blocked[j] = Some(b);
                                }
                            }
                            b
                        }
                    };
                    if !hidden {
                        bits |= 1 << sensor.id.min(7);
                        sigma = sigma.min(sensor.sigma_base_m + sensor.sigma_per_m * range);
                    }
                }
            }
            Occlusion::Outline => {
                let own = Rect::of(subject);
                let corners = own.corners();
                for (i, sensor) in sensors.iter().enumerate() {
                    let m = mounts[i];
                    // The nearest corner this sensor sees, if any.
                    let mut nearest = f64::INFINITY;
                    for q in corners {
                        let (dx, dy) = (q.0 - m.x, q.1 - m.y);
                        let range = math::hypot(dx, dy);
                        if range >= nearest {
                            continue;
                        }
                        let bearing = v2xw_msg::j2945::wrap_pi(
                            math::atan2(dy, dx) - heading - sensor.azimuth_rad,
                        );
                        if range > sensor.range_m || bearing.abs() > sensor.half_fov_rad {
                            continue;
                        }
                        if corner_hidden(world, m, q, &own, subject.actor, &bodies) {
                            continue;
                        }
                        nearest = range;
                    }
                    if !nearest.is_finite() {
                        continue;
                    }
                    let pd = sensor.pd_near
                        + (sensor.pd_far - sensor.pd_near)
                            * (nearest / sensor.range_m).clamp(0.0, 1.0);
                    let draw = rng
                        .stream(RngDomain::Perception, EntityRef::Node(observer.node))
                        .bool(pd);
                    if draw {
                        bits |= 1 << sensor.id.min(7);
                        sigma = sigma.min(sensor.sigma_base_m + sensor.sigma_per_m * nearest);
                    }
                }
            }
        }
        if bits == 0 {
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
                sensors: bits,
            },
        ));
    }
    seen
}

/// Whether the line from `a` to the subject at `b` is blocked by a building or by the body
/// of another road user nearer the sensor (the centre-line rule).
fn occluded(
    world: &World,
    a: Vec3,
    b: Vec3,
    subject: ActorId,
    observer: Option<ActorId>,
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
        if o.actor == subject || Some(o.actor) == observer || o.class != ObjectClass::Vehicle {
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

/// A road user's body as an oriented rectangle on the ground.
#[derive(Debug, Clone, Copy)]
struct Rect {
    centre: (f64, f64),
    /// The unit heading `(cos, sin)`.
    axis: (f64, f64),
    half_len: f64,
    half_wid: f64,
}

impl Rect {
    /// A vehicle's body extends a length ahead of its reference point (the rear bumper,
    /// as the engine places it); a vulnerable road user's is centred on it.
    fn of(x: &Subject) -> Rect {
        let (s, c) = math::sin_cos(x.k.heading_rad);
        let len = x.k.dims.length_m.max(0.2);
        let ahead = if x.class == ObjectClass::Vehicle {
            0.5 * len
        } else {
            0.0
        };
        Rect {
            centre: (x.k.pos.x + ahead * c, x.k.pos.y + ahead * s),
            axis: (c, s),
            half_len: 0.5 * len,
            half_wid: 0.5 * x.k.dims.width_m.max(0.2),
        }
    }

    fn corners(&self) -> [(f64, f64); 4] {
        let (c, s) = self.axis;
        let (l, w) = (self.half_len, self.half_wid);
        let (x, y) = self.centre;
        [
            (x + l * c - w * s, y + l * s + w * c),
            (x + l * c + w * s, y + l * s - w * c),
            (x - l * c + w * s, y - l * s - w * c),
            (x - l * c - w * s, y - l * s + w * c),
        ]
    }

    /// Whether the segment from `a` to `b` passes through the rectangle's interior over a
    /// positive length (a segment grazing an edge or ending on it does not). The segment
    /// is clipped against the rectangle in its own frame (the Liang–Barsky slab test).
    fn crossed_by(&self, a: (f64, f64), b: (f64, f64)) -> bool {
        const EPS: f64 = 1e-6;
        let (c, s) = self.axis;
        let local = |p: (f64, f64)| {
            let (dx, dy) = (p.0 - self.centre.0, p.1 - self.centre.1);
            (dx * c + dy * s, -dx * s + dy * c)
        };
        let (p0, p1) = (local(a), local(b));
        let d = (p1.0 - p0.0, p1.1 - p0.1);
        let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
        let (hl, hw) = (self.half_len - EPS, self.half_wid - EPS);
        if hl <= 0.0 || hw <= 0.0 {
            return false;
        }
        for (p, q) in [
            (-d.0, p0.0 + hl),
            (d.0, hl - p0.0),
            (-d.1, p0.1 + hw),
            (d.1, hw - p0.1),
        ] {
            if p.abs() < 1e-12 {
                if q < 0.0 {
                    return false;
                }
                continue;
            }
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
        let len = math::hypot(d.0, d.1);
        (t1 - t0) * len > EPS
    }
}

/// Whether the line from sensor `m` to corner `q` of the subject `own` is blocked: by a
/// building, by another vehicle's body, or by the subject's own body (a corner on its far
/// side is not on its silhouette).
fn corner_hidden(
    world: &World,
    m: Vec3,
    q: (f64, f64),
    own: &Rect,
    subject: ActorId,
    bodies: &[(ActorId, Rect)],
) -> bool {
    let a = (m.x, m.y);
    if own.crossed_by(a, q) {
        return true;
    }
    if bodies
        .iter()
        .any(|(id, r)| *id != subject && r.crossed_by(a, q))
    {
        return true;
    }
    !world
        .buildings_on_segment(m, Vec3::new(q.0, q.1, m.z))
        .is_empty()
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

    fn empty_world() -> World {
        // A ring road and no building: only vehicles occlude.
        v2xw_mobility::worlds::ring(&v2xw_mobility::worlds::RingParams::default())
            .expect("the ring world builds")
    }

    fn car(id: u32, x: f64, y: f64, heading_deg: f64, dims: Dims) -> Subject {
        let mut k = Kinematics::at_rest(0, Vec3::new(x, y, 0.0));
        k.heading_rad = heading_deg.to_radians();
        k.dims = dims;
        Subject {
            actor: ActorId::new(id),
            k,
            class: ObjectClass::Vehicle,
        }
    }

    fn observer_of(s: &Subject, suite: usize) -> Observer {
        Observer {
            node: NodeId::new(s.actor.index()),
            actor: Some(s.actor),
            k: s.k,
            frame: Frame::Vehicle,
            suite,
        }
    }

    /// Who `observer` detects in one scan, with a perfect detector (pd 1).
    fn seen_by(
        sensors: &[SensorSpec],
        occlusion: Occlusion,
        observer: &Observer,
        others: &[Subject],
    ) -> Vec<(u32, u8)> {
        let world = empty_world();
        let mut rng = RngRegistry::new(7);
        detect(sensors, occlusion, &world, &mut rng, 0, observer, others)
            .into_iter()
            .map(|(a, p)| (a.index(), p.sensors))
            .collect()
    }

    fn perfect(mut s: Vec<SensorSpec>) -> Vec<SensorSpec> {
        for x in &mut s {
            x.pd_near = 1.0;
            x.pd_far = 1.0;
        }
        s
    }

    const TRUCK: Dims = Dims::new(12.0, 2.5, 3.5);

    /// The scan before 2026-10-26, kept verbatim as the reference the default suite must
    /// reproduce draw for draw.
    fn legacy_detect(
        world: &World,
        rng: &mut RngRegistry,
        observer: &Observer,
        candidates: &[Subject],
    ) -> Vec<(ActorId, Vec3, f64, u8)> {
        let o = observer.k;
        let (s, c) = math::sin_cos(o.heading_rad);
        let mount = Vec3::new(
            o.pos.x + c * (o.dims.length_m - 1.0).max(0.0),
            o.pos.y + s * (o.dims.length_m - 1.0).max(0.0),
            o.pos.z + 0.6,
        );
        let mut out = Vec::new();
        for subject in candidates {
            if Some(subject.actor) == observer.actor {
                continue;
            }
            let p = subject.k.pos;
            let (dx, dy) = (p.x - mount.x, p.y - mount.y);
            let range = math::hypot(dx, dy);
            let bearing = v2xw_msg::j2945::wrap_pi(math::atan2(dy, dx) - o.heading_rad);
            let mut sensors = 0u8;
            let mut sigma = f64::INFINITY;
            for sensor in &default_sensors() {
                if range > sensor.range_m || bearing.abs() > sensor.half_fov_rad {
                    continue;
                }
                let pd = sensor.pd_near
                    + (sensor.pd_far - sensor.pd_near) * (range / sensor.range_m).clamp(0.0, 1.0);
                if rng
                    .stream(RngDomain::Perception, EntityRef::Node(observer.node))
                    .bool(pd)
                {
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
            out.push((subject.actor, Vec3::new(p.x + ex, p.y + ey, p.z), sigma, sensors));
        }
        out
    }

    /// The default suite and the centre-line rule reproduce the model before 2026-10-26
    /// draw for draw, through the configured path too (a `perception` block that states
    /// the default suite), over dense random scenes with occluders.
    #[test]
    fn the_default_suite_reproduces_the_legacy_detections_draw_for_draw() {
        let world = empty_world();
        let mut scene = RngRegistry::new(11);
        let configured = Perception::from_spec(&crate::scenario::schema::PerceptionSpec {
            vehicle: Some(
                default_sensors()
                    .iter()
                    .map(|s| crate::scenario::schema::SensorConf {
                        id: s.id,
                        kind: match s.kind {
                            SensorKind::Radar => crate::scenario::schema::SensorKindSpec::Radar,
                            _ => crate::scenario::schema::SensorKindSpec::Camera,
                        },
                        range_m: s.range_m,
                        fov_deg: 2.0 * s.half_fov_rad.to_degrees(),
                        azimuth_deg: 0.0,
                        mount: crate::scenario::schema::MountSpec::Front,
                        ahead_m: -1.0,
                        left_m: 0.0,
                        height_m: 0.6,
                        pd_near: 0.99,
                        pd_far: 0.90,
                        sigma_base_m: None,
                        sigma_range_fraction: None,
                    })
                    .collect(),
            ),
            ..Default::default()
        });
        let mut compared = 0usize;
        for scene_no in 0..40u32 {
            let st = scene.stream(RngDomain::Perception, EntityRef::Node(NodeId::new(scene_no)));
            let subjects: Vec<Subject> = (0..60u32)
                .map(|i| {
                    let dims = if st.bool(0.2) { TRUCK } else { Dims::CAR };
                    car(
                        i + 1,
                        st.uniform(-150.0, 150.0),
                        st.uniform(-40.0, 40.0),
                        st.uniform(-180.0, 180.0),
                        dims,
                    )
                })
                .collect();
            for obs in subjects.iter().take(6) {
                let o = observer_of(obs, 0);
                let mut a = RngRegistry::new(u64::from(scene_no));
                let mut b = RngRegistry::new(u64::from(scene_no));
                let mut c = RngRegistry::new(u64::from(scene_no));
                let legacy = legacy_detect(&world, &mut a, &o, &subjects);
                let now: Vec<_> = detect(default_sensors().as_slice(), Occlusion::CentreLine, &world, &mut b, 0, &o, &subjects)
                    .into_iter()
                    .map(|(id, p)| (id, p.pos, p.sigma_m, p.sensors))
                    .collect();
                let conf: Vec<_> = detect(configured.suite(0), configured.occlusion(), &world, &mut c, 0, &o, &subjects)
                    .into_iter()
                    .map(|(id, p)| (id, p.pos, p.sigma_m, p.sensors))
                    .collect();
                assert_eq!(legacy, now, "scene {scene_no}");
                assert_eq!(legacy, conf, "scene {scene_no}, configured path");
                compared += legacy.len();
                // The stream is left where the old model left it, too.
                let next = |r: &mut RngRegistry| {
                    r.stream(RngDomain::Perception, EntityRef::Node(o.node)).uniform(0.0, 1.0)
                };
                assert_eq!(next(&mut a).to_bits(), next(&mut b).to_bits());
            }
        }
        assert!(compared > 200, "the scenes produced only {compared} detections");
    }

    /// Outline occlusion: a car squarely behind a truck is hidden; one beside the truck's
    /// line, with a corner past the truck's edge, is seen — which the centre-line rule,
    /// aiming at the car's reference point, gets wrong.
    #[test]
    fn a_car_behind_a_truck_is_hidden_and_a_partly_visible_one_is_seen() {
        let sensors = perfect(default_sensors());
        let me = car(1, 0.0, 0.0, 0.0, Dims::CAR);
        let o = observer_of(&me, 0);
        let truck = car(2, 20.0, 0.0, 0.0, TRUCK);
        let hidden = car(3, 40.0, 0.0, 0.0, Dims::CAR);
        let partial = car(4, 40.0, 2.0, 0.0, Dims::CAR);
        let out = seen_by(&sensors, Occlusion::Outline, &o, &[truck, hidden]);
        assert_eq!(out.iter().map(|x| x.0).collect::<Vec<_>>(), vec![2], "{out:?}");
        let out = seen_by(&sensors, Occlusion::Outline, &o, &[truck, partial]);
        assert_eq!(out.iter().map(|x| x.0).collect::<Vec<_>>(), vec![2, 4], "{out:?}");
        // The centre-line rule hides the partly visible car.
        let out = seen_by(&sensors, Occlusion::CentreLine, &o, &[truck, partial]);
        assert_eq!(out.iter().map(|x| x.0).collect::<Vec<_>>(), vec![2], "{out:?}");
    }

    /// A truck crossing the line of sight side-on hides the car beyond it under the
    /// outline rule; the disc at the truck's rear bumper did not.
    #[test]
    fn a_crossing_truck_hides_what_the_disc_rule_saw() {
        let sensors = perfect(default_sensors());
        let me = car(1, 0.0, 0.0, 0.0, Dims::CAR);
        let o = observer_of(&me, 0);
        let truck = car(2, 20.0, -6.0, 90.0, TRUCK);
        let beyond = car(3, 40.0, -1.0, 0.0, Dims::CAR);
        let out = seen_by(&sensors, Occlusion::Outline, &o, &[truck, beyond]);
        assert_eq!(out.iter().map(|x| x.0).collect::<Vec<_>>(), vec![2], "{out:?}");
        let out = seen_by(&sensors, Occlusion::CentreLine, &o, &[truck, beyond]);
        assert_eq!(out.iter().map(|x| x.0).collect::<Vec<_>>(), vec![2, 3], "{out:?}");
    }

    /// A rear radar sees a car overtaking in the next lane, which the front suite cannot.
    #[test]
    fn a_rear_radar_detects_an_overtaking_car() {
        let mut suite = perfect(default_sensors());
        suite.push(SensorSpec {
            id: 4,
            kind: SensorKind::Radar,
            range_m: 80.0,
            half_fov_rad: 75f64.to_radians(),
            pd_near: 1.0,
            pd_far: 1.0,
            sigma_base_m: 0.10,
            sigma_per_m: 0.005,
            azimuth_rad: std::f64::consts::PI,
            anchor: MountAnchor::Rear,
            offset_m: [0.0, 0.0, 0.5],
        });
        let me = car(1, 0.0, 0.0, 30.0, Dims::CAR);
        let o = observer_of(&me, 0);
        // Ten metres behind and one lane to the left, in the observer's own frame.
        let (s, c) = math::sin_cos(30f64.to_radians());
        let (bx, by) = (-10.0, 3.5);
        let overtaking = car(2, c * bx - s * by, s * bx + c * by, 30.0, Dims::CAR);
        assert!(seen_by(&perfect(default_sensors()), Occlusion::Outline, &o, &[overtaking]).is_empty());
        assert!(
            seen_by(&perfect(default_sensors()), Occlusion::CentreLine, &o, &[overtaking]).is_empty()
        );
        for occlusion in [Occlusion::CentreLine, Occlusion::Outline] {
            let out = seen_by(&suite, occlusion, &o, &[overtaking]);
            assert_eq!(out, vec![(2, 1 << 4)], "{occlusion:?}: only the rear radar sees it");
        }
    }

    /// A roadside unit's cameras, azimuths from east, watch every approach.
    #[test]
    fn a_roadside_suite_sees_every_approach_and_only_its_openings() {
        let rsu = Observer {
            node: NodeId::new(900),
            actor: None,
            k: Kinematics::at_rest(0, Vec3::new(0.0, 0.0, 6.0)),
            frame: Frame::Fixed,
            suite: 1,
        };
        let cams = perfect(default_rsu_sensors());
        let around = [
            car(1, 40.0, 0.0, 180.0, Dims::CAR),
            car(2, 0.0, 40.0, 270.0, Dims::CAR),
            car(3, -40.0, 0.0, 0.0, Dims::CAR),
            car(4, 0.0, -40.0, 90.0, Dims::CAR),
        ];
        let out = seen_by(&cams, Occlusion::Outline, &rsu, &around);
        assert_eq!(
            out,
            vec![(1, 1 << 1), (2, 1 << 2), (3, 1 << 3), (4, 1 << 4)],
            "each approach by its own camera"
        );
        // One camera facing east sees nothing to the west.
        let east = &cams[..1];
        let out = seen_by(east, Occlusion::Outline, &rsu, &around);
        assert_eq!(out, vec![(1, 1 << 1)]);
    }

    #[test]
    fn a_segment_grazing_a_body_does_not_cross_it() {
        let r = Rect::of(&car(1, 0.0, 0.0, 0.0, Dims::new(4.0, 2.0, 1.5)));
        // Body spans x 0..4, y -1..1.
        assert!(r.crossed_by((-5.0, 0.0), (10.0, 0.5)));
        assert!(!r.crossed_by((-5.0, 1.0), (10.0, 1.0)), "along an edge");
        assert!(!r.crossed_by((-5.0, 3.0), (10.0, 2.0)), "passes by");
        assert!(!r.crossed_by((-5.0, 0.0), (0.0, 0.0)), "ends on the body");
        assert!(r.crossed_by((2.0, 5.0), (2.0, -5.0)), "crosses side-on");
    }
}
