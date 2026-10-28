//! WP-21: condition-triggered timeline items, end to end.
//!
//! An item with a `when` clause is armed at its `t` and fires at the first mobility step
//! at which the clause holds on the run's ground truth, through the same control path a
//! timed item takes; its `scenario.event` record says which firing it is and what the
//! clause bound. Each check was shown red under an injected fault before it was kept
//! (adoption plan WP-21 status notes).

use serde_json::{Value, json};
use v2xw_engine::scenario::schema::TimelineItem;
use v2xw_engine::{Engine, MemoryRecorder, Scenario};

/// A small signalised grid with steady Poisson demand and no radios.
fn grid(duration_s: f64, seed: u64) -> Scenario {
    let mut s = Scenario::minimal();
    s.seed = seed;
    s.actors.vehicles.equipped_fraction = 0.0;
    s.world.source = v2xw_world::WorldSourceSpec::procedural(
        "world/source/procedural-grid",
        json!({
            "cols": 3, "rows": 3, "block_x_m": 150.0, "block_y_m": 150.0,
            "lanes_per_direction": 1, "speed_limit_mps": 13.89, "signalised": true
        }),
    );
    s.time.duration_s = duration_s;
    s.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
    s.actors.vehicles.demand.rate_veh_per_h = Some(1800.0);
    s
}

fn with(mut s: Scenario, item: Value) -> Scenario {
    s.events = vec![serde_json::from_value::<TimelineItem>(item).expect("the item parses")];
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    s
}

/// Every `scenario.event` record, and every record of the run as bytes.
fn run(s: Scenario) -> (Vec<Value>, Vec<u8>) {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    let mut events = Vec::new();
    let mut bytes = Vec::new();
    for (t, r) in rec.records() {
        bytes.extend_from_slice(&t.to_le_bytes());
        bytes.extend_from_slice(r.channel.as_bytes());
        bytes.extend_from_slice(&r.json);
        if r.channel == "scenario.event" {
            events.push(serde_json::from_slice(&r.json).expect("json"));
        }
    }
    (events, bytes)
}

/// `(t ns, firing)` of each start record.
fn firings(events: &[Value]) -> Vec<(u64, Option<u64>)> {
    events
        .iter()
        .filter(|e| e["phase"] == "start")
        .map(|e| (e["t"].as_u64().expect("t"), e["firing"].as_u64()))
        .collect()
}

/// A `probability` clause of 1 fires at the first step at or after `t`, then again each
/// time the cool-down has passed, up to `max_fires`; a clause of 0 never fires, so the
/// item is not on the calendar at `t`. Faults: seeding `when` items at `t` like timed
/// ones (the p = 0 item fires at 2 s); ignoring the cool-down (fires at 2.0, 2.1, 2.2 s).
#[test]
fn a_when_item_fires_on_its_clause_with_its_knobs() {
    let item = |p: f64| {
        json!({"t": 2.0, "type": "weather.front", "value": "rain",
               "when": {"kind": "probability", "p": p}, "max_fires": 3, "cooldown_s": 1.5})
    };
    let (always, _) = run(with(grid(8.0, 7), item(1.0)));
    assert_eq!(
        firings(&always),
        vec![
            (2_000_000_000, Some(1)),
            (3_500_000_000, Some(2)),
            (5_000_000_000, Some(3))
        ],
        "{always:?}"
    );
    let (never, _) = run(with(grid(8.0, 7), item(0.0)));
    assert!(firings(&never).is_empty(), "{never:?}");
    // A timed item is what it was: once, at t, with no firing count.
    let (timed, _) = run(with(
        grid(8.0, 7),
        json!({"t": 2.0, "type": "weather.front", "value": "rain"}),
    ));
    assert_eq!(firings(&timed), vec![(2_000_000_000, None)]);
}

/// A seeded clause replays: the same seed fires at the same steps and the whole record
/// stream is byte-identical; another seed fires at other steps.
#[test]
fn a_probability_clause_replays_byte_identically() {
    let item = json!({"t": 0.5, "type": "weather.front", "value": "rain",
                      "when": {"kind": "probability", "p": 0.2}, "max_fires": 1000});
    let (a, bytes_a) = run(with(grid(6.0, 11), item.clone()));
    let (b, bytes_b) = run(with(grid(6.0, 11), item.clone()));
    assert!(firings(&a).len() > 5, "{} firings", firings(&a).len());
    assert_eq!(firings(&a), firings(&b));
    assert!(bytes_a == bytes_b, "two runs of one seed differ");
    let (c, _) = run(with(grid(6.0, 12), item));
    assert_ne!(firings(&a), firings(&c));
}
