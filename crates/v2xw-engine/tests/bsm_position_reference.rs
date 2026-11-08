//! A BSM reports the centre of the
//! vehicle rectangle, the SAE J2945/1 (2020) 6.2.3 `COORDSYSREF-001` position reference.
//!
//! Mobility publishes each vehicle at its rear bumper (`VehicleClass::published_reference`).
//! The engine moves a vehicle's own GNSS truth to the centre (`run::own_position_truth`, in
//! `update_beliefs`), so its BSM carries the centre once the receiver's error is taken out.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use v2xw_core::belief::PositionEstimate;
use v2xw_core::card::ModelCard;
use v2xw_core::geom::Vec3;
use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::kinematics::Kinematics;
use v2xw_core::model::Model;
use v2xw_core::rng::RngRegistry;
use v2xw_engine::{Engine, MemoryRecorder, Scenario};
use v2xw_metrics::channels::{GtKinematicsView, NodeTxView, decode};
use v2xw_mobility::worlds::RingParams;
use v2xw_mobility::{
    EngineParams, GnssEnv, GnssModel, IdmPreset, IntersectionMode, MobCtx, Mobility, MobilityCtx,
    NativeMobility, NoDemand, VehicleClass,
};
use v2xw_threat::consistency::{Footprint, ReferencePoint, overlap_ratio};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// A receiver with no error: its fix is the truth it is handed.
struct Exact(ModelCard);

impl Model for Exact {
    fn card(&self) -> &ModelCard {
        &self.0
    }
}

impl GnssModel for Exact {
    fn estimate(
        &mut self,
        _ctx: &mut dyn MobCtx,
        _node: NodeId,
        gt: &Kinematics,
        _env: &GnssEnv,
    ) -> PositionEstimate {
        PositionEstimate::perfect(gt)
    }
}

fn exact() -> Box<dyn GnssModel> {
    let card = v2xw_mobility::GaussMarkovGnss::default().card().clone();
    Box::new(Exact(card))
}

#[test]
fn a_bsm_reports_the_centre_of_the_vehicle_rectangle() {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("loads");
    s.time.duration_s = 6.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(3000.0);
    let mut engine = Engine::build(s, "").expect("builds");
    engine.set_gnss_for_test(exact());
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs");

    // Each node's published state at every mobility step.
    let mut gt: BTreeMap<NodeId, Vec<GtKinematicsView>> = BTreeMap::new();
    let mut bsms: Vec<NodeTxView> = Vec::new();
    for (_, r) in recorder.records() {
        if r.channel == "gt.kinematics" {
            let k: GtKinematicsView = decode(r).expect("gt.kinematics decodes");
            if let Some(n) = k.node {
                gt.entry(n).or_default().push(k);
            }
        } else if r.channel == "node.tx" {
            let t: NodeTxView = decode(r).expect("node.tx decodes");
            if t.msg_type.as_deref() == Some("bsm") {
                bsms.push(t);
            }
        }
    }
    let car = VehicleClass::Passenger;
    let want = 0.5 * car.dims().length_m;
    let mut checked = 0;
    let mut worst: f64 = 0.0;
    for t in &bsms {
        let c = t.content.as_ref().expect("a BSM carries its content");
        let (Some(x), Some(y)) = (c.claimed_x_m, c.claimed_y_m) else {
            continue;
        };
        let at = t.t_generated.unwrap_or(t.t);
        let Some(states) = gt.get(&t.node) else {
            continue;
        };
        // The node's fix is one of the two published states around the BSM: the last one
        // at or before it, dead reckoned to the BSM's instant along its own velocity
        // (`run_nodes`), or, when the mobility step that ran before the node's phase has
        // already published the state at the step's end, that state as it is (a belief
        // newer than the instant is not reckoned back). Either way the offset from the
        // published point is the half length along the heading, and nothing across it.
        let before = states.iter().rev().find(|k| k.t <= at);
        let after = states.iter().find(|k| k.t > at);
        let mut best: Option<f64> = None;
        for (k, reckon) in [(before, true), (after, false)] {
            let Some(k) = k else { continue };
            if k.class.as_deref() != Some(car.as_str()) {
                continue;
            }
            let Some(h) = k.heading_rad else { continue };
            let dt = if reckon {
                (at - k.t) as f64 * 1e-9
            } else {
                0.0
            };
            let (sin_h, cos_h) = v2xw_core::math::sin_cos(h);
            let ex = k.x_m + k.speed_mps * dt * cos_h;
            let ey = k.y_m + k.speed_mps * dt * sin_h;
            let along = (x - ex) * cos_h + (y - ey) * sin_h;
            let across = -(x - ex) * sin_h + (y - ey) * cos_h;
            let e = (along - want).abs().max(across.abs());
            best = Some(best.map_or(e, |b: f64| b.min(e)));
        }
        let Some(e) = best else { continue };
        worst = worst.max(e);
        checked += 1;
    }
    assert!(
        checked > 100,
        "only {checked} passenger-car BSMs were checked"
    );
    assert!(
        worst <= 0.01,
        "a passenger car's BSM is {worst:.3} m off the centre of its rectangle, {want} m ahead \
         of the rear bumper mobility publishes"
    );
}

/// An 8 km single-lane ring whose first 62.5 m are straight (as the `v2xw-mobility`
/// fixture `tests/position_reference.rs`).
fn place(vehicles: &[(VehicleClass, f64)]) -> Vec<Kinematics> {
    let world = v2xw_mobility::worlds::ring(&RingParams::of_length(8000.0)).expect("a ring");
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
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        engine
            .init(&mut ctx, Box::new(NoDemand::new()))
            .expect("init");
    }
    let route = v2xw_mobility::worlds::ring_cycle(&world, 0);
    let ids: Vec<ActorId> = vehicles
        .iter()
        .map(|&(class, s_front)| {
            let driver = IdmPreset::Kesting2010.profile(class);
            let id = engine
                .spawn_with_route(&world, 0, class, driver, route.clone(), s_front)
                .expect("spawned");
            engine.set_speed(id, 0.0).expect("at rest");
            id
        })
        .collect();
    let mut ctx = MobilityCtx::new(0, &world, &rng);
    let update = engine.step(&mut ctx, params.step);
    ids.iter()
        .map(|id| {
            update
                .states
                .iter()
                .find(|(a, _)| a == id)
                .map(|(_, k)| *k)
                .expect("a published state")
        })
        .collect()
}

#[test]
fn a_car_queued_behind_a_bus_does_not_overlap_it_in_their_bsms() {
    let (bus, car) = (VehicleClass::Bus, VehicleClass::Passenger);
    let lb = bus.dims().length_m;
    // The car's front bumper 1.0 m behind the bus's rear bumper, both at rest.
    let placed = place(&[(bus, 50.0), (car, 50.0 - lb - 1.0)]);
    let [kb, kc] = placed[..] else {
        panic!("a bus and a car")
    };
    // What each one's BSM says with the receiver's error taken out: the engine's own
    // position truth, read by a receiver as the centre of the footprint.
    let footprint = |class: VehicleClass, k: &Kinematics| {
        let p: Vec3 = v2xw_engine::run::own_position_truth(class, k).pos;
        let d = class.dims();
        Footprint::from_claim(
            p.x,
            p.y,
            k.heading_rad,
            d.length_m,
            d.width_m,
            ReferencePoint::Centre,
        )
    };
    let (fb, fc) = (footprint(bus, &kb), footprint(car, &kc));
    let overlap = overlap_ratio(&fb, &fc);
    assert!(
        overlap == 0.0,
        "the car's and the bus's BSM footprints overlap ({overlap:.3}) with a 1.0 m gap"
    );
    // And the gap the two BSMs give is that metre.
    let (sin_h, cos_h) = v2xw_core::math::sin_cos(kc.heading_rad);
    let ahead = (fb.cx - fc.cx) * cos_h + (fb.cy - fc.cy) * sin_h;
    let gap = ahead - 0.5 * (lb + car.dims().length_m);
    assert!((gap - 1.0).abs() < 0.1, "the BSMs put a {gap:.3} m gap");
}
