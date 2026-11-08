//! WINNER+ B1 NLOS (Manhattan two-leg) for building-blocked links, against the default
//! LOS-plus-Sommer composition, on the shipped grid with its buildings.

use std::path::Path;

use v2xw_engine::{Engine, MemoryRecorder, Scenario};
use v2xw_metrics::channels::{PhyRxView, RxOutcome};

fn run(nlos: &str) -> Vec<PhyRxView> {
    let mut s = Scenario::load(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/phase1-grid.yaml"),
    )
    .expect("the shipped scenario loads");
    s.time.duration_s = 3.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(3_600_000.0);
    s.actors.vehicles.demand.params = serde_json::json!({ "max_total_vehicles": 40 });
    s.radio.models.insert(
        "propagation".into(),
        v2xw_engine::scenario::schema::ModelChoice {
            id: "propagation/winner-plus-b1".into(),
            params: serde_json::json!({ "nlos": nlos }),
        },
    );
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "does not load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    rec.records()
        .iter()
        .filter(|(_, r)| r.channel == <PhyRxView as v2xw_metrics::channels::ChannelView>::CHANNEL)
        .map(|(_, r)| v2xw_metrics::channels::decode(r).expect("decodes"))
        .collect()
}

fn pdr(rx: &[PhyRxView], lo: f64, hi: f64) -> (usize, f64) {
    let v: Vec<&PhyRxView> = rx
        .iter()
        .filter(|r| r.dist_m.is_some_and(|d| d >= lo && d < hi))
        .collect();
    let ok = v.iter().filter(|r| r.outcome == RxOutcome::Ok).count();
    (v.len(), ok as f64 / v.len().max(1) as f64)
}

#[test]
fn the_manhattan_law_prices_blocked_links_and_differs_from_sommer() {
    let sommer = run("sommer");
    let manhattan = run("manhattan");
    let mut differs = false;
    for (lo, hi) in [(0.0, 100.0), (100.0, 200.0), (200.0, 300.0), (300.0, 500.0)] {
        let (n, a) = pdr(&sommer, lo, hi);
        let (m, b) = pdr(&manhattan, lo, hi);
        println!("VERDICT {lo:>3}-{hi:<3} m: sommer {a:.3} (n {n}), manhattan {b:.3} (n {m})");
        differs |= (a - b).abs() > 0.01;
    }
    assert!(
        differs,
        "the NLOS choice changed nothing: blocked links are not priced by it"
    );
}

#[test]
fn an_unknown_nlos_treatment_is_refused() {
    let mut s = Scenario::load(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/phase1-grid.yaml"),
    )
    .expect("loads");
    s.radio.models.insert(
        "propagation".into(),
        v2xw_engine::scenario::schema::ModelChoice {
            id: "propagation/winner-plus-b1".into(),
            params: serde_json::json!({ "nlos": "knife-edge" }),
        },
    );
    assert!(!v2xw_engine::scenario::validate(&s).is_empty());
}
