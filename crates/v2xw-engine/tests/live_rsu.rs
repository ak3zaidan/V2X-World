//! The roadside CRL path on a slow backhaul and through a unit that is down (live-rsu
//! track, docs/validation/live-rsu-2026-10-25.md §3).
//!
//! `scenarios/live-rsu/scms-lat2000-bc3.yaml`: Midtown, 30 units with the `crl` role on
//! fibre whose one-way latency is 2,000 ms, every vehicle relay-only, a CRL every 10 s,
//! the carousel at 3 s (not a divisor of the cadence, so a tick can fall inside a unit's
//! download) and 3 % constant-position attackers, so lists are published inside the
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
//! Costly (a Midtown world and 90 s of radio): `--ignored`.

use std::collections::BTreeMap;
use std::path::Path;

use v2xw_engine::scenario::{TimelineItem, TimelineKind};
use v2xw_engine::{Engine, MemoryRecorder, Scenario};

const S: u64 = 1_000_000_000;
const OFF_UNIT: u64 = 5;
const OFF_AT_S: u64 = 30;

fn records(r: &MemoryRecorder, channel: &str) -> Vec<serde_json::Value> {
    r.records()
        .iter()
        .filter(|(_, rec)| rec.channel == channel)
        .map(|(_, rec)| serde_json::from_slice(&rec.json).expect("json"))
        .collect()
}

#[test]
#[ignore = "a Midtown run of 90 s; cargo test -p v2xw-engine --test live_rsu -- --ignored"]
fn a_unit_broadcasts_a_list_only_once_its_backhaul_brought_it_and_not_while_it_is_down() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/live-rsu/scms-lat2000-bc3.yaml"))
        .expect("the live-rsu scenario loads");
    s.time.duration_s = 90.0;
    let mut params = BTreeMap::new();
    params.insert("target".to_string(), serde_json::json!(OFF_UNIT));
    s.events.push(TimelineItem {
        t: OFF_AT_S as f64,
        until: Some(90.0),
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
