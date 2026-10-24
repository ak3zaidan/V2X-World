//! The SM-MBR v1.0 §2.2 on-device report store, measured from its fate records
//! (`security.scms.report_records`; paper plan P1-E6's acceptance checks):
//!
//! * an attack-free run loses no attack evidence (there is none to lose, and the
//!   accounting must not invent any);
//! * a zero-quota run loses all of it: every report is deleted as it is stored, so none
//!   reaches an MA. Fault: let a connected vehicle send a new report without storing it
//!   first (the engine before this change, `Engine::route_report` without
//!   `Phase2::admit_report`), and attack evidence is delivered;
//! * the default store (64 KiB) loses none of it in a short run and delivers some;
//! * the records are off by default, so recordings that do not ask keep their digests.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::report_records::{DeviceExposure, Fate, MaReceived, ReportFate};
use v2xw_engine::{Engine, RunRecorder, Scenario};

struct Out {
    fates: Vec<ReportFate>,
    received: Vec<MaReceived>,
    exposure: Vec<DeviceExposure>,
    channels: BTreeSet<String>,
}

fn doc(attack: bool, records: bool, lifecycle: Value) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(root.join("scenarios").join("credential-lifecycle.yaml"))
        .expect("the shipped scenario loads");
    let mut d = serde_json::to_value(&s).expect("serialises");
    d["time"]["duration_s"] = json!(200.0);
    if attack {
        d["threats"]["attackers"][0]["fraction"] = json!(0.2);
    } else {
        d["threats"]["attackers"] = json!([]);
    }
    // The PKI view is not what these tests read; off keeps the run lean.
    let mut scms = json!({"lifecycle": lifecycle, "pki_view": false});
    if records {
        scms["report_records"] = json!(true);
    }
    d["security"]["scms"] = scms;
    d
}

fn run(d: Value) -> Out {
    let s = Scenario::from_document(d).expect("deserialises");
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut out = Out {
        fates: Vec::new(),
        received: Vec::new(),
        exposure: Vec::new(),
        channels: BTreeSet::new(),
    };
    engine.run(&mut out).expect("runs");
    out
}

/// Keeps only the report records (a run's whole recording does not fit the test's
/// memory budget four times over).
impl RunRecorder for Out {
    fn write(&mut self, _at: SimTime, r: &OwnedRecord) {
        if !self.channels.contains(&*r.channel) {
            self.channels.insert(r.channel.to_string());
        }
        match &*r.channel {
            "pki.report_fate" => self
                .fates
                .push(serde_json::from_slice(&r.json).expect("fate")),
            "ma.received" => self
                .received
                .push(serde_json::from_slice(&r.json).expect("rx")),
            "gt.device_exposure" => {
                self.exposure
                    .push(serde_json::from_slice(&r.json).expect("exposure"));
            }
            _ => {}
        }
    }
}

/// Each report's fates in order, by report id.
fn lives(o: &Out) -> BTreeMap<&str, Vec<&ReportFate>> {
    let mut m: BTreeMap<&str, Vec<&ReportFate>> = BTreeMap::new();
    for f in &o.fates {
        m.entry(f.report_id.as_str()).or_default().push(f);
    }
    m
}

fn lost(f: Fate) -> bool {
    matches!(
        f,
        Fate::EvictedQuota | Fate::DecayedOut | Fate::Expired | Fate::LeftWithDevice
    )
}

/// `(attack reports stored, of them lost on the device, of them delivered)`.
fn attack_evidence(o: &Out) -> (usize, usize, usize) {
    let mut stored = 0;
    let mut gone = 0;
    let mut delivered = 0;
    for life in lives(o).values() {
        if !life[0].truth.subject_attacker {
            continue;
        }
        assert_eq!(life[0].fate, Fate::Stored, "a report's life starts stored");
        stored += 1;
        gone += usize::from(life.iter().any(|f| lost(f.fate)));
        delivered += usize::from(life.iter().any(|f| f.fate == Fate::Delivered));
    }
    (stored, gone, delivered)
}

fn attacked() -> &'static Out {
    static RUN: OnceLock<Out> = OnceLock::new();
    RUN.get_or_init(|| run(doc(true, true, json!({}))))
}

#[test]
fn an_attack_free_run_loses_no_attack_evidence() {
    let o = run(doc(false, true, json!({})));
    let (stored, gone, _) = attack_evidence(&o);
    println!(
        "attack-free: {} fates, {} at the MA, {} devices",
        o.fates.len(),
        o.received.len(),
        o.exposure.len()
    );
    assert_eq!((stored, gone), (0, 0));
    assert!(!o.exposure.is_empty(), "no device exposure recorded");
    assert!(o.exposure.iter().all(|e| !e.attacker));
}

#[test]
fn a_zero_quota_run_loses_all_attack_evidence() {
    let o = run(doc(true, true, json!({"report_store_quota_bytes": 0})));
    let (stored, gone, delivered) = attack_evidence(&o);
    println!("zero quota: attack reports {stored}, lost {gone}, delivered {delivered}");
    assert!(stored > 0, "no attack report was generated");
    assert_eq!(gone, stored, "a report survived a zero quota");
    assert_eq!(
        delivered, 0,
        "attack evidence reached an MA through a zero quota"
    );
    assert!(
        o.received.is_empty(),
        "the MA received {} reports",
        o.received.len()
    );
}

#[test]
fn the_default_store_keeps_and_delivers_attack_evidence() {
    let o = attacked();
    let (stored, gone, delivered) = attack_evidence(o);
    println!("default: attack reports {stored}, lost {gone}, delivered {delivered}");
    assert!(stored > 0);
    assert_eq!(gone, 0, "the 64 KiB store lost attack evidence in 200 s");
    assert!(delivered > 0, "no attack evidence reached an MA");
    // Every delivery is an MA reception of the same report, and vice versa.
    let rx: std::collections::BTreeSet<&str> =
        o.received.iter().map(|r| r.report_id.as_str()).collect();
    let dl: std::collections::BTreeSet<&str> = o
        .fates
        .iter()
        .filter(|f| f.fate == Fate::Delivered)
        .map(|f| f.report_id.as_str())
        .collect();
    assert_eq!(rx, dl);
    assert!(
        o.exposure
            .iter()
            .any(|e| e.attacker && e.attack_start.is_some())
    );
}

#[test]
fn the_report_records_are_off_by_default() {
    let o = run(doc(true, false, json!({})));
    for c in ["pki.report_fate", "ma.received", "gt.device_exposure"] {
        assert!(!o.channels.iter().any(|x| x == c), "{c} recorded unasked");
    }
}
