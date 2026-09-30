//! Cross-validation of the hand-written J2735 PSM codec against `pycrate`, in both
//! directions and inside its `MessageFrame`.
//!
//! The PSM is what every pedestrian's and cyclist's device sends (J2945/9), and until this
//! test its bytes were "real UPER, not yet oracle-validated": written from the standard's
//! field list with the BSM's shared types and never checked by an independent
//! implementation. The harness is [`tests/oracle/generic_oracle.py`], which takes any PDU
//! the compiled modules hold.
//!
//! # Running it
//!
//! As `tests/j2735_oracle.rs`: `V2XW_J2735_ORACLE_DIR` names a directory holding
//! `j2735_all.py` (compiled from the J2735 modules, never committed — build decision D3) and
//! `.venv/bin/python` with `pycrate`. Without it the test **skips with a message**.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use v2xw_core::rng::RngStream;
use v2xw_msg::j2735::bsm::PositionalAccuracy;
use v2xw_msg::j2735::psm::{self, PersonalDeviceUserType, PersonalSafetyMessage};

const RANDOM_VECTORS: usize = 150;
const SEED: [u8; 32] = *b"v2xw-j2735-psm-oracle-seed-2026\0";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn type_name(t: PersonalDeviceUserType) -> &'static str {
    match t {
        PersonalDeviceUserType::Unavailable => "unavailable",
        PersonalDeviceUserType::Pedestrian => "aPEDESTRIAN",
        PersonalDeviceUserType::Pedalcyclist => "aPEDALCYCLIST",
        PersonalDeviceUserType::PublicSafetyWorker => "aPUBLICSAFETYWORKER",
        PersonalDeviceUserType::Animal => "anANIMAL",
    }
}

fn psm_json(m: &PersonalSafetyMessage) -> Value {
    let mut position = json!({ "lat": m.lat, "long": m.lon });
    if let Some(e) = m.elev {
        position["elevation"] = json!(e);
    }
    json!({
        "basicType": type_name(m.basic_type),
        "secMark": m.sec_mark,
        "msgCnt": m.msg_cnt,
        "id": { "__hex__": hex(&m.id) },
        "position": position,
        "accuracy": {
            "semiMajor": m.accuracy.semi_major,
            "semiMinor": m.accuracy.semi_minor,
            "orientation": m.accuracy.orientation,
        },
        "speed": m.speed,
        "heading": m.heading,
    })
}

fn range(rng: &mut RngStream, lo: i64, hi: i64) -> i64 {
    lo + rng.below((hi - lo + 1) as u64) as i64
}

fn random_psm(rng: &mut RngStream) -> PersonalSafetyMessage {
    let kinds = [
        PersonalDeviceUserType::Unavailable,
        PersonalDeviceUserType::Pedestrian,
        PersonalDeviceUserType::Pedalcyclist,
        PersonalDeviceUserType::PublicSafetyWorker,
        PersonalDeviceUserType::Animal,
    ];
    PersonalSafetyMessage {
        basic_type: kinds[rng.below(5) as usize],
        sec_mark: range(rng, 0, 65_535) as u16,
        msg_cnt: range(rng, 0, 127) as u8,
        id: [
            rng.below(256) as u8,
            rng.below(256) as u8,
            rng.below(256) as u8,
            rng.below(256) as u8,
        ],
        lat: range(rng, -900_000_000, 900_000_001) as i32,
        lon: range(rng, -1_799_999_999, 1_800_000_001) as i32,
        elev: rng.bool(0.5).then(|| range(rng, -4_096, 61_439) as i32),
        accuracy: PositionalAccuracy {
            semi_major: range(rng, 0, 255) as u8,
            semi_minor: range(rng, 0, 255) as u8,
            orientation: range(rng, 0, 65_535) as u16,
        },
        speed: range(rng, 0, 8_191) as u16,
        heading: range(rng, 0, 28_800) as u16,
    }
}

fn boundary() -> Vec<(String, PersonalSafetyMessage)> {
    let base = PersonalSafetyMessage {
        basic_type: PersonalDeviceUserType::Pedestrian,
        sec_mark: 0,
        msg_cnt: 0,
        id: [0; 4],
        lat: -900_000_000,
        lon: -1_799_999_999,
        elev: None,
        accuracy: PositionalAccuracy {
            semi_major: 0,
            semi_minor: 0,
            orientation: 0,
        },
        speed: 0,
        heading: 0,
    };
    let top = PersonalSafetyMessage {
        basic_type: PersonalDeviceUserType::Animal,
        sec_mark: 65_535,
        msg_cnt: 127,
        id: [0xff; 4],
        lat: 900_000_001,
        lon: 1_800_000_001,
        elev: Some(61_439),
        accuracy: PositionalAccuracy {
            semi_major: 255,
            semi_minor: 255,
            orientation: 65_535,
        },
        speed: 8_191,
        heading: 28_800,
    };
    vec![
        ("psm/minima".to_string(), base),
        ("psm/maxima".to_string(), top),
        (
            "psm/elevation-floor".to_string(),
            PersonalSafetyMessage {
                elev: Some(-4_096),
                ..base
            },
        ),
    ]
}

fn oracle_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("V2XW_J2735_ORACLE_DIR")?);
    dir.join("j2735_all.py").is_file().then_some(dir)
}

fn python(dir: &Path) -> PathBuf {
    std::env::var_os("V2XW_J2735_ORACLE_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.join(".venv/bin/python"))
}

/// Every PSM vector, three ways: Rust's octets equal pycrate's, pycrate reads Rust's octets
/// back to the same value, and the `MessageFrame` (DSRCmsgID 32) agrees too.
#[test]
fn rust_psm_encodings_match_pycrate_byte_for_byte() {
    let Some(dir) = oracle_dir() else {
        eprintln!(
            "SKIPPED: set V2XW_J2735_ORACLE_DIR to a directory holding j2735_all.py and \
             .venv/bin/python (see tests/j2735_oracle.rs); until this runs the PSM's bytes \
             are not oracle-validated."
        );
        return;
    };
    let mut rng = RngStream::from_key(SEED);
    let mut vectors = boundary();
    for i in 0..RANDOM_VECTORS {
        vectors.push((format!("psm/random/{i:03}"), random_psm(&mut rng)));
    }
    let payload: Vec<Value> = vectors
        .iter()
        .map(|(name, m)| {
            json!({
                "name": name,
                "pdu": "PersonalSafetyMessage",
                "value": psm_json(m),
                "rust_hex": hex(&psm::encode_psm(m).expect("encodes").bytes),
                "frame_id": psm::PSM_MESSAGE_ID,
                "rust_frame_hex": hex(&psm::encode_message_frame(m).expect("frames").bytes),
            })
        })
        .collect();
    // And pycrate's octets decode to the same PSM in Rust (the decoder is checked too).
    let work = std::env::temp_dir().join("v2xw-j2735-psm-oracle");
    std::fs::create_dir_all(&work).expect("scratch");
    let vectors_path = work.join("vectors.json");
    let results_path = work.join("results.json");
    std::fs::write(&vectors_path, serde_json::to_vec(&payload).expect("json")).expect("writes");
    let output = Command::new(python(&dir))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/generic_oracle.py"))
        .arg(&vectors_path)
        .arg(&results_path)
        .env("V2XW_J2735_ORACLE_DIR", &dir)
        .output()
        .expect("runs the oracle");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "the oracle failed: {stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{stdout}");
    let report: Value =
        serde_json::from_slice(&std::fs::read(&results_path).expect("results")).expect("json");
    let results = report["pycrate_results"].as_array().expect("results");
    assert_eq!(results.len(), vectors.len());
    let mut failures = Vec::new();
    for (r, (name, m)) in results.iter().zip(&vectors) {
        if let Some(e) = r.get("error") {
            failures.push(format!("{name}: pycrate raised {e}"));
            continue;
        }
        for key in ["encode_match", "decode_match", "frame_match"] {
            if r[key] != json!(true) {
                failures.push(format!(
                    "{name}: {key} failed (rust {}, pycrate {})",
                    hex(&psm::encode_psm(m).expect("encodes").bytes),
                    r["py_hex"]
                ));
            }
        }
        let py = r["py_hex"].as_str().expect("hex");
        let bytes: Vec<u8> = (0..py.len() / 2)
            .map(|i| u8::from_str_radix(&py[i * 2..i * 2 + 2], 16).expect("hex"))
            .collect();
        match psm::decode_psm(&bytes) {
            Ok(back) if back == *m => {}
            Ok(back) => failures.push(format!("{name}: Rust read pycrate's octets as {back:?}")),
            Err(e) => failures.push(format!("{name}: Rust could not read pycrate's octets: {e}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PSM vectors disagreed with pycrate:\n{}",
        failures.len(),
        vectors.len(),
        failures.join("\n")
    );
}
