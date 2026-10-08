//! Layer 3, ETSI messages: the CAM, DENM and VAM encoders against asn1tools, an ASN.1
//! compiler and UPER codec written independently of the Rust bindings, reading the ETSI
//! ASN.1 modules in `third_party/asn1/etsi`.

use std::path::Path;

use serde_json::json;
use v2xw_core::belief::{FixQuality, PositionEstimate};
use v2xw_core::geo::GeoOrigin;
use v2xw_core::geom::{Dims, Vec3};
use v2xw_core::time::WallClock;
use v2xw_msg::cam::{CamInput, ParticipantType, build_cam, encode_cam, timestamp_its};
use v2xw_msg::denm::{DenmCause, DenmInput, EventId, build_denm, encode_denm};
use v2xw_msg::vam::{VamInput, VruProfile, build_vam, encode_vam};

use crate::{Check, Cost, Layer, Mode, Outcome};

/// The checks of this module.
#[must_use]
pub fn checks() -> Vec<Check> {
    vec![Check {
        id: "NET-06",
        layer: Layer::Network,
        title: "The ETSI CAM, DENM and VAM encoders against an independent ASN.1 codec: 120 CAMs, 60 DENMs and 60 VAMs decode, re-encode to the same octets, and carry the station id and position sent",
        reference: "ETSI TS 103 900 V2.1.1 (CAM), TS 103 831 V2.1.1 (DENM), TS 103 300-3 V2.2.1 (VAM) over TS 102 894-2 V2.1.1 (CDD), ASN.1 from third_party/asn1/etsi; ITU-T X.691 UPER (canonical, so a conformant encoding re-encodes identically); decoded by asn1tools (reference/etsi_uper.py); positions as 0.1 µdegree (CDD Latitude/Longitude)",
        tolerance: "every message decodes and re-encodes octet for octet; station id exact; latitude and longitude within 1 unit of round(1e7·degrees)",
        fault: "The encoder emits one extra presence bit after the 48-bit ITS PDU header (an OPTIONAL the ASN.1 does not have)",
        cost: Cost::Fast,
        run: etsi_messages,
    }]
}

/// SplitMix64 for the random inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * ((self.next() >> 11) as f64 / (1u64 << 53) as f64)
    }
}

/// Inserts a zero bit at bit offset `at` (MSB first), growing the buffer if needed.
fn insert_bit(bytes: &[u8], at: usize) -> Vec<u8> {
    let n = bytes.len() * 8;
    let bit = |i: usize| (bytes[i / 8] >> (7 - i % 8)) & 1;
    let mut out = vec![0u8; bytes.len() + 1];
    let mut j = 0;
    for i in 0..n {
        if i == at {
            j += 1;
        }
        if bit(i) == 1 {
            out[j / 8] |= 1 << (7 - j % 8);
        }
        j += 1;
    }
    out
}

fn position(rng: &mut Rng) -> PositionEstimate {
    let speed = rng.range(0.0, 35.0);
    let heading = rng.range(-3.1, 3.1);
    PositionEstimate {
        pos: Vec3::new(
            rng.range(-5_000.0, 5_000.0),
            rng.range(-5_000.0, 5_000.0),
            rng.range(-20.0, 200.0),
        ),
        vel: Vec3::new(speed * heading.cos(), speed * heading.sin(), 0.0),
        heading_rad: heading,
        semi_major_m: rng.range(0.5, 20.0),
        semi_minor_m: rng.range(0.2, 0.5),
        orientation_rad: rng.range(0.0, 3.1),
        time_ns: 0,
        fix: FixQuality::ThreeD,
    }
}

fn etsi_messages(mode: Mode) -> Outcome {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/asn1/etsi");
    let cdd = root.join("cdd_ts102894_2/ETSI-ITS-CDD.asn");
    let clock = WallClock::new(1_767_225_600);
    let mut rng = Rng(0xE751);
    let origins = [
        GeoOrigin::new(40.7440, -73.9900, 0.0),
        GeoOrigin::new(52.5200, 13.4050, 34.0),
        GeoOrigin::new(-33.8688, 151.2093, 10.0),
    ];
    let types = [
        ParticipantType::PassengerCar,
        ParticipantType::Bus,
        ParticipantType::HeavyTruck,
        ParticipantType::Motorcycle,
        ParticipantType::LightTruck,
    ];
    let causes = [
        DenmCause::TrafficCondition,
        DenmCause::Accident,
        DenmCause::Roadworks,
        DenmCause::Adhesion,
        DenmCause::ObstacleOnTheRoad,
        DenmCause::HumanPresenceOnTheRoad,
        DenmCause::DangerousEndOfQueue,
        DenmCause::EmergencyVehicleApproaching,
        DenmCause::StationaryVehicle,
    ];
    // (type, modules, [(hex, station, lat, lon)])
    let mut sets: Vec<(&str, Vec<String>, Vec<(String, u32, i64, i64)>)> = Vec::new();
    let expect = |o: &GeoOrigin, p: &PositionEstimate| {
        let (lat, lon, _) = o.to_geodetic(p.pos);
        ((lat * 1e7).round() as i64, (lon * 1e7).round() as i64)
    };
    let fault = |b: Vec<u8>| {
        if mode.faulted() {
            insert_bit(&b, 48)
        } else {
            b
        }
    };
    let mut problems = Vec::new();

    let mut cams = Vec::new();
    for k in 0..120u32 {
        let origin = origins[(k % 3) as usize];
        let p = position(&mut rng);
        let station = (rng.next() & 0xFFFF_FFFF) as u32;
        let ts = match timestamp_its(
            clock,
            u64::from(k) * 100_000_000 + rng.next() % 1_000_000_000,
        ) {
            Ok(t) => t,
            Err(e) => return Outcome::judge(false, format!("timestamp: {e:?}")),
        };
        let mut input = CamInput::new(station, types[(k % 5) as usize], p, origin, Dims::CAR, ts);
        if k % 2 == 0 {
            input.longitudinal_acceleration_mps2 = Some(rng.range(-8.0, 4.0));
            input.yaw_rate_rad_s = Some(rng.range(-0.5, 0.5));
            input.curvature_inv_m = Some(rng.range(-0.05, 0.05));
            input.speed_accuracy_mps = Some(rng.range(0.05, 2.0));
            input.heading_accuracy_rad = Some(rng.range(0.01, 0.3));
        }
        match build_cam(&input).and_then(|c| encode_cam(&c)) {
            Ok(e) => {
                let (lat, lon) = expect(&origin, &p);
                cams.push((hex(&fault(e.bytes)), station, lat, lon));
            }
            Err(e) => problems.push(format!("CAM {k} does not encode: {e}")),
        }
    }
    sets.push((
        "CAM",
        vec![
            root.join("cam_ts103900/CAM-PDU-Descriptions.asn")
                .display()
                .to_string(),
            cdd.display().to_string(),
        ],
        cams,
    ));

    let mut denms = Vec::new();
    for k in 0..60u32 {
        let origin = origins[(k % 3) as usize];
        let p = position(&mut rng);
        let station = (rng.next() & 0xFFFF_FFFF) as u32;
        let ts = match timestamp_its(clock, u64::from(k) * 1_000_000_000) {
            Ok(t) => t,
            Err(e) => return Outcome::judge(false, format!("timestamp: {e:?}")),
        };
        let id = EventId {
            originating_station_id: station,
            sequence_number: (rng.next() & 0xFFFF) as u16,
        };
        let input = DenmInput::new(
            id,
            station,
            types[(k % 5) as usize],
            ts,
            p,
            origin,
            causes[(k % 9) as usize],
        );
        match build_denm(&input).and_then(|d| encode_denm(&d)) {
            Ok(e) => {
                let (lat, lon) = expect(&origin, &p);
                denms.push((hex(&fault(e.bytes)), station, lat, lon));
            }
            Err(e) => problems.push(format!("DENM {k} does not encode: {e}")),
        }
    }
    sets.push((
        "DENM",
        vec![
            root.join("denm_ts103831/DENM-PDU-Descriptions.asn")
                .display()
                .to_string(),
            cdd.display().to_string(),
        ],
        denms,
    ));

    let mut vams = Vec::new();
    for k in 0..60u32 {
        let origin = origins[(k % 3) as usize];
        let mut p = position(&mut rng);
        p.vel = Vec3::new(p.vel.x / 10.0, p.vel.y / 10.0, 0.0);
        let station = (rng.next() & 0xFFFF_FFFF) as u32;
        let ts = match timestamp_its(clock, u64::from(k) * 500_000_000) {
            Ok(t) => t,
            Err(e) => return Outcome::judge(false, format!("timestamp: {e:?}")),
        };
        let input = VamInput {
            station_id: station,
            profile: if k % 2 == 0 {
                VruProfile::Pedestrian
            } else {
                VruProfile::Bicyclist
            },
            position: p,
            origin,
            generation_time: ts,
            longitudinal_acceleration_mps2: (k % 3 == 0).then(|| rng.range(-2.0, 2.0)),
            include_low_frequency: k % 4 == 0,
        };
        match build_vam(&input).and_then(|v| encode_vam(&v)) {
            Ok(e) => {
                let (lat, lon) = expect(&origin, &p);
                vams.push((hex(&fault(e.bytes)), station, lat, lon));
            }
            Err(e) => problems.push(format!("VAM {k} does not encode: {e}")),
        }
    }
    sets.push((
        "VAM",
        vec![
            root.join("vam_ts103300_3/VAM-PDU-Descriptions.asn")
                .display()
                .to_string(),
            cdd.display().to_string(),
        ],
        vams,
    ));

    let mut summary = Vec::new();
    for (ty, asn, items) in &sets {
        let hexes: Vec<&str> = items.iter().map(|i| i.0.as_str()).collect();
        let out = match crate::python(
            "etsi_uper.py",
            &json!({"type": ty, "asn": asn, "items": hexes}),
        ) {
            Ok(v) => v,
            Err(e) => return Outcome::skip(e),
        };
        let mut bad = 0;
        for (got, (_, station, lat, lon)) in
            out["items"].as_array().into_iter().flatten().zip(items)
        {
            let why = if let Some(e) = got.get("error") {
                Some(format!("{ty}: {e}"))
            } else if got["reencoded_equal"].as_bool() != Some(true) {
                Some(format!("{ty}: re-encodes differently"))
            } else if got["stationId"].as_u64() != Some(u64::from(*station)) {
                Some(format!("{ty}: station {} vs {station}", got["stationId"]))
            } else if (got["latitude"].as_i64().unwrap_or(i64::MAX) - lat).abs() > 1
                || (got["longitude"].as_i64().unwrap_or(i64::MAX) - lon).abs() > 1
            {
                Some(format!(
                    "{ty}: position {}, {} vs {lat}, {lon}",
                    got["latitude"], got["longitude"]
                ))
            } else {
                None
            };
            if let Some(w) = why {
                bad += 1;
                if problems.len() < 3 {
                    problems.push(w);
                }
            }
        }
        summary.push(format!(
            "{ty}: {} of {} conform",
            items.len() - bad,
            items.len()
        ));
        if bad > 0 && problems.is_empty() {
            problems.push(format!("{ty}: {bad} failed"));
        }
    }
    Outcome::judge(
        problems.is_empty(),
        if problems.is_empty() {
            summary.join("; ")
        } else {
            format!("{}; first: {}", summary.join("; "), problems.join("; "))
        },
    )
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
