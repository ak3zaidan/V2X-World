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
//! # Under load: rate, quality and the cut (WP-12, each behind its own knob)
//!
//! Re-read on 2026-10-27 from the same TS (clauses 6.1.2.1, 6.1.2.3, 6.1.3.2, 7.1.8.6,
//! Annex D and Tables F.1, F.2, F.4). Artery's `CpService` was read for behaviour only
//! and no code was copied.
//!
//! | Rule | Value | Source | Knob |
//! |---|---|---|---|
//! | `T_GenCpm` follows the congestion control: the generation interval is the DCC's minimum gap between transmissions (`T_off`, as the CAM's `T_GenCam_Dcc`), clamped to `[T_GenCpmMin, T_GenCpmMax]` | 100 ms to 1,000 ms | §6.1.2.1 bounds and Annex D (under a resource limit the frequency is reduced first, the size only at `T_GenCpmMax`); EN 302 637-2 §6.1.3 for the CAM's clamp it mirrors | `messages.cpm_dcc_limited` |
//! | An object is selected only when its `objectPerceptionQuality` **exceeds** `ObjectPerceptionQualityThreshold` | 3 | §6.1.2.3; Table F.1 | `messages.cpm_quality_threshold` |
//! | `objectPerceptionQuality = ⌊(w_d·r_d + w_c·r_c + w_oa·r_oa)/(w_d + w_c + w_oa)⌋`, `r = ⌊EMA·15⌋` of the detection confidence `c_t` and the detection success `d_t` (EMA factor α), `r_oa = min(⌊oa/100 ms⌋, 15)` | α 0.5, all weights 1 | §7.1.8.6; Table F.4 | (with the threshold) |
//! | Objects go into CPMs in descending order of the utility `p_conf + p_pos + p_speed + p_orient + p_time`: `p_conf` is quality/15, each other term the change since last inclusion ramped linearly from 0 at its min to 1 at its max threshold | position 0–8 m, speed 0–1 m/s, orientation 0–8°, last inclusion 100–1,000 ms | §6.1.3.2; Table F.2 | `messages.cpm_max_objects` |
//! | When the selection is larger than the per-CPM budget, the lowest-utility objects are cut and stay due for the next event | 255 (the ASN.1 bound) unless set | §6.1.2.3 (lowest VoI omitted under a resource limit), Annex D | `messages.cpm_max_objects` |
//!
//! Choices this build makes where the TS is silent, stated: a never-included object has
//! every change term at its maximum (it has no last inclusion to compare with); the
//! detection confidence `c_t` is the detecting sensors' own detection probability at
//! that range (the perception model's), and `c_t = 0`, `d_t = 0` on a scan in which a
//! confirmed track was not re-measured. With every knob unset the service behaves, and
//! encodes, exactly as before.
//!
//! Not modelled: perception regions, the look-ahead that pulls next event's Type-B
//! objects into this one (a "may"), segmentation of one event into several CPMs by
//! `MTU_CPM` (the cut above stands in for a size budget), and the multi-channel operation
//! of Annex D. Every value is a named constant, so a study can change it.

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
/// `T_GenCpmMin` for a vehicle ITS-S (Table F.1).
pub const T_GEN_CPM_MIN: Duration = Duration::from_millis(100);
/// `ObjectPerceptionQualityThreshold` (Table F.1).
pub const OBJECT_PERCEPTION_QUALITY_THRESHOLD: u8 = 3;
/// The moving-average factor α of §7.1.8.6 (Table F.4).
pub const QUALITY_EMA_ALPHA: f64 = 0.5;
/// The weights `w_d`, `w_c`, `w_oa` of §7.1.8.6 (Table F.4).
pub const QUALITY_WEIGHTS: (f64, f64, f64) = (1.0, 1.0, 1.0);
/// `min/maxPositionChangePriorityThreshold`, metres (Table F.2).
pub const POSITION_PRIORITY_M: (f64, f64) = (0.0, 8.0);
/// `min/maxGroundSpeedChangePriorityThreshold`, m/s (Table F.2).
pub const SPEED_PRIORITY_MPS: (f64, f64) = (0.0, 1.0);
/// `min/maxGroundVelocityOrientationChangePriorityThreshold`, radians (0° and 8°,
/// Table F.2).
pub const ORIENTATION_PRIORITY_RAD: (f64, f64) = (0.0, 8.0 * core::f64::consts::PI / 180.0);
/// `min/maxLastInclusionTimePriorityThreshold`, seconds (100 ms and 1,000 ms, Table F.2).
pub const LAST_INCLUSION_PRIORITY_S: (f64, f64) = (0.1, 1.0);

/// The CP service's load behaviour (WP-12). [`CpmConfig::default`] is the service as it
/// was before: no DCC limit, no quality threshold, no utility order, the 255 cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpmConfig {
    /// `T_GenCpm` follows the DCC's `T_off`, clamped to `[T_GenCpmMin, T_GenCpmMax]`
    /// (`messages.cpm_dcc_limited`).
    pub dcc_limited: bool,
    /// Select only objects whose §7.1.8.6 quality exceeds this, and encode that quality
    /// (`messages.cpm_quality_threshold`; the TS recommends 3).
    pub quality_threshold: Option<u8>,
    /// The most objects one CPM carries; the lowest-utility objects beyond it are cut.
    pub max_objects: usize,
    /// List the selected objects in descending utility (§6.1.3.2). On whenever the
    /// scenario sets `messages.cpm_max_objects`.
    pub utility_order: bool,
}

impl Default for CpmConfig {
    fn default() -> Self {
        CpmConfig {
            dcc_limited: false,
            quality_threshold: None,
            max_objects: MAX_OBJECTS,
            utility_order: false,
        }
    }
}

/// The §7.1.8.6 state of one object: the moving averages of its detection confidence and
/// detection success, and the measurement instant last seen.
#[derive(Debug, Clone, Copy)]
struct QualityTrack {
    ema_c: f64,
    ema_d: f64,
    last_measured: SimTime,
}

/// A linear ramp from 0 at `lo` to 1 at `hi` (§6.1.3.2's piecewise terms).
fn ramp(x: f64, (lo, hi): (f64, f64)) -> f64 {
    if x <= lo {
        0.0
    } else if x >= hi {
        1.0
    } else {
        (x - lo) / (hi - lo)
    }
}

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
    /// The detection confidence `c_t` of its latest measurement, `0..=1` (TS 103 324
    /// §7.1.8.6): the detecting sensors' own detection probability at that range.
    pub detection_confidence: f64,
}

/// One sensor, as the sensor information container describes it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SensorIn {
    /// `sensorId`.
    pub id: u8,
    /// `SensorType`: 1 radar, 2 lidar, 3 mono-optical.
    pub sensor_type: u8,
    /// Range, metres.
    pub range_m: f64,
    /// Half the horizontal opening, radians.
    pub half_fov_rad: f64,
    /// The opening's centre, radians counter-clockwise from the frame's x axis (the
    /// vehicle's heading; east at a roadside unit).
    pub azimuth_rad: f64,
    /// Where the sensor sits, metres from the station's reference position in the same
    /// frame, when the configured suite states it (`perception` in the scenario).
    pub mount_m: Option<[f64; 3]>,
}

/// What the next CPM carries.
#[derive(Debug, Clone, PartialEq)]
pub struct CpmContent {
    /// The objects selected for inclusion.
    pub objects: Vec<ObjectIn>,
    /// Each object's §7.1.8.6 `objectPerceptionQuality`, in the order of `objects`, when
    /// the quality threshold is in force (`None`: the encoder's legacy σ mapping).
    pub quality: Option<Vec<u8>>,
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
    /// `T_GenCpm` when not [`T_GEN_CPM`] (network audit: `messages.cpm_interval_ms`).
    t_gen: Option<Duration>,
    /// The load behaviour (WP-12).
    config: CpmConfig,
    /// The §7.1.8.6 moving averages, per live object.
    quality: BTreeMap<u16, QualityTrack>,
}

impl CpmService {
    /// The same service with another `T_GenCpm` (TS 103 324 §6.1.2.3: 100 ms to
    /// `T_GenCpmMax`, 1 000 ms).
    #[must_use]
    pub fn with_generation_interval(mut self, t_gen: Duration) -> Self {
        self.t_gen = Some(t_gen);
        self
    }

    /// The same service with the WP-12 load behaviour `config`.
    #[must_use]
    pub fn with_config(mut self, config: CpmConfig) -> Self {
        self.config = config;
        self
    }

    /// Takes the sensors' latest object list and the suite.
    pub fn set_perception(&mut self, objects: Vec<ObjectIn>, sensors: Vec<SensorIn>) {
        // Objects the sensors no longer report are forgotten, so a re-acquired object is
        // "first perceived" again.
        let live: Vec<u16> = objects.iter().map(|o| o.id).collect();
        self.included.retain(|id, _| live.contains(id));
        self.quality.retain(|id, _| live.contains(id));
        // §7.1.8.6 steps 1-3: one discrete instant per perception update. A confirmed
        // track the sensors did not re-measure this time scores `d_t = 0`, `c_t = 0`.
        let alpha = QUALITY_EMA_ALPHA;
        for o in &objects {
            match self.quality.get_mut(&o.id) {
                None => {
                    self.quality.insert(
                        o.id,
                        QualityTrack {
                            ema_c: o.detection_confidence.clamp(0.0, 1.0),
                            ema_d: 1.0,
                            last_measured: o.measured_at,
                        },
                    );
                }
                Some(q) => {
                    let detected = o.measured_at > q.last_measured;
                    let (c, d) = if detected {
                        (o.detection_confidence.clamp(0.0, 1.0), 1.0)
                    } else {
                        (0.0, 0.0)
                    };
                    q.ema_c = alpha * c + (1.0 - alpha) * q.ema_c;
                    q.ema_d = alpha * d + (1.0 - alpha) * q.ema_d;
                    q.last_measured = q.last_measured.max(o.measured_at);
                }
            }
        }
        self.objects = objects;
        self.sensors = sensors;
    }

    /// The objects the sensors report now (the vehicle's own perception, which the DENM
    /// triggers also read: [`crate::events`]).
    pub fn objects(&self) -> &[ObjectIn] {
        &self.objects
    }

    /// How many CPMs this service has generated.
    pub fn sent(&self) -> u64 {
        self.sent
    }

    /// The object's `objectPerceptionQuality` at `now` (TS 103 324 §7.1.8.6 steps 4-5,
    /// Table F.4's weights); 0 for an object the service has no history of.
    pub fn perception_quality(&self, o: &ObjectIn, now: SimTime) -> u8 {
        let Some(q) = self.quality.get(&o.id) else {
            return 0;
        };
        let r_c = (q.ema_c * 15.0).floor().clamp(0.0, 15.0);
        let r_d = (q.ema_d * 15.0).floor().clamp(0.0, 15.0);
        let age_ms = now.saturating_sub(o.first_seen) / 1_000_000;
        let r_oa = (age_ms / 100).min(15) as f64;
        let (w_d, w_c, w_oa) = QUALITY_WEIGHTS;
        ((w_d * r_d + w_c * r_c + w_oa * r_oa) / (w_d + w_c + w_oa))
            .floor()
            .clamp(0.0, 15.0) as u8
    }

    /// The object utility of §6.1.3.2: `p_conf + p_pos + p_speed + p_orient + p_time`,
    /// each change term ramped over Table F.2's thresholds. A never-included object has
    /// every change term at 1.
    pub fn utility(&self, o: &ObjectIn, now: SimTime) -> f64 {
        let p_conf = f64::from(self.perception_quality(o, now)) / 15.0;
        let Some(last) = self.included.get(&o.id) else {
            return p_conf + 4.0;
        };
        let speed = v2xw_core::math::hypot(o.vel.x, o.vel.y);
        let d_orient = v2xw_msg::j2945::wrap_pi(ground_course(o) - last.heading).abs();
        let since_s = now.saturating_sub(last.at) as f64 * 1e-9;
        p_conf
            + ramp(o.pos.distance_2d(last.pos), POSITION_PRIORITY_M)
            + ramp((speed - last.speed).abs(), SPEED_PRIORITY_MPS)
            + ramp(d_orient, ORIENTATION_PRIORITY_RAD)
            + ramp(since_s, LAST_INCLUSION_PRIORITY_S)
    }

    /// The generation interval in force under the DCC's `t_off`: `T_GenCpm`, or with
    /// [`CpmConfig::dcc_limited`] the larger of it and `t_off`, clamped to
    /// `[T_GenCpmMin, T_GenCpmMax]`.
    pub fn generation_interval(&self, t_off: Duration) -> Duration {
        let base = self.t_gen.unwrap_or(T_GEN_CPM);
        if self.config.dcc_limited {
            base.max(t_off).max(T_GEN_CPM_MIN).min(T_GEN_CPM_MAX)
        } else {
            base
        }
    }

    /// The CPM due at `now`, if any, with the objects the inclusion rules select; the
    /// channel unrestricted.
    pub fn due(&mut self, now: SimTime) -> Option<CpmContent> {
        self.due_under_dcc(now, Duration::ZERO)
    }

    /// [`CpmService::due`] with the DCC's current minimum gap between transmissions
    /// (`DccState::t_off`), which the generation interval follows under
    /// [`CpmConfig::dcc_limited`].
    pub fn due_under_dcc(&mut self, now: SimTime, t_off: Duration) -> Option<CpmContent> {
        let t_gen = self.generation_interval(t_off);
        if self
            .last_check
            .is_some_and(|t| now.saturating_sub(t) < t_gen.as_nanos())
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
        let threshold = self.config.quality_threshold;
        let cut = self.config.max_objects.min(MAX_OBJECTS);
        let ordered = self.config.utility_order;
        let mut chosen: Vec<ObjectIn> = Vec::new();
        for o in &self.objects {
            // §6.1.2.3: only an object whose quality exceeds the threshold is selected.
            if threshold.is_some_and(|th| self.perception_quality(o, now) <= th) {
                continue;
            }
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
            // Without the utility order, the first 255 in perception order, as before.
            if include && (ordered || chosen.len() < cut) {
                chosen.push(*o);
            }
        }
        if ordered || chosen.len() > cut {
            // §6.1.3.2: descending utility, ties by object id (a total order, so the
            // result does not depend on the perception list's order); the cut keeps the
            // highest-utility objects and the rest stay due for the next event.
            let mut ranked: Vec<(f64, ObjectIn)> =
                chosen.iter().map(|o| (self.utility(o, now), *o)).collect();
            ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.id.cmp(&b.1.id)));
            ranked.truncate(cut);
            chosen = ranked.into_iter().map(|(_, o)| o).collect();
        }
        let quality = threshold
            .is_some()
            .then(|| chosen.iter().map(|o| self.perception_quality(o, now)).collect());
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
            quality,
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
            detection_confidence: 0.96,
        }
    }

    const MS: u64 = 1_000_000;

    /// WP-12: with `cpm_dcc_limited`, a DCC gap of 300 ms makes the generation interval
    /// 300 ms; below `T_GenCpmMin` it stays 100 ms, above `T_GenCpmMax` it stops at 1 s;
    /// without the knob the DCC changes nothing (the default path).
    #[test]
    fn the_generation_interval_follows_the_dcc_within_the_standards_bounds() {
        let count = |config: CpmConfig, t_off_ms: u64| {
            let mut s = CpmService::default().with_config(config);
            let mut n = 0;
            // A car that moves 5 m every 100 ms is due at every generation event.
            for k in 0..30u64 {
                s.set_perception(
                    vec![object(1, 5.0 * k as f64, 50.0, ObjectKind::Vehicle)],
                    vec![],
                );
                n += usize::from(
                    s.due_under_dcc(k * 100 * MS, Duration::from_millis(t_off_ms))
                        .is_some(),
                );
            }
            n
        };
        let on = CpmConfig {
            dcc_limited: true,
            ..CpmConfig::default()
        };
        // 3 s at 100 ms checks: 30 events unrestricted, 10 at 300 ms, 3 at the 1 s cap.
        assert_eq!(count(on, 0), 30);
        assert_eq!(count(on, 50), 30, "T_GenCpmMin is a floor");
        assert_eq!(count(on, 300), 10, "the CPM rate follows a 300 ms DCC interval");
        assert_eq!(count(on, 5_000), 3, "T_GenCpmMax caps it");
        assert_eq!(count(CpmConfig::default(), 300), 30, "off: the DCC is ignored");
        assert_eq!(
            CpmService::default()
                .with_config(on)
                .generation_interval(Duration::from_millis(300)),
            Duration::from_millis(300)
        );
    }

    /// TS 103 324 §7.1.8.6 by hand, with Table F.4's α = 0.5 and unit weights.
    #[test]
    fn the_perception_quality_is_the_standards_weighted_rating() {
        let mut s = CpmService::default();
        let mut o = object(1, 20.0, 0.0, ObjectKind::Vehicle);
        o.detection_confidence = 0.96;
        s.set_perception(vec![o], vec![]);
        // t = 0: EMA_c 0.96 -> r_c 14, r_d 15, r_oa 0: floor(29 / 3) = 9.
        assert_eq!(s.perception_quality(&o, 0), 9);
        // Two scans without a new measurement: EMA_c 0.24 -> 3, EMA_d 0.25 -> 3; at
        // 200 ms r_oa = 2: floor(8 / 3) = 2.
        s.set_perception(vec![o], vec![]);
        s.set_perception(vec![o], vec![]);
        assert_eq!(s.perception_quality(&o, 200 * MS), 2);
        // Re-measured at 300 ms with c 0.96: EMA_c 0.6 -> 9, EMA_d 0.625 -> 9, r_oa 3:
        // floor(21 / 3) = 7.
        o.measured_at = 300 * MS;
        s.set_perception(vec![o], vec![]);
        assert_eq!(s.perception_quality(&o, 300 * MS), 7);
        // The age rating saturates at 15 (1.5 s).
        assert_eq!(s.perception_quality(&o, 10_000 * MS), floor3(9 + 9 + 15));
    }

    fn floor3(sum: u32) -> u8 {
        (sum / 3) as u8
    }

    /// §6.1.2.3: with the threshold (3), a confirmed track that has gone unmeasured
    /// decays below it and is not selected, while a fresh detection beside it is.
    #[test]
    fn a_track_below_the_quality_threshold_is_not_sent() {
        let config = CpmConfig {
            quality_threshold: Some(OBJECT_PERCEPTION_QUALITY_THRESHOLD),
            ..CpmConfig::default()
        };
        let mut s = CpmService::default().with_config(config);
        let stale = object(1, 20.0, 0.0, ObjectKind::Vehicle);
        s.set_perception(vec![stale], vec![]);
        let first = s.due(0).expect("first");
        assert_eq!(first.objects.len(), 1);
        assert_eq!(first.quality, Some(vec![9]));
        s.set_perception(vec![stale], vec![]);
        // At 200 ms the stale track rates 2; a new car (quality 9) is due.
        let mut fresh = object(2, 40.0, 0.0, ObjectKind::Vehicle);
        fresh.measured_at = 200 * MS;
        fresh.first_seen = 200 * MS;
        // The stale one has also moved 5 m in its last report, so the inclusion rules
        // alone would select it.
        let mut moved = stale;
        moved.pos.x += 5.0;
        s.set_perception(vec![moved, fresh], vec![]);
        let got = s.due(200 * MS).expect("the fresh car");
        let ids: Vec<u16> = got.objects.iter().map(|o| o.id).collect();
        assert_eq!(ids, vec![2], "the stale track must be held back");
        assert_eq!(got.quality, Some(vec![9]));
        // Without the threshold both go.
        let mut plain = CpmService::default();
        plain.set_perception(vec![stale], vec![]);
        plain.due(0);
        plain.set_perception(vec![stale], vec![]);
        plain.set_perception(vec![moved, fresh], vec![]);
        assert_eq!(plain.due(200 * MS).expect("both").objects.len(), 2);
    }

    /// §6.1.3.2: when the selection exceeds the budget, the highest-utility objects are
    /// kept, in descending utility; the cut ones stay due and go at the next event.
    #[test]
    fn the_cut_keeps_the_highest_utility_objects() {
        let config = CpmConfig {
            max_objects: 2,
            utility_order: true,
            ..CpmConfig::default()
        };
        let mut s = CpmService::default().with_config(config);
        // Four new cars; their confidences give qualities 6, 9, 7 and 8.
        let objects: Vec<ObjectIn> = [0.2, 0.95, 0.5, 0.6]
            .iter()
            .enumerate()
            .map(|(k, c)| {
                let mut o = object(k as u16 + 1, 20.0 + 10.0 * k as f64, 0.0, ObjectKind::Vehicle);
                o.detection_confidence = *c;
                o
            })
            .collect();
        s.set_perception(objects.clone(), vec![]);
        let q: Vec<u8> = objects.iter().map(|o| s.perception_quality(o, 0)).collect();
        assert_eq!(q, vec![6, 9, 7, 8]);
        let ids: Vec<u16> = s.due(0).expect("first").objects.iter().map(|o| o.id).collect();
        assert_eq!(ids, vec![2, 4], "the two highest-utility objects, best first");
        // The two cut cars are still new at the next event and go then.
        let next: Vec<u16> = s
            .due(100 * MS)
            .expect("the cut ones")
            .objects
            .iter()
            .map(|o| o.id)
            .collect();
        assert_eq!(next, vec![3, 1]);
        // A position change ranks above a smaller one (p_pos, 0-8 m).
        let mut t = CpmService::default().with_config(config);
        let a = object(1, 0.0, 0.0, ObjectKind::Vehicle);
        let b = object(2, 50.0, 0.0, ObjectKind::Vehicle);
        let c = object(3, 100.0, 0.0, ObjectKind::Vehicle);
        t.set_perception(vec![a, b, c], vec![]);
        t.due(0);
        t.due(100 * MS); // the third
        let mut a2 = a;
        a2.pos.x += 4.5;
        let mut b2 = b;
        b2.pos.x += 7.5;
        let mut c2 = c;
        c2.pos.x += 6.0;
        t.set_perception(vec![a2, b2, c2], vec![]);
        let ids: Vec<u16> = t
            .due(200 * MS)
            .expect("moved")
            .objects
            .iter()
            .map(|o| o.id)
            .collect();
        assert_eq!(ids, vec![2, 3]);
    }

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
