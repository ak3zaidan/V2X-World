//! The answers (schema.md §8): who really sent what, which attack, what happened.
//!
//! Nothing in the system sees these tables. They are keyed by the engine's ids
//! (`device_id`, `tx_id`) and joined to the observer views through keys any holder of the
//! bytes can compute (`spdu_digest`, `cert_id`, `report_id`) or through the opaque
//! `rx_id` / `receiver_ref`.

use super::{TableDef, Ty, col};

/// `truth/devices`.
pub const DEVICES: TableDef = TableDef {
    name: "truth/devices",
    visibility: "ORACLE",
    doc: "One row per device: its class, label and attack.",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col("device_id", Ty::I64, "", "engine node id"),
        col("class", Ty::Str, "", "vehicle, rsu or vru"),
        col("provider", Ty::Str, "", "SCMS provider"),
        col(
            "gnss_class",
            Ty::Str,
            "",
            "BSM timing class (GNSS fix and stamping behaviour), when the model runs",
        ),
        col("label", Ty::Str, "", "benign, faulty or malicious"),
        col("fault_kind", Ty::Str, "", "the fault of a faulty device"),
        col(
            "attack_id",
            Ty::Str,
            "",
            "attack short name (ConstPos, ...)",
        ),
        col(
            "attack_params",
            Ty::ListStr,
            "",
            "the scenario's attack parameters, name=value",
        ),
        col(
            "attack_start_ns",
            Ty::I64,
            "ns",
            "scheduled or first falsification",
        ),
        col("attack_end_ns", Ty::I64, "ns", "last falsification"),
        col("joined_ns", Ty::I64, "ns", "first in the run"),
        col("left_ns", Ty::I64, "ns", "last in the run"),
        col("la_link", Ty::Str, "", "the device's linkage token"),
        col(
            "receiver_ref",
            Ty::Str,
            "",
            "its receiver_ref (every OBU has one; local/rx and local/ego hold only the instrumented ones)",
        ),
        col(
            "instrumented",
            Ty::Bool,
            "",
            "whether local/rx and local/ego hold its receptions",
        ),
    ],
    extra: &[],
};

/// `truth/pseudonyms`.
pub const PSEUDONYMS: TableDef = TableDef {
    name: "truth/pseudonyms",
    visibility: "ORACLE",
    doc: "Certificate to device.",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col("cert_id", Ty::Str, "", "HashedId8"),
        col("device_id", Ty::I64, "", "engine node id"),
        col(
            "first_use_ns",
            Ty::I64,
            "ns",
            "first transmission signed with it",
        ),
        col(
            "last_use_ns",
            Ty::I64,
            "ns",
            "last transmission signed with it",
        ),
        col("n_tx", Ty::I64, "", "transmissions signed with it"),
    ],
    extra: &[],
};

/// `truth/transmissions`.
pub const TRANSMISSIONS: TableDef = TableDef {
    name: "truth/transmissions",
    visibility: "ORACLE",
    doc: "One row per transmission: sender, label, attack, crosswalk codes and the true state.",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col("tx_id", Ty::I64, "", "engine message id"),
        col("device_id", Ty::I64, "", "sender node id"),
        col("t_ns", Ty::I64, "ns", "transmission time"),
        col(
            "spdu_digest",
            Ty::Str,
            "",
            "joins local/rx and global evidence",
        ),
        col("cert_id", Ty::Str, "", "signing certificate"),
        col(
            "label",
            Ty::Str,
            "",
            "message label: benign, faulty or malicious (falsified)",
        ),
        col("device_label", Ty::Str, "", "the sender's device label"),
        col("attack_id", Ty::Str, "", "attack short name when falsified"),
        col(
            "family",
            Ty::Str,
            "",
            "the attack's family, what kind of lie it tells (position, speed, heading, timing, \
             identity, credentials, flooding, ...); null when the message was not falsified",
        ),
        col(
            "veremi_ext_code",
            Ty::I64,
            "",
            "VeReMi Extension attack code (0 benign; null when the codebook has no such attack)",
        ),
        col(
            "veremi1_code",
            Ty::I64,
            "",
            "VeReMi 1.0 attack code (0 benign; null when the codebook has no such attack)",
        ),
        col("nextgen_name", Ty::Str, "", "closest VeReMi NextGen class"),
        col("dare_name", Ty::Str, "", "closest DARE class"),
        col(
            "fields_falsified",
            Ty::ListStr,
            "",
            "content fields the attacker changed",
        ),
        col("magnitude", Ty::F64, "", "size of the falsification"),
        col("x_m", Ty::F64, "m", "true position east at transmission"),
        col("y_m", Ty::F64, "m", "true position north at transmission"),
        col("speed_mps", Ty::F64, "m/s", "true speed"),
        col("heading_deg", Ty::F64, "deg", "true heading"),
        col("accel_mps2", Ty::F64, "m/s^2", "true acceleration"),
        col(
            "gnss_error_m",
            Ty::F64,
            "m",
            "the sender's honest GNSS error",
        ),
        col(
            "fix_to_stamp_ms",
            Ty::F64,
            "ms",
            "the sender's fix-to-timestamp lag; null in v1 (the engine does not record it)",
        ),
    ],
    extra: &[],
};

/// `truth/receptions`.
pub const RECEPTIONS: TableDef = TableDef {
    name: "truth/receptions",
    visibility: "ORACLE",
    doc: "One row per local/rx row: which transmission it was.",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col("rx_id", Ty::Str, "", "joins local/rx"),
        col("tx_id", Ty::I64, "", "engine message id"),
        col("rx_device_id", Ty::I64, "", "receiver node id"),
        col("tx_device_id", Ty::I64, "", "sender node id"),
        col("label", Ty::Str, "", "the transmission's label"),
        col("attack_id", Ty::Str, "", "attack short name when falsified"),
        col(
            "true_distance_m",
            Ty::F64,
            "m",
            "true sender-receiver distance",
        ),
    ],
    extra: &[],
};

/// `truth/reports`.
pub const REPORTS: TableDef = TableDef {
    name: "truth/reports",
    visibility: "ORACLE",
    doc: "One row per report: who filed it against whom, and whether it was true.",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col(
            "report_id",
            Ty::Str,
            "",
            "joins local/reports and global/reports",
        ),
        col("reporter_device_id", Ty::I64, "", "reporter node id"),
        col("subject_cert_id", Ty::Str, "", "accused certificate"),
        col("subject_device_id", Ty::I64, "", "accused node id"),
        col("subject_label", Ty::Str, "", "accused device's label"),
        col(
            "verdict",
            Ty::Str,
            "",
            "true_accusation or false_accusation",
        ),
        col(
            "poisoned",
            Ty::Bool,
            "",
            "a forgery an attacker filed on purpose (ReportPoisoning), not a detection",
        ),
        col("fate", Ty::Str, "", "store fate"),
        col("t_created_ns", Ty::I64, "ns", "created"),
        col("t_at_ma_ns", Ty::I64, "ns", "decrypted at the MA"),
        col(
            "reporter_checks",
            Ty::ListStr,
            "",
            "the reporter's fired checks (engine names; on the wire only as SM-MBR codes)",
        ),
        col(
            "nonstandard_triggers",
            Ty::ListStr,
            "",
            "fired checks with no SM-MBR code: the wire cannot carry them",
        ),
    ],
    extra: &[],
};

/// `truth/timeline`.
pub const TIMELINE: TableDef = TableDef {
    name: "truth/timeline",
    visibility: "ORACLE",
    doc: "Per misbehaving or falsely accused device: how long it stayed trusted, stage by stage.",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col("device_id", Ty::I64, "", "node id"),
        col("label", Ty::Str, "", "device label"),
        col("falsely_accused", Ty::Bool, "", "benign but reported"),
        col("attack_start_ns", Ty::I64, "ns", "attack start (scheduled)"),
        col(
            "first_falsified_ns",
            Ty::I64,
            "ns",
            "first falsified transmission",
        ),
        col(
            "first_detection_ns",
            Ty::I64,
            "ns",
            "first local check that fired against it",
        ),
        col(
            "first_report_created_ns",
            Ty::I64,
            "ns",
            "first report against it created",
        ),
        col(
            "first_report_uploaded_ns",
            Ty::I64,
            "ns",
            "first such report uploaded",
        ),
        col(
            "first_report_at_ma_ns",
            Ty::I64,
            "ns",
            "first such report decrypted at the MA",
        ),
        col(
            "decision_ns",
            Ty::I64,
            "ns",
            "first revoke decision against it",
        ),
        col("crl_published_ns", Ty::I64, "ns", "first CRL entry for it"),
        col(
            "crl_first_neighbour_ns",
            Ty::I64,
            "ns",
            "first neighbour that downloaded that CRL",
        ),
        col("crl_median_neighbour_ns", Ty::I64, "ns", "median neighbour"),
        col(
            "crl_p95_neighbour_ns",
            Ty::I64,
            "ns",
            "95th-percentile neighbour",
        ),
        col(
            "n_neighbours",
            Ty::I64,
            "",
            "receivers that decoded at least one of its messages",
        ),
        col(
            "last_malicious_accepted_ns",
            Ty::I64,
            "ns",
            "last falsified message a receiver decoded and did not reject",
        ),
    ],
    extra: &[],
};

/// `truth/app_effects`.
pub const APP_EFFECTS: TableDef = TableDef {
    name: "truth/app_effects",
    visibility: "ORACLE",
    doc: "Warnings an application raised because of a falsified or faulty sender.",
    cols: &[
        col("run_id", Ty::Str, "", "run key"),
        col("t_ns", Ty::I64, "ns", "warning time"),
        col("app", Ty::Str, "", "application"),
        col("receiver_device_id", Ty::I64, "", "warned node id"),
        col(
            "tx_id",
            Ty::I64,
            "",
            "triggering message, where the app knows it",
        ),
        col(
            "subject_cert_id",
            Ty::Str,
            "",
            "certificate of the message that triggered it",
        ),
        col("subject_device_id", Ty::I64, "", "its sender"),
        col("subject_label", Ty::Str, "", "its sender's label"),
        col("severity", Ty::Str, "", "warning severity"),
        col(
            "driver_reaction",
            Ty::Str,
            "",
            "how the driver model reacted; null in v1 (the apps do not record it)",
        ),
    ],
    extra: &[],
};

/// Every truth table.
pub const TABLES: &[&TableDef] = &[
    &DEVICES,
    &PSEUDONYMS,
    &TRANSMISSIONS,
    &RECEPTIONS,
    &REPORTS,
    &TIMELINE,
    &APP_EFFECTS,
];

/// The `p`th percentile (nearest rank) of sorted values.
#[must_use]
pub fn percentile(sorted: &[i64], p: f64) -> Option<i64> {
    if sorted.is_empty() {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
    Some(sorted[rank.min(sorted.len()) - 1])
}

#[cfg(test)]
mod tests {
    #[test]
    fn nearest_rank_percentiles() {
        let v = [10, 20, 30, 40];
        assert_eq!(super::percentile(&v, 50.0), Some(20));
        assert_eq!(super::percentile(&v, 95.0), Some(40));
        assert_eq!(super::percentile(&v, 0.0), Some(10));
        assert_eq!(super::percentile(&[], 50.0), None);
    }
}
