//! The local view: what each vehicle's OBU knows (schema.md §6).
//!
//! Every column here is something the device itself holds: the bytes it decoded, its own
//! radio and security layer's results, its own position, the reports it filed and its own
//! credential events. No column names the sender's device, the engine's message id or a
//! label; the leakage linter ([`super::lint_release`]) refuses the release if one does.

use super::{Col, TableDef, Ty, col};

const POS_ACCURACY: &[Col] = &[
    col(
        "semi_major_m",
        Ty::F64,
        "m",
        "J2735 PositionalAccuracy semi-major axis (1 sigma)",
    ),
    col(
        "semi_minor_m",
        Ty::F64,
        "m",
        "J2735 PositionalAccuracy semi-minor axis (1 sigma)",
    ),
    col(
        "orientation_deg",
        Ty::F64,
        "deg",
        "semi-major axis orientation, clockwise from north",
    ),
];

const ACCEL: &[Col] = &[
    col(
        "long_mps2",
        Ty::F64,
        "m/s^2",
        "J2735 AccelerationSet4Way longitudinal",
    ),
    col("lat_mps2", Ty::F64, "m/s^2", "lateral"),
    col("vert_mps2", Ty::F64, "m/s^2", "vertical"),
    col("yaw_rate_dps", Ty::F64, "deg/s", "yaw rate"),
];

/// `local/ego`.
pub const EGO: TableDef = TableDef {
    name: "local/ego",
    visibility: "NODE",
    doc: "The instrumented receiver's own state at its BSM rate (VeReMi type 2), as its own \
          stack put it in its own BSMs.",
    cols: &[
        col("run_id", Ty::Str, "", "run key (runs.parquet)"),
        col(
            "receiver_ref",
            Ty::Str,
            "",
            "opaque id of this OBU (hash of the run seed and the device; never a node, message or certificate id)",
        ),
        col("t_ns", Ty::I64, "ns", "time since run start"),
        col("x_m", Ty::F64, "m", "own position, local ENU east"),
        col("y_m", Ty::F64, "m", "own position, local ENU north"),
        col("lat_deg", Ty::F64, "deg", "own latitude as broadcast"),
        col("lon_deg", Ty::F64, "deg", "own longitude as broadcast"),
        col("speed_mps", Ty::F64, "m/s", "own speed"),
        col(
            "heading_deg",
            Ty::F64,
            "deg",
            "own heading, J2735, clockwise from north",
        ),
        col(
            "accel_mps2",
            Ty::F64,
            "m/s^2",
            "own longitudinal acceleration",
        ),
        col(
            "pos_accuracy_m",
            Ty::F64,
            "m",
            "own positional accuracy semi-major axis",
        ),
        col(
            "own_cert_id",
            Ty::Str,
            "",
            "HashedId8 of the pseudonym certificate it signs with",
        ),
    ],
    extra: &[],
};

/// `local/rx`.
pub const RX: TableDef = TableDef {
    name: "local/rx",
    visibility: "NODE",
    doc: "One row per message an instrumented receiver decoded (VeReMi type 3, extended with \
          the radio and security layers).",
    cols: RX_COLS,
    extra: &[],
};

/// The columns of `local/rx`, shared with `ml/local`.
pub const RX_COLS: &[Col] = &[
    col("run_id", Ty::Str, "", "run key"),
    col(
        "receiver_ref",
        Ty::Str,
        "",
        "opaque id of the receiving OBU",
    ),
    col(
        "rx_id",
        Ty::Str,
        "",
        "opaque id of this reception row (hash of seed, receiver and reception; joins truth/receptions)",
    ),
    col("t_rx_ns", Ty::I64, "ns", "reception time since run start"),
    col(
        "spdu_digest",
        Ty::Str,
        "",
        "first 8 bytes of SHA-256 over the received 1609.2 SPDU (hex)",
    ),
    col("rssi_dbm", Ty::F64, "dBm", "received signal strength"),
    col(
        "cbr",
        Ty::F64,
        "",
        "receiver's channel busy ratio at reception",
    ),
    col("size_bytes", Ty::I64, "bytes", "size of the received frame"),
    col("psid", Ty::I64, "", "1609.2 PSID of the SPDU (0x20 = BSM)"),
    col(
        "cert_id",
        Ty::Str,
        "",
        "HashedId8 of the signing pseudonym certificate",
    ),
    col(
        "signer_kind",
        Ty::Str,
        "",
        "1609.2 SignerIdentifier choice: certificate or digest",
    ),
    col(
        "issuer_id",
        Ty::Str,
        "",
        "HashedId8 of the signing certificate's issuer",
    ),
    col(
        "verify_result",
        Ty::Str,
        "",
        "ok, bad_signature, expired, not_yet_valid, on_crl, unknown_issuer, not_checked or other",
    ),
    col(
        "verify_latency_us",
        Ty::I64,
        "us",
        "time the receiver's security layer spent verifying",
    ),
    col(
        "generation_time_us",
        Ty::I64,
        "us",
        "1609.2 generationTime (TAI microseconds since 2004)",
    ),
    col("msg_type", Ty::Str, "", "message type (bsm, spat, ...)"),
    col("msg_cnt", Ty::I64, "", "J2735 msgCnt"),
    col("temp_id", Ty::Str, "", "J2735 TemporaryID (hex)"),
    col("sec_mark_ms", Ty::I64, "ms", "J2735 DSecond"),
    col("lat_deg", Ty::F64, "deg", "claimed latitude"),
    col("lon_deg", Ty::F64, "deg", "claimed longitude"),
    col("x_m", Ty::F64, "m", "claimed position, local ENU east"),
    col("y_m", Ty::F64, "m", "claimed position, local ENU north"),
    col("elevation_m", Ty::F64, "m", "claimed elevation"),
    col(
        "pos_accuracy",
        Ty::Struct(POS_ACCURACY),
        "",
        "J2735 PositionalAccuracy",
    ),
    col("speed_mps", Ty::F64, "m/s", "claimed speed"),
    col(
        "heading_deg",
        Ty::F64,
        "deg",
        "claimed heading, clockwise from north",
    ),
    col("accel", Ty::Struct(ACCEL), "", "J2735 AccelerationSet4Way"),
    col(
        "brakes",
        Ty::I64,
        "",
        "J2735 BrakeSystemStatus wheelBrakes bits",
    ),
    col(
        "transmission",
        Ty::Str,
        "",
        "J2735 TransmissionState (forwardGears, ..., unavailable)",
    ),
    col("length_m", Ty::F64, "m", "J2735 VehicleSize length"),
    col("width_m", Ty::F64, "m", "J2735 VehicleSize width"),
    col("event_flags", Ty::I64, "", "J2735 VehicleEventFlags bits"),
    col(
        "ego_x_m",
        Ty::F64,
        "m",
        "receiver's own position east at reception (from local/ego)",
    ),
    col(
        "ego_y_m",
        Ty::F64,
        "m",
        "receiver's own position north at reception",
    ),
    col(
        "ego_speed_mps",
        Ty::F64,
        "m/s",
        "receiver's own speed at reception",
    ),
    col(
        "ego_heading_deg",
        Ty::F64,
        "deg",
        "receiver's own heading at reception",
    ),
    col(
        "range_m",
        Ty::F64,
        "m",
        "distance from the receiver's own position to the claimed position",
    ),
];

/// `local/checks`.
pub const CHECKS: TableDef = TableDef {
    name: "local/checks",
    visibility: "NODE",
    doc: "One row per check that fired in a report this OBU filed (every OBU).",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col(
            "receiver_ref",
            Ty::Str,
            "",
            "opaque id of the OBU that ran the check",
        ),
        col("t_ns", Ty::I64, "ns", "time the check's report was created"),
        col(
            "rx_id",
            Ty::Str,
            "",
            "the reception the check ran on, when it is in local/rx",
        ),
        col(
            "cert_id",
            Ty::Str,
            "",
            "HashedId8 of the checked sender's certificate",
        ),
        col("suite", Ty::Str, "", "sm-mbr-us, research or ts103759"),
        col("check", Ty::Str, "", "check name"),
        col("fired", Ty::Bool, "", "whether the check fired"),
        col("score", Ty::F64, "", "the check's score"),
        col(
            "excess",
            Ty::F64,
            "",
            "how far past its threshold the check was",
        ),
    ],
    extra: &[],
};

/// `local/reports`.
pub const REPORTS: TableDef = TableDef {
    name: "local/reports",
    visibility: "NODE",
    doc: "Every misbehaviour report this OBU created, and what became of it in its store \
          (every OBU).",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col(
            "receiver_ref",
            Ty::Str,
            "",
            "opaque id of the reporting OBU",
        ),
        col(
            "report_id",
            Ty::Str,
            "",
            "first 8 bytes of SHA-256 over the signed MBR (hex)",
        ),
        col(
            "t_created_ns",
            Ty::I64,
            "ns",
            "time the report entered the OBU's store",
        ),
        col(
            "subject_cert_id",
            Ty::Str,
            "",
            "HashedId8 of the accused certificate",
        ),
        col("mbr_type", Ty::Str, "", "SM-MBR misbehaviour type"),
        col(
            "subtypes",
            Ty::ListStr,
            "",
            "check names the report carries",
        ),
        col(
            "evidence_spdu_digests",
            Ty::ListStr,
            "",
            "spdu_digest of every evidence message",
        ),
        col("size_bytes", Ty::I64, "bytes", "size of the signed MBR"),
        col(
            "fate",
            Ty::Str,
            "",
            "uploaded, evicted, expired or pending_at_end",
        ),
        col(
            "t_uploaded_ns",
            Ty::I64,
            "ns",
            "time the upload left the OBU",
        ),
        col("path", Ty::Str, "", "cellular or rsu_relay"),
        col(
            "weight_at_upload",
            Ty::F64,
            "",
            "the store's weight of the report",
        ),
    ],
    extra: &[],
};

/// `local/pki`.
pub const PKI: TableDef = TableDef {
    name: "local/pki",
    visibility: "NODE",
    doc: "The OBU's own credential life (every OBU).",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col("receiver_ref", Ty::Str, "", "opaque id of the OBU"),
        col("t_ns", Ty::I64, "ns", "time since run start"),
        col(
            "event",
            Ty::Str,
            "",
            "enrolled, batch_downloaded, pseudonym_change, crl_downloaded or download_refused",
        ),
        col(
            "cert_id",
            Ty::Str,
            "",
            "HashedId8 of the certificate the event concerns",
        ),
        col("crl_series", Ty::I64, "", "CRL series"),
        col("crl_number", Ty::I64, "", "CRL number"),
        col("crl_entries", Ty::I64, "", "entries in the downloaded CRL"),
        col(
            "bytes",
            Ty::I64,
            "bytes",
            "bytes transferred; null in v1 (not recorded per event)",
        ),
        col(
            "latency_ms",
            Ty::F64,
            "ms",
            "request to response; null in v1 (not recorded per event)",
        ),
    ],
    extra: &[],
};

/// Every local table.
pub const TABLES: &[&TableDef] = &[&EGO, &RX, &CHECKS, &REPORTS, &PKI];

/// The SM-MBR v1.0 checks deployed in the US CV pilots (schema.md §6.1); any other check
/// name is the research suite's.
pub const SM_MBR_CHECKS: &[&str] = &[
    "ConstantPosition",
    "RandomPosition",
    "MaxSpeed",
    "MaxAcceleration",
    "BrakeAcceleration",
    "Distance",
];

/// The suite a check name belongs to.
#[must_use]
pub fn suite_of(check: &str) -> &'static str {
    if SM_MBR_CHECKS.iter().any(|c| c.eq_ignore_ascii_case(check)) {
        "sm-mbr-us"
    } else {
        "research"
    }
}
