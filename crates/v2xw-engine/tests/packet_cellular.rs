//! `cellular/uu/packet-pf` selected from a scenario: backend traffic rides TCP/TLS over
//! the packet-level Uu, and the run report says how it compares with the simple model.

use std::path::{Path, PathBuf};

use serde_json::json;
use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::{Engine, MemoryRecorder, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

#[test]
fn packet_level_cellular_carries_the_backend_and_reports_the_comparison() {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("loads");
    s.time.duration_s = 25.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(300.0);
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = json!(4);
        params["rows"] = json!(4);
    }
    let p = v2xw_engine::phase2::CAMP_SCMS;
    s.actors.backend.protocol = Some(p.to_string());
    s.security.protocol = Some(ModelChoice {
        id: p.to_string(),
        params: json!({
            "i_period_s": 20, "cert_lifetime_s": 21, "certs_per_period": 3,
            "pool_periods": 2, "topup_below_periods": 1, "cert_shuffle_window_s": 1,
            "first_batch_delay_s": 1, "download_poll_interval_s": 1
        }),
    });
    s.time.calendar = v2xw_engine::scenario::Calendar::Compressed;
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/packet-pf".to_string(),
        params: json!({"rat": "lte", "background_load": 0.5, "isd_m": 500}),
    });
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    let a = &report.phase2.access;
    println!("{a:?}");
    assert!(a.cellular_vehicles > 0);
    assert!(
        a.tcp_connections > 0,
        "backend messages open TCP connections"
    );
    assert!(!a.uu_vs_simple.is_empty(), "the comparison is reported");
    // Every message of the backend flows, not just a probe, went through the scheduler.
    assert!(
        a.carrier.uu_messages > 0,
        "the kernels' messages ride the packet-level Uu: {:?}",
        a.carrier
    );
    let topup = a.exchange_latency.get("topup").copied().unwrap_or_default();
    assert!(
        topup.count > 0,
        "top-up latency is measured: {:?}",
        a.exchange_latency
    );
    assert!(
        topup.mean_ms() > 10.0,
        "a top-up costs real round trips: {topup:?}"
    );
}
