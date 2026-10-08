//! The radio overrides (`radio.channel_model`, `radio.sidelink`) and the WINNER+ B1 law, end to end: every override reaches the run, an
//! override the run would ignore is refused, and 802.11p collisions are told apart into
//! hidden terminals.

use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, Scenario};
use v2xw_metrics::channels::{PhyRxView, RxOutcome};

fn grid_fleet(n: u32, secs: f64) -> Scenario {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
    let mut s = Scenario::load(root.join("phase1-grid.yaml")).expect("the shipped scenario loads");
    s.time.duration_s = secs;
    s.actors.vehicles.demand.rate_veh_per_h = Some(3_600_000.0);
    s.actors.vehicles.demand.params = serde_json::json!({ "max_total_vehicles": n });
    s
}

/// `patch` as `{channel: {...}, sidelink: {...}}`: the channel numbers go to
/// `radio.channel_model`, the sidelink ones to `radio.sidelink`.
fn tuned(mut s: Scenario, patch: serde_json::Value) -> Scenario {
    if let Some(c) = patch.get("channel") {
        s.radio.channel_model = serde_json::from_value(c.clone()).expect("radio.channel_model");
    }
    if let Some(c) = patch.get("sidelink") {
        s.radio.sidelink = serde_json::from_value(c.clone()).expect("radio.sidelink");
    }
    s
}

fn model(mut s: Scenario, family: &str, id: &str) -> Scenario {
    s.radio
        .models
        .insert(family.into(), v2xw_engine::scenario::schema::ModelChoice::new(id));
    s
}

fn refused(s: &Scenario, field: &str) -> bool {
    v2xw_engine::scenario::validate(s)
        .iter()
        .any(|e| format!("{e:?}").contains(field))
}

fn rx(scenario: Scenario) -> Vec<PhyRxView> {
    let errors = v2xw_engine::scenario::validate(&scenario);
    assert!(errors.is_empty(), "does not load: {errors:?}");
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs");
    recorder
        .records()
        .iter()
        .filter(|(_, r)| r.channel == <PhyRxView as v2xw_metrics::channels::ChannelView>::CHANNEL)
        .map(|(_, r)| v2xw_metrics::channels::decode(r).expect("decodes"))
        .collect()
}

fn mean_rssi(rx: &[PhyRxView], lo: f64, hi: f64) -> Option<f64> {
    let v: Vec<f64> = rx
        .iter()
        .filter(|r| r.dist_m.is_some_and(|d| d >= lo && d < hi))
        .filter_map(|r| r.rssi_dbm)
        .collect();
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// The loader refuses what the run would ignore, and accepts the same value where it acts.
#[test]
fn an_override_the_run_would_ignore_is_refused() {
    let s = grid_fleet(10, 1.0);
    // The shipped grid runs the medium tier's log-distance law, where the exponent acts;
    // TR 37.885 has its own slopes.
    let exp = tuned(s.clone(), serde_json::json!({"channel": {"path_loss_exponent": 3.0}}));
    assert!(!refused(&exp, "path_loss_exponent"));
    let exp_tr = model(exp, "propagation", "propagation/tr37885");
    assert!(refused(&exp_tr, "path_loss_exponent"));
    // Sidelink knobs on an 802.11p run, and a keep probability that is not a legal value.
    let keep = tuned(
        s.clone(),
        serde_json::json!({"sidelink": {"resource_keep_probability": 0.4}}),
    );
    assert!(refused(&keep, "radio.sidelink"));
    let mut lte = tuned(
        s.clone(),
        serde_json::json!({"sidelink": {"resource_keep_probability": 0.3}}),
    );
    lte.radio.rat = serde_json::from_value(serde_json::json!("lte-v2x-pc5")).unwrap();
    assert!(refused(&lte, "resource_keep_probability"));
    lte.radio.sidelink.resource_keep_probability = Some(0.4);
    assert!(!refused(&lte, "resource_keep_probability"));
    // A Nakagami shape needs the Nakagami law named; a far m needs its breakpoint.
    let nak = tuned(s.clone(), serde_json::json!({"channel": {"nakagami_shape": 1.0}}));
    assert!(refused(&nak, "nakagami_shape"));
    let nak = tuned(
        s.clone(),
        serde_json::json!({"channel": {"fading": "nakagami", "nakagami_shape": 1.0}}),
    );
    assert!(!refused(&nak, "nakagami_shape"));
    let far = tuned(
        s,
        serde_json::json!({"channel": {"fading": "nakagami", "nakagami_shape_far": 0.75}}),
    );
    assert!(refused(&far, "nakagami_breakpoint_m"));
}

/// WINNER+ B1's 40 dB/decade beyond a 20 m breakpoint costs about 23 dB at 250 m against
/// the TR 37.885 urban LOS law the geometric law uses on a clear street.
#[test]
fn winner_plus_b1_is_the_steeper_law_it_is_published_as() {
    let mut base = grid_fleet(40, 3.0);
    base.world.buildings.enabled = false;
    let tr = rx(model(base.clone(), "propagation", "propagation/tr37885"));
    let b1 = rx(model(base, "propagation", "propagation/winner-plus-b1"));
    let (t, w) = (
        mean_rssi(&tr, 150.0, 300.0).expect("tr37885 links at 150-300 m"),
        mean_rssi(&b1, 150.0, 300.0).expect("winner links at 150-300 m"),
    );
    println!("mean RSSI 150-300 m: tr37885 {t:.1} dBm, winner-plus-b1 {w:.1} dBm");
    assert!(t - w > 10.0, "WINNER+ B1 should be far steeper: {t:.1} vs {w:.1}");
}

/// Shadowing σ = 0 removes the spread at one distance; the default keeps it.
#[test]
fn shadowing_sigma_reaches_the_link_budget() {
    let mut base = grid_fleet(30, 3.0);
    base.world.buildings.enabled = false;
    let base = model(base, "propagation", "propagation/winner-plus-b1");
    let spread = |rx: &[PhyRxView]| {
        // The deviation from the law at each attempt's own distance.
        let devs: Vec<f64> = rx
            .iter()
            .filter_map(|r| Some(r.rssi_dbm? + v2xw_radio::winner::b1_los_db(r.dist_m?, 1.5, 1.5, 5.9e9)))
            .collect();
        let m = devs.iter().sum::<f64>() / devs.len() as f64;
        (devs.iter().map(|d| (d - m) * (d - m)).sum::<f64>() / devs.len() as f64).sqrt()
    };
    let on = spread(&rx(model(base.clone(), "fading", "fading/none")));
    let off = spread(&rx(model(
        tuned(base, serde_json::json!({"channel": {"shadowing_sigma_db": 0.0}})),
        "fading",
        "fading/none",
    )));
    println!("received-power spread about the law: σ default {on:.2} dB, σ 0 {off:.2} dB");
    assert!(on > off + 1.0, "{on} vs {off}");
}

/// A dense 802.11p fleet among buildings loses frames to hidden terminals, which are now
/// reported as such rather than as plain collisions.
#[test]
fn hidden_terminals_are_reported() {
    let frames = rx(grid_fleet(250, 4.0));
    let mut hist = std::collections::BTreeMap::<String, usize>::new();
    for r in &frames {
        for c in r.all_causes() {
            *hist.entry(c.to_string()).or_default() += 1;
        }
    }
    println!("{} attempts, causes {hist:?}", frames.len());
    let count = |c: &str| {
        frames
            .iter()
            .filter(|r| r.outcome != RxOutcome::Ok && r.all_causes().contains(&c))
            .count()
    };
    let (hidden, collision) = (count("hidden-terminal"), count("collision"));
    println!("losses: hidden-terminal {hidden}, collision {collision}");
    assert!(hidden > 0, "no hidden terminal among {} attempts", frames.len());
}
