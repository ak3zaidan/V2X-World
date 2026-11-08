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
    assert_eq!(
        leaked, 0,
        "link streams held for nodes the sweep already retired"
    );
    // And the sweep runs: what waits for it is the grace period's despawns plus one batch,
    // not the run's whole turnover.
    assert!(
        (pending as u64) * 2 < despawned,
        "{pending} of {despawned} despawned nodes still hold their links' state"
    );
    // Dropping state no surviving link can reach changes nothing recorded.
    assert_eq!(swept, unswept, "the sweep changed the record");
}

/// **Memory.** A trip's, actor's and node's streams go when they end, and
/// the tripwire shows none of them was drawn again, the one way forgetting could move a value:
/// a run's stream cache follows the fleet on the map, not the fleet it has ever carried.
#[test]
fn a_finished_trips_streams_are_forgotten_and_no_value_moves() {
    let run = |sweep: bool| {
        let mut engine = Engine::build(turnover(), "").expect("builds");
        engine.set_link_sweep(sweep);
        engine.watch_forgotten_streams();
        let mut digest = DigestRecorder::new();
        let report = engine.run(&mut digest).expect("runs");
        let despawned: u64 = report.despawn_causes.values().sum();
        (
            engine.rng_stream_kinds(),
            engine.rng_redrawn_after_forget(),
            despawned,
            digest.digest_hex(),
        )
    };
    let (swept, redrawn, despawned, with_sweep) = run(true);
    let (kept, _, _, without_sweep) = run(false);
    eprintln!("despawned {despawned}; (custom, actor, node) streams {swept:?} against {kept:?}");
    assert!(despawned >= 15, "the run needs turnover: {despawned}");
    assert!(
        redrawn.is_empty(),
        "forgotten streams were drawn again, so values moved: {redrawn:?}"
    );
    // Most of this short run's fleet is still on the map, so the test is on what went: a
    // departed trip's many keys, and at least one actor and one node key per departure.
    let gone = despawned as usize;
    assert!(
        kept.0 - swept.0 >= 6 * gone,
        "trip streams not forgotten: {} against {}, {gone} trips done",
        swept.0,
        kept.0
    );
    assert!(
        kept.1 - swept.1 >= gone,
        "actor streams not forgotten: {} against {}, {gone} gone",
        swept.1,
        kept.1
    );
    assert!(
        kept.2 - swept.2 >= gone,
        "node streams not forgotten: {} against {}, {gone} gone",
        swept.2,
        kept.2
    );
    assert_eq!(with_sweep, without_sweep, "forgetting changed the record");
}

/// **Memory.** A frame a node handed over before it left still runs its
/// MAC afterwards; that must not bring the retired node's MAC window back (135 windows
/// at 21 live radios on the churn soak), since only a node on the map reports one.
#[test]
fn a_retired_nodes_mac_window_does_not_come_back() {
    let mut engine = Engine::build(turnover(), "").expect("builds");
    let mut digest = DigestRecorder::new();
    let report = engine.run(&mut digest).expect("runs");
    let census = engine.census();
    let of = |key: &str| {
        census
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(0, |(_, v)| *v)
    };
    let despawned: u64 = report.despawn_causes.values().sum();
    assert!(despawned >= 15, "the run needs turnover: {despawned}");
    // The situation the guard is for occurs, and no retired node holds a window.
    let (timers, windows) = engine.retired_mac_state();
    assert!(
        timers > 0,
        "no retired node's MAC timer ran: the check below could not fail"
    );
    assert_eq!(windows, 0, "{windows} MAC windows of retired nodes");
    assert!(
        of("mac_window") <= of("nodes"),
        "{} MAC windows for {} live nodes",
        of("mac_window"),
        of("nodes")
    );
}

/// **Memory.** The EDCA MAC's per-node state goes with its node, whatever
/// a frame left behind, and the record does not change (the sweep test above). The sidelink's
/// per-UE sensing history goes the same way.
#[test]
fn a_retired_nodes_mac_state_is_dropped_and_the_record_does_not_change() {
    let run = |scenario: Scenario, sweep: bool| {
        let mut engine = Engine::build(scenario, "").expect("builds");
        engine.set_link_sweep(sweep);
        engine.watch_forgotten_streams();
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
            of("mac.node_states") + of("mac.ue_states"),
            report.despawn_causes.values().sum::<u64>(),
            engine.rng_redrawn_after_forget(),
            digest.digest_hex(),
        )
    };
    let lte = || {
        let mut s = turnover();
        s.radio.rat = v2xw_engine::scenario::schema::Rat::LteV2xPc5;
        s
    };
    for (name, scenario) in [("dsrc", turnover as fn() -> Scenario), ("lte", lte)] {
        let (swept, despawned, redrawn, a) = run(scenario(), true);
        let (kept, _, _, b) = run(scenario(), false);
        eprintln!("{name}: {swept} MAC states against {kept}, {despawned} despawned");
        assert!(despawned >= 15, "{name}: the run needs turnover");
        assert!(swept < kept, "{name}: {swept} states against {kept}");
        assert!(redrawn.is_empty(), "{name}: {redrawn:?}");
        assert_eq!(a, b, "{name}: dropping it changed the record");
    }
}

/// **Memory.** What the engine keeps about a departed node's radio goes with it, and
/// the record does not change: its PHY transmit intervals and air-time ledger entry, its GNSS
/// receiver's error state, and its set of own frames on the air. Before, each stayed for every
/// vehicle that ever ran, outside every census store. On the churn soak (a heap diff of
/// `malloc_history -allBySize` at 1 h and 3 h, about 980 vehicles in between) they were the
/// growth: the GNSS states +336 kB, the transmit-interval lists +314 kB and their ledger
/// +29 kB, the on-air sets +104 kB; the footprint ended 1.141 times its 1.5 h value against a
/// 1.1 bound.
#[test]
fn a_retired_nodes_radio_and_gnss_state_is_dropped_and_the_record_does_not_change() {
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
            [of("phy.node_entries"), of("gnss.states"), of("on_air_tx")],
            of("nodes"),
            engine.link_sweep_backlog().0 as u64,
            report.despawn_causes.values().sum::<u64>(),
            digest.digest_hex(),
        )
    };
    let (held, nodes, pending, despawned, a) = run(true);
    let (kept, _, _, _, b) = run(false);
    eprintln!(
        "despawned {despawned}, {nodes} live, {pending} awaiting the sweep: [PHY entries, \
         GNSS states, on-air sets] {held:?} against {kept:?} without the sweep"
    );
    assert!(despawned >= 15, "the run needs turnover: {despawned}");
    // What the sweep has passed is gone: each swept departure held a GNSS state, at least one
    // PHY entry and (its last frame) an on-air set, and now holds none.
    let swept = despawned.saturating_sub(pending);
    assert!(
        swept >= 10,
        "too few departures swept to test anything: {swept}"
    );
    let kept_on: Vec<String> = ["PHY entries", "GNSS states", "on-air sets"]
        .iter()
        .enumerate()
        .filter(|&(i, _)| kept[i].saturating_sub(held[i]) < swept / 2)
        .map(|(i, name)| format!("{name} {} against {}", held[i], kept[i]))
        .collect();
    assert!(
        kept_on.is_empty(),
        "not forgotten ({swept} swept): {}",
        kept_on.join("; ")
    );
    assert_eq!(a, b, "dropping it changed the record");
}
