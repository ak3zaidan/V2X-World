//! Engine defects the live SCMS knob runs found (docs/validation/live-scms-2026-10-25.md),
//! each pinned on a short cut of its live variant so the test goes red without the fix.

use std::path::Path;

use v2xw_engine::{Engine, NullRecorder, RunReport, Scenario};

fn live(name: &str, secs: f64) -> RunReport {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/live-scms").join(name)).expect("loads");
    s.time.duration_s = secs;
    let mut engine = Engine::build(s, "").expect("builds");
    engine.run(&mut NullRecorder::new()).expect("runs")
}

/// Finding 5 (VOD): under on-demand verification 97 % of receptions are delivered
/// unverified, yet every one was fed to the detector suite and counted in
/// `messages_checked` (45,071 -> 49,194 on the 180 s run). Detection judges authenticated
/// BSMs only ([SM-MBR] 2.3.1.2.1), so no checked message may be unverified, and the
/// unverified ones are counted as not checked.
#[test]
fn detection_never_judges_an_unverified_message() {
    // 70 s: the loader refuses a run that ends before the attack starts (60 s).
    let r = live("vod.yaml", 70.0);
    let p = &r.phase2;
    println!(
        "vod: checked {} not checked {} states {:?}",
        p.messages_checked, p.messages_not_checked, p.verification_states
    );
    assert_eq!(
        p.verification_states
            .get("Unverified")
            .copied()
            .unwrap_or(0),
        0,
        "{:?}",
        p.verification_states
    );
    assert!(
        p.messages_not_checked > p.messages_checked,
        "on-demand leaves most receptions unverified: {} not checked against {} checked",
        p.messages_not_checked,
        p.messages_checked
    );
}
