//! `net.geonet`: a breakdown's warning is forwarded hop by hop, and the setting changes
//! the outcome (so the check can go red: with flooding the transmission count differs
//! from CBF's, and with no `net.geonet` there is no multi-hop result at all).

use std::path::Path;

use v2xw_engine::scenario::schema::Geonet;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn scenario(geonet: Option<Geonet>) -> Scenario {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/connected-intersections.yaml");
    let mut s = Scenario::load(path).expect("the shipped scenario loads");
    s.time.duration_s = 14.0;
    s.metrics = vec![];
    s.actors.vehicles.demand.rate_veh_per_h = Some(12_000.0);
    s.net.layer = "gn-btp".to_string();
    s.security.envelope = "etsi103097".to_string();
    s.messages.sets = vec!["cam".into(), "denm".into()];
    s.messages.codec_tier = "uper".to_string();
    s.events = serde_json::from_value(serde_json::json!([
        {"t": 10.0, "until": 13.0, "type": "safety.breakdown"},
    ]))
    .expect("timeline items");
    s.net.geonet = geonet;
    s
}

fn run(s: Scenario) -> RunReport {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs")
}

#[test]
fn a_breakdown_warning_spreads_multi_hop_and_the_forwarding_setting_matters() {
    assert!(run(scenario(None)).geonet.is_empty());
    let cbf = run(scenario(Some(Geonet { range_m: 200.0, ..Geonet::default() })));
    let flood = run(scenario(Some(Geonet { range_m: 200.0, area_forwarding: "simple".into(), ..Geonet::default() })));
    eprintln!("cbf {:?}\nflood {:?}", cbf.geonet, flood.geonet);
    let (c, f) = (&cbf.geonet[0], &flood.geonet[0]);
    assert!(c.in_area > 5, "a dense grid has vehicles within 1 km: {c:?}");
    assert!(c.reached >= 2, "{c:?}");
    assert!(c.max_hops >= 1, "some vehicle is reached through a relay: {c:?}");
    // Flooding rebroadcasts at once; CBF waits out its contention timers (up to 100 ms),
    // so the slowest vehicles hear it later under CBF.
    assert!(f.transmissions >= c.transmissions, "{f:?} vs {c:?}");
    assert!(f.latency_p95_us < c.latency_p95_us, "{f:?} vs {c:?}");
    // The setting is validated: geonet on WSMP is refused.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/connected-intersections.yaml");
    let mut bad = Scenario::load(path).expect("loads");
    assert_eq!(bad.net.layer, "wsmp");
    bad.net.geonet = Some(Geonet::default());
    let err = bad.validate();
    let msg = format!("{err:?}");
    assert!(err.is_err() && msg.contains("net.geonet"), "{msg}");
}
