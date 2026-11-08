//! The global view: what the Misbehavior Authority knows (schema.md §7).
//!
//! The report fields are the US CV-pilot report's (SM-MBR v1.0 and its ASN.1 v1.1:
//! `EeMaApplicationSpecificMisbehaviorReport`, `Psid20Mbr`, `SignedBsmsWithCertificate`),
//! decoded. The MA never learns the reporter's enrolment identity, so no reporter device
//! appears; `la_link` is null until the MA's policy asks the Linkage Authorities.

use super::{Col, TableDef, Ty, col};

const LOCATION: &[Col] = &[
    col("lat_deg", Ty::F64, "deg", "latitude"),
    col("lon_deg", Ty::F64, "deg", "longitude"),
    col("elevation_m", Ty::F64, "m", "elevation"),
];

const SUBTYPE: &[Col] = &[
    col("type", Ty::I64, "", "SM-MBR misbehaviour type code"),
    col("subtype", Ty::I64, "", "SM-MBR subtype code"),
    col("name", Ty::Str, "", "subtype name"),
    col(
        "params",
        Ty::ListStr,
        "",
        "the subtype's parameters as name=value strings, in the SM-MBR unit",
    ),
];

const EVIDENCE: &[Col] = &[
    col(
        "spdu_digest",
        Ty::Str,
        "",
        "first 8 bytes of SHA-256 over the evidence SPDU",
    ),
    col("generation_time_us", Ty::I64, "us", "1609.2 generationTime"),
    col("sec_mark_ms", Ty::I64, "ms", "J2735 DSecond"),
    col("msg_cnt", Ty::I64, "", "J2735 msgCnt"),
    col("lat_deg", Ty::F64, "deg", "claimed latitude"),
    col("lon_deg", Ty::F64, "deg", "claimed longitude"),
    col("speed_mps", Ty::F64, "m/s", "claimed speed"),
    col("heading_deg", Ty::F64, "deg", "claimed heading"),
    col(
        "accel_long_mps2",
        Ty::F64,
        "m/s^2",
        "claimed longitudinal acceleration",
    ),
    col("brakes", Ty::I64, "", "J2735 wheelBrakes bits"),
    col(
        "pos_accuracy_m",
        Ty::F64,
        "m",
        "claimed positional accuracy semi-major axis",
    ),
];

/// The columns of `global/reports`, shared with `ml/global`.
pub const REPORT_COLS: &[Col] = &[
    col("run_id", Ty::Str, "", "run key"),
    col(
        "ma_id",
        Ty::I64,
        "",
        "the MA instance that decrypted the report",
    ),
    col(
        "report_id",
        Ty::Str,
        "",
        "first 8 bytes of SHA-256 over the signed MBR (hex)",
    ),
    col("t_received_ns", Ty::I64, "ns", "time the MA decrypted it"),
    col(
        "generation_time_us",
        Ty::I64,
        "us",
        "1609.2 generationTime of the report",
    ),
    col(
        "generation_location",
        Ty::Struct(LOCATION),
        "",
        "where the reporter was",
    ),
    col("psid", Ty::I64, "", "PSID of the report (0x20)"),
    col(
        "reporter_cert_id",
        Ty::Str,
        "",
        "HashedId8 of the certificate that signed the report",
    ),
    col(
        "subject_cert_id",
        Ty::Str,
        "",
        "HashedId8 of the accused certificate",
    ),
    col(
        "subject_issuer",
        Ty::Str,
        "",
        "issuer of the accused certificate",
    ),
    col(
        "mbr_type",
        Ty::Str,
        "",
        "1VInconsistent, Implausible or ObsImplausible",
    ),
    col(
        "subtypes",
        Ty::ListStruct(SUBTYPE),
        "",
        "SM-MBR subtype codes",
    ),
    col(
        "evidence",
        Ty::ListStruct(EVIDENCE),
        "",
        "the signed BSMs the report carries, decoded",
    ),
    col(
        "evidence_start_ns",
        Ty::I64,
        "ns",
        "first evidence message, run time",
    ),
    col(
        "evidence_end_ns",
        Ty::I64,
        "ns",
        "last evidence message, run time",
    ),
    col(
        "set_aside",
        Ty::Str,
        "",
        "null, or why the MA set the report aside",
    ),
    col(
        "la_link",
        Ty::Str,
        "",
        "linkage token: null unless a decision's case asked the LAs about the report",
    ),
    col(
        "la_link_t_ns",
        Ty::I64,
        "ns",
        "the decision that asked the LAs (global/decisions.t_ns)",
    ),
];

/// `global/reports`.
pub const REPORTS: TableDef = TableDef {
    name: "global/reports",
    visibility: "MA",
    doc: "One row per misbehaviour report the MA decrypted.",
    cols: REPORT_COLS,
    extra: &[],
};

/// `global/decisions`.
pub const DECISIONS: TableDef = TableDef {
    name: "global/decisions",
    visibility: "MA",
    doc: "One row per MA decision.",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col("ma_id", Ty::I64, "", "deciding MA instance"),
        col("t_ns", Ty::I64, "ns", "decision time"),
        col("subject_kind", Ty::Str, "", "pseudonym or device"),
        col(
            "subject_cert_id",
            Ty::Str,
            "",
            "HashedId8 of the certificate decided on",
        ),
        col(
            "la_link",
            Ty::Str,
            "",
            "linkage token when the subject is a device",
        ),
        col("decision", Ty::Str, "", "revoke, dismiss or monitor"),
        col("rule", Ty::Str, "", "the policy rule that decided"),
        col(
            "report_ids",
            Ty::ListStr,
            "",
            "report_id of every report the decision used",
        ),
        col("n_reports", Ty::I64, "", "reports used"),
        col(
            "n_distinct_reporters",
            Ty::I64,
            "",
            "distinct reporting certificates",
        ),
    ],
    extra: &[],
};

/// `global/crl`.
pub const CRL: TableDef = TableDef {
    name: "global/crl",
    visibility: "MA",
    doc: "The CRL entries the MA caused to be published.",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col("t_issued_ns", Ty::I64, "ns", "issue time"),
        col("crl_series", Ty::I64, "", "CRL series"),
        col("crl_number", Ty::I64, "", "CRL number"),
        col("entry_kind", Ty::Str, "", "linkage_seed or hash_id"),
        col(
            "la_link",
            Ty::Str,
            "",
            "linkage token of the revoked device",
        ),
        col(
            "hash_id",
            Ty::Str,
            "",
            "HashedId of the revoked certificate, for a hash_id entry",
        ),
        col("i_period", Ty::I64, "", "i-period the linkage seed is for"),
        col(
            "t_effective_ns",
            Ty::I64,
            "ns",
            "time the entry takes effect",
        ),
    ],
    extra: &[],
};

/// Every global table.
pub const TABLES: &[&TableDef] = &[&REPORTS, &DECISIONS, &CRL];
