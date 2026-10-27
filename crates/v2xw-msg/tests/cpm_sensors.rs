//! WP-14: a CPM's sensor information container states each sensor's mount
//! (`shapeReferencePoint`) and its opening about its boresight, and a roadside unit's CPM
//! carries the `originatingRsuContainer` (TS 103 324 V2.1.1 §7.1.3, §7.1.4).

use v2xw_core::belief::{FixQuality, PositionEstimate};
use v2xw_core::geo::GeoOrigin;
use v2xw_core::geom::Vec3;
use v2xw_core::time::WallClock;
use v2xw_msg::asn1::cdd::Shape;
use v2xw_msg::asn1::cpm_sensors::SensorInformationContainer;
use v2xw_msg::cam;
use v2xw_msg::cpm::{self, CONTAINER_SENSOR_INFORMATION, CpmInput, CpmSensor};

/// The input [`build_cpm`](cpm::build_cpm) takes for a station at the origin.
pub fn input(rsu: bool, sensors: Vec<CpmSensor>) -> CpmInput {
    let wall = WallClock::parse_rfc3339("2027-03-04T08:00:00Z").expect("t0");
    CpmInput {
        station_id: 77,
        position: PositionEstimate {
            pos: Vec3::new(10.0, 20.0, 6.0),
            vel: Vec3::ZERO,
            heading_rad: 0.0,
            semi_major_m: 0.0,
            semi_minor_m: 0.0,
            orientation_rad: 0.0,
            time_ns: 0,
            fix: FixQuality::Rtk,
        },
        origin: GeoOrigin::new(40.75, -73.99, 10.0),
        reference_time: cam::timestamp_its(wall, 1_000_000_000).expect("timestamp"),
        objects: Vec::new(),
        sensors: Some(sensors),
        rsu,
    }
}

/// The two sensors the tests state: a roadside camera facing north from 1.5 m east and
/// 0.5 m south of the mast, 6 m up; and a vehicle's rear radar.
pub fn sensors() -> Vec<CpmSensor> {
    vec![
        CpmSensor {
            id: 1,
            sensor_type: 3,
            range_m: 80.0,
            half_fov_rad: 45f64.to_radians(),
            azimuth_rad: 90f64.to_radians(),
            mount_m: Some([1.5, -0.5, 6.0]),
        },
        CpmSensor {
            id: 4,
            sensor_type: 1,
            range_m: 80.0,
            half_fov_rad: 75f64.to_radians(),
            azimuth_rad: 180f64.to_radians(),
            mount_m: None,
        },
    ]
}

fn radial(c: &cpm::CpmInput) -> Vec<(Option<(i16, i16, Option<i16>)>, u16, u16)> {
    let m = cpm::build_cpm(c).expect("builds");
    let bytes = cpm::encode_cpm(&m).expect("encodes").bytes;
    let back = cpm::decode_cpm(&bytes).expect("decodes");
    let container = back
        .payload
        .cpm_containers
        .0
        .0
        .iter()
        .find(|x| x.container_id.0 == CONTAINER_SENSOR_INFORMATION)
        .expect("a sensor information container");
    let info: SensorInformationContainer =
        rasn::uper::decode(container.container_data.as_bytes()).expect("the container decodes");
    info.0
        .iter()
        .map(|s| match s.perception_region_shape.as_ref() {
            Some(Shape::radial(r)) => (
                r.shape_reference_point.as_ref().map(|p| {
                    (
                        p.x_coordinate.0,
                        p.y_coordinate.0,
                        p.z_coordinate.as_ref().map(|z| z.0),
                    )
                }),
                r.horizontal_opening_angle_start.0,
                r.horizontal_opening_angle_end.0,
            ),
            other => panic!("not a radial shape: {other:?}"),
        })
        .collect()
}

#[test]
fn a_sensor_states_its_mount_and_its_opening_about_its_boresight() {
    let shapes = radial(&input(true, sensors()));
    assert_eq!(
        shapes,
        vec![
            // North-facing ±45°: 45° round to 135°, mount in centimetres.
            (Some((150, -50, Some(600))), 450, 1_350),
            // Rear-facing ±75°: 105° round to 255°, no mount stated.
            (None, 1_050, 2_550),
        ]
    );
    // A boresight on the x axis keeps the opening it always had: 360 − half round to half.
    let ahead = CpmSensor {
        id: 1,
        sensor_type: 1,
        range_m: 250.0,
        half_fov_rad: 9f64.to_radians(),
        ..CpmSensor::default()
    };
    assert_eq!(radial(&input(false, vec![ahead])), vec![(None, 3_510, 90)]);
    // A full circle is 0 round to 359.9°.
    let round = CpmSensor {
        half_fov_rad: std::f64::consts::PI,
        ..ahead
    };
    assert_eq!(radial(&input(false, vec![round])), vec![(None, 0, 3_599)]);
}

#[test]
fn a_roadside_cpm_says_so_and_a_vehicle_cpm_does_not() {
    for (rsu, expect) in [(true, true), (false, false)] {
        let m = cpm::build_cpm(&input(rsu, sensors())).expect("builds");
        let bytes = cpm::encode_cpm(&m).expect("encodes").bytes;
        let back = cpm::decode_cpm(&bytes).expect("decodes");
        assert_eq!(cpm::from_rsu(&back), expect);
        let ids: Vec<u8> = back
            .payload
            .cpm_containers
            .0
            .0
            .iter()
            .map(|c| c.container_id.0)
            .collect();
        assert_eq!(ids, vec![if rsu { 2 } else { 1 }, 3], "{ids:?}");
    }
}
