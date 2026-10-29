//! Glances off the road (WP-39, `drivers` §6) in the engine: a follower whose eyes are off
//! the road when the car ahead brakes responds later, by what is left of the glance, and
//! a warning from its unit (`MobilityCommand::DriverAlerted`) brings it back at once.

use v2xw_core::ids::ActorId;
use v2xw_core::rng::RngRegistry;
use v2xw_mobility::drivers::{AttentionParams, HumanDriverParams};
use v2xw_mobility::{
    EngineParams, IdmPreset, IntersectionMode, Mobility, MobilityCommand, MobilityCtx,
    NativeMobility, NoDemand, VehicleClass,
};

/// The engine's step, 100 ms (`EngineParams::default`).
const DT_NS: u64 = 100_000_000;

/// Seconds from the leader's hard brake to the follower's first deceleration of
/// 1 m/s² or more. The pair drives at 20 m/s, 40 m apart, on a flat 25 m/s ring; the
/// leader brakes at 6 m/s² after `warm_s`. With `alert`, the follower's unit warns it from
/// the brake on, every step, as a warning that stands while the leader brakes.
fn brake_response_s(distracted: bool, alert: bool, warm_s: f64) -> f64 {
    let world = v2xw_mobility::worlds::graded_ring(8000.0, 0.0, 25.0).expect("a ring");
    let cycle = v2xw_mobility::worlds::ring_cycle(&world, 0);
    let rng = RngRegistry::new(39);
    let params = EngineParams {
        intersections: IntersectionMode::None,
        lane_changes: false,
        dynamic_rerouting: false,
        driver_heterogeneity: false,
        human: HumanDriverParams {
            reaction_time_s: 0.8,
            attention: AttentionParams {
                enabled: distracted,
                ..AttentionParams::default()
            },
            ..HumanDriverParams::default()
        },
        ..EngineParams::default()
    };
    let mut engine = NativeMobility::new(params);
    {
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        engine
            .init(&mut ctx, Box::new(NoDemand::new()))
            .expect("init");
    }
    let route: Vec<_> = cycle.iter().cycle().take(6).copied().collect();
    let class = VehicleClass::Passenger;
    let driver = IdmPreset::Kesting2010.profile(class);
    let leader = engine
        .spawn_with_route(&world, 0, class, driver, route.clone(), 40.0)
        .expect("leader");
    let follower = engine
        .spawn_with_route(&world, 0, class, driver, route, 0.0)
        .expect("follower");
    for id in [leader, follower] {
        engine.set_speed(id, 20.0).expect("speed");
    }
    let speed = |e: &NativeMobility, id: ActorId| {
        e.longitudinal_states()
            .iter()
            .find(|s| s.0 == id)
            .map(|s| s.3)
            .expect("on the road")
    };
    let brake_at = (warm_s * 1e9) as u64 / DT_NS * DT_NS;
    let mut t = 0u64;
    let mut v_prev = speed(&engine, follower);
    loop {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        if t == brake_at {
            engine.command(
                &mut ctx,
                MobilityCommand::Brake {
                    actor: leader,
                    decel_mps2: 6.0,
                    hold_until: None,
                },
            );
        }
        if alert && t >= brake_at {
            engine.command(&mut ctx, MobilityCommand::DriverAlerted { actor: follower });
        }
        engine.step(&mut ctx, params.step);
        t += DT_NS;
        let v = speed(&engine, follower);
        if t > brake_at && (v_prev - v) / 0.1 >= 1.0 {
            return (t - brake_at) as f64 / 1e9;
        }
        v_prev = v;
        assert!(t < brake_at + 20_000_000_000, "the follower never braked");
    }
}

#[test]
fn a_distracted_follower_brakes_later() {
    // 40 brake instants 0.37 s apart sample the follower's glance process. Each brake is
    // run twice from the same history: as it comes, and with a warning from the
    // follower's unit standing from the brake on, which keeps its eyes on the road. A
    // follower whose eyes are off the road at the brake, or leave it while the closing
    // builds up (the IDM's braking grows over about a second), responds later by what is
    // left of the glance; the warned one never later. With 0.92 s glances every 4.48 s
    // on average that is a third to a half of the brakes.
    assert_eq!(EngineParams::default().step.as_nanos(), DT_NS);
    let (mut later, mut extra_sum) = (0, 0.0);
    for i in 0..40 {
        let warm = 8.0 + 0.37 * f64::from(i);
        let distracted = brake_response_s(true, false, warm);
        let warned = brake_response_s(true, true, warm);
        assert!(
            distracted >= warned - 1e-9,
            "{warm}: distracted {distracted} < warned {warned}"
        );
        if distracted > warned + 0.15 {
            later += 1;
        }
        extra_sum += distracted - warned;
    }
    let mean_extra = extra_sum / 40.0;
    let attentive = brake_response_s(false, false, 9.0);
    eprintln!("later in {later}/40, mean extra {mean_extra:.3} s; attentive at 9 s {attentive}");
    assert!((6..=32).contains(&later), "{later} of 40 responses later");
    assert!(mean_extra > 0.05 && mean_extra < 0.6, "{mean_extra} s");
    // Same seed, same glances: byte-identical.
    assert_eq!(
        brake_response_s(true, false, 9.0).to_bits(),
        brake_response_s(true, false, 9.0).to_bits()
    );
}
