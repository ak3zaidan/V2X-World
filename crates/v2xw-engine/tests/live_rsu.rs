//! The roadside CRL path on a slow backhaul and through a unit that is down (live-rsu
//! track, docs/validation/live-rsu-2026-10-25.md §3).
//!
//! `scenarios/live-rsu/scms-lat2000-bc3.yaml`: Midtown, 30 units with the `crl` role on
//! fibre whose one-way latency is 2,000 ms, every vehicle relay-only, a CRL every 10 s,
//! the carousel at 4 s here (the file has 3 s; see the test body), so a tick can fall inside a unit's
//! download, and 3 % constant-position attackers, so lists are published inside the
//! window. Unit 5 is switched off from 30 s to the end.
//!
//! * A unit downloads each new list over its backhaul and can broadcast it only once it
//!   has landed: no unit's CRL frame of a new list comes earlier than 2 s after the list
//!   was published. Before the fix the unit installed at the publication instant (the
//!   download's arrival was dropped) and the next carousel tick put the list on the air
//!   whatever the backhaul.
//! * A unit that is down transmits nothing: no CRL frame from unit 5 after 30 s. Before
//!   the fix the carousel kept broadcasting from it (8 frames in 120 s).
//!
//! Costly (a Midtown world and 120 s of radio): `--ignored`.

use std::collections::BTreeMap;
use std::path::Path;

use v2xw_engine::scenario::{TimelineItem, TimelineKind};
use v2xw_engine::{Engine, MemoryRecorder, Scenario};

const S: u64 = 1_000_000_000;
const OFF_UNIT: u64 = 5;
const OFF_AT_S: u64 = 30;

/// Loads a scenario of the repository with its map path made absolute: `cargo test` starts
/// in the crate directory, where the relative `worlds/cache/...` path does not exist.
fn load(root: &Path, rel: &str) -> Scenario {
    let mut s = Scenario::load(root.join(rel)).expect("the live-rsu scenario loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    s
}

fn records(r: &MemoryRecorder, channel: &str) -> Vec<serde_json::Value> {
    r.records()
        .iter()
        .filter(|(_, rec)| rec.channel == channel)
        .map(|(_, rec)| serde_json::from_slice(&rec.json).expect("json"))
        .collect()
}

#[test]
#[ignore = "a Midtown run of 120 s; cargo test -p v2xw-engine --test live_rsu -- --ignored"]
fn a_unit_broadcasts_a_list_only_once_its_backhaul_brought_it_and_not_while_it_is_down() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = load(&root, "scenarios/live-rsu/scms-lat2000-bc3.yaml");
    // The file's own 120 s (2026-10-27): the first list of this run is published at 90.004 s
    // (f4982788 release), so a 90 s cut published none and the test could not check anything.
    s.time.duration_s = 120.0;
    // A 4 s carousel (2026-10-26): lists come on the 10 s cadence grid, and 3 s divides the
    // 30 s between lists that S240 published, so with 3 s a later list met a tick 2.146 s
    // after it on the release too and this check could not fail; with 4 s a list 10 or 30 s
    // after the arming one meets a tick 0.146 s after its publication.
    s.security
        .scms
        .as_mut()
        .and_then(|c| c.lifecycle.as_mut())
        .expect("the scenario sets the lifecycle")
        .crl_broadcast_interval_s = Some(4.0);
    let mut params = BTreeMap::new();
    params.insert("target".to_string(), serde_json::json!(OFF_UNIT));
    s.events.push(TimelineItem {
        t: OFF_AT_S as f64,
        until: Some(120.0),
        kind: TimelineKind::Outage,
        params,
    });
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");

    let published: Vec<u64> = records(&rec, "proto.revocation")
        .iter()
        .filter(|v| v["stage"] == "published")
        .filter_map(|v| v["t"].as_u64())
        .collect();
    let crl: Vec<(u64, u64)> = records(&rec, "node.tx")
        .iter()
        .filter(|v| v["msg_type"] == "crl")
        .filter_map(|v| Some((v["t"].as_u64()?, v["node"].as_u64()?)))
        .collect();
    assert!(!published.is_empty(), "no list was published in the window");
    assert!(!crl.is_empty(), "no unit broadcast a list");
    // Every unit's first frame after each publication waits for the 2 s download.
    for (i, p) in published.iter().enumerate() {
        let next = published.get(i + 1).copied().unwrap_or(u64::MAX);
        if let Some((t, n)) = crl.iter().find(|(t, _)| *t >= *p && *t < next) {
            assert!(
                t - p >= 2 * S,
                "unit {n} broadcast the list published at {p} ns after {} ms",
                (t - p) / 1_000_000
            );
        }
    }
    let from_off = crl
        .iter()
        .filter(|(t, n)| *n == OFF_UNIT && *t > OFF_AT_S * S)
        .count();
    assert_eq!(from_off, 0, "unit {OFF_UNIT} broadcast while it was down");
}

/// A unit relays the list it downloaded, unchanged ([PRIMER] p.5), so each list rides each
/// unit's line once, as the unit's own fetch (live-rsu §3, 2026-10-26).
///
/// `scenarios/live-rsu/scms-base.yaml`: the same 30 units on metro fibre (every fetch lands
/// inside its mobility step), every vehicle relay-only, 120 s. A unit's line then carries
/// the reports it forwards and its CRL fetches, and nothing else: no certificate-cycle
/// exchange falls in the window. Before the fix the list that started the carousel went
/// down every unit's line a second time to time the start: 366 = 186 reports + 120 fetches
/// + 60 on the 10-25 release. After that fix the start still booked the list for a unit in
/// the cycle whose own fetch was not due that step (its RA served the list one
/// inter-provider hop later), and the unit fetched it again itself: 214 = 108 + 90 + 16 on
/// the 3cbc95a4 release (scms-fix4-b LR.once). Such a unit now relays what it installed.
#[test]
#[ignore = "a Midtown run of 120 s; cargo test -p v2xw-engine --test live_rsu -- --ignored"]
fn each_list_rides_each_units_backhaul_once() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let s = load(&root, "scenarios/live-rsu/scms-base.yaml");
    let mut engine = Engine::build(s, "").expect("builds");
    let report = engine.run(&mut MemoryRecorder::new()).expect("runs");
    let p = &report.phase2;
    let line = &p.access.carrier;
    assert!(p.crl_broadcasts > 0, "the carousel never ran");
    assert!(p.rsu_crls_installed > 0, "no unit fetched a list");
    assert!(p.reports_uploaded_relay > 0, "no report rode a unit's line");
    assert_eq!(line.backhaul_retx, 0, "the fibre is lossless");
    assert_eq!(
        line.backhaul_messages,
        p.reports_uploaded_relay + p.rsu_crls_installed,
        "line messages beyond the {} relayed reports and the {} fetches",
        p.reports_uploaded_relay,
        p.rsu_crls_installed
    );
}
