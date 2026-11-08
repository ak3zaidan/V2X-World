//! The `ml/` convenience tables (schema.md §8): an observer table joined with its label.
//!
//! They are the declared exception to the leakage rule and are named as joined, the way
//! VeReMi NextGen ships its rows; `local/` and `global/` stay clean. The leakage linter
//! skips this folder by name and nothing else.

use super::{Col, TableDef, Ty, col};

const LOCAL_LABELS: &[Col] = &[
    col(
        "label",
        Ty::Str,
        "",
        "JOINED from truth/receptions: the transmission's label",
    ),
    col(
        "attack_id",
        Ty::Str,
        "",
        "JOINED: attack short name when falsified",
    ),
    col(
        "family",
        Ty::Str,
        "",
        "JOINED: the attack's family, what kind of lie it tells (position, speed, heading, \
         timing, identity, credentials, flooding, ...); null when benign",
    ),
    col(
        "veremi_ext_code",
        Ty::I64,
        "",
        "JOINED: VeReMi Extension code",
    ),
];

const GLOBAL_LABELS: &[Col] = &[
    col(
        "verdict",
        Ty::Str,
        "",
        "JOINED from truth/reports: true_accusation or false_accusation",
    ),
    col(
        "subject_label",
        Ty::Str,
        "",
        "JOINED: the accused device's label",
    ),
];

/// `ml/local`: `local/rx` with the reception's label.
pub const LOCAL: TableDef = TableDef {
    name: "ml/local",
    visibility: "JOINED",
    doc: "local/rx joined with truth/receptions.label and attack_id, for training.",
    cols: super::local::RX_COLS,
    extra: LOCAL_LABELS,
};

/// `ml/global`: `global/reports` with the report's verdict.
pub const GLOBAL: TableDef = TableDef {
    name: "ml/global",
    visibility: "JOINED",
    doc: "global/reports joined with truth/reports.verdict, for training.",
    cols: super::global::REPORT_COLS,
    extra: GLOBAL_LABELS,
};

/// Both.
pub const TABLES: &[&TableDef] = &[&LOCAL, &GLOBAL];
