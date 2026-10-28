//! WP-21: condition-triggered timeline items, end to end.
//!
//! An item with a `when` clause is armed at its `t` and fires at the first mobility step
//! at which the clause holds on the run's ground truth, through the same control path a
//! timed item takes; its `scenario.event` record says which firing it is and what the
//! clause bound. Each check was shown red under an injected fault before it was kept
//! (adoption plan WP-21 status notes).

use std::collections::BTreeMap;

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
            "lanes_per_direction": 2, "speed_limit_mps": 13.89, "signalised": true,
            "corner_radius_m": 4.5
        }),
    );
    s.time.duration_s = duration_s;
    s.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
    s.actors.vehicles.demand.rate_veh_per_h = Some(3600.0);
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

// ---- P21.3: the time-to-collision trigger ----------------------------------------------

#[allow(dead_code)]
mod regression;

/// One published vehicle state: lane, distance along it, speed, longitudinal
/// acceleration and body length.
#[derive(Debug, Clone, Copy)]
struct State {
    lane: u32,
    s_m: f64,
    v_mps: f64,
    acc_mps2: f64,
    length_m: f64,
}

type States = BTreeMap<(u64, u32), State>;

/// The run's vehicle states by `(t, actor)`, its `scenario.event` records, and every
/// record as bytes.
fn states_and_events(s: Scenario) -> (States, Vec<Value>, Vec<u8>) {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    let mut states = BTreeMap::new();
    let mut events = Vec::new();
    let mut bytes = Vec::new();
    for (t, r) in rec.records() {
        bytes.extend_from_slice(&t.to_le_bytes());
        bytes.extend_from_slice(&r.json);
        match r.channel {
            "gt.kinematics" => {
                let v: Value = serde_json::from_slice(&r.json).expect("json");
                let class = v["class"].as_str().unwrap_or("");
                // A pedestrian or a cyclist is not a vehicle the trigger reads.
                let Ok(class) = serde_json::from_value::<v2xw_mobility::VehicleClass>(json!(class))
                else {
                    continue;
                };
                let (Some(lane), Some(s_m)) = (v["lane"].as_u64(), v["lane_pos_m"].as_f64()) else {
                    continue;
                };
                states.insert(
                    (
                        v["t"].as_u64().expect("t"),
                        v["actor"].as_u64().expect("actor") as u32,
                    ),
                    State {
                        lane: lane as u32,
                        s_m,
                        v_mps: v["speed_mps"].as_f64().expect("speed"),
                        acc_mps2: v["acc_mps2"].as_f64().unwrap_or(0.0),
                        length_m: class.dims().length_m,
                    },
                );
            }
            "scenario.event" => events.push(serde_json::from_slice(&r.json).expect("json")),
            _ => {}
        }
    }
    (states, events, bytes)
}

/// The time to collision of follower `f` with leader `l` at `t`, recomputed from the
/// records: bumper gap over closing speed, `None` when not closing.
fn ttc(states: &States, t: u64, f: u32, l: u32) -> Option<f64> {
    let (sf, sl) = (states.get(&(t, f))?, states.get(&(t, l))?);
    let closing = sf.v_mps - sl.v_mps;
    (sf.lane == sl.lane && sl.s_m > sf.s_m && closing > 0.0)
        .then(|| (sl.s_m - sf.s_m - sf.length_m) / closing)
}

/// The least TTC of every same-lane adjacent pair at `t`.
fn least_ttc(states: &States, t: u64) -> Option<f64> {
    let mut by_lane: BTreeMap<u32, Vec<(f64, u32)>> = BTreeMap::new();
    for ((_, a), st) in states.range((t, 0)..=(t, u32::MAX)) {
        by_lane.entry(st.lane).or_default().push((st.s_m, *a));
    }
    let mut least: Option<f64> = None;
    for v in by_lane.values_mut() {
        v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for w in v.windows(2) {
            if let Some(x) = ttc(states, t, w[0].1, w[1].1) {
                least = Some(least.map_or(x, |m: f64| m.min(x)));
            }
        }
    }
    least
}

const STEP_NS: u64 = 100_000_000;
/// The records carry speed and position on their D9 grids; the clause read the
/// unquantised state, so the recomputation is held to 10 ms of TTC.
const TOL_S: f64 = 0.01;

fn ttc_scenario(seed: u64) -> Scenario {
    let mut s = grid(40.0, seed);
    // Items 0-3 script a lead vehicle's full ABS stop (8 m/s², UN R13-H dry asphalt) every
    // 6 s from 10 s (the auto pick has a follower within 50 m), because the calibrated
    // drivers alone rarely close below 2 s (seed 3 of this grid: 2.36 s at the least
    // with one 0.5 g stop); item 4 is the trigger under test, armed from 5 s, which a car
    // closing on a queue may also fire first.
    s.events = [
        json!({"t": 10.0, "type": "safety.hard-brake", "decel_mps2": 8.0}),
        json!({"t": 16.0, "type": "safety.hard-brake", "decel_mps2": 8.0}),
        json!({"t": 22.0, "type": "safety.hard-brake", "decel_mps2": 8.0}),
        json!({"t": 28.0, "type": "safety.hard-brake", "decel_mps2": 8.0}),
        json!({
            "t": 5.0, "type": "safety.hard-brake", "target": "when.follower",
            "when": {"kind": "ttc_below", "ttc_s": 2.0}
        }),
    ]
    .into_iter()
    .map(|v| serde_json::from_value::<TimelineItem>(v).expect("parses"))
    .collect();
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    s
}

/// Under five seeds the hard brake fires at the first step at which a follower's time to
/// collision with the vehicle ahead is below 2 s: at that step the bound pair's TTC,
/// recomputed from the published states, is below 2 s, and one step earlier no pair's
/// was; the bound follower then brakes hard. Two seeds are run twice and replay byte for
/// byte. Faults: the comparator turned to `>` (fires on a pair far above 2 s); the clause
/// evaluated after the mobility update, on the next step's states (the bound pair's TTC
/// at the firing instant is not what fired it).
#[test]
fn hard_brake_fires_at_ttc_two_seconds() {
    for seed in 1..=5u64 {
        let (states, events, bytes) = states_and_events(ttc_scenario(seed));
        let fired: Vec<&Value> = events
            .iter()
            .filter(|e| e["phase"] == "start" && e["index"] == 4)
            .collect();
        let vehicles: std::collections::BTreeSet<u32> = states.keys().map(|k| k.1).collect();
        assert_eq!(
            fired.len(),
            1,
            "seed {seed}: {} vehicles, least TTC of the run {:?}; {events:?}",
            vehicles.len(),
            states
                .keys()
                .map(|k| k.0)
                .collect::<std::collections::BTreeSet<u64>>()
                .into_iter()
                .filter_map(|t| least_ttc(&states, t))
                .reduce(f64::min)
        );
        let e = fired[0];
        let t = e["t"].as_u64().expect("t");
        let f = e["follower"].as_u64().expect("a follower is bound") as u32;
        let l = e["leader"].as_u64().expect("a leader is bound") as u32;
        assert!(
            t >= 5_000_000_000,
            "seed {seed}: fired at {t} before it was armed"
        );
        let at = ttc(&states, t, f, l);
        let before = least_ttc(&states, t - STEP_NS);
        eprintln!(
            "seed {seed}: fired at {:.1} s, pair {f}->{l} TTC {at:?} s; least TTC a step \
             earlier {before:?}",
            t as f64 / 1e9
        );
        let at = at.expect("the bound pair is closing at the firing step");
        assert!(at < 2.0 + TOL_S, "seed {seed}: TTC {at} at the firing step");
        if t > 5_000_000_000 {
            assert!(
                before.is_none_or(|b| b >= 2.0 - TOL_S),
                "seed {seed}: a pair was already at TTC {before:?} one step earlier"
            );
        }
        // The bound follower is the vehicle that brakes, at about 0.5 g.
        let hardest = (1..=10)
            .filter_map(|k| states.get(&(t + k * STEP_NS, f)))
            .map(|s| s.acc_mps2)
            .fold(0.0_f64, f64::min);
        assert!(
            hardest <= -4.0,
            "seed {seed}: follower {f} braked only {hardest} m/s²"
        );
        if seed <= 2 {
            let (_, _, again) = states_and_events(ttc_scenario(seed));
            assert!(again == bytes, "seed {seed}: two runs differ");
        }
    }
}

/// The shipped example reproduces its pinned record digest
/// (`tests/regression/ttc-triggered-brake.json`; re-bless with `V2XW_BLESS_SCMS_DIGESTS=1`).
#[test]
fn the_example_scenario_reproduces_its_pinned_digest() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios/edge/ttc-triggered-brake.yaml");
    let s = Scenario::load(path).expect("the example loads");
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    // The trigger (the last item) fires in the example, on a bound pair.
    let (_, events, _) = states_and_events(s.clone());
    let last = s.events.len() - 1;
    assert!(
        events
            .iter()
            .any(|e| e["phase"] == "start" && e["index"] == last && e["follower"].is_u64()),
        "the example's trigger never fired: {events:?}"
    );
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = v2xw_engine::DigestRecorder::new();
    engine.run(&mut rec).expect("runs");
    regression::check(
        "ttc-triggered-brake",
        &regression::Pinned {
            record_digest: rec.digest_hex(),
            frame_digest: rec.frame_digest_hex(),
            records: rec.written(),
        },
    );
}
