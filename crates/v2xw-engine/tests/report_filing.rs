//! OF-2 (docs/design/scms-real/testing/oracle-final.md): under the US SCMS with the PDU layer
//! on, a misbehaviour report upload *is* its bytes, a SecuredMisbehaviorReport POSTed with an
//! enrolment-signed `Download-Req` ([SM-MBR] §3–4). On the conformance run 1,695 of 2,991
//! cellular uploads were 1,119-octet messages with no PDU: detections the device could not
//! build a report for (no v1.1 subtype, too few of the accused's BSMs heard) still went to
//! the MA on the modelled path.
//!
//! On `scenarios/scms-oracle-repro.yaml` (the conformance run's first 640 s, the research
//! detector suite, which has checks with no v1.1 subtype):
//!
//! * by default, every device upload carries its SPDUs, and the detections that cannot be
//!   filed are counted by reason in `reports_not_filed` and never reach the access link;
//! * `modelled_reports: 1` (the research setting, not the US deployment) puts them back on
//!   the modelled path, sized only, and files none away.

use std::path::Path;

use serde_json::Value;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

/// Device report uploads on `pki.flow`: (cellular with SPDUs, cellular without, other
/// transports without). A roadside unit's own reports ride its backhaul with no SPDU
/// (R-UNSEC: it holds no pseudonym to sign one with), so the third count is bounded by
/// `reports_from_rsus` rather than zero.
fn uploads(rec: &MemoryRecorder) -> (u64, u64, u64) {
    let (mut with, mut without, mut other) = (0, 0, 0);
    for (_, r) in rec.records() {
        if r.channel != "pki.flow" {
            continue;
        }
        let m: Value = serde_json::from_slice(&r.json).expect("json");
        if m["kind"] != "msg"
            || m["name"] != "report"
            || !m["from"].as_str().is_some_and(|f| f.starts_with("ee/"))
        {
            continue;
        }
        let bytes = m["pdus"].as_array().is_some_and(|a| !a.is_empty());
        match (m["transport"] == "cellular-uu", bytes) {
            (true, true) => with += 1,
            (true, false) => without += 1,
            (false, false) => other += 1,
            (false, true) => {}
        }
    }
    (with, without, other)
}

fn run(modelled: bool) -> (RunReport, MemoryRecorder) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/scms-oracle-repro.yaml")).expect("loads");
    if modelled {
        let p = s.security.protocol.as_mut().expect("an SCMS protocol");
        p.params["modelled_reports"] = serde_json::json!(1.0);
    }
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    (report, rec)
}

#[test]
fn a_report_the_device_cannot_build_is_never_uploaded() {
    let (r, rec) = run(false);
    let p = &r.phase2;
    let (with, without, other) = uploads(&rec);
    println!(
        "cellular uploads with SPDUs {with}, without {without}; others without {other} \
         (units' own {}); not filed {} {:?}; real {}; unmapped {}; unsecured {:?}",
        p.reports_from_rsus,
        p.reports_not_filed,
        p.reports_not_filed_by,
        p.reports_mbr_real,
        p.reports_mbr_unmapped,
        p.reports_mbr_unsecured_by,
    );
    assert!(with > 0, "no real report was uploaded");
    assert_eq!(without, 0, "{without} cellular uploads carry no SPDU");
    assert!(
        other <= p.reports_from_rsus,
        "{other} sized-only uploads on the roadside path, more than the units' own {}",
        p.reports_from_rsus
    );
    assert!(
        p.reports_not_filed > 0,
        "the research suite's unmappable detections were not counted as not filed"
    );
    assert_eq!(
        p.reports_not_filed_by.values().sum::<u64>(),
        p.reports_not_filed,
        "every report not filed has its reason"
    );
    // What still takes the modelled path is the units' own reports (R-UNSEC), never a
    // vehicle's.
    assert!(
        p.reports_mbr_unmapped + p.reports_mbr_unsecured <= p.reports_from_rsus,
        "{} unmapped and {} unsecured reports went out modelled, more than the units' own {}",
        p.reports_mbr_unmapped,
        p.reports_mbr_unsecured,
        p.reports_from_rsus
    );
}

#[test]
fn the_research_setting_keeps_the_modelled_path_and_files_nothing_away() {
    let (r, rec) = run(true);
    let p = &r.phase2;
    let (with, without, _) = uploads(&rec);
    println!(
        "modelled_reports: cellular uploads with SPDUs {with}, without {without}; \
         not filed {}; unmapped {}",
        p.reports_not_filed, p.reports_mbr_unmapped
    );
    assert!(with > 0, "no real report was uploaded");
    assert!(
        without > 0,
        "the modelled path carried nothing over cellular"
    );
    assert_eq!(p.reports_not_filed, 0, "a report was filed away");
    assert!(
        p.reports_mbr_unmapped > 0,
        "no unmappable report went out modelled"
    );
}

/// R-KEMFILE (docs/validation/live-scms-2026-10-25.md, finding 2): a post-quantum KEM
/// turns the 1609.2.1 PDU layer off, and `Phase2::real_mbr` returned `Off` before the US
/// profile's filing gates, so every detection was uploaded on the modelled path. The
/// SM-MBR filing rules (a v1.1 subtype must map the checks; no roadside reporter) do not
/// depend on the encryption: on the 180 s live KEM variant (ConstPos from 60 s) the
/// ML-KEM-768 run must refuse what the US profile refuses. Before the fix its
/// `reports_not_filed` was 0 (255 in the classical base). 100 s is too short: the base
/// binary sent one report by then.
#[test]
fn a_post_quantum_kem_keeps_the_us_filing_rules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let s = Scenario::load(root.join("scenarios/live-scms/kem.yaml")).expect("loads");
    assert_eq!(s.security.kem, "ml-kem-768");
    let mut engine = Engine::build(s, "").expect("builds");
    let r = engine
        .run(&mut v2xw_engine::NullRecorder::new())
        .expect("runs");
    let p = &r.phase2;
    println!(
        "kem: sent {} received {} not filed {} {:?}",
        p.reports_sent, p.reports_received, p.reports_not_filed, p.reports_not_filed_by
    );
    assert!(
        p.reports_not_filed > 0,
        "under ML-KEM-768 no detection was refused: the US filing gates were skipped"
    );
    assert!(
        p.reports_not_filed_by
            .keys()
            .any(|k| k.contains("subtype") || k.contains("roadside")),
        "{:?}",
        p.reports_not_filed_by
    );
}
