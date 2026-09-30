//! Building, encoding and decoding a real ETSI Collective Perception Message
//! (TS 103 324 V2.1.1), over the Release 2 common data dictionary.
//!
//! The encoder is generated from the committed forge modules (`third_party/asn1/etsi/
//! cpm_ts103324`), like the CAM's and the DENM's; this module turns the simulator's own
//! quantities — a station's position belief and the objects its sensors report — into the
//! generated types.
//!
//! # What goes in
//!
//! * **Management container**: `referenceTime` (the generation instant) and
//!   `referencePosition` (the station's belief).
//! * **Originating vehicle container**: the station's heading as `orientationAngle`.
//! * **Sensor information container**, when the service says it is due: each sensor's id,
//!   type and a radial perception-region shape (its range and horizontal opening).
//! * **Perceived object container**: each object's id, `measurementDeltaTime`, position
//!   and velocity **relative to the reference position** in a Cartesian frame whose x axis
//!   points east and y axis north (TS 103 324 V2.1.1's coordinate system for a vehicle
//!   originating station, recalled; the document is not in this repository), its
//!   dimensions, age and classification, and the sensors that saw it.
//!
//! Every value is converted on the CDD's own units: centimetres for coordinates, 0.01 m/s
//! for velocity components, decimetres for dimensions, milliseconds for time.

use rasn::types::{Any, SequenceOf};
use v2xw_core::belief::PositionEstimate;
use v2xw_core::geo::GeoOrigin;
use v2xw_core::geom::Vec3;

use crate::asn1::cdd::{
    AngleConfidence, CardinalNumber1B, CartesianAngle, CartesianAngleValue,
    CartesianCoordinateLarge, CartesianCoordinateWithConfidence,
    CartesianPosition3dWithConfidence, ConfidenceLevel, CoordinateConfidence,
    DeltaTimeMilliSecondSigned, Identifier1B, Identifier2B, ItsPduHeader, MessageId,
    ObjectClass, ObjectClassDescription, ObjectClassWithConfidence, ObjectDimension,
    ObjectDimensionConfidence, ObjectDimensionValue, ObjectPerceptionQuality, OrdinalNumber1B,
    PerceivedObject, RadialShape, SensorType, SequenceOfIdentifier1B, Shape, SpeedConfidence,
    StandardLength12b, StationId, TimestampIts, TrafficParticipantType, Velocity3dWithConfidence,
    VelocityCartesian, VelocityComponent, VelocityComponentValue, VruProfileAndSubprofile,
    VruSubProfileBicyclist, VruSubProfilePedestrian, Wgs84Angle, Wgs84AngleConfidence,
    Wgs84AngleValue,
};
use crate::asn1::cpm_asn1::{
    CollectivePerceptionMessage, ConstraintWrappedCpmContainers, CpmContainerId, CpmPayload,
    CpmManagementContainer as ManagementContainer, WrappedCpmContainer, WrappedCpmContainers,
};
use crate::asn1::cpm_objects::{PerceivedObjectContainer, PerceivedObjects};
use crate::asn1::cpm_sensors::{SensorInformation, SensorInformationContainer};
use crate::asn1::cpm_stations::OriginatingVehicleContainer;
use crate::codec::{Encoded, MsgType, uper_decode, uper_encode};
use crate::error::CodecError;
use crate::units;

/// `MessageId` of a CPM: `cpm(14)` in the CDD.
pub const CPM_MESSAGE_ID: u8 = 14;
/// The CPM's ITS PDU protocol version.
pub const CPM_PROTOCOL_VERSION: u8 = 2;

/// `originatingVehicleContainer`.
pub const CONTAINER_ORIGINATING_VEHICLE: u8 = 1;
/// `sensorInformationContainer`.
pub const CONTAINER_SENSOR_INFORMATION: u8 = 3;
/// `perceivedObjectContainer`.
pub const CONTAINER_PERCEIVED_OBJECTS: u8 = 5;

/// What an object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpmObjectClass {
    /// A passenger car (`TrafficParticipantType` 5).
    Vehicle,
    /// A pedestrian.
    Pedestrian,
    /// A cyclist.
    Cyclist,
}

/// One perceived object, in the simulator's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpmObject {
    /// The station-local object id.
    pub id: u16,
    /// Measurement time minus reference time, milliseconds.
    pub measurement_delta_ms: i32,
    /// Where it is, world ENU metres.
    pub pos: Vec3,
    /// Its velocity, m/s.
    pub vel: Vec3,
    /// Length and width, metres.
    pub length_m: f64,
    /// Width, metres.
    pub width_m: f64,
    /// How long it has been tracked, milliseconds.
    pub age_ms: u32,
    /// Its class.
    pub class: CpmObjectClass,
    /// Position error σ, metres.
    pub sigma_m: f64,
    /// Which sensors saw it, as sensor ids.
    pub sensor_ids: [u8; 3],
    /// How many of `sensor_ids` are set.
    pub sensor_count: u8,
}

/// One sensor, in the simulator's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpmSensor {
    /// `sensorId`.
    pub id: u8,
    /// `SensorType`: 1 radar, 3 mono-optical.
    pub sensor_type: u8,
    /// Range, metres.
    pub range_m: f64,
    /// Half the horizontal opening, radians.
    pub half_fov_rad: f64,
}

/// Everything [`build_cpm`] needs.
#[derive(Debug, Clone, PartialEq)]
pub struct CpmInput {
    /// The sending station, its pseudonym's id.
    pub station_id: u32,
    /// The station's belief.
    pub position: PositionEstimate,
    /// The world's geodetic origin.
    pub origin: GeoOrigin,
    /// `referenceTime`.
    pub reference_time: TimestampIts,
    /// The objects.
    pub objects: Vec<CpmObject>,
    /// The sensors, when the sensor information container goes.
    pub sensors: Option<Vec<CpmSensor>>,
}

fn cm(m: f64) -> i32 {
    // CartesianCoordinateLarge ::= INTEGER (-131072..131071), centimetres.
    (m * 100.0).round().clamp(-131_072.0, 131_071.0) as i32
}

fn coordinate_confidence(sigma_m: f64) -> u16 {
    // CoordinateConfidence ::= INTEGER (1..4096), centimetres at 95 %: 4095 out of range.
    let c = (sigma_m * 1.96 * 100.0).round();
    if c > 4_094.0 { 4_095 } else { c.max(1.0) as u16 }
}

fn velocity_component(v: f64) -> VelocityComponent {
    // VelocityComponentValue ::= INTEGER (-16383..16383), 0.01 m/s; SpeedConfidence 1..127.
    VelocityComponent::new(
        VelocityComponentValue((v * 100.0).round().clamp(-16_382.0, 16_382.0) as i16),
        SpeedConfidence(127),
    )
}

fn dimension(m: f64) -> ObjectDimension {
    // ObjectDimensionValue ::= INTEGER (1..256), decimetres; confidence 1..32 (unavailable
    // is 32).
    ObjectDimension::new(
        ObjectDimensionValue((m * 10.0).round().clamp(1.0, 255.0) as u16),
        ObjectDimensionConfidence(32),
    )
}

fn class_of(c: CpmObjectClass) -> ObjectClass {
    match c {
        CpmObjectClass::Vehicle => ObjectClass::vehicleSubClass(TrafficParticipantType(5)),
        CpmObjectClass::Pedestrian => ObjectClass::vruSubClass(
            VruProfileAndSubprofile::pedestrian(VruSubProfilePedestrian(0)),
        ),
        CpmObjectClass::Cyclist => ObjectClass::vruSubClass(
            VruProfileAndSubprofile::bicyclistAndLightVruVehicle(VruSubProfileBicyclist(0)),
        ),
    }
}

fn encode_part<T: rasn::Encode>(value: &T) -> Result<Vec<u8>, CodecError> {
    uper_encode(MsgType::Cpm, value)
}

/// The station's belief as a CDD `ReferencePosition`, the DENM's construction.
fn reference_position(
    p: &PositionEstimate,
    origin: GeoOrigin,
) -> Result<crate::asn1::cdd::ReferencePosition, CodecError> {
    use crate::asn1::cdd::{
        Altitude, AltitudeConfidence, AltitudeValue, HeadingValue, Latitude, Longitude,
        PosConfidenceEllipse, ReferencePosition, SemiAxisLength,
    };
    let (lat_deg, lon_deg, alt_m) = origin.to_geodetic(p.pos);
    let latitude = units::latitude(lat_deg).ok_or(CodecError::OutOfRange {
        field: "cpm.managementContainer.referencePosition.latitude",
        asn1_type: "Latitude",
        value: lat_deg as i64,
        min: -90,
        max: 90,
    })?;
    let longitude = units::longitude(lon_deg).ok_or(CodecError::OutOfRange {
        field: "cpm.managementContainer.referencePosition.longitude",
        asn1_type: "Longitude",
        value: lon_deg as i64,
        min: -180,
        max: 180,
    })?;
    Ok(ReferencePosition::new(
        Latitude(latitude),
        Longitude(longitude),
        PosConfidenceEllipse::new(
            SemiAxisLength(units::semi_axis_length(p.semi_major_m)),
            SemiAxisLength(units::semi_axis_length(p.semi_minor_m)),
            HeadingValue(units::wgs84_angle(p.orientation_rad)),
        ),
        Altitude::new(
            AltitudeValue(units::altitude(alt_m)),
            AltitudeConfidence::unavailable,
        ),
    ))
}

/// Fills a CPM.
///
/// # Errors
/// [`CodecError::OutOfRange`] when the belief cannot be put on the globe, or an encoding
/// refusal for a container.
pub fn build_cpm(input: &CpmInput) -> Result<CollectivePerceptionMessage, CodecError> {
    let header = ItsPduHeader::new(
        OrdinalNumber1B(CPM_PROTOCOL_VERSION),
        MessageId(CPM_MESSAGE_ID),
        StationId(input.station_id),
    );
    let reference_position = reference_position(&input.position, input.origin)?;
    let management = ManagementContainer::new(
        input.reference_time.clone(),
        reference_position,
        None,
        None,
    );
    let mut containers: Vec<WrappedCpmContainer> = Vec::new();
    let vehicle = OriginatingVehicleContainer::new(
        Wgs84Angle::new(
            Wgs84AngleValue(units::wgs84_angle(input.position.heading_rad)),
            Wgs84AngleConfidence(127),
        ),
        None,
        None,
        None,
    );
    containers.push(WrappedCpmContainer::new(
        CpmContainerId(CONTAINER_ORIGINATING_VEHICLE),
        Any::new(encode_part(&vehicle)?),
    ));
    if let Some(sensors) = &input.sensors
        && !sensors.is_empty()
    {
        let list: Vec<SensorInformation> = sensors
            .iter()
            .map(|s| {
                let half = s.half_fov_rad.to_degrees();
                // CartesianAngleValue ::= INTEGER (0..3601), 0.1°, counter-clockwise from
                // the x axis; the opening runs from `360 − half` round to `half`.
                let start = ((360.0 - half) * 10.0).round().clamp(0.0, 3_599.0) as u16;
                let end = (half * 10.0).round().clamp(0.0, 3_599.0) as u16;
                SensorInformation::new(
                    Identifier1B(s.id),
                    SensorType(s.sensor_type),
                    Some(Shape::radial(RadialShape::new(
                        None,
                        StandardLength12b((s.range_m * 10.0).round().clamp(0.0, 4_095.0) as u16),
                        CartesianAngleValue(start),
                        CartesianAngleValue(end),
                        None,
                        None,
                    ))),
                    Some(ConfidenceLevel(95)),
                    true,
                )
            })
            .collect();
        containers.push(WrappedCpmContainer::new(
            CpmContainerId(CONTAINER_SENSOR_INFORMATION),
            Any::new(encode_part(&SensorInformationContainer(SequenceOf::from(list)))?),
        ));
    }
    if !input.objects.is_empty() {
        let objects: Vec<PerceivedObject> = input
            .objects
            .iter()
            .take(255)
            .map(|o| perceived_object(o, input.position.pos))
            .collect();
        let container = PerceivedObjectContainer::new(
            CardinalNumber1B(objects.len() as u8),
            PerceivedObjects(SequenceOf::from(objects)),
        );
        containers.push(WrappedCpmContainer::new(
            CpmContainerId(CONTAINER_PERCEIVED_OBJECTS),
            Any::new(encode_part(&container)?),
        ));
    }
    Ok(CollectivePerceptionMessage::new(
        header,
        CpmPayload::new(
            management,
            ConstraintWrappedCpmContainers(WrappedCpmContainers(SequenceOf::from(containers))),
        ),
    ))
}

fn perceived_object(o: &CpmObject, reference: Vec3) -> PerceivedObject {
    let conf = CoordinateConfidence(coordinate_confidence(o.sigma_m));
    let position = CartesianPosition3dWithConfidence::new(
        CartesianCoordinateWithConfidence::new(
            CartesianCoordinateLarge(cm(o.pos.x - reference.x)),
            conf.clone(),
        ),
        CartesianCoordinateWithConfidence::new(
            CartesianCoordinateLarge(cm(o.pos.y - reference.y)),
            conf,
        ),
        None,
    );
    let velocity = Velocity3dWithConfidence::cartesianVelocity(VelocityCartesian::new(
        velocity_component(o.vel.x),
        velocity_component(o.vel.y),
        None,
    ));
    let heading = v2xw_core::math::atan2(o.vel.y, o.vel.x);
    let heading_deci = (heading.to_degrees().rem_euclid(360.0) * 10.0).round() as u16;
    let angles = crate::asn1::cdd::EulerAnglesWithConfidence::new(
        CartesianAngle::new(
            CartesianAngleValue(heading_deci.min(3_599)),
            AngleConfidence(127),
        ),
        None,
        None,
    );
    let sensors: Vec<Identifier1B> = o.sensor_ids[..usize::from(o.sensor_count.min(3))]
        .iter()
        .map(|s| Identifier1B(*s))
        .collect();
    PerceivedObject {
        object_id: Some(Identifier2B(o.id)),
        measurement_delta_time: DeltaTimeMilliSecondSigned(
            o.measurement_delta_ms.clamp(-2_048, 2_047) as i16,
        ),
        position,
        velocity: Some(velocity),
        acceleration: None,
        angles: (v2xw_core::math::hypot(o.vel.x, o.vel.y) > 0.5).then_some(angles),
        z_angular_velocity: None,
        lower_triangular_correlation_matrices: None,
        object_dimension_z: None,
        object_dimension_y: Some(dimension(o.width_m)),
        object_dimension_x: Some(dimension(o.length_m)),
        object_age: Some(DeltaTimeMilliSecondSigned(o.age_ms.min(2_047) as i16)),
        object_perception_quality: Some(ObjectPerceptionQuality(
            (15.0 - o.sigma_m * 5.0).round().clamp(0.0, 15.0) as u8,
        )),
        sensor_id_list: (!sensors.is_empty())
            .then(|| SequenceOfIdentifier1B(SequenceOf::from(sensors))),
        classification: Some(ObjectClassDescription(SequenceOf::from(vec![
            ObjectClassWithConfidence::new(class_of(o.class), ConfidenceLevel(90)),
        ]))),
        map_position: None,
    }
}

/// UPER-encodes a CPM.
///
/// # Errors
/// The generated encoder's refusal.
pub fn encode_cpm(cpm: &CollectivePerceptionMessage) -> Result<Encoded, CodecError> {
    Ok(Encoded::uper(uper_encode(MsgType::Cpm, cpm)?))
}

/// UPER-decodes a CPM.
///
/// # Errors
/// The generated decoder's refusal.
pub fn decode_cpm(bytes: &[u8]) -> Result<CollectivePerceptionMessage, CodecError> {
    uper_decode(MsgType::Cpm, bytes)
}

/// The objects a received CPM reports, back in world ENU metres, and its sender's
/// reference position: what a receiver fuses.
pub fn objects_of(
    cpm: &CollectivePerceptionMessage,
    origin: GeoOrigin,
) -> Option<(Vec3, Vec<(u16, Vec3, Vec3, CpmObjectClass, f64, f64)>)> {
    let m = &cpm.payload.management_container;
    let lat = f64::from(m.reference_position.latitude.0) * 1e-7;
    let lon = f64::from(m.reference_position.longitude.0) * 1e-7;
    let reference = origin.to_enu(lat, lon, 0.0);
    let mut out = Vec::new();
    for c in cpm.payload.cpm_containers.0.0.iter() {
        if c.container_id.0 != CONTAINER_PERCEIVED_OBJECTS {
            continue;
        }
        let Ok(container) = rasn::uper::decode::<PerceivedObjectContainer>(c.container_data.as_bytes())
        else {
            continue;
        };
        for o in container.perceived_objects.0.iter() {
            let x = f64::from(o.position.x_coordinate.value.0) * 0.01;
            let y = f64::from(o.position.y_coordinate.value.0) * 0.01;
            let vel = match &o.velocity {
                Some(Velocity3dWithConfidence::cartesianVelocity(v)) => Vec3::new(
                    f64::from(v.x_velocity.value.0) * 0.01,
                    f64::from(v.y_velocity.value.0) * 0.01,
                    0.0,
                ),
                _ => Vec3::ZERO,
            };
            let class = o
                .classification
                .as_ref()
                .and_then(|c| c.0.first())
                .map_or(CpmObjectClass::Vehicle, |c| match &c.object_class {
                    ObjectClass::vruSubClass(VruProfileAndSubprofile::pedestrian(_)) => {
                        CpmObjectClass::Pedestrian
                    }
                    ObjectClass::vruSubClass(_) => CpmObjectClass::Cyclist,
                    _ => CpmObjectClass::Vehicle,
                });
            let length = o
                .object_dimension_x
                .as_ref()
                .map_or(4.5, |d| f64::from(d.value.0) * 0.1);
            let width = o
                .object_dimension_y
                .as_ref()
                .map_or(1.8, |d| f64::from(d.value.0) * 0.1);
            out.push((
                o.object_id.as_ref().map_or(0, |i| i.0),
                Vec3::new(reference.x + x, reference.y + y, reference.z),
                vel,
                class,
                length,
                width,
            ));
        }
    }
    Some((reference, out))
}
