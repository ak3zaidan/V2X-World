//! The class power rows (WP-25, `VehicleClass::dynamics`) on a real climb: one vehicle at
//! a time on `worlds::graded_ring`, a 4 km 5 % climb, with `traffic.vehicle_dynamics`
//! grade and power_limit on (docs/design/research/grade-dynamics.md).

use v2xw_core::rng::RngRegistry;
use v2xw_mobility::{
    EngineParams, IdmPreset, IntersectionMode, Mobility, MobilityCtx, NativeMobility, NoDemand,
    VehicleClass, VehicleDynamicsParams,
};

/// The speed of one `class` vehicle `at_m` metres into the 5 % climb, entered at
/// `v_start` m/s.
fn climb_speed(
    class: VehicleClass,
    dynamics: VehicleDynamicsParams,
    v_start: f64,
    at_m: f64,
) -> f64 {
    let world = v2xw_mobility::worlds::graded_ring(8000.0, 0.05, 25.0).expect("a graded ring");
    let cycle = v2xw_mobility::worlds::ring_cycle(&world, 0);
    let rng = RngRegistry::new(22);
    let params = EngineParams {
        intersections: IntersectionMode::None,
        lane_changes: false,
        dynamic_rerouting: false,
        driver_heterogeneity: false,
        vehicle_dynamics: dynamics,
        ..EngineParams::default()
    };
    let mut engine = NativeMobility::new(params);
    {
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        engine
            .init(&mut ctx, Box::new(NoDemand::new()))
            .expect("init");
    }
    let route = cycle.iter().cycle().take(4).copied().collect();
    let driver = IdmPreset::Kesting2010.profile(class);
    let id = engine
        .spawn_with_route(&world, 0, class, driver, route, 0.0)
        .expect("spawned");
    engine.set_speed(id, v_start).expect("speed");
    let (mut t, mut driven) = (0u64, 0.0);
    for _ in 0..4000 {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        engine.step(&mut ctx, params.step);
        let v = engine.longitudinal_states()[0].3;
        driven += v * params.step.as_secs_f64();
        if driven >= at_m {
            return v;
        }
        t += params.step.as_nanos();
    }
    panic!("{class} did not drive {at_m} m");
}

const BOTH: VehicleDynamicsParams = VehicleDynamicsParams {
    grade: true,
    power_limit: true,
};

#[test]
fn a_rigid_truck_outclimbs_the_design_trailer() {
    // NCHRP 505 Table 30's straight trucks (82.9 lb/hp, 50 kg/kW) against the Green Book
    // design truck (120 kg/kW): 2 km into the climb from the same 23.6 m/s, the trailer is
    // down near its 12.9 m/s crawl and the rigid truck keeps most of its speed (its power
    // crawl is 22.95 m/s; TWOPAS, NCHRP 505 App. E, gives 22.58).
    let truck = climb_speed(VehicleClass::Truck, BOTH, 23.6, 2000.0);
    let trailer = climb_speed(VehicleClass::Trailer, BOTH, 23.6, 2000.0);
    let truck_grade_only = climb_speed(
        VehicleClass::Truck,
        VehicleDynamicsParams {
            grade: true,
            power_limit: false,
        },
        23.6,
        2000.0,
    );
    eprintln!(
        "2 km in: truck {truck:.3}, trailer {trailer:.3}, truck grade-only {truck_grade_only:.3} m/s"
    );
    assert!(trailer < 14.0, "trailer {trailer}");
    assert!(
        truck > trailer + 6.0,
        "truck {truck} against trailer {trailer}"
    );
    assert!(truck > 20.0, "truck {truck}");
}
