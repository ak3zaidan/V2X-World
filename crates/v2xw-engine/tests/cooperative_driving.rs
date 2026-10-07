//! Cooperative driving runs on what the network delivered.
//!
//! A CACC fleet on the connected-intersections grid: its link is fed from the BSMs each
//! follower's node actually delivered, associated by position with its radar leader. The
//! same fleet with no OBUs hears nothing and associates nothing — the check that can fail
//! if the controller were reading ground truth instead of the network.

use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

fn cacc(equipped: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("connected-intersections.yaml"))
        .expect("the shipped scenario loads");
    s.time.duration_s = 20.0;
    s.metrics = vec![];
    s.actors.vehicles.demand.rate_veh_per_h = Some(6_000.0);
    s.actors.vehicles.equipped_fraction = equipped;
    s.actors.vehicles.driving = serde_json::from_value(serde_json::json!({
        "by_class": {"passenger": {"model": "cacc-ploeg"}}
    }))
    .expect("driving block");
    s
}

#[test]
fn cacc_hears_its_leader_over_the_network_and_nothing_without_obus() {
    let errors = v2xw_engine::scenario::validate::validate(&cacc(1.0));
    assert!(errors.is_empty(), "{errors:?}");

    let mut engine = Engine::build(cacc(1.0), "").expect("builds");
    engine.run(&mut MemoryRecorder::new()).expect("runs");
    let stats = engine.cooperative_stats().expect("a cooperative fleet");
    eprintln!("equipped: {stats:?}");
    assert!(stats.associated > 0, "no BSM associated with a radar leader: {stats:?}");
    assert!(
        stats.offered > stats.associated,
        "every message heard was from the leader, which the grid cannot produce: {stats:?}"
    );

    let mut dark = Engine::build(cacc(0.0), "").expect("builds");
    dark.run(&mut MemoryRecorder::new()).expect("runs");
    let none = dark.cooperative_stats().expect("still a cooperative fleet");
    assert_eq!(none.associated, 0, "{none:?}");
}

#[test]
fn an_unknown_model_is_refused_at_validation() {
    let mut s = cacc(1.0);
    s.actors.vehicles.driving.by_class.insert(
        v2xw_mobility::VehicleClass::Truck,
        v2xw_mobility::carfollowing::library::CfChoice::of("warp-drive"),
    );
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(
        errors.iter().any(|e| e.to_string().contains("warp-drive")),
        "{errors:?}"
    );
}
