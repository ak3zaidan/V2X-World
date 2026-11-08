//! What the engine forgets of a vehicle that has left: the modelled crypto
//! entries its backend wrote to the process-wide directories and the J2945/1 PER rings it sent
//! or heard on. Both are dropped by the link sweep once the grace has passed, so a run's memory
//! follows the fleet on the air, and dropping them changes nothing the run records.
//!
//! In a binary of its own: the modelled crypto directory is process-wide, so another test's
//! run in the same process would move the count this test reads.

use std::path::{Path, PathBuf};

use v2xw_engine::{DigestRecorder, Engine, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The turnover grid of `long_run.rs`: vehicles cross three 120 m blocks by four and leave
/// within a minute or so, with steady arrivals; signed BSMs, modelled crypto, J2945/1 DCC.
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
    s
}

#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
fn a_departed_vehicles_crypto_entries_and_per_rings_are_forgotten_and_no_value_moves() {
    let run = |sweep: bool| {
        let mut engine = Engine::build(turnover(), "").expect("builds");
        engine.set_link_sweep(sweep);
        let mut digest = DigestRecorder::new();
        let report = engine.run(&mut digest).expect("runs");
        let census = engine.census();
        let of = |key: &str| {
            census
                .iter()
                .find(|(k, _)| *k == key)
                .map_or(0, |(_, v)| *v)
        };
        (
            of("crypto.directory"),
            of("j2945.per_rings"),
            of("metrics.held"),
            of("nodes"),
            report.despawn_causes.values().sum::<u64>(),
            digest.digest_hex(),
        )
    };
    // The sweeping run first: the directory is process-wide, and a second run of the same
    // seed writes the same entries, so the unswept run must come after to count all of its own.
    let (dir_swept, per_swept, held_swept, live, despawned, a) = run(true);
    let (dir_kept, per_kept, held_kept, _, _, b) = run(false);
    eprintln!(
        "despawned {despawned}, {live} nodes live at the end: directory {dir_swept} against \
         {dir_kept}, PER rings {per_swept} against {per_kept}, metric links {held_swept} against {held_kept}"
    );
    assert!(despawned >= 15, "the run needs turnover: {despawned}");
    assert!(
        dir_kept > 0 && per_kept > 0 && held_kept > 0,
        "nothing to forget: the check could not fail"
    );
    // Each vehicle swept past the grace takes its entries with it: at least one each (a
    // modelled vehicle writes 40 directory entries and sends on a PER ring), so the unswept
    // run holds more by at least half the departures (the sweep's backlog is under half,
    // `long_run.rs`). With the forget off the two runs hold the same and this goes red.
    let gone = despawned / 2;
    assert!(
        dir_kept >= dir_swept + gone,
        "crypto directory {dir_swept} with the sweep against {dir_kept} without, {despawned} gone"
    );
    assert!(
        per_kept >= per_swept + gone,
        "PER rings {per_swept} with the sweep against {per_kept} without, {despawned} gone"
    );
    assert!(
        held_kept >= held_swept + gone,
        "metric providers' links {held_swept} with the sweep against {held_kept} without"
    );
    assert_eq!(a, b, "forgetting changed the record");
}
