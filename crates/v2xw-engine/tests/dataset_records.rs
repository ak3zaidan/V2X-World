//! The dataset release's records (`docs/dataset/record-contract.md`,
//! `v2xw_engine::dataset_records`): present and correct on a run that names the `dataset`
//! exporter, absent on one that does not, and joined from the global view (the MA's
//! decoded evidence) to the local one (the sender's `node.tx`) by digests computed from
//! the bytes.
//!
//! Two short runs of the sample scenario (`scenarios/live-atk/ma-legacy.yaml`, scaled
//! down: 16 vehicles, a quarter ConstPos attackers from 4 s, a 2 s RA shuffle and a 5 s
//! CRL cadence so the whole report → decision → CRL chain fits in 30 s), identical but
//! for the exporter list. The run without the exporter must record exactly what the run
//! with it records once the dataset fields are taken out: the gate adds records and never
//! changes the simulation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use serde_json::{Value, json};
use v2xw_core::Visibility;
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::{Engine, RunRecorder, Scenario};

/// What a run recorded, per channel, as parsed JSON (the bulky channels the tests do not
/// read are kept only as a digest).
#[derive(Default)]
struct Out {
    rows: BTreeMap<String, Vec<Value>>,
    /// Per channel, a running digest of every record with the dataset keys taken out.
    stripped: BTreeMap<String, (u64, [u8; 32])>,
    /// Writer-rule violations: a ground-truth-tainted record on a NODE channel, an
    /// unknown channel, an off-grid float.
    refused: Vec<String>,
}

/// Channels the tests read row by row.
const KEEP: &[&str] = &[
    "node.tx",
    "node.rx_sec",
    "node.pki_event",
    "node.report_built",
    "ma.received",
    "ma.decision",
    "ma.crl_entry",
    "pki.report_fate",
    "gt.tx_truth",
    "gt.device",
    "gt.subject",
    "gt.attack.action",
];

/// Every key the dataset gate adds to an existing channel.
const NEW_TX_CONTENT: &[&str] = &[
    "spdu_digest",
    "digest_basis",
    "psid",
    "generation_time_us",
    "accel_lat_mps2",
    "accel_vert_mps2",
    "yaw_rate_dps",
    "brakes",
    "event_flags",
    "length_m",
    "width_m",
    "pos_semi_major_m",
    "pos_semi_minor_m",
    "pos_orientation_deg",
    "accel_long_mps2",
    "transmission",
];
const NEW_MA_RECEIVED: &[&str] = &[
    "report_digest",
    "psid",
    "mbr_type",
    "subtype_codes",
    "nonstandard_triggers",
    "generation_time_us",
    "generation_location",
    "evidence",
    "la_link_t",
];
const NEW_FATE: &[&str] = &[
    "report_digest",
    "size_bytes",
    "path",
    "evidence_spdu_digests",
    "checks",
    "score",
];
const NEW_DECISION: &[&str] = &[
    "ma",
    "rule",
    "subject_kind",
    "la_link",
    "report_digests",
    "n_reports",
    "n_distinct_reporters",
];
/// The channels the gate adds.
const NEW_CHANNELS: &[&str] = &[
    "node.rx_sec",
    "node.pki_event",
    "node.report_built",
    "ma.crl_entry",
    "gt.tx_truth",
    "gt.device",
];

fn strip(channel: &str, v: &mut Value) {
    let Some(m) = v.as_object_mut() else {
        return;
    };
    let keys: &[&str] = match channel {
        "node.tx" => {
            if let Some(c) = m.get_mut("content").and_then(Value::as_object_mut) {
                for k in NEW_TX_CONTENT {
                    c.remove(*k);
                }
            }
            &[]
        }
        "ma.received" => NEW_MA_RECEIVED,
        "pki.report_fate" => NEW_FATE,
        "ma.decision" => NEW_DECISION,
        _ => &[],
    };
    for k in keys {
        m.remove(*k);
    }
}

impl RunRecorder for Out {
    fn write(&mut self, _at: SimTime, r: &OwnedRecord) {
        let channel = r.channel.to_string();
        match v2xw_record::channels::by_name(&channel) {
            None => self
                .refused
                .push(format!("{channel}: not in the channel table")),
            Some(spec) => {
                if spec.visibility.allowed_on_node_channel() && r.visibility.is_gt_tainted() {
                    self.refused.push(format!(
                        "{channel}: a {:?} record on a NODE channel",
                        r.visibility
                    ));
                }
                // A Derived channel (metric.sample) carries each sample with its
                // source's tag; every other channel's records carry the table's tag.
                if spec.visibility != Visibility::Derived && spec.visibility != r.visibility {
                    self.refused.push(format!(
                        "{channel}: record says {:?}, table says {:?}",
                        r.visibility, spec.visibility
                    ));
                }
            }
        }
        if let Err(e) = v2xw_record::grid::scan_record(&channel, &r.json) {
            self.refused.push(format!("{channel}: {e}"));
        }
        let mut v: Value = serde_json::from_slice(&r.json).expect("a record is JSON");
        if KEEP.contains(&channel.as_str()) {
            self.rows
                .entry(channel.clone())
                .or_default()
                .push(v.clone());
        }
        strip(&channel, &mut v);
        let bytes = serde_json::to_vec(&v).expect("serialises");
        let e = self.stripped.entry(channel).or_insert((0, [0; 32]));
        e.0 += 1;
        let mut buf = e.1.to_vec();
        buf.extend_from_slice(&bytes);
        e.1 = v2xw_core::hash::sha256(&buf);
    }
}

fn doc(dataset: bool) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(
        root.join("scenarios")
            .join("live-atk")
            .join("ma-legacy.yaml"),
    )
    .expect("the shipped scenario loads");
    let mut d = serde_json::to_value(&s).expect("serialises");
    d["time"]["duration_s"] = json!(30.0);
    d["actors"]["vehicles"]["demand"]["fleet_size"] = json!(16);
    d["threats"]["attackers"][0]["fraction"] = json!(0.25);
    d["threats"]["attackers"][0]["schedule"] = json!({"from_s": 4.0, "to_s": 150.0});
    let p = &mut d["security"]["protocol"]["params"];
    p["report_shuffle_window_s"] = json!(2);
    p["crl_cadence_s"] = json!(5);
    p["crl_fetch_interval_s"] = json!(5);
    d["security"]["scms"]["report_records"] = json!(true);
    d["security"]["scms"]["pki_view"] = json!(false);
    let mut exporters: Vec<Value> = d["exporters"].as_array().cloned().unwrap_or_default();
    exporters.retain(|x| x["id"] != "dataset");
    if dataset {
        exporters.push(json!({"id": "dataset"}));
    }
    d["exporters"] = Value::Array(exporters);
    d
}

fn run(d: Value) -> Out {
    let s = Scenario::from_document(d).expect("deserialises");
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut out = Out::default();
    engine.run(&mut out).expect("runs");
    out
}

fn with() -> &'static Out {
    static RUN: OnceLock<Out> = OnceLock::new();
    RUN.get_or_init(|| run(doc(true)))
}

fn without() -> &'static Out {
    static RUN: OnceLock<Out> = OnceLock::new();
    RUN.get_or_init(|| run(doc(false)))
}

fn rows<'a>(o: &'a Out, channel: &str) -> &'a [Value] {
    o.rows.get(channel).map_or(&[], Vec::as_slice)
}

fn hex16(v: &Value) -> bool {
    v.as_str()
        .is_some_and(|s| s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Every BSM on the air carries its digest, psid, Time64 stamp and the rest of Part I;
/// every receiver's verdict names the message it judged by the sender's own digest.
#[test]
fn the_local_view_fields_are_recorded_when_the_exporter_is_named() {
    let o = with();
    let tx = rows(o, "node.tx");
    let bsms: Vec<&Value> = tx.iter().filter(|r| r["msg_type"] == "bsm").collect();
    assert!(bsms.len() > 100, "{} BSMs", bsms.len());
    let mut by_msg: BTreeMap<u64, (&str, &str)> = BTreeMap::new();
    for r in &bsms {
        let c = &r["content"];
        assert!(hex16(&c["spdu_digest"]), "{r}");
        assert_eq!(c["digest_basis"], "bytes", "{r}");
        assert_eq!(c["psid"], 0x20, "{r}");
        assert!(
            c["generation_time_us"]
                .as_u64()
                .is_some_and(|t| t > 600_000_000_000_000)
        );
        for k in [
            "brakes",
            "length_m",
            "width_m",
            "yaw_rate_dps",
            "pos_semi_major_m",
            "transmission",
        ] {
            assert!(!c[k].is_null(), "{k} missing: {r}");
        }
        by_msg.insert(
            r["msg"].as_u64().expect("msg"),
            (
                c["spdu_digest"].as_str().unwrap_or_default(),
                r["pseudonym"].as_str().unwrap_or_default(),
            ),
        );
    }
    // Digests are distinct per message (they hash the signed bytes, generation time in).
    let distinct: BTreeSet<&str> = by_msg.values().map(|(d, _)| *d).collect();
    assert_eq!(distinct.len(), by_msg.len(), "two BSMs share a digest");

    let sec = rows(o, "node.rx_sec");
    assert!(sec.len() > bsms.len(), "{} verdicts", sec.len());
    let mut joined = 0;
    for r in sec {
        assert!(r["verify_result"].is_string(), "{r}");
        assert!(
            r.get("tx").is_none(),
            "a NODE verdict names the sender: {r}"
        );
        if let Some((d, cert)) = r["msg"].as_u64().and_then(|m| by_msg.get(&m)) {
            assert_eq!(r["spdu_digest"], *d, "{r}");
            assert_eq!(r["cert_id"], *cert, "{r}");
            joined += 1;
        }
    }
    assert!(joined > 100, "{joined} verdicts joined to a BSM");
    assert!(sec.iter().any(|r| r["verify_result"] == "ok"));
    assert!(sec.iter().any(|r| r["cbr"].as_f64().is_some()));

    // The device's own PKI life starts enrolled, once per device.
    let pki = rows(o, "node.pki_event");
    let enrolled: Vec<&Value> = pki.iter().filter(|r| r["event"] == "enrolled").collect();
    let nodes: BTreeSet<u64> = enrolled.iter().filter_map(|r| r["node"].as_u64()).collect();
    assert_eq!(nodes.len(), enrolled.len(), "a device enrolled twice");
    assert!(!enrolled.is_empty());

    // Ground truth: one gt.tx_truth per transmission, one gt.device per device on the air.
    assert_eq!(rows(o, "gt.tx_truth").len(), tx.len());
    let dev = rows(o, "gt.device");
    let dev_nodes: BTreeSet<u64> = dev.iter().filter_map(|r| r["node"].as_u64()).collect();
    assert_eq!(dev_nodes.len(), dev.len(), "a device listed twice");
    let senders: BTreeSet<u64> = tx.iter().filter_map(|r| r["node"].as_u64()).collect();
    assert_eq!(dev_nodes, senders);
    assert!(dev.iter().any(|r| r["attacker"] == true));
}

/// The global view: each report the MA opened carries its digest, SM-MBR type,
/// subtypes and decoded evidence; the device's own log of the same report carries the
/// same digest; the decisions and CRL entries name the reports and link tokens.
#[test]
fn the_global_view_fields_are_recorded_when_the_exporter_is_named() {
    let o = with();
    let rx = rows(o, "ma.received");
    assert!(!rx.is_empty(), "no report reached the MA");
    let mut digests: BTreeMap<&str, &str> = BTreeMap::new();
    let mut evidence_of: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    let mut windows = 0;
    for r in rx {
        assert_eq!(r["psid"], 0x20, "{r}");
        assert!(hex16(&r["report_digest"]), "{r}");
        assert!(r["mbr_type"].is_string(), "{r}");
        assert!(
            !r["subtype_codes"].as_array().expect("subtypes").is_empty(),
            "{r}"
        );
        assert!(r["nonstandard_triggers"].is_array(), "{r}");
        assert!(r["generation_location"]["lat_deg"].is_number(), "{r}");
        let ev = r["evidence"].as_array().expect("evidence");
        assert!(!ev.is_empty(), "{r}");
        for e in ev {
            assert!(hex16(&e["spdu_digest"]), "{e}");
            assert!(e["generation_time_us"].is_u64(), "{e}");
        }
        // The evidence window spans the BSMs attached, on the sender's clock: two
        // BSMs ~100 ms apart are a window of that length (v1.0: start == end always).
        let gens: Vec<u64> = ev
            .iter()
            .filter_map(|e| e["generation_time_us"].as_u64())
            .collect();
        let (s, e) = (
            r["evidence_start"].as_u64().expect("start"),
            r["evidence_end"].as_u64().expect("end"),
        );
        let span_us = gens.iter().max().unwrap() - gens.iter().min().unwrap();
        assert!(
            (e - s).abs_diff(span_us * 1_000) < 1_000,
            "window {s}..{e} for BSMs {span_us} us apart: {r}"
        );
        windows += usize::from(span_us > 0);
        evidence_of.insert(
            r["report_id"].as_str().expect("id"),
            ev.iter().map(|e| &e["spdu_digest"]).collect(),
        );
        digests.insert(
            r["report_id"].as_str().expect("id"),
            r["report_digest"].as_str().expect("digest"),
        );
    }
    // When each MA received each report (by the report's digest).
    let received_at: BTreeMap<(u64, &str), u64> = rx
        .iter()
        .map(|r| {
            (
                (
                    r["ma"].as_u64().expect("ma"),
                    r["report_digest"].as_str().expect("digest"),
                ),
                r["t"].as_u64().expect("t"),
            )
        })
        .collect();
    let fates = rows(o, "pki.report_fate");
    let mut checked = 0;
    for f in fates.iter().filter(|f| f["fate"] == "delivered") {
        let id = f["report_id"].as_str().expect("id");
        if let Some(d) = digests.get(id) {
            assert_eq!(f["report_digest"], *d, "{f}");
            assert!(f["size_bytes"].as_u64().is_some_and(|b| b > 0), "{f}");
            assert!(f["path"] == "cellular" || f["path"] == "rsu_relay", "{f}");
            // The reporter's own copy of the evidence it attached (local/reports).
            let own: Vec<&Value> = f["evidence_spdu_digests"]
                .as_array()
                .expect("digests")
                .iter()
                .collect();
            assert_eq!(own, evidence_of[id], "{f}");
            checked += 1;
        }
    }
    assert!(checked > 0, "no delivered fate joined to an MA reception");
    assert!(windows > 0, "no report with two evidence BSMs to time");
    // The reporter knows its own checks and score from the moment it stores the report,
    // whether or not the report ever reaches an MA.
    let stored: Vec<&Value> = fates.iter().filter(|f| f["fate"] == "stored").collect();
    assert!(!stored.is_empty());
    for f in &stored {
        assert!(
            f["checks"].as_array().is_some_and(|a| !a.is_empty()),
            "a stored report without its checks: {f}"
        );
        assert!(f["score"].is_number(), "{f}");
    }
    let dec = rows(o, "ma.decision");
    assert!(!dec.is_empty(), "the MA decided nothing in the run");
    for d in dec {
        assert!(d["rule"].as_str().is_some_and(|s| !s.is_empty()), "{d}");
        assert!(d["n_reports"].as_u64().is_some_and(|n| n > 0), "{d}");
        assert!(
            d["n_distinct_reporters"].as_u64() <= d["n_reports"].as_u64(),
            "{d}"
        );
        assert!(hex16(&d["la_link"]), "{d}");
        let known: BTreeSet<&str> = digests.values().copied().collect();
        for x in d["report_digests"].as_array().expect("digests") {
            assert!(known.contains(x.as_str().unwrap_or_default()), "{d}");
            // A decision cites only reports its MA had received by then.
            let at = received_at[&(d["ma"].as_u64().expect("ma"), x.as_str().unwrap())];
            assert!(
                at <= d["t"].as_u64().expect("t"),
                "report at {at} after {d}"
            );
        }
    }
    // The CRL entries in issue order, across providers.
    let issued: Vec<u64> = rows(o, "ma.crl_entry")
        .iter()
        .map(|c| c["t_issued"].as_u64().expect("t_issued"))
        .collect();
    assert!(issued.windows(2).all(|w| w[0] <= w[1]), "{issued:?}");
    for c in rows(o, "ma.crl_entry") {
        assert_eq!(c["entry_kind"], "linkage_seed", "{c}");
        assert!(hex16(&c["la_link"]), "{c}");
        assert!(c["crl_number"].as_u64().is_some_and(|n| n > 0), "{c}");
    }
}

/// Every report that reached the proxy is recorded as its reporter built it
/// (`node.report_built`): digest, size, path and evidence, whether or not it reached an
/// MA before the run ended (first v1.1 build: 1,083 `local/reports` rows uploaded with no
/// report id and no evidence, the reports still in the RA's shuffle at the end). Fault:
/// drop the push in `Phase2::report_at_proxy`.
#[test]
fn every_report_that_left_its_device_is_recorded_as_built() {
    let o = with();
    let built = rows(o, "node.report_built");
    let fates = rows(o, "pki.report_fate");
    let uploaded: BTreeMap<&str, u64> = fates
        .iter()
        .filter(|f| f["fate"] == "uploaded")
        .filter_map(|f| Some((f["report_id"].as_str()?, f["t"].as_u64()?)))
        .collect();
    let delivered: BTreeMap<&str, &Value> = fates
        .iter()
        .filter(|f| f["fate"] == "delivered")
        .filter_map(|f| Some((f["report_id"].as_str()?, f)))
        .collect();
    assert!(!uploaded.is_empty(), "no report was uploaded");
    let mut by_id: BTreeMap<&str, &Value> = BTreeMap::new();
    for b in built {
        let id = b["report_id"].as_str().expect("id");
        assert!(by_id.insert(id, b).is_none(), "a report built twice: {b}");
        assert!(hex16(&b["report_digest"]), "{b}");
        assert!(b["size_bytes"].as_u64().is_some_and(|n| n > 0), "{b}");
        assert!(b["path"] == "cellular" || b["path"] == "rsu_relay", "{b}");
        assert!(
            b["evidence_spdu_digests"]
                .as_array()
                .is_some_and(|a| !a.is_empty()),
            "{b}"
        );
        assert!(b["reporter"].is_u64() && b["subject"].is_string(), "{b}");
        let up = uploaded
            .get(id)
            .unwrap_or_else(|| panic!("built, never uploaded: {b}"));
        assert!(
            b["t"].as_u64().unwrap() >= *up,
            "built before its upload: {b}"
        );
        // A delivered report's fate carries the same bytes.
        if let Some(f) = delivered.get(id) {
            for k in [
                "report_digest",
                "size_bytes",
                "path",
                "evidence_spdu_digests",
            ] {
                assert_eq!(f[k], b[k], "{k}: {f} vs {b}");
            }
        }
    }
    // Every upload made a second or more before the end reached the proxy and was built;
    // and some were built that no MA received (the case the record exists for).
    let end = 30 * 1_000_000_000_u64;
    for (id, t) in &uploaded {
        if *t + 1_000_000_000 < end {
            assert!(
                by_id.contains_key(id),
                "uploaded at {t}, never recorded as built: {id}"
            );
        }
    }
    assert!(
        by_id.keys().any(|id| !delivered.contains_key(id)),
        "every built report reached an MA: the run does not exercise the record"
    );
}

/// A flood's copies are falsified messages too: each copy of a DoSDisruptive beacon (one
/// signing, several frames) has its own `gt.attack.action` naming its message id, so the
/// dataset labels it (first v1.1 recording: three of every four DoSDisruptive frames,
/// and every Sybil ghost, were labelled benign). Fault: drop the ghost frame's record in
/// `ghost_frames`.
fn flood() -> &'static Out {
    static RUN: OnceLock<Out> = OnceLock::new();
    RUN.get_or_init(|| {
        let mut d = doc(true);
        d["time"]["duration_s"] = json!(10.0);
        d["threats"]["attackers"] = json!([{
            "id": "threat/attacker/f2md/DoSDisruptive",
            "fraction": 0.25,
            "schedule": {"from_s": 2.0, "to_s": 150.0}
        }]);
        run(d)
    })
}

/// One signing, several frames: a flood's beacon and its copies share the sender and the
/// signing instant. Each group's `node.tx` rows, in message order (the beacon first).
fn flood_groups(o: &Out) -> Vec<Vec<&Value>> {
    let mut signings: BTreeMap<(u64, u64), Vec<&Value>> = BTreeMap::new();
    for t in rows(o, "node.tx") {
        if let (Some(n), Some(s)) = (t["node"].as_u64(), t["t_sign_start"].as_u64()) {
            signings.entry((n, s)).or_default().push(t);
        }
    }
    let mut groups: Vec<Vec<&Value>> = signings.into_values().filter(|g| g.len() > 1).collect();
    for g in &mut groups {
        g.sort_by_key(|t| t["msg"].as_u64());
    }
    groups
}

#[test]
fn every_copy_of_a_flood_has_its_own_attack_record() {
    let o = flood();
    assert!(o.refused.is_empty(), "{:?}", o.refused);
    let atk: BTreeSet<u64> = rows(o, "gt.attack.action")
        .iter()
        .filter_map(|a| a["msg"].as_u64())
        .collect();
    // One signing, several frames: the beacon and its copies share the sender and the
    // signing instant.
    let mut signings: BTreeMap<(u64, u64), Vec<u64>> = BTreeMap::new();
    for t in rows(o, "node.tx") {
        if let (Some(n), Some(s), Some(m)) = (
            t["node"].as_u64(),
            t["t_sign_start"].as_u64(),
            t["msg"].as_u64(),
        ) {
            signings.entry((n, s)).or_default().push(m);
        }
    }
    let mut copies = 0;
    for ms in signings.values().filter(|ms| ms.len() > 1) {
        for m in ms {
            assert!(
                atk.contains(m),
                "frame {m} of {ms:?} has no gt.attack.action"
            );
        }
        copies += ms.len() - 1;
    }
    assert!(copies > 0, "the flood sent no copies");
}

/// Each DoSDisruptive copy goes on the air stamped at the multiplied rate: copy k of a
/// beacon carries the beacon's 1609.2 generationTime and J2735 secMark plus k quarters of
/// the 100 ms interval (multiplier 4), read from its own octets, and the random
/// neighbour's position it claims, so no two frames of a flood share their bytes (first
/// v1.1 build: every copy carried its beacon's stamps and position, and its digest).
/// Fault: sign the beacon's payload in `ghost_frames`, or drop the clock offset.
#[test]
fn every_copy_of_a_flood_carries_its_own_stamps_on_the_air() {
    let o = flood();
    let groups = flood_groups(o);
    assert!(!groups.is_empty(), "the flood sent no copies");
    let mut copies = 0;
    let mut moved = 0;
    for g in &groups {
        let c0 = &g[0]["content"];
        let gen0 = c0["generation_time_us"]
            .as_u64()
            .expect("the beacon's generationTime");
        let sm0 = c0["sec_mark_ms"].as_u64().expect("the beacon's secMark");
        let digests: BTreeSet<&str> = g
            .iter()
            .filter_map(|t| t["content"]["spdu_digest"].as_str())
            .collect();
        assert_eq!(
            digests.len(),
            g.len(),
            "two frames of one flood share their bytes: {g:?}"
        );
        for (k, t) in g.iter().enumerate().skip(1) {
            let c = &t["content"];
            let k = k as u64;
            assert_eq!(
                c["generation_time_us"].as_u64(),
                Some(gen0 + k * 25_000),
                "copy {k}'s generationTime on the air: {t}"
            );
            assert_eq!(
                c["sec_mark_ms"].as_u64(),
                Some((sm0 + k * 25) % 60_000),
                "copy {k}'s secMark on the air: {t}"
            );
            // The position on the air is the one the copy claims: away from its beacon's
            // claim, away from its beacon's latitude and longitude.
            let dx = c["claimed_x_m"].as_f64().unwrap() - c0["claimed_x_m"].as_f64().unwrap();
            let dy = c["claimed_y_m"].as_f64().unwrap() - c0["claimed_y_m"].as_f64().unwrap();
            if dx.hypot(dy) > 5.0 {
                assert!(
                    c["lat_deg"] != c0["lat_deg"] || c["lon_deg"] != c0["lon_deg"],
                    "copy {k} claims another position but carries its beacon's: {t}"
                );
                moved += 1;
            }
            copies += 1;
        }
    }
    assert!(copies > 10, "{copies} copies");
    assert!(moved > 0, "no copy restated as a neighbour");
    // Its truth names the fields it falsified beyond the repetition.
    let restated = rows(o, "gt.attack.action")
        .iter()
        .filter(|a| {
            let f: Vec<&str> = a["fields"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            f.contains(&"repetitions") && f.contains(&"position")
        })
        .count();
    assert!(
        restated > 0,
        "no copy's record names the position it states"
    );
}

/// The join the dataset exists for: every evidence BSM the MA decoded from a report
/// has the digest its sender recorded on `node.tx`, signed by the accused certificate.
#[test]
fn the_mas_evidence_digests_join_the_senders_records() {
    let o = with();
    let sent: BTreeMap<&str, &str> = rows(o, "node.tx")
        .iter()
        .filter_map(|r| {
            Some((
                r["content"]["spdu_digest"].as_str()?,
                r["pseudonym"].as_str()?,
            ))
        })
        .collect();
    let mut joined = 0;
    for r in rows(o, "ma.received") {
        for e in r["evidence"].as_array().into_iter().flatten() {
            let d = e["spdu_digest"].as_str().expect("digest");
            let signer = sent
                .get(d)
                .unwrap_or_else(|| panic!("evidence {d} is no BSM on the air: {r}"));
            assert_eq!(r["subject_cert"], *signer, "{r}");
            joined += 1;
        }
    }
    assert!(joined > 0, "no evidence to join");
    for f in rows(o, "pki.report_fate") {
        for d in f["evidence_spdu_digests"].as_array().into_iter().flatten() {
            assert!(sent.contains_key(d.as_str().unwrap_or_default()), "{f}");
        }
    }
}

/// The gate: without the exporter none of the new keys or channels is recorded, and
/// every channel holds exactly what the gated run holds with the dataset keys taken out.
#[test]
fn nothing_new_is_recorded_without_the_exporter() {
    let a = with();
    let b = without();
    for c in NEW_CHANNELS {
        assert!(
            !b.stripped.contains_key(*c),
            "{c} recorded without the exporter"
        );
        assert!(
            a.stripped.contains_key(*c),
            "{c} not recorded with the exporter"
        );
    }
    for r in rows(b, "node.tx") {
        for k in NEW_TX_CONTENT {
            assert!(r["content"].get(*k).is_none(), "{k} on node.tx: {r}");
        }
    }
    for (channel, keys) in [
        ("ma.received", NEW_MA_RECEIVED),
        ("pki.report_fate", NEW_FATE),
        ("ma.decision", NEW_DECISION),
    ] {
        for r in rows(b, channel) {
            for k in keys {
                assert!(r.get(*k).is_none(), "{k} on {channel}: {r}");
            }
        }
    }
    let differ: Vec<&String> = b
        .stripped
        .iter()
        .filter(|(c, v)| a.stripped.get(*c) != Some(*v))
        .map(|(c, _)| c)
        .collect();
    assert!(
        differ.is_empty(),
        "the gate changed what these channels record: {differ:?}"
    );
}

/// Every record of both runs obeys the recorder's rules: a registered channel, the
/// visibility tag the channel table gives it (a ground-truth-tainted record is never on a
/// NODE channel), every float on its grid.
#[test]
fn every_record_obeys_the_writer_rules() {
    for o in [with(), without()] {
        assert!(
            o.refused.is_empty(),
            "{:?}",
            &o.refused[..o.refused.len().min(5)]
        );
    }
}

/// No NODE row the dataset reads carries a ground-truth key, at any depth (the leakage
/// linter of `v2xw_record::dataset::leakage`).
#[test]
fn the_node_records_pass_the_leakage_linter() {
    let o = with();
    for channel in [
        "node.tx",
        "node.rx_sec",
        "node.pki_event",
        "node.report_built",
        "ma.received",
        "ma.decision",
        "ma.crl_entry",
    ] {
        let r = rows(o, channel);
        assert!(
            !r.is_empty() || channel == "ma.crl_entry",
            "no {channel} rows"
        );
        let lint = v2xw_record::dataset::leakage::lint_rows(channel, r);
        assert!(lint.is_clean(), "{channel}: {}", lint.summary());
    }
}

/// A dataset run with no metric provider still records each receiver's channel busy
/// ratio (the exporter's `local/rx.cbr`), and the BSM timing classes a calibration
/// declares out of specification label their honest devices `faulty`, with the fault.
#[test]
fn a_dataset_run_without_metrics_records_cbr_and_labels_faulty_classes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let text = std::fs::read_to_string(
        root.join("data")
            .join("calibration")
            .join("dataset-faulty-devices.yaml"),
    )
    .expect("the shipped fault calibration reads");
    // Most devices faulty, so a short run is sure to have both faults.
    let text = text
        .replace("share: 0.25", "share: 0.1")
        .replace("share: 0.60", "share: 0.1")
        .replace("share: 0.075", "share: 0.4");
    let dir = std::env::temp_dir().join(format!("v2xw-ds-faulty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cal = dir.join("faulty.yaml");
    std::fs::write(&cal, text).expect("writes");
    let mut d = doc(true);
    d["metrics"] = json!([]);
    d["time"]["duration_s"] = json!(12.0);
    d["actors"]["vehicles"]["bsm_timing"] =
        json!({"enabled": true, "calibration": cal.to_string_lossy()});
    let o = run(d);
    assert!(o.refused.is_empty(), "{:?}", o.refused);
    assert!(
        rows(&o, "node.rx_sec")
            .iter()
            .any(|r| r["cbr"].as_f64().is_some()),
        "no receiver CBR without a metric provider"
    );
    let dev = rows(&o, "gt.device");
    let mut kinds = BTreeSet::new();
    for r in dev.iter().filter(|r| r["class"] == "vehicle") {
        let class = r["gnss_class"].as_str().expect("a vehicle's timing class");
        let want = match class {
            "faulty-stale-fix" => Some("timestamp_lag"),
            "faulty-gnss-walk" => Some("gnss_error"),
            "locked" | "free" => None,
            other => panic!("unknown class {other}: {r}"),
        };
        let want = want.filter(|_| r["attacker"] != true);
        assert_eq!(r["faulty"], want.is_some(), "{r}");
        assert_eq!(r["fault_kind"].as_str(), want, "{r}");
        kinds.extend(want);
    }
    assert_eq!(kinds.len(), 2, "both faults labelled: {kinds:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
