//! While an SCMS runs, a departed vehicle's modelled crypto entries stay: its
//! Misbehavior Authority verifies a report's evidence BSMs through them, and a report about a
//! vehicle that has left reaches it long after the link sweep's grace (the report interval,
//! the RA's shuffle). Forgotten, valid evidence about a departed sender would fail to verify.
//!
//! In a binary of its own: the modelled crypto directory is process-wide, so another test's
//! run in the same process would move the count this test reads.

use std::path::{Path, PathBuf};

use serde_json::json;
use v2xw_engine::scenario::schema::ModelChoice;
use v2xw_engine::{DigestRecorder, Engine, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The turnover grid of `long_run.rs` with the CAMP SCMS of `scms_records.rs` (short periods,
/// modelled crypto, the legacy-12 detectors).
fn turnover_with_scms() -> Scenario {
    let mut s =
        Scenario::load(scenarios().join("phase1-grid.yaml")).expect("the shipped scenario loads");
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = json!(3);
        params["rows"] = json!(4);
        params["block_x_m"] = json!(120.0);
    }
    s.time.duration_s = 150.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(3_600.0);
    let id = v2xw_engine::phase2::CAMP_SCMS.to_string();
    s.actors.backend.protocol = Some(id.clone());
    s.security.protocol = Some(ModelChoice {
        id,
        params: json!({
            "i_period_s": 20, "cert_lifetime_s": 21, "certs_per_period": 3, "pool_periods": 2,
            "topup_below_periods": 1, "cert_shuffle_window_s": 1, "first_batch_delay_s": 1,
            "download_poll_interval_s": 1, "report_shuffle_window_s": 2, "crl_cadence_s": 5,
            "crl_fetch_interval_s": 5
        }),
    });
    s.time.calendar = v2xw_engine::scenario::Calendar::Compressed;
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast"}),
    });
    s.detection.local = vec![ModelChoice::new(v2xw_engine::phase2::LEGACY_12)];
    s
}

#[test]
fn an_scms_run_keeps_a_departed_vehicles_crypto_entries() {
    let run = |sweep: bool| {
        let s = turnover_with_scms();
        let errors = v2xw_engine::scenario::validate::validate(&s);
        assert!(errors.is_empty(), "the scenario should load: {errors:?}");
        let mut engine = Engine::build(s, "").expect("builds");
        engine.set_link_sweep(sweep);
        let mut digest = DigestRecorder::new();
        let report = engine.run(&mut digest).expect("runs");
        assert!(engine.phase2_mut().is_some(), "the SCMS runs");
        let census = engine.census();
        let of = |key: &str| {
            census
                .iter()
                .find(|(k, _)| *k == key)
                .map_or(0, |(_, v)| *v)
        };
        (
            of("crypto.directory"),
            report.despawn_causes.values().sum::<u64>(),
        )
    };
    // The sweeping run first (see `departed_state.rs`): a forget would show as fewer entries
    // than the unswept run of the same seed writes.
    let (swept, despawned) = run(true);
    let (kept, _) = run(false);
    eprintln!("despawned {despawned}: directory {swept} with the sweep against {kept} without");
    assert!(despawned >= 15, "the run needs turnover: {despawned}");
    assert!(kept > 0, "no directory entry: the check could not fail");
    assert_eq!(
        swept, kept,
        "an SCMS run forgot departed vehicles' entries its MA may still verify evidence with"
    );
}
