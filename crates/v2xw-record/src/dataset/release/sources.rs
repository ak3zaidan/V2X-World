//! The one mapping table from a release column to the recording field it is read from.
//!
//! Every value the release writer reads out of a recording goes through [`pick`], which
//! looks the column up here. A row names its channel and one or more dotted field paths,
//! tried in order: the first is the field the record contract (docs/dataset/record-contract.md)
//! names, the later ones are fields the engine already records that carry the same value.
//! A field nobody has recorded yet reads as `null`, so a recording made before a field
//! existed still exports, with that column empty, and wiring a new field is an edit to this
//! table alone.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

/// A transmission as the sender put it on the air.
pub const TX: &str = "node.tx";
/// A reception attempt at one receiver's PHY.
pub const PHY_RX: &str = "phy.rx";
/// The record contract's per-reception security record.
pub const RX_SEC: &str = "node.rx_sec";
/// The verification record the engine already writes; read when [`RX_SEC`] is absent.
pub const VERIFY: &str = "node.verify";
/// The receiver's channel busy ratio, once a second.
pub const CBR: &str = "mac.cbr";
/// A report the MA decrypted.
pub const MA_RECEIVED: &str = "ma.received";
/// A report's life in its reporter's store and on its way up.
pub const REPORT_FATE: &str = "pki.report_fate";
/// A report its reporter built for upload: the digest, size, path and evidence of the
/// bytes that left the device. Its fields are named as [`REPORT_FATE`]'s, and the writer
/// reads it with the `fate.*` columns.
pub const REPORT_BUILT: &str = "node.report_built";
/// An MA decision.
pub const DECISION: &str = "ma.decision";
/// A CRL entry the MA caused (the record contract's channel).
pub const CRL_ENTRY: &str = "ma.crl_entry";
/// The revocation pipeline's stage records; its `issued` stage stands in for
/// [`CRL_ENTRY`] when that channel is absent.
pub const REVOCATION: &str = "proto.revocation";
/// The OBU's own credential events.
pub const PKI_EVENT: &str = "node.pki_event";
/// The true state at each transmission (ground truth).
pub const TX_TRUTH: &str = "gt.tx_truth";
/// One record per device (ground truth).
pub const DEVICE: &str = "gt.device";
/// A device's exposure window and linkage value (ground truth).
pub const EXPOSURE: &str = "gt.device_exposure";
/// Certificate to actor (ground truth).
pub const SUBJECT: &str = "gt.subject";
/// One falsification by an attacker (ground truth).
pub const ATTACK: &str = "gt.attack.action";
/// True kinematics, used here only for the actor-to-node map.
pub const KINEMATICS: &str = "gt.kinematics";
/// An application warning.
pub const APP_WARNING: &str = "app.warning";
/// An OBU's credential store, with each certificate's COER octets.
pub const CREDENTIALS: &str = "node.credentials";

/// One row of the mapping table.
#[derive(Debug, Clone, Copy)]
pub struct Source {
    /// `table.column`, e.g. `local/rx.msg_cnt`.
    pub column: &'static str,
    /// The channel the value comes from.
    pub channel: &'static str,
    /// Dotted field paths, `|`-separated, tried in order.
    pub paths: &'static str,
}

const fn s(column: &'static str, channel: &'static str, paths: &'static str) -> Source {
    Source {
        column,
        channel,
        paths,
    }
}

/// The table. Column keys are `<record kind>.<column>`; a record kind is the channel's
/// role in the writer (`tx`, `rx`, `sec`, ...), not a release table, because one recorded
/// value feeds several tables.
pub const SOURCES: &[Source] = &[
    // ---- node.tx: the on-air content (for an attacker, the falsified values) ----------
    s("tx.t_ns", TX, "t"),
    s("tx.node", TX, "node"),
    s("tx.msg", TX, "msg"),
    s("tx.msg_type", TX, "msg_type"),
    s("tx.spdu_digest", TX, "content.spdu_digest|spdu_digest"),
    s("tx.digest_basis", TX, "content.digest_basis|digest_basis"),
    s("tx.cert_id", TX, "cert_id|pseudonym"),
    s("tx.signer_kind", TX, "signer"),
    s("tx.issuer_id", TX, "issuer_id"),
    s("tx.psid", TX, "content.psid|psid"),
    s("tx.size_bytes", TX, "size_bytes|bytes_on_wire"),
    s(
        "tx.msg_cnt",
        TX,
        "msg_cnt|content.msg_cnt|content.msg_count",
    ),
    s("tx.temp_id", TX, "temp_id|content.temp_id"),
    s("tx.sec_mark_ms", TX, "sec_mark_ms|content.sec_mark_ms"),
    s(
        "tx.generation_time_us",
        TX,
        "content.generation_time_us|generation_time_us",
    ),
    s("tx.lat_deg", TX, "lat_deg|content.lat_deg"),
    s("tx.lon_deg", TX, "lon_deg|content.lon_deg"),
    s("tx.x_m", TX, "x_m|content.claimed_x_m"),
    s("tx.y_m", TX, "y_m|content.claimed_y_m"),
    s("tx.elevation_m", TX, "elevation_m|content.elev_m"),
    s(
        "tx.pos_semi_major_m",
        TX,
        "content.pos_semi_major_m|pos_accuracy.semi_major_m",
    ),
    s(
        "tx.pos_semi_minor_m",
        TX,
        "content.pos_semi_minor_m|pos_accuracy.semi_minor_m",
    ),
    s(
        "tx.pos_orientation_deg",
        TX,
        "content.pos_orientation_deg|pos_accuracy.orientation_deg",
    ),
    s("tx.speed_mps", TX, "speed_mps|content.speed_mps"),
    s("tx.heading_deg", TX, "heading_deg|content.heading_deg"),
    s(
        "tx.accel_long_mps2",
        TX,
        "content.accel_long_mps2|accel.long_mps2",
    ),
    s(
        "tx.accel_lat_mps2",
        TX,
        "content.accel_lat_mps2|accel.lat_mps2",
    ),
    s(
        "tx.accel_vert_mps2",
        TX,
        "content.accel_vert_mps2|accel.vert_mps2",
    ),
    s(
        "tx.yaw_rate_dps",
        TX,
        "content.yaw_rate_dps|accel.yaw_rate_dps",
    ),
    s("tx.brakes", TX, "content.brakes|brakes"),
    s("tx.transmission", TX, "content.transmission|transmission"),
    s("tx.event_flags", TX, "content.event_flags|event_flags"),
    s("tx.length_m", TX, "content.length_m|length_m"),
    s("tx.width_m", TX, "content.width_m|width_m"),
    // ---- phy.rx -----------------------------------------------------------------------
    s("rx.t_ns", PHY_RX, "t_end"),
    s("rx.tx", PHY_RX, "tx"),
    s("rx.rx", PHY_RX, "rx"),
    s("rx.msg", PHY_RX, "msg"),
    s("rx.rssi_dbm", PHY_RX, "rssi_dbm"),
    s("rx.outcome", PHY_RX, "outcome"),
    s("rx.dist_m", PHY_RX, "dist_m"),
    // ---- reception security: node.rx_sec, else node.verify -----------------------------
    s("sec.rx", RX_SEC, "rx|node"),
    s("sec.msg", RX_SEC, "msg"),
    s("sec.spdu_digest", RX_SEC, "spdu_digest"),
    s("sec.cert_id", RX_SEC, "cert_id"),
    s("sec.verify_result", RX_SEC, "verify_result"),
    s("sec.verify_latency_us", RX_SEC, "verify_latency_us"),
    s("sec.cbr", RX_SEC, "cbr"),
    s("verify.rx", VERIFY, "rx|node"),
    s("verify.msg", VERIFY, "msg"),
    s("verify.verify_result", VERIFY, "verify_result|outcome"),
    s(
        "verify.verify_latency_us",
        VERIFY,
        "verify_latency_us|cost_us",
    ),
    s("verify.cbr", VERIFY, "cbr"),
    // ---- mac.cbr ----------------------------------------------------------------------
    s("cbr.node", CBR, "node"),
    s("cbr.cbr", CBR, "cbr"),
    // ---- ma.received --------------------------------------------------------------------
    s("mar.t_ns", MA_RECEIVED, "t"),
    s("mar.internal_id", MA_RECEIVED, "report_id"),
    s("mar.report_id", MA_RECEIVED, "report_digest"),
    s("mar.ma_id", MA_RECEIVED, "ma"),
    s("mar.psid", MA_RECEIVED, "psid"),
    s("mar.reporter_cert_id", MA_RECEIVED, "reporter_cert"),
    s("mar.subject_cert_id", MA_RECEIVED, "subject_cert"),
    s("mar.subject_issuer", MA_RECEIVED, "issuer"),
    s("mar.mbr_type", MA_RECEIVED, "mbr_type"),
    s("mar.subtype_codes", MA_RECEIVED, "subtype_codes"),
    s("mar.subtype_names", MA_RECEIVED, "subtypes"),
    s(
        "mar.nonstandard_triggers",
        MA_RECEIVED,
        "nonstandard_triggers",
    ),
    s("mar.generation_time_us", MA_RECEIVED, "generation_time_us"),
    s(
        "mar.generation_location",
        MA_RECEIVED,
        "generation_location",
    ),
    s("mar.evidence", MA_RECEIVED, "evidence"),
    s("mar.evidence_start_ns", MA_RECEIVED, "evidence_start"),
    s("mar.evidence_end_ns", MA_RECEIVED, "evidence_end"),
    s("mar.excess", MA_RECEIVED, "excess"),
    s("mar.set_aside", MA_RECEIVED, "set_aside"),
    s("mar.la_link", MA_RECEIVED, "la_link"),
    s("mar.la_link_t_ns", MA_RECEIVED, "la_link_t"),
    // ---- pki.report_fate ----------------------------------------------------------------
    s("fate.t_ns", REPORT_FATE, "t"),
    s("fate.internal_id", REPORT_FATE, "report_id"),
    s("fate.report_id", REPORT_FATE, "report_digest"),
    s("fate.reporter", REPORT_FATE, "reporter"),
    s("fate.fate", REPORT_FATE, "fate"),
    s("fate.weight", REPORT_FATE, "weight"),
    s("fate.subject_cert_id", REPORT_FATE, "subject"),
    s("fate.size_bytes", REPORT_FATE, "size_bytes"),
    s("fate.path", REPORT_FATE, "path"),
    s(
        "fate.evidence_spdu_digests",
        REPORT_FATE,
        "evidence_spdu_digests",
    ),
    s("fate.truth_subject_node", REPORT_FATE, "truth.subject_node"),
    s("fate.truth_forged", REPORT_FATE, "truth.forged"),
    s("fate.checks", REPORT_FATE, "checks"),
    s("fate.score", REPORT_FATE, "score"),
    // ---- ma.decision --------------------------------------------------------------------
    s("dec.t_ns", DECISION, "t"),
    s("dec.ma_id", DECISION, "ma"),
    s("dec.rule", DECISION, "rule"),
    s("dec.subject_kind", DECISION, "subject_kind"),
    s("dec.subject_cert_id", DECISION, "subject"),
    s("dec.la_link", DECISION, "la_link"),
    s("dec.decision", DECISION, "decision"),
    s("dec.report_ids", DECISION, "report_digests"),
    s("dec.n_reports", DECISION, "n_reports"),
    s("dec.n_distinct_reporters", DECISION, "n_distinct_reporters"),
    // ---- CRL: ma.crl_entry, else proto.revocation's `issued` stage ---------------------
    s("crl.t_issued_ns", CRL_ENTRY, "t_issued|t"),
    s("crl.crl_series", CRL_ENTRY, "crl_series"),
    s("crl.crl_number", CRL_ENTRY, "crl_number"),
    s("crl.entry_kind", CRL_ENTRY, "entry_kind"),
    s("crl.la_link", CRL_ENTRY, "la_link"),
    s("crl.i_period", CRL_ENTRY, "i_period"),
    s("crl.t_effective_ns", CRL_ENTRY, "t_effective"),
    s("crl.hash_id", CRL_ENTRY, "hash_id|cert_id"),
    s("rev.stage", REVOCATION, "stage"),
    s("rev.t_issued_ns", REVOCATION, "t_issued|t"),
    s("rev.crl_series", REVOCATION, "crl_series"),
    s("rev.crl_number", REVOCATION, "crl_number"),
    s("rev.entry_kind", REVOCATION, "entry_kind"),
    s("rev.la_link", REVOCATION, "la_link"),
    s("rev.i_period", REVOCATION, "i_period"),
    s("rev.t_effective_ns", REVOCATION, "t_effective"),
    s("rev.hash_id", REVOCATION, "hash_id|id"),
    // ---- node.pki_event -------------------------------------------------------------------
    s("pki.t_ns", PKI_EVENT, "t"),
    s("pki.node", PKI_EVENT, "node"),
    s("pki.event", PKI_EVENT, "event"),
    s("pki.cert_id", PKI_EVENT, "cert_id"),
    s("pki.crl_series", PKI_EVENT, "crl_series"),
    s("pki.crl_number", PKI_EVENT, "crl_number"),
    s("pki.crl_entries", PKI_EVENT, "crl_entries"),
    s("pki.bytes", PKI_EVENT, "bytes"),
    s("pki.latency_ms", PKI_EVENT, "latency_ms"),
    // ---- ground truth -------------------------------------------------------------------
    s("txt.msg", TX_TRUTH, "msg"),
    s("txt.node", TX_TRUTH, "node"),
    s("txt.t_ns", TX_TRUTH, "t"),
    s("txt.x_m", TX_TRUTH, "x_m"),
    s("txt.y_m", TX_TRUTH, "y_m"),
    s("txt.speed_mps", TX_TRUTH, "speed_mps"),
    s("txt.heading_deg", TX_TRUTH, "heading_deg"),
    s("txt.accel_mps2", TX_TRUTH, "accel_mps2"),
    s("txt.gnss_error_m", TX_TRUTH, "gnss_error_m"),
    s("txt.fix_to_stamp_ms", TX_TRUTH, "fix_to_stamp_ms"),
    s("dev.node", DEVICE, "node"),
    s("dev.class", DEVICE, "class"),
    s("dev.provider", DEVICE, "provider"),
    s("dev.gnss_class", DEVICE, "gnss_class"),
    s("dev.faulty", DEVICE, "faulty"),
    s("dev.fault_kind", DEVICE, "fault_kind"),
    s("dev.attacker", DEVICE, "attacker"),
    s("dev.attack_id", DEVICE, "attack_id"),
    s("dev.scheduled_attack", DEVICE, "scheduled_attack"),
    s("cred.certificates", CREDENTIALS, "certificates"),
    s("exp.node", EXPOSURE, "node"),
    s("exp.la_link", EXPOSURE, "la_link"),
    s("exp.joined_ns", EXPOSURE, "joined"),
    s("exp.left_ns", EXPOSURE, "left"),
    s("exp.attacker", EXPOSURE, "attacker"),
    s("exp.attack_start_ns", EXPOSURE, "attack_start"),
    s("subj.cert_id", SUBJECT, "subject"),
    s("subj.actor", SUBJECT, "actor"),
    s("atk.t_ns", ATTACK, "t"),
    s("atk.actor", ATTACK, "actor"),
    s("atk.attacker", ATTACK, "attacker"),
    s("atk.msg", ATTACK, "msg"),
    s("atk.fields", ATTACK, "fields"),
    s("atk.magnitude", ATTACK, "magnitude"),
    s("atk.params", ATTACK, "params"),
    s("kin.actor", KINEMATICS, "actor"),
    s("kin.node", KINEMATICS, "node"),
    s("app.t_ns", APP_WARNING, "t"),
    s("app.node", APP_WARNING, "node"),
    s("app.app", APP_WARNING, "app"),
    s("app.subject_cert_id", APP_WARNING, "subject"),
    s("app.severity", APP_WARNING, "severity"),
    s("app.fired", APP_WARNING, "fired"),
    s("app.msg", APP_WARNING, "msg"),
    s("app.reaction", APP_WARNING, "driver_reaction|reaction"),
];

fn index() -> &'static BTreeMap<&'static str, &'static Source> {
    static INDEX: OnceLock<BTreeMap<&'static str, &'static Source>> = OnceLock::new();
    INDEX.get_or_init(|| SOURCES.iter().map(|s| (s.column, s)).collect())
}

fn path_get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    if cur.is_null() { None } else { Some(cur) }
}

/// The value of `column` in record `v`, or `Value::Null`.
///
/// # Panics
/// If `column` is not in [`SOURCES`]: a column the writer reads must be in the table, and
/// a typo is a build defect the unit tests catch on the first call.
#[must_use]
pub fn pick(column: &str, v: &Value) -> Value {
    let Some(src) = index().get(column) else {
        panic!("release column {column:?} has no row in dataset/release/sources.rs");
    };
    src.paths
        .split('|')
        .find_map(|p| path_get(v, p))
        .cloned()
        .unwrap_or(Value::Null)
}

/// [`pick`] as an integer (floats are rounded toward zero, strings parsed).
#[must_use]
pub fn int(column: &str, v: &Value) -> Option<i64> {
    as_int(&pick(column, v))
}

/// [`pick`] as a float.
#[must_use]
pub fn float(column: &str, v: &Value) -> Option<f64> {
    pick(column, v).as_f64()
}

/// [`pick`] as a string (numbers are spelled out).
#[must_use]
pub fn string(column: &str, v: &Value) -> Option<String> {
    as_str(&pick(column, v))
}

#[allow(clippy::cast_possible_truncation)]
fn f64_to_i64(f: f64) -> i64 {
    f as i64
}

/// A JSON value as an integer, if it is one.
#[must_use]
pub fn as_int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().map(|u| u as i64))
            .or_else(|| n.as_f64().map(f64_to_i64)),
        Value::Bool(b) => Some(i64::from(*b)),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// A JSON value as a string, if it is a scalar.
#[must_use]
pub fn as_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The channels a source row reads, sorted and deduplicated.
#[must_use]
pub fn channels() -> Vec<&'static str> {
    let mut out: Vec<&str> = SOURCES.iter().map(|s| s.channel).collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_recorded_path_wins_and_a_missing_one_is_null() {
        let v = serde_json::json!({"pseudonym": "aa", "content": {"msg_count": 7}});
        assert_eq!(pick("tx.cert_id", &v), Value::String("aa".into()));
        let v2 = serde_json::json!({"cert_id": "bb", "pseudonym": "aa"});
        assert_eq!(pick("tx.cert_id", &v2), Value::String("bb".into()));
        assert_eq!(int("tx.msg_cnt", &v), Some(7));
        assert_eq!(pick("tx.spdu_digest", &v), Value::Null);
    }

    #[test]
    fn column_keys_are_unique() {
        let mut keys: Vec<&str> = SOURCES.iter().map(|s| s.column).collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n, "a release column is mapped twice");
    }
}
