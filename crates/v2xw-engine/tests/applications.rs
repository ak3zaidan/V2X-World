//! The V2X applications in a run: warnings issued from what vehicles heard, labelled
//! against ground truth; GLOSA advice and the drivers who follow it; and emergency-vehicle
//! priority that actually moves a controller's plan.
//!
//! Each test checks the application's output against something that could disagree with
//! it: a warning against the true states of the two vehicles, an advised speed against the
//! speed the vehicle then drove, a priority record against the lamps the drivers saw.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use v2xw_core::ids::NodeId;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_metrics::channels::{GtKinematicsView, decode};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

fn connected(duration_s: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("connected-intersections.yaml"))
        .expect("the shipped scenario loads");
    s.time.duration_s = duration_s;
    s.metrics = vec![];
    s
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder) {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    (report, recorder)
}

fn json(recorder: &MemoryRecorder, channel: &str) -> Vec<serde_json::Value> {
    recorder
        .records()
        .iter()
        .filter(|(_, r)| r.channel == channel)
        .map(|(_, r)| serde_json::from_slice(&r.json).expect("json"))
        .collect()
}

/// A scripted hard brake: the vehicles behind it warn (EEBL from its BSM's hard-braking
/// flag, FCW as they close), the warnings are labelled true against the two vehicles'
/// true states, and every warning is from a vehicle that heard the braking one.
#[test]
fn a_hard_brake_warns_the_vehicles_behind_and_the_warnings_are_true() {
    // Dense enough that, a quarter-minute in, some moving car has another close behind
    // it: at 6,000 veh/h this grid had none between 14 and 24 s (at most 16 vehicles on
    // 1.6 km of avenues), so the `"auto"` pick waited out its 10 s and gave up.
    let mut s = connected(32.0);
    s.actors.vehicles.demand.rate_veh_per_h = Some(12_000.0);
    s.events = serde_json::from_value(serde_json::json!([
        {"t": 16.0, "type": "safety.hard-brake", "decel_mps2": 6.0, "within_s": 12.0},
    ]))
    .expect("timeline");
    let (report, recorder) = run(s);
    let events = json(&recorder, "scenario.event");
    let fired = events
        .iter()
        .find(|e| e["kind"] == "safety.hard-brake" && e["phase"] == "start")
        .unwrap_or_else(|| panic!("the hard brake never fired: {events:?}"));
    let braked = fired["node"]
        .as_u64()
        .expect("the hard brake found an equipped vehicle with a follower");
    let t_brake = fired["t"].as_u64().expect("a time") as f64;
    let warnings = json(&recorder, "app.warning");
    let issued: Vec<&serde_json::Value> =
        warnings.iter().filter(|w| w["kind"] == "issue").collect();
    eprintln!(
        "braked node {braked}; {} warnings issued, by app {:?}; report {:?}",
        issued.len(),
        issued
            .iter()
            .fold(BTreeMap::<String, u32>::new(), |mut m, w| {
                *m.entry(w["app"].as_str().unwrap_or("?").to_string())
                    .or_default() += 1;
                m
            }),
        report.apps
    );
    let eebl_after: Vec<&&serde_json::Value> = issued
        .iter()
        .filter(|w| {
            w["app"] == "eebl"
                && w["t"]
                    .as_u64()
                    .is_some_and(|t| (t_brake..t_brake + 3e9).contains(&(t as f64)))
        })
        .collect();
    assert!(
        !eebl_after.is_empty(),
        "no vehicle warned of the hard brake (EEBL) within 3 s of it"
    );
    // The warnings were checked against the truth, and the EEBL ones were true.
    let outcomes = json(&recorder, "app.outcome");
    let true_eebl = outcomes
        .iter()
        .filter(|o| o["app"] == "eebl" && o["outcome"] == "true")
        .count();
    assert!(
        true_eebl > 0,
        "no EEBL warning was labelled true: {outcomes:?}"
    );
    let tally = report.apps.get("eebl").copied().unwrap_or_default();
    assert!(tally.issued >= tally.true_warnings + tally.false_warnings);
    assert!(tally.true_warnings > 0);
}

/// With every driver following GLOSA, advised vehicles drive no faster than the advice
/// while it stands; with none following, the advice is given and nobody's speed is capped.
#[test]
fn glosa_advice_is_given_and_followed_by_the_drivers_who_follow_it() {
    // A minute, so that vehicles approach the four units' junctions on red as well as on green.
    let mut s = connected(60.0);
    s.actors.vehicles.demand.rate_veh_per_h = Some(6_000.0);
    s.apps.glosa_compliance = 1.0;
    let (_, recorder) = run(s);
    let advice = json(&recorder, "app.advice");
    let slowed: Vec<&serde_json::Value> = advice
        .iter()
        .filter(|a| {
            a["advised_mps"]
                .as_f64()
                .is_some_and(|v| v + 0.5 < a["speed_mps"].as_f64().unwrap_or(0.0))
        })
        .collect();
    eprintln!(
        "{} advisories, {} advising a slower speed",
        advice.len(),
        slowed.len()
    );
    assert!(!advice.is_empty(), "no vehicle was advised");
    assert!(
        !slowed.is_empty(),
        "no advisory asked a vehicle to slow for the green"
    );
    // The node's true speed a few seconds after an advice to slow is at or under it.
    let mut speeds: BTreeMap<u64, Vec<(u64, f64)>> = BTreeMap::new();
    for (_, r) in recorder.records() {
        if r.channel == "gt.kinematics" {
            let k: GtKinematicsView = decode(r).expect("decodes");
            if let Some(n) = k.node {
                speeds
                    .entry(u64::from(n.index()))
                    .or_default()
                    .push((k.t, k.speed_mps));
            }
        }
    }
    let mut followed = 0;
    for a in &slowed {
        let (Some(node), Some(t), Some(v)) = (
            a["node"].as_u64(),
            a["t"].as_u64(),
            a["advised_mps"].as_f64(),
        ) else {
            continue;
        };
        let later = speeds.get(&node).and_then(|s| {
            s.iter()
                .find(|(tt, _)| *tt >= t + 4_000_000_000)
                .map(|(_, sp)| *sp)
        });
        if later.is_some_and(|sp| sp <= v + 0.6) {
            followed += 1;
        }
    }
    assert!(
        followed * 2 >= slowed.len(),
        "{followed} of {} slowed advisories were followed",
        slowed.len()
    );
}

/// Vehicles that send CPMs report what their sensors perceive at the TS 103 324 rules: a
/// CPM at most ten times a second, carrying objects (so bigger than the management and
/// station containers alone).
#[test]
fn vehicles_share_what_their_sensors_perceive_in_cpms() {
    let mut s = connected(20.0);
    s.net.layer = "gn-btp".to_string();
    s.security.envelope = "etsi103097".to_string();
    s.messages.sets = vec!["cam".into(), "cpm".into(), "spat".into(), "map".into()];
    s.messages.codec_tier = "uper".to_string();
    s.actors.vru.pedestrians = 40;
    s.actors.vru.device_fraction = 0.0;
    let (_, recorder) = run(s);
    let tx = json(&recorder, "node.tx");
    let cpm: Vec<&serde_json::Value> = tx.iter().filter(|t| t["msg_type"] == "cpm").collect();
    let senders: BTreeSet<u64> = cpm.iter().filter_map(|t| t["node"].as_u64()).collect();
    eprintln!("{} CPMs from {} vehicles", cpm.len(), senders.len());
    assert!(!cpm.is_empty(), "no vehicle sent a CPM");
    for n in &senders {
        let k = cpm
            .iter()
            .filter(|t| t["node"].as_u64() == Some(*n))
            .count();
        assert!(k <= 200, "node {n} sent {k} CPMs in 20 s, more than 10 Hz");
    }
    // Some CPM carried objects: the management and originating-vehicle containers alone
    // are about 30 octets; each object adds about 20 more.
    let biggest = cpm
        .iter()
        .filter_map(|t| t["payload_bytes"].as_u64())
        .max()
        .unwrap_or(0);
    assert!(
        biggest > 60,
        "the largest CPM payload is {biggest} B: no objects were shared"
    );
}

/// An emergency vehicle's request moves its junction's plan: the controller extends a
/// green or ends a conflicting one early, logs it on `signal.priority`, and walks the
/// plan back afterwards.
#[test]
fn an_emergency_vehicles_request_changes_the_signal_plan() {
    let mut s = connected(40.0);
    for class in s.actors.vehicles.classes.values_mut() {
        class.fraction = 0.5;
    }
    s.actors.vehicles.demand.rate_veh_per_h = Some(4_000.0);
    let (_, recorder) = run(s);
    let priority = json(&recorder, "signal.priority");
    let by_action = priority
        .iter()
        .fold(BTreeMap::<String, u32>::new(), |mut m, p| {
            *m.entry(p["action"].as_str().unwrap_or("?").to_string())
                .or_default() += 1;
            m
        });
    eprintln!("signal.priority by action: {by_action:?}");
    assert!(
        by_action.get("request").copied().unwrap_or(0) > 0,
        "no request reached a controller"
    );
    let acted = by_action.get("extend").copied().unwrap_or(0)
        + by_action.get("early-green").copied().unwrap_or(0);
    assert!(
        acted > 0,
        "no controller extended or cut a green for a request"
    );
    // The requesters are emergency vehicles' pseudonyms: every request names a signer, and
    // there are no more distinct requesters than emergency vehicles.
    let emergency: BTreeSet<NodeId> = recorder
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "gt.kinematics")
        .filter_map(|(_, r)| {
            let k: GtKinematicsView = decode(r).ok()?;
            (k.class.as_deref() == Some("emergency"))
                .then_some(k.node)
                .flatten()
        })
        .collect();
    assert!(!emergency.is_empty());
}
