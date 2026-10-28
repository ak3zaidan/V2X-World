//! WP-17 P17.9: the SPaT a NEMA dual-ring junction publishes stays consistent with the
//! controller. Every change of every signal group happens inside the
//! [`minEndTime`, `maxEndTime`] window the SPaT stated the step before, and while the
//! calls do not change, `likelyTime` is within the projection's step of the change.

use std::collections::{BTreeMap, BTreeSet};

use v2xw_core::geo::GeoOrigin;
use v2xw_core::ids::LaneId;
use v2xw_core::time::WallClock;
use v2xw_engine::infra::IntersectionFeed;
use v2xw_engine::nema::control::{NemaControllers, PROJECTION_STEP_S, PedestrianIndex};
use v2xw_engine::scenario::schema::{SignalControl, SignalControlKind};
use v2xw_world::ImportOptions;
use v2xw_world::procedural::{GridParams, grid};

/// Tenths of the hour to seconds; `None` for J2735's "unknown".
fn secs(tenths: u16) -> Option<f64> {
    (tenths != v2xw_msg::j2735::spat::TIME_MARK_UNKNOWN && tenths < 36_000)
        .then(|| f64::from(tenths) / 10.0)
}

#[test]
fn published_changes_fall_inside_the_spat_window() {
    let mut world = grid(
        &GridParams::legacy().with_size(3, 3).with_signals(true),
        &ImportOptions::default(),
    )
    .expect("grid");
    let control = SignalControl {
        control: SignalControlKind::Nema,
        ..SignalControl::default()
    };
    let mut ctl = NemaControllers::new(&control, &world);
    let idx = ctl.plan_indices().next().expect("a dual-ring plan");
    let mut feed = IntersectionFeed::build(&world, idx, GeoOrigin::NULL_ISLAND).expect("feed");
    let wall = WallClock::parse_rfc3339("2026-06-01T08:00:00Z").expect("t0");
    let junction = world.roads.junction(world.signals[idx].junction).clone();
    let lanes: Vec<LaneId> = junction.incoming.clone();
    let dt = 0.1;
    let none = PedestrianIndex::default();
    // Per group: (state, min, max, likely) published at the previous step.
    let mut last: BTreeMap<u8, (u64, f64, f64, f64)> = BTreeMap::new();
    let mut calls_since = 0.0;
    let mut prev_calls: BTreeSet<LaneId> = BTreeSet::new();
    let (mut changes, mut likely_checked, mut inside) = (0, 0, 0);
    for step in 0..12_000u64 {
        let now = step as f64 * dt;
        // A keyed draw per lane and 6 s window: queues come and go.
        let calls: BTreeSet<LaneId> = lanes
            .iter()
            .copied()
            .filter(|l| {
                let h = (u64::from(l.index()) + 3).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ (step / 60).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                (h >> 60) < 7
            })
            .collect();
        if calls != prev_calls {
            calls_since = now;
            prev_calls = calls.clone();
        }
        ctl.step(&mut world, &calls, &calls, &none, now, dt);
        feed.sync(&world.signals[idx]);
        feed.set_actuation(ctl.outlook(idx, now));
        let t_ns = (now * 1e9).round() as u64;
        let spat = feed.spat_with(t_ns, wall, None);
        for m in &spat.intersections[0].states {
            let e = &m.events[0];
            let state = e.event_state.index();
            let timing = e.timing.as_ref().expect("timed");
            if let Some(&(was, min, max, likely)) = last.get(&m.signal_group)
                && was != state
            {
                changes += 1;
                let ok = now + 1e-6 >= min && now <= max + dt + 1e-6;
                assert!(
                    ok,
                    "group {} changed at {now} s outside [{min}, {max}]",
                    m.signal_group
                );
                inside += 1;
                if now - calls_since > 2.0 * PROJECTION_STEP_S + 1.0 {
                    likely_checked += 1;
                    assert!(
                        (now - likely).abs() <= PROJECTION_STEP_S + 2.0 * dt + 1e-6,
                        "group {} changed at {now} s, likely {likely} s",
                        m.signal_group
                    );
                }
            }
            // Seconds into the hour: t0 is on the hour, and the run is under one.
            let min = secs(timing.min_end_time).unwrap_or(0.0);
            let max = timing.max_end_time.and_then(secs).unwrap_or(f64::INFINITY);
            let likely = timing.likely_time.and_then(secs).unwrap_or(f64::NAN);
            last.insert(m.signal_group, (state, min, max, likely));
        }
    }
    assert!(changes > 100, "{changes} group changes");
    assert_eq!(inside, changes);
    assert!(likely_checked > 10, "{likely_checked} likely times checked");
}
