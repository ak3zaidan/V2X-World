//! The device-class features the Misbehaviour Authority reads from what it already
//! receives (`ma.received`; paper plan 2026-10 §1.4 item F) and the evasion attacker that
//! writes them (`threat/attacker/evasion/BenignClassMimic`, P1-E8):
//!
//! * the secMark phase is the judged BSM's own `secMark`, so an all-locked fleet's
//!   reports read locked. Fault: derive it from the 1609.2 `generationTime` (the engine
//!   before this change), whose 27 ms header jitter scatters a locked device's phase;
//! * a mimic's reports read free, at the accuracy it states;
//! * a mimic with no disguise is its payload: the MA receives exactly what it receives
//!   from the legacy attacker.

use std::path::Path;

use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::report_records::{DeviceExposure, MaReceived};
use v2xw_engine::{Engine, RunRecorder, Scenario};

#[derive(Default)]
struct Out {
    received: Vec<MaReceived>,
    exposure: Vec<DeviceExposure>,
}

impl RunRecorder for Out {
    fn write(&mut self, _at: SimTime, r: &OwnedRecord) {
        match &*r.channel {
            "ma.received" => self
                .received
                .push(serde_json::from_slice(&r.json).expect("ma.received parses")),
            "gt.device_exposure" => self
                .exposure
                .push(serde_json::from_slice(&r.json).expect("exposure parses")),
            _ => {}
        }
    }
}

fn doc(attacker: Value) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(root.join("scenarios").join("credential-lifecycle.yaml"))
        .expect("the shipped scenario loads");
    let mut d = serde_json::to_value(&s).expect("serialises");
    d["time"]["duration_s"] = json!(200.0);
    d["threats"]["attackers"] = json!([attacker]);
    d["security"]["scms"] = json!({"report_records": true, "pki_view": false});
    // Every OBU locked (the built-in provisional classes, 27 ms header jitter).
    d["actors"]["vehicles"]["bsm_timing"] = json!({"enabled": true, "locked_share": 1.0});
    d
}

fn attacker(id: &str, params: Value) -> Value {
    json!({"id": id, "fraction": 0.2, "params": params,
           "schedule": {"from_s": 60.0, "to_s": 300.0}})
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

/// The reports about attackers, as the MA received them.
fn about_attackers(o: &Out) -> Vec<&MaReceived> {
    let links: std::collections::BTreeSet<&str> = o
        .exposure
        .iter()
        .filter(|e| e.attacker)
        .map(|e| e.la_link.as_str())
        .collect();
    o.received
        .iter()
        .filter(|r| links.contains(r.la_link.as_str()))
        .collect()
}

fn locked(r: &MaReceived) -> bool {
    r.secmark_phase_ms <= 1 || r.secmark_phase_ms >= 49
}

fn legacy() -> Out {
    run(doc(attacker(
        "threat/attacker/legacy/ConstPos",
        json!(null),
    )))
}

#[test]
fn the_ma_reads_the_evidence_secmark_phase_and_a_mimic_reads_free() {
    let plain = legacy();
    let a = about_attackers(&plain);
    let share = a.iter().filter(|r| locked(r)).count() as f64 / a.len().max(1) as f64;
    println!(
        "legacy, all locked: {} attack reports, {share:.3} locked",
        a.len()
    );
    assert!(a.len() >= 10, "too few attack reports reached the MA");
    assert!(
        share >= 0.8,
        "a locked fleet's reports read free: {share:.3}"
    );

    let mimic = run(doc(attacker(
        "threat/attacker/evasion/BenignClassMimic",
        json!({"payload": "ConstPos", "secmark_shift_ms": 25, "stated_accuracy_m": 9.0}),
    )));
    let m = about_attackers(&mimic);
    let free = m.iter().filter(|r| !locked(r)).count();
    let acc: Vec<f64> = m.iter().map(|r| r.pos_accuracy_m).collect();
    println!(
        "mimic: {} attack reports, {free} free, accuracy {:?}..{:?}",
        m.len(),
        acc.iter().copied().reduce(f64::min),
        acc.iter().copied().reduce(f64::max)
    );
    assert!(!m.is_empty(), "no report about a mimic reached the MA");
    assert_eq!(free, m.len(), "a mimic's report read locked");
    assert!(
        acc.iter().all(|a| (8.0..=10.0).contains(a)),
        "the MA did not read the stated accuracy: {acc:?}"
    );
    // The same devices attack in both runs (the selection draws no attack-id input).
    let who = |o: &Out| -> Vec<String> {
        o.exposure
            .iter()
            .filter(|e| e.attacker)
            .map(|e| e.la_link.clone())
            .collect()
    };
    assert_eq!(who(&plain), who(&mimic));
    // `gt.device_exposure`'s attack start is the first *falsified* message. The disguise
    // only restamps secMark (no claim changes), so it edits bytes on the air from the
    // window's start while its payload lies later: the mimic's attack starts when the
    // plain attacker's does. Fault: read it from the first on-air edit (the engine before
    // 2026-10-27), which starts every mimic's budget 0.1-4 s early (P1-E8 test 2b).
    let starts = |o: &Out| -> Vec<(String, Option<SimTime>)> {
        o.exposure
            .iter()
            .filter(|e| e.attacker)
            .map(|e| (e.la_link.clone(), e.attack_start))
            .collect()
    };
    assert!(
        starts(&plain)
            .iter()
            .any(|(_, s)| s.is_some_and(|s| s > 60 * v2xw_core::time::NS_PER_S)),
        "no attacker's first lie lies after its window opens: the check is vacuous"
    );
    assert_eq!(
        starts(&plain),
        starts(&mimic),
        "a secMark-only disguise moved the attack start"
    );

    // With no disguise the mimic is its payload, report for report.
    let bare = run(doc(attacker(
        "threat/attacker/evasion/BenignClassMimic",
        json!({"payload": "ConstPos"}),
    )));
    assert_eq!(
        plain.received, bare.received,
        "an undisguised mimic changed what the MA received"
    );
}

#[test]
fn a_bad_disguise_is_refused_at_check() {
    for params in [
        json!({"payload": "DoS"}),
        json!({"payload": "Nope"}),
        json!({"secmark_shift_ms": 0}),
        json!({"secmark_shift_ms": 12.5}),
        json!({"stated_accuracy_m": -1.0}),
        json!({"const_pos_offset_m": "x"}),
    ] {
        let d = doc(attacker(
            "threat/attacker/evasion/BenignClassMimic",
            params.clone(),
        ));
        let s = Scenario::from_document(d).expect("deserialises");
        assert!(
            !v2xw_engine::scenario::validate(&s).is_empty(),
            "{params} was accepted"
        );
    }
    let d = doc(attacker("threat/attacker/evasion/Nope", json!(null)));
    let s = Scenario::from_document(d).expect("deserialises");
    assert!(!v2xw_engine::scenario::validate(&s).is_empty());
}
