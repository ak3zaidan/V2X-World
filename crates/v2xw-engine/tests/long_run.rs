//! What a long run keeps: state held per directed radio link is dropped once an end of the
//! link has despawned, so a run's memory follows the traffic on the map and not the
//! traffic it has ever carried — and dropping it changes nothing the run records.
//!
//! Before the sweep, every pair of nodes that ever heard each other kept its shadowing RNG
//! stream and its shadowing process for the rest of the run. Over a 30-minute run with
//! vehicles arriving and leaving that was memory without bound, and every per-link lookup
//! paid for the dead pairs.

use std::path::{Path, PathBuf};

use v2xw_engine::{DigestRecorder, Engine, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The procedural grid cut to three 120 m blocks by four, so vehicles cross it and leave
/// within a minute or so, with steady arrivals, under the high tier's geometric law — the law that keeps
/// a shadowing process and an RNG stream per directed link.
fn turnover() -> Scenario {
    let mut s =
        Scenario::load(scenarios().join("phase1-grid.yaml")).expect("the shipped scenario loads");
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = serde_json::json!(3);
        params["rows"] = serde_json::json!(4);
        params["block_x_m"] = serde_json::json!(120.0);
    }
    s.time.duration_s = 150.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(3_600.0);
    s.radio.tiers.propagation = v2xw_core::card::Tier::High;
    s
}

#[test]
fn a_despawned_nodes_link_state_is_swept_and_the_record_does_not_change() {
    let run = |sweep: bool| {
        let mut engine = Engine::build(turnover(), "").expect("builds");
        engine.set_link_sweep(sweep);
        let mut digest = DigestRecorder::new();
        let report = engine.run(&mut digest).expect("runs");
        let despawned: u64 = report.despawn_causes.values().sum();
        (engine.link_sweep_backlog(), despawned, digest.digest_hex())
    };
    let ((pending, leaked, _), despawned, swept) = run(true);
    let ((_, _, kept), _, unswept) = run(false);
    eprintln!(
        "despawned {despawned}; swept: awaiting {pending}, leaked {leaked}; \
         without the sweep {kept} link streams of despawned nodes are kept"
    );
    assert!(
        despawned >= 15,
        "the run must carry real turnover for this to test anything; {despawned} despawns"
    );
    // Without the sweep the dead links' state is there to be dropped: the check below is
    // not passing because there was nothing to sweep.
    assert!(kept > 0, "no link of a despawned node held a stream");
    // Nothing the sweep has passed is still held.
    assert_eq!(leaked, 0, "link streams held for nodes the sweep already retired");
    // And the sweep runs: what waits for it is the grace period's despawns plus one batch,
    // not the run's whole turnover.
    assert!(
        (pending as u64) * 2 < despawned,
        "{pending} of {despawned} despawned nodes still hold their links' state"
    );
    // Dropping state no surviving link can reach changes nothing recorded.
    assert_eq!(swept, unswept, "the sweep changed the record");
}
