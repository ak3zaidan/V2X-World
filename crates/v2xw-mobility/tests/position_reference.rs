//! The centre of the vehicle rectangle, the SAE J2945/1
//! 6.2.3 `COORDSYSREF-001` BSM position, found from the state a real mobility tier
//! publishes.
//!
//! The fixture places vehicles with `NativeMobility` on a straight stretch of a large ring,
//! takes the [`Kinematics`] it publishes, and moves each by
//! [`VehicleClass::rectangle_centre`]. The truth it is checked against is the body itself:
//! the midpoint of the published rear and the front bumper the tier drives. The engine
//! adds the same shift to the GNSS truth (`update_beliefs`), so these are the positions its
//! BSMs carry with GNSS noise off.

use v2xw_core::geom::Vec3;
use v2xw_core::ids::ActorId;
use v2xw_core::kinematics::Kinematics;
use v2xw_core::rng::RngRegistry;
use v2xw_mobility::worlds::RingParams;
use v2xw_mobility::{
    EngineParams, IdmPreset, IntersectionMode, Mobility, MobilityCtx, NativeMobility, NoDemand,
    VehicleClass,
};
use v2xw_world::World;

/// An 8 km single-lane ring: 128 sides of 62.5 m, so the first 62.5 m of its first lane
/// are straight.
fn ring() -> World {
    v2xw_mobility::worlds::ring(&RingParams::of_length(8000.0)).expect("a ring")
}

/// Places one stationary vehicle per `(class, front bumper arc length)` on the ring's
/// first lane, steps once, and returns each one's published state with its front bumper.
fn place(world: &World, vehicles: &[(VehicleClass, f64)]) -> Vec<(Kinematics, Vec3)> {
    let rng = RngRegistry::new(4);
    let params = EngineParams {
        intersections: IntersectionMode::None,
        lane_changes: false,
        dynamic_rerouting: false,
        driver_heterogeneity: false,
        ..EngineParams::default()
    };
    let mut engine = NativeMobility::new(params);
    {
        let mut ctx = MobilityCtx::new(0, world, &rng);
        engine
            .init(&mut ctx, Box::new(NoDemand::new()))
            .expect("init");
    }
    let route = v2xw_mobility::worlds::ring_cycle(world, 0);
    let ids: Vec<ActorId> = vehicles
        .iter()
        .map(|&(class, s_front)| {
            let driver = IdmPreset::Kesting2010.profile(class);
            let id = engine
                .spawn_with_route(world, 0, class, driver, route.clone(), s_front)
                .expect("spawned");
            engine.set_speed(id, 0.0).expect("at rest");
            id
        })
        .collect();
    let mut ctx = MobilityCtx::new(0, world, &rng);
    let update = engine.step(&mut ctx, params.step);
    let fronts = engine.longitudinal_states();
    ids.iter()
        .map(|id| {
            let k = update
                .states
                .iter()
                .find(|(a, _)| a == id)
                .map(|(_, k)| *k)
                .expect("a published state");
            let (_, lane, s_front, _) = *fronts.iter().find(|(a, ..)| a == id).expect("a front");
            (k, world.lane(lane).point_at(s_front))
        })
        .collect()
}

#[test]
fn the_rectangle_centre_of_a_published_car_is_the_middle_of_its_body() {
    let world = ring();
    let car = VehicleClass::Passenger;
    let [(k, front)] = place(&world, &[(car, 40.0)])[..] else {
        panic!("one car")
    };
    let rear = k.pos;
    let truth = Vec3::new(0.5 * (rear.x + front.x), 0.5 * (rear.y + front.y), rear.z);
    let c = car.rectangle_centre(&k);
    let err = v2xw_core::math::hypot(c.x - truth.x, c.y - truth.y);
    assert!(
        err < 0.01,
        "the centre is {err:.3} m from the middle of the body ({c:?} vs {truth:?})"
    );
    // And the published point is the rear bumper the shift assumes: a body length behind
    // the front.
    let body = v2xw_core::math::hypot(front.x - rear.x, front.y - rear.y);
    assert!(
        (body - car.dims().length_m).abs() < 0.01,
        "rear to front is {body} m"
    );
}

#[test]
fn a_car_queued_one_metre_behind_a_bus_does_not_overlap_it_at_the_centres() {
    let world = ring();
    let (bus, car) = (VehicleClass::Bus, VehicleClass::Passenger);
    let (lb, lc) = (bus.dims().length_m, car.dims().length_m);
    let placed = place(&world, &[(bus, 50.0), (car, 50.0 - lb - 1.0)]);
    let [(kb, _), (kc, _)] = placed[..] else {
        panic!("a bus and a car")
    };
    // A receiver reads each position as the centre of the body (`apps.rs` gaps,
    // `consistency.rs` footprints): the bumper gap is the centre distance along the
    // follower's heading, less the two half lengths.
    let (cb, cc) = (bus.rectangle_centre(&kb), car.rectangle_centre(&kc));
    let (sin_h, cos_h) = v2xw_core::math::sin_cos(kc.heading_rad);
    let ahead = (cb.x - cc.x) * cos_h + (cb.y - cc.y) * sin_h;
    let gap = ahead - 0.5 * (lb + lc);
    assert!(
        gap > 0.9 && gap < 1.1,
        "the gap read from the two centres is {gap:.3} m, not 1.0 m"
    );
}
