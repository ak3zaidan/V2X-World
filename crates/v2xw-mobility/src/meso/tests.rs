//! The queue network's laws, one test each, and the tiered coupling end to end.

use super::tiered::{TierRegion, TieredMobility, TieredParams};
use super::*;
use crate::ctx::MobilityCtx;
use crate::demand::poisson::{PoissonDemand, PoissonParams};
use crate::demand::OdParams;
use crate::engine::EngineParams;
use crate::worlds::{RingParams, ring, ring_cycle};
use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{NS_PER_MS, NS_PER_S};
use v2xw_world::{ImportOptions, procedural::GridParams};

const STEP: Duration = Duration::from_millis(100);

/// A one-lane, 1 km ring at 10 m/s: eight 125 m edges, two 62.5 m segments each.
fn ring_world() -> World {
    ring(&RingParams {
        speed_limit_mps: 10.0,
        ..RingParams::default()
    })
    .expect("ring")
}

fn car(id: u32, lanes: Vec<LaneId>) -> MesoVehicle {
    MesoVehicle {
        id: ActorId::new(id),
        seq: u64::from(id),
        class: VehicleClass::Passenger,
        driver: DriverProfile {
            desired_speed_mps: 30.0,
            ..class_driver(VehicleClass::Passenger)
        },
        traits: DriverTraits::NEUTRAL,
        route: Route {
            lanes,
            length_m: 0.0,
            cost_s: 0.0,
        },
        route_index: 0,
        segment: 0,
        entered: 0,
        earliest_exit: 0,
        entry_frac: 0.0,
        spawned: 0,
    }
}

/// Steps `core` from `from` for `steps` mobility steps, returning every exit.
fn run(core: &mut MesoCore, world: &World, from: SimTime, steps: u64) -> Vec<MesoExit> {
    let mut out = Vec::new();
    for k in 0..steps {
        let t0 = from + k * STEP.as_nanos();
        out.extend(core.advance(world, t0, STEP.after(t0), &mut Closed));
    }
    out
}

#[test]
fn a_free_segment_is_crossed_at_free_flow() {
    let world = ring_world();
    let lanes = ring_cycle(&world, 0);
    let mut core = MesoCore::new(&world, MesoParams::default(), ClassMask::MOTOR_TRAFFIC);
    let seg_len = core.segments()[0].length_m;
    assert!((seg_len - 62.5).abs() < 0.5, "segment {seg_len} m");
    core.insert(&world, car(0, lanes.clone()), 0.0, 0, false).expect("room");
    let v = core.vehicle(ActorId::new(0)).expect("in");
    // 62.5 m at the 10 m/s limit: 6.25 s.
    assert_eq!(v.earliest_exit, Duration::from_secs_f64(seg_len / 10.0).as_nanos());
    run(&mut core, &world, 0, 63);
    let v = core.vehicle(ActorId::new(0)).expect("still in");
    assert_eq!(v.segment, 1, "crossed exactly one segment in 6.3 s");
    assert_eq!(v.entered, Duration::from_secs_f64(seg_len / 10.0).as_nanos());
    // Its published position moves with it.
    let k = core.state_of(&world, v.id, 63 * STEP.as_nanos()).expect("state");
    assert!((k.ground_speed_mps() - 10.0).abs() < 1e-6);
}

#[test]
fn a_queue_discharges_at_the_jam_headway() {
    let world = ring_world();
    let lanes = ring_cycle(&world, 0);
    let mut core = MesoCore::new(&world, MesoParams::default(), ClassMask::MOTOR_TRAFFIC);
    // Seven cars stacked at the end of segment 0 (52.5 m of 62.5): jammed, the next
    // segment free.
    for i in 0..7 {
        core.insert(&world, car(i, lanes.clone()), 62.0, 0, true).expect("forced");
    }
    assert!(core.segments()[0].jammed());
    // Each leaves a jam-free headway after the one before: 1.73 s.
    let mut left = Vec::new();
    for k in 0..200u64 {
        let t0 = k * STEP.as_nanos();
        core.advance(&world, t0, STEP.after(t0), &mut Closed);
        for v in core.vehicles() {
            if v.segment == 1 && !left.iter().any(|(id, _)| *id == v.id) {
                left.push((v.id, v.entered));
            }
        }
    }
    assert!(left.len() >= 4, "{} left", left.len());
    // The first departures leave the queue jammed; their spacing is τ_jf.
    let gap = ns_to_secs(left[2].1 - left[1].1);
    assert!((gap - 1.73).abs() < 1e-6, "headway {gap} s");
    // Once the occupancy falls under the jam threshold the headway is τ_ff.
    let last = ns_to_secs(left[left.len() - 1].1 - left[left.len() - 2].1);
    assert!((last - 1.13).abs() < 1e-6, "free headway {last} s");
}

#[test]
fn a_full_segment_holds_back_the_one_upstream() {
    let world = ring_world();
    let lanes = ring_cycle(&world, 0);
    let mut core = MesoCore::new(&world, MesoParams::default(), ClassMask::MOTOR_TRAFFIC);
    // Fill segment 1 to capacity with cars that never leave (a red forever stands in for
    // them: give them a route that ends nowhere they can reach in the test's time).
    let cap = core.segments()[1].capacity_m;
    let fp = car(0, lanes.clone()).footprint_m();
    let n = (cap / fp).floor() as u32;
    for i in 0..n {
        core.insert(&world, car(100 + i, lanes.clone()), 62.5 + 62.0, 0, false).expect("room");
    }
    assert!(!core.can_enter(1, fp));
    core.insert(&world, car(0, lanes.clone()), 61.0, 0, false).expect("room upstream");
    // Freeze segment 1's queue by closing its gate for the length of the test.
    core.segments[1].free_at = 1_000 * NS_PER_S;
    run(&mut core, &world, 0, 50);
    let v = core.vehicle(ActorId::new(0)).expect("in");
    assert_eq!(v.segment, 0, "held at the head of segment 0 while segment 1 is full");
    let k = core.state_of(&world, v.id, 50 * STEP.as_nanos()).expect("state");
    assert_eq!(k.ground_speed_mps(), 0.0, "and standing");
}

fn grid() -> World {
    v2xw_world::procedural::grid(&GridParams::legacy().with_signals(true), &ImportOptions::default())
        .expect("grid")
}

fn demand(world: &World) -> Box<dyn Demand> {
    Box::new(
        PoissonDemand::new(
            world,
            PoissonParams {
                arrival_rate_per_s: 1.5,
                duration: Duration::from_secs(300),
                ..PoissonParams::default()
            },
            OdParams::default(),
        )
        .expect("demand"),
    )
}

fn tiered(world: &World) -> TieredMobility {
    let b = world.bbox;
    let (cx, cy) = (0.5 * (b.min.x + b.max.x), 0.5 * (b.min.y + b.max.y));
    let r = 0.25 * (b.max.x - b.min.x).min(b.max.y - b.min.y);
    TieredMobility::new(
        TieredParams {
            region: TierRegion::Circle {
                centre_m: [cx, cy],
                radius_m: r,
            },
            meso: MesoParams::default(),
        },
        NativeMobilityAlias::new(EngineParams::default()),
    )
}

use crate::engine::NativeMobility as NativeMobilityAlias;

/// Runs the tiered model for `secs` and returns every update.
fn run_tiered(world: &World, secs: u64) -> (Vec<MobilityUpdate>, tiered::TierCounters) {
    let rng = RngRegistry::new(7);
    let mut m = tiered(world);
    {
        let mut ctx = MobilityCtx::new(0, world, &rng);
        m.init(&mut ctx, demand(world)).expect("init");
    }
    let mut out = Vec::new();
    let mut t = 0;
    while t < secs * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, world, &rng);
        out.push(m.step(&mut ctx, STEP));
        t += 100 * NS_PER_MS;
    }
    (out, m.counters())
}

#[test]
fn a_vehicle_crosses_into_the_region_and_out_again_as_one_actor() {
    let world = grid();
    let (updates, counters) = run_tiered(&world, 300);
    let peak = updates.iter().map(|u| u.states.len()).max().unwrap_or(0);
    eprintln!("tiered grid: {counters:?}, peak {peak} vehicles");
    assert!(counters.handed_in > 0, "{counters:?}");
    assert!(counters.handed_out > 0, "{counters:?}");
    let mut alive: BTreeSet<ActorId> = BTreeSet::new();
    let mut gone: BTreeSet<ActorId> = BTreeSet::new();
    for u in &updates {
        // I-M1, and one state per actor whichever tier drives it.
        let ids: Vec<ActorId> = u.states.iter().map(|(a, _)| *a).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "ordered and unique at {}", u.t);
        for s in &u.spawned {
            assert!(!gone.contains(&s.actor) && alive.insert(s.actor), "spawned once: {:?}", s.actor);
        }
        for (a, _) in &u.despawned {
            assert!(alive.remove(a), "despawned once, after a spawn: {a:?}");
            gone.insert(*a);
        }
        // Everything published was announced and has not left.
        for a in &ids {
            assert!(alive.contains(a), "{a:?} published without a spawn at {}", u.t);
        }
        // And no vehicle jumps: a crossing keeps the position continuous.
    }
    for w in updates.windows(2) {
        let before: BTreeMap<ActorId, Kinematics> = w[0].states.iter().copied().collect();
        for (a, k) in &w[1].states {
            if let Some(p) = before.get(a) {
                let d = p.pos.distance_2d(k.pos);
                assert!(d < 25.0, "{a:?} moved {d} m in one step at {}", k.t);
            }
        }
    }
    assert!(!gone.is_empty(), "some trips completed");
}

#[test]
fn the_tiered_run_is_deterministic() {
    let world = grid();
    let (a, ca) = run_tiered(&world, 120);
    let (b, cb) = run_tiered(&world, 120);
    assert_eq!(ca, cb);
    assert_eq!(a, b);
}

use std::collections::BTreeSet;

/// The microscopic decide pass runs on the pool (`crate::par`); one, two and four threads
/// must publish the same run, bit for bit, with enough vehicles that the pool is used.
#[test]
fn the_micro_step_is_identical_on_one_two_and_four_threads() {
    let world = grid();
    let run = |threads: usize| -> (Vec<MobilityUpdate>, usize) {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool");
        pool.install(|| {
            let rng = RngRegistry::new(3);
            let mut m = NativeMobilityAlias::new(EngineParams::default());
            let demand = PoissonDemand::new(
                &world,
                PoissonParams {
                    arrival_rate_per_s: 4.0,
                    duration: Duration::from_secs(90),
                    ..PoissonParams::default()
                },
                OdParams::default(),
            )
            .expect("demand");
            {
                let mut ctx = MobilityCtx::new(0, &world, &rng);
                m.init(&mut ctx, Box::new(demand)).expect("init");
            }
            let mut out = Vec::new();
            let mut peak = 0;
            let mut t = 0;
            while t < 90 * NS_PER_S {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                let u = m.step(&mut ctx, STEP);
                peak = peak.max(u.states.len());
                out.push(u);
                t += 100 * NS_PER_MS;
            }
            (out, peak)
        })
    };
    let (one, peak) = run(1);
    assert!(
        peak >= crate::par::PAR_MIN_ITEMS,
        "{peak} vehicles at most: the pool was never used, so this proves nothing"
    );
    let (two, _) = run(2);
    let (four, _) = run(4);
    assert!(one == two, "two threads changed the run");
    assert!(one == four, "four threads changed the run");
}
