//! The Collective Perception basic service of a vehicle: which perceived objects go into
//! the next CPM, and when a CPM is due (ETSI TS 103 324 V2.1.1).
//!
//! The objects come from the vehicle's own sensors (the engine's perception model hands
//! them in, as the GNSS model hands in the position belief); this module decides, with the
//! standard's generation rules, what the vehicle says about them.
//!
//! # The rules
//!
//! Read on 2026-10-06 from ETSI TS 103 324 V2.1.1 (2023-06), clause 6.1.2 and Annex F
//! (Table F.1, the recommended values), downloaded from etsi.org:
//!
//! | Rule | Value | Source |
//! |---|---|---|
//! | Generation events every `T_GenCpm`, with `T_GenCpmMin ≤ T_GenCpm ≤ T_GenCpmMax` | 100 ms | §6.1.2.1; Table F.1 recommends `T_GenCpmMin` = 100 ms for a vehicle |
//! | A CPM goes at least every `T_GenCpmMax` | 1,000 ms | Table F.1 |
//! | **Type-B** (vehicles, and motorcyclists): included when first detected since the last generation event, or its position moved more than `minPositionChangeThreshold`, or its ground speed changed more than `minGroundSpeedChangeThreshold`, or its ground velocity's orientation by at least `minGroundVelocityOrientationChangeThreshold`, since it was last included, or it was last included `T_GenCpmMax` ago or more | 4 m, 0.5 m/s, 4°, 1 s | §6.1.2.3 rule 2 a–e; Table F.1 |
//! | **Type-A** (pedestrians, cyclists, animals): included when first detected since the last generation event; and when any Type-A object has gone `T_GenCpmMax / 2` without inclusion, **all** Type-A objects are included | 500 ms | §6.1.2.3 rule 1 a–b |
//! | The sensor information container goes in the first CPM and whenever it was last sent `T_AddSensorInformation` ago or more | 1,000 ms | §6.1.2.2; Table F.1 |
//! | At most 255 perceived objects per CPM (`PerceivedObjects SIZE(0..255)`) | 255 | the ASN.1, read |
//!
//! Not modelled: the object perception quality threshold (Table F.1's
//! `ObjectPerceptionQualityThreshold` of 3, computed per §7.1.8.6 — the perception model
//! hands in confirmed tracks only, which stands in for it), perception regions, the
//! look-ahead that pulls next event's Type-B objects into this one (a "may"), and the
//! multi-channel operation of Annex D. Every value is a named constant, so a study can
//! change it.

use std::collections::BTreeMap;

use v2xw_core::geom::Vec3;
use v2xw_core::time::{Duration, SimTime};

/// `T_GenCpm`: how often generation is checked.
pub const T_GEN_CPM: Duration = Duration::from_millis(100);
/// `T_GenCpmMax`: the longest between two CPMs.
pub const T_GEN_CPM_MAX: Duration = Duration::from_millis(1_000);
/// Include a vehicle object that moved this far since its last inclusion, metres.
pub const POSITION_CHANGE_M: f64 = 4.0;
/// ... or whose speed changed this much, m/s.
pub const SPEED_CHANGE_MPS: f64 = 0.5;
/// ... or whose heading changed this much, radians (4°).
pub const HEADING_CHANGE_RAD: f64 = 4.0 * core::f64::consts::PI / 180.0;
/// ... or that was last included this long ago.
pub const OBJECT_REFRESH: Duration = Duration::from_millis(1_000);
/// `T_GenCpmMax / 2`: when any Type-A object (a person, cyclist or animal) has gone
/// this long without inclusion, every Type-A object is included.
pub const VRU_REFRESH: Duration = Duration::from_millis(500);
/// `T_AddSensorInformation`.
pub const SENSOR_INFORMATION_EVERY: Duration = Duration::from_millis(1_000);
/// The most objects one CPM carries.
pub const MAX_OBJECTS: usize = 255;

/// What a perceived object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    /// A vehicle.
    Vehicle,
    /// A pedestrian.
    Pedestrian,
    /// A cyclist.
    Cyclist,
}

impl ObjectKind {
    /// Whether the object is Type-A in TS 103 324 §6.1.2.3 (a VRU whose profile is a
    /// pedestrian or a cyclist; a motorcyclist is Type-B, like a vehicle).
    pub const fn is_vru(self) -> bool {
        matches!(self, ObjectKind::Pedestrian | ObjectKind::Cyclist)
    }
}

/// One object the vehicle's sensors report, in world ENU metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectIn {
    /// The object id, local to this vehicle.
    pub id: u16,
    /// Where.
    pub pos: Vec3,
    /// Velocity, m/s.
    pub vel: Vec3,
    /// Heading, ENU radians.
    pub heading_rad: f64,
    /// Length and width, metres.
    pub length_m: f64,
    /// Width, metres.
    pub width_m: f64,
    /// What it is.
    pub kind: ObjectKind,
    /// Position error σ, metres.
    pub sigma_m: f64,
    /// When it was measured, on the simulation's timeline.
    pub measured_at: SimTime,
    /// When the vehicle first perceived it.
    pub first_seen: SimTime,
    /// The sensors that saw it, a bit per sensor id.
    pub sensors: u8,
}

/// One sensor, as the sensor information container describes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SensorIn {
    /// `sensorId`.
    pub id: u8,
    /// `SensorType`: 1 radar, 3 mono-optical.
    pub sensor_type: u8,
    /// Range, metres.
    pub range_m: f64,
    /// Half the horizontal opening, radians.
    pub half_fov_rad: f64,
}

/// What the next CPM carries.
#[derive(Debug, Clone, PartialEq)]
pub struct CpmContent {
    /// The objects selected for inclusion.
    pub objects: Vec<ObjectIn>,
    /// The sensors, when the sensor information container is due.
    pub sensors: Option<Vec<SensorIn>>,
}

#[derive(Debug, Clone, Copy)]
struct Included {
    at: SimTime,
    pos: Vec3,
    speed: f64,
    heading: f64,
}

/// The orientation of an object's ground velocity, which rule 2 d compares; its body
/// heading when it stands (a standing object's velocity has no direction).
fn ground_course(o: &ObjectIn) -> f64 {
    if v2xw_core::math::hypot(o.vel.x, o.vel.y) > 0.1 {
        v2xw_core::math::atan2(o.vel.y, o.vel.x)
    } else {
        o.heading_rad
    }
}

/// A vehicle's CP basic service.
#[derive(Debug, Clone, Default)]
pub struct CpmService {
    objects: Vec<ObjectIn>,
    sensors: Vec<SensorIn>,
    included: BTreeMap<u16, Included>,
    last_cpm: Option<SimTime>,
    last_check: Option<SimTime>,
    last_sensor_info: Option<SimTime>,
    sent: u64,
}

impl CpmService {
    /// Takes the sensors' latest object list and the suite.
    pub fn set_perception(&mut self, objects: Vec<ObjectIn>, sensors: Vec<SensorIn>) {
        // Objects the sensors no longer report are forgotten, so a re-acquired object is
        // "first perceived" again.
        let live: Vec<u16> = objects.iter().map(|o| o.id).collect();
        self.included.retain(|id, _| live.contains(id));
        self.objects = objects;
        self.sensors = sensors;
    }

    /// How many CPMs this service has generated.
    pub fn sent(&self) -> u64 {
        self.sent
    }

    /// The CPM due at `now`, if any, with the objects the inclusion rules select.
    pub fn due(&mut self, now: SimTime) -> Option<CpmContent> {
        if self
            .last_check
            .is_some_and(|t| now.saturating_sub(t) < T_GEN_CPM.as_nanos())
        {
            return None;
        }
        self.last_check = Some(now);
        // Rule 1 b: one Type-A object overdue brings every Type-A object along.
        let type_a_due = self.objects.iter().any(|o| {
            o.kind.is_vru()
                && self
                    .included
                    .get(&o.id)
                    .is_some_and(|last| now.saturating_sub(last.at) >= VRU_REFRESH.as_nanos())
        });
        let mut chosen: Vec<ObjectIn> = Vec::new();
        for o in &self.objects {
            let speed = v2xw_core::math::hypot(o.vel.x, o.vel.y);
            let include = match self.included.get(&o.id) {
                None => true,
                Some(_) if o.kind.is_vru() => type_a_due,
                Some(last) => {
                    o.pos.distance_2d(last.pos) > POSITION_CHANGE_M
                        || (speed - last.speed).abs() > SPEED_CHANGE_MPS
                        || v2xw_msg::j2945::wrap_pi(ground_course(o) - last.heading).abs()
                            >= HEADING_CHANGE_RAD
                        || now.saturating_sub(last.at) >= OBJECT_REFRESH.as_nanos()
                }
            };
            if include && chosen.len() < MAX_OBJECTS {
                chosen.push(*o);
            }
        }
        let sensor_info_due = self
            .last_sensor_info
            .is_none_or(|t| now.saturating_sub(t) >= SENSOR_INFORMATION_EVERY.as_nanos());
        let max_due = self
            .last_cpm
            .is_none_or(|t| now.saturating_sub(t) >= T_GEN_CPM_MAX.as_nanos());
        if chosen.is_empty() && !max_due {
            return None;
        }
        for o in &chosen {
            self.included.insert(
                o.id,
                Included {
                    at: now,
                    pos: o.pos,
                    speed: v2xw_core::math::hypot(o.vel.x, o.vel.y),
                    heading: ground_course(o),
                },
            );
        }
        let sensors = (sensor_info_due && !self.sensors.is_empty()).then(|| {
            self.last_sensor_info = Some(now);
            self.sensors.clone()
        });
        self.last_cpm = Some(now);
        self.sent += 1;
        Some(CpmContent {
            objects: chosen,
            sensors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(id: u16, x: f64, v: f64, kind: ObjectKind) -> ObjectIn {
        ObjectIn {
            id,
            pos: Vec3::new(x, 0.0, 0.0),
            vel: Vec3::new(v, 0.0, 0.0),
            heading_rad: 0.0,
            length_m: 4.5,
            width_m: 1.8,
            kind,
            sigma_m: 0.3,
            measured_at: 0,
            first_seen: 0,
            sensors: 1,
        }
    }

    const MS: u64 = 1_000_000;

    /// A new object goes at once; a steady one again only after 1 s or 4 m; a pedestrian
    /// every 500 ms; a CPM at least once a second even with nothing new.
    #[test]
    fn objects_are_included_by_the_standards_rules() {
        let mut s = CpmService::default();
        s.set_perception(vec![object(1, 20.0, 0.0, ObjectKind::Vehicle)], vec![]);
        assert_eq!(s.due(0).expect("first").objects.len(), 1);
        // 100 ms later, unchanged: nothing, and no CPM (the last was 100 ms ago).
        assert!(s.due(100 * MS).is_none());
        // Moved 5 m: included.
        s.set_perception(vec![object(1, 25.0, 0.0, ObjectKind::Vehicle)], vec![]);
        assert_eq!(s.due(200 * MS).expect("moved").objects.len(), 1);
        // Steady for 1 s: a CPM goes (T_GenCpmMax) and carries the object (1 s refresh).
        for k in 3..12 {
            let got = s.due(k * 100 * MS);
            assert!(got.is_none(), "an early CPM at {k}00 ms: {got:?}");
        }
        let late = s.due(1_200 * MS).expect("refresh");
        assert_eq!(late.objects.len(), 1);
        // A pedestrian is re-included every 500 ms.
        let mut p = CpmService::default();
        p.set_perception(vec![object(7, 10.0, 1.4, ObjectKind::Pedestrian)], vec![]);
        assert!(p.due(0).is_some());
        assert!(p.due(300 * MS).is_none());
        assert_eq!(p.due(500 * MS).expect("VRU refresh").objects.len(), 1);
    }

    /// TS 103 324 §6.1.2.3 rule 1 b: when one pedestrian is overdue, every pedestrian goes
    /// with it, even one included 200 ms ago; vehicles keep their own rules.
    #[test]
    fn an_overdue_pedestrian_brings_every_pedestrian() {
        let mut s = CpmService::default();
        s.set_perception(vec![object(1, 10.0, 1.4, ObjectKind::Pedestrian)], vec![]);
        assert_eq!(s.due(0).expect("first").objects.len(), 1);
        // A second pedestrian appears at 300 ms: it alone is new.
        let both = vec![
            object(1, 10.0, 1.4, ObjectKind::Pedestrian),
            object(2, 12.0, 1.4, ObjectKind::Pedestrian),
            object(3, 30.0, 10.0, ObjectKind::Vehicle),
        ];
        s.set_perception(both.clone(), vec![]);
        let at300 = s.due(300 * MS).expect("new objects");
        let ids: Vec<u16> = at300.objects.iter().map(|o| o.id).collect();
        assert_eq!(ids, vec![2, 3]);
        // At 500 ms pedestrian 1 is overdue, so both pedestrians go; the vehicle, steady
        // since 300 ms, does not.
        s.set_perception(both, vec![]);
        let at500 = s.due(500 * MS).expect("Type-A refresh");
        let ids: Vec<u16> = at500.objects.iter().map(|o| o.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    /// Rule 2 d compares the ground velocity's orientation, not the body's heading: a car
    /// whose velocity turns 5° is included though its reported heading did not change.
    #[test]
    fn a_turning_velocity_includes_a_vehicle() {
        let mut s = CpmService::default();
        s.set_perception(vec![object(1, 20.0, 10.0, ObjectKind::Vehicle)], vec![]);
        assert!(s.due(0).is_some());
        let mut turned = object(1, 21.0, 10.0, ObjectKind::Vehicle);
        let (sn, cs) = v2xw_core::math::sin_cos(5.0_f64.to_radians());
        turned.vel = Vec3::new(10.0 * cs, 10.0 * sn, 0.0);
        s.set_perception(vec![turned], vec![]);
        assert_eq!(s.due(100 * MS).expect("turned").objects.len(), 1);
    }
}
