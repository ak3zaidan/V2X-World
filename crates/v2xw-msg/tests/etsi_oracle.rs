//! Cross-validation of the ETSI CAM, DENM and VAM encoders against `asn1tools`, an
//! independent ASN.1 compiler and UPER codec.
//!
//! The ETSI messages are encoded by Rust that `rasn-compiler` generated from the committed
//! forge modules ("byte-exact (generated from the module)" in [`v2xw_msg::evidence`]) —
//! but nothing independent had read their octets, so a defect in `rasn` itself, or in how
//! this crate fills a field the standard gives meaning to, would not show. This test hands
//! each encoding to `asn1tools` compiled from the **same committed modules**: it must
//! decode the octets, re-encode them to the same octets, and decode to the values the
//! simulator meant (a DENM's sub-cause and trace, a CAM path's chained deltas).
//!
//! # Running it
//!
//! `V2XW_ETSI_ORACLE_PYTHON` names a Python with `asn1tools` installed
//! (`uv pip install asn1tools`). Without it the test **skips with a message**.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use v2xw_core::belief::{FixQuality, PositionEstimate};
use v2xw_core::geo::GeoOrigin;
use v2xw_core::geom::{Dims, Vec3};
use v2xw_core::rng::RngStream;
use v2xw_core::time::{Duration, WallClock};
use v2xw_msg::cam::{self, CamInput, CamLowFrequency, ExteriorLightMask, ParticipantType};
use v2xw_msg::denm::{self, DenmCause, DenmInput, EventId, RelevanceDirection, TerminationKind};
use v2xw_msg::vam::{self, VamInput, VruProfile};

const SEED: [u8; 32] = *b"v2xw-etsi-oracle-seed-2026-09-30";
const RANDOM: usize = 60;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn origin() -> GeoOrigin {
    GeoOrigin::new(48.137, 11.575, 520.0)
}

fn belief(rng: &mut RngStream) -> PositionEstimate {
    let heading = rng.uniform(0.0, 2.0 * core::f64::consts::PI);
    let speed = rng.uniform(0.0, 35.0);
    let (s, c) = v2xw_core::math::sin_cos(heading);
    PositionEstimate {
        pos: Vec3::new(rng.uniform(-3_000.0, 3_000.0), rng.uniform(-3_000.0, 3_000.0), 3.0),
        vel: Vec3::new(speed * c, speed * s, 0.0),
        heading_rad: heading,
        semi_major_m: rng.uniform(0.5, 8.0),
        semi_minor_m: rng.uniform(0.3, 4.0),
        orientation_rad: rng.uniform(0.0, 3.0),
        time_ns: 0,
        fix: FixQuality::ThreeD,
    }
}

fn ts(rng: &mut RngStream) -> v2xw_msg::asn1::cdd::TimestampIts {
    let wall = WallClock::parse_rfc3339("2027-03-04T08:00:00Z").expect("t0");
    cam::timestamp_its(wall, rng.below(3_600_000_000_000)).expect("timestamp")
}

fn cams(rng: &mut RngStream) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for i in 0..RANDOM {
        let p = belief(rng);
        let mut input = CamInput::new(
            rng.below(u64::from(u32::MAX)) as u32,
            ParticipantType::PassengerCar,
            p,
            origin(),
            Dims::CAR,
            ts(rng),
        );
        input.longitudinal_acceleration_mps2 = Some(rng.uniform(-9.0, 4.0));
        input.lateral_acceleration_mps2 = Some(rng.uniform(-4.0, 4.0));
        input.yaw_rate_rad_s = Some(rng.uniform(-0.5, 0.5));
        input.curvature_inv_m = Some(rng.uniform(-0.05, 0.05));
        input.curvature_from_yaw_rate = true;
        if i % 2 == 0 {
            let n = rng.below(24);
            let mut pos = p.pos;
            input.low_frequency = Some(CamLowFrequency {
                vehicle_role: cam::VehicleRole::Default,
                exterior_lights: ExteriorLightMask(rng.below(256) as u8),
                path_history: (1..=n)
                    .map(|k| {
                        pos = Vec3::new(pos.x - rng.uniform(1.0, 40.0), pos.y + rng.uniform(-5.0, 5.0), pos.z);
                        cam::PathHistoryPoint {
                            pos,
                            age: Duration::from_millis(k * rng.uniform(100.0, 900.0) as u64),
                        }
                    })
                    .collect(),
            });
        }
        let message = cam::build_cam(&input).expect("builds");
        out.push((
            format!("cam/random/{i:03}"),
            cam::encode_cam(&message).expect("encodes").bytes,
        ));
    }
    out
}

fn denms(rng: &mut RngStream) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let causes = [
        (DenmCause::DangerousSituation, 1),
        (DenmCause::StationaryVehicle, 2),
        (DenmCause::Roadworks, 0),
        (DenmCause::HumanPresenceOnTheRoad, 0),
    ];
    for i in 0..RANDOM {
        let (cause, sub) = causes[i % causes.len()];
        let mut input = DenmInput::new(
            EventId {
                originating_station_id: rng.below(u64::from(u32::MAX)) as u32,
                sequence_number: rng.below(65_536) as u16,
            },
            rng.below(u64::from(u32::MAX)) as u32,
            ParticipantType::PassengerCar,
            ts(rng),
            belief(rng),
            origin(),
            cause,
        );
        input.sub_cause = sub;
        input.validity = Duration::from_secs(1 + rng.below(600));
        input.transmission_interval = Some(Duration::from_millis(100 + rng.below(900)));
        input.traffic_direction = Some(match i % 4 {
            0 => RelevanceDirection::All,
            1 => RelevanceDirection::Upstream,
            2 => RelevanceDirection::Downstream,
            _ => RelevanceDirection::Opposite,
        });
        input.trace = (0..rng.below(20))
            .map(|_| {
                (
                    rng.below(20_000) as i32 - 10_000,
                    rng.below(20_000) as i32 - 10_000,
                    0,
                    (1 + rng.below(1_000)) as u16,
                )
            })
            .collect();
        let message = if i % 7 == 6 {
            denm::build_termination_denm(&input, TerminationKind::Cancellation)
        } else {
            denm::build_denm(&input)
        }
        .expect("builds");
        out.push((
            format!("denm/random/{i:03}"),
            denm::encode_denm(&message).expect("encodes").bytes,
        ));
    }
    out
}

fn vams(rng: &mut RngStream) -> Vec<(String, Vec<u8>)> {
    (0..RANDOM / 2)
        .map(|i| {
            let input = VamInput {
                station_id: rng.below(u64::from(u32::MAX)) as u32,
                profile: if i % 2 == 0 {
                    VruProfile::Pedestrian
                } else {
                    VruProfile::Bicyclist
                },
                position: belief(rng),
                origin: origin(),
                generation_time: ts(rng),
                longitudinal_acceleration_mps2: (i % 3 == 0).then(|| rng.uniform(-3.0, 3.0)),
                include_low_frequency: i % 2 == 1,
            };
            let message = vam::build_vam(&input).expect("builds");
            (
                format!("vam/random/{i:03}"),
                vam::encode_vam(&message).expect("encodes").bytes,
            )
        })
        .collect()
}

fn cpms(rng: &mut RngStream) -> Vec<(String, Vec<u8>)> {
    use v2xw_msg::cpm::{self, CpmInput, CpmObject, CpmObjectClass, CpmSensor};
    (0..RANDOM / 2)
        .map(|i| {
            let p = belief(rng);
            let objects: Vec<CpmObject> = (0..rng.below(12))
                .map(|k| CpmObject {
                    id: (k + 1) as u16,
                    measurement_delta_ms: rng.below(200) as i32 - 100,
                    pos: Vec3::new(
                        p.pos.x + rng.uniform(-200.0, 200.0),
                        p.pos.y + rng.uniform(-200.0, 200.0),
                        p.pos.z,
                    ),
                    vel: Vec3::new(rng.uniform(-20.0, 20.0), rng.uniform(-20.0, 20.0), 0.0),
                    length_m: rng.uniform(0.3, 12.0),
                    width_m: rng.uniform(0.3, 2.6),
                    age_ms: rng.below(5_000) as u32,
                    class: match k % 3 {
                        0 => CpmObjectClass::Vehicle,
                        1 => CpmObjectClass::Pedestrian,
                        _ => CpmObjectClass::Cyclist,
                    },
                    sigma_m: rng.uniform(0.1, 3.0),
                    sensor_ids: [1, 3, 0],
                    sensor_count: 2,
                })
                .collect();
            let input = CpmInput {
                station_id: rng.below(u64::from(u32::MAX)) as u32,
                position: p,
                origin: origin(),
                reference_time: ts(rng),
                objects,
                sensors: (i % 2 == 0).then(|| {
                    vec![
                        CpmSensor {
                            id: 1,
                            sensor_type: 1,
                            range_m: 250.0,
                            half_fov_rad: 9f64.to_radians(),
                        },
                        CpmSensor {
                            id: 3,
                            sensor_type: 3,
                            range_m: 80.0,
                            half_fov_rad: 26f64.to_radians(),
                        },
                    ]
                }),
            };
            let message = cpm::build_cpm(&input).expect("builds");
            (
                format!("cpm/random/{i:03}"),
                cpm::encode_cpm(&message).expect("encodes").bytes,
            )
        })
        .collect()
}

/// Every ETSI message the simulator sends decodes in an independent implementation and
/// re-encodes to the same octets; and the fields this crate gives meaning to read back as
/// meant.
#[test]
fn etsi_encodings_are_read_identically_by_asn1tools() {
    let Some(python) = std::env::var_os("V2XW_ETSI_ORACLE_PYTHON").map(PathBuf::from) else {
        eprintln!(
            "SKIPPED: set V2XW_ETSI_ORACLE_PYTHON to a Python with asn1tools installed; until \
             this runs the ETSI octets are checked only by the generator that wrote them."
        );
        return;
    };
    let mut rng = RngStream::from_key(SEED);
    let mut vectors: Vec<(String, &str, Vec<u8>)> = Vec::new();
    for (n, b) in cams(&mut rng) {
        vectors.push((n, "CAM", b));
    }
    for (n, b) in denms(&mut rng) {
        vectors.push((n, "DENM", b));
    }
    for (n, b) in vams(&mut rng) {
        vectors.push((n, "VAM", b));
    }
    for (n, b) in cpms(&mut rng) {
        vectors.push((n, "CollectivePerceptionMessage", b));
    }
    let payload: Vec<Value> = vectors
        .iter()
        .map(|(name, pdu, bytes)| json!({"name": name, "pdu": pdu, "rust_hex": hex(bytes)}))
        .collect();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let work = std::env::temp_dir().join("v2xw-etsi-oracle");
    std::fs::create_dir_all(&work).expect("scratch");
    let vpath = work.join("vectors.json");
    let rpath = work.join("results.json");
    std::fs::write(&vpath, serde_json::to_vec(&payload).expect("json")).expect("writes");
    let root = manifest.join("../../third_party/asn1/etsi");
    let mut cmd = Command::new(&python);
    cmd.arg(manifest.join("tests/oracle/etsi_oracle.py"))
        .arg(&root)
        .arg(&vpath)
        .arg(&rpath);
    // The CPM's container modules, then its PDU, as the build compiles them.
    for m in [
        "CPM-OriginatingStationContainers.asn",
        "CPM-SensorInformationContainer.asn",
        "CPM-PerceptionRegionContainer.asn",
        "CPM-PerceivedObjectContainer.asn",
        "CPM-PDU-Descriptions.asn",
    ] {
        cmd.arg(format!("cpm_ts103324/{m}"));
    }
    let output = cmd.output().expect("runs the oracle");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "the oracle failed: {stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{stdout}");
    let report: Value = serde_json::from_slice(&std::fs::read(&rpath).expect("results"))
        .expect("json");
    let results = report["results"].as_array().expect("results");
    assert_eq!(results.len(), vectors.len());
    let failures: Vec<String> = results
        .iter()
        .filter(|r| r.get("error").is_some() || r["encode_match"] != json!(true))
        .map(|r| format!("{}: {}", r["name"], r.get("error").unwrap_or(&r["py_hex"])))
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} ETSI vectors disagreed with asn1tools:\n{}",
        failures.len(),
        vectors.len(),
        failures.join("\n")
    );
    // A stationary-vehicle DENM reads back as stationaryVehicle(94) / vehicleBreakdown(2).
    let stationary = results
        .iter()
        .find(|r| {
            r["pdu"] == "DENM"
                && r["decoded"]["denm"]["situation"]["eventType"]["ccAndScc"]["__choice__"][0]
                    == "stationaryVehicle94"
        })
        .expect("a stationary-vehicle DENM among the vectors");
    assert_eq!(
        stationary["decoded"]["denm"]["situation"]["eventType"]["ccAndScc"]["__choice__"][1],
        json!(2)
    );
}
