//! WP-15: the complete `.rou.xml` import.
//!
//! * A vType's `accel`, `decel`, `tau`, `sigma` and `minGap` reach the actor the native
//!   engine puts on the road (and its stated `departSpeed` and `speedFactor`).
//! * A sample of the LuST scenario (MIT licence, `worlds/sumo/lust-sample.rou.xml`) imports
//!   with every attribute counted in the ledger: none is dropped silently.

use std::path::PathBuf;

use v2xw_core::rng::RngRegistry;
use v2xw_core::time::Duration;
use v2xw_interop::routes::{RouteFile, RouteFileDemand};
use v2xw_mobility::traits::Mobility;
use v2xw_mobility::{EngineParams, MobilityCtx, NativeMobility};
use v2xw_world::sumo::{SumoOptions, import_sumo_net};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn a_vtype_reaches_the_actor() {
    let opts = SumoOptions::default().imported_at("1970-01-01T00:00:00Z");
    let (world, _) = import_sumo_net(root().join("worlds/sumo/grid3.net.xml"), &opts).unwrap();
    let xml = r#"<routes>
  <vType id="calibrated" vClass="passenger" accel="1.1" decel="3.3" tau="1.7" sigma="0.2" minGap="3.1" maxSpeed="12" speedFactor="normc(1.2,0.05,1.0,1.4)"/>
  <vehicle id="a" type="calibrated" depart="0" departSpeed="5" departLane="0"><route edges="A1B1 B1C1"/></vehicle>
  <vehicle id="b" depart="0"><route edges="B0B1 B1B2"/></vehicle>
</routes>"#;
    let file = RouteFile::parse(xml).unwrap();
    let rng = RngRegistry::new(3);
    let mut engine = NativeMobility::new(EngineParams::default());
    let mut ctx = MobilityCtx::new(0, &world, &rng);
    engine
        .init(&mut ctx, Box::new(RouteFileDemand::new(file)))
        .unwrap();
    let mut spawned = Vec::new();
    let mut t = 0;
    for _ in 0..5 {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let u = engine.step(&mut ctx, Duration::from_millis(100));
        t = u.t;
        spawned.extend(u.spawned);
    }
    assert_eq!(spawned.len(), 2, "{spawned:?}");
    let a = spawned.iter().find(|s| s.seq == 0).unwrap();
    assert_eq!(a.driver.max_accel_mps2, 1.1);
    assert_eq!(a.driver.comfort_decel_mps2, 3.3);
    assert_eq!(a.driver.time_headway_s, 1.7);
    assert_eq!(a.driver.min_gap_m, 3.1);
    assert_eq!(a.driver.sigma, Some(0.2));
    assert_eq!(a.driver.desired_speed_mps, 12.0);
    // departSpeed 5 on an empty lane.
    let speed = |k: &v2xw_core::kinematics::Kinematics| k.vel.x.hypot(k.vel.y);
    assert!((speed(&a.kinematics) - 5.0).abs() < 1e-9, "{}", speed(&a.kinematics));
    // A vehicle with no vType drives SUMO's DEFAULT_VEHTYPE for its class: the passenger
    // car's 2.6 / 4.5 m/s², tau 1 s and the class's minGap, with no sigma of its own.
    let b = spawned.iter().find(|s| s.seq == 1).unwrap();
    let base = v2xw_mobility::carfollowing::krauss::Krauss::class_profile(
        v2xw_mobility::VehicleClass::Passenger,
    );
    assert_eq!(b.driver.max_accel_mps2, base.max_accel_mps2);
    assert_eq!(b.driver.comfort_decel_mps2, base.comfort_decel_mps2);
    assert_eq!(b.driver.time_headway_s, 1.0);
    assert_eq!(b.driver.sigma, None);
    assert_eq!(speed(&b.kinematics), 0.0);
}

/// The Krauss model dawdles with the driver's own `sigma` when the vType states one.
#[test]
fn the_krauss_model_reads_the_vtype_sigma() {
    use v2xw_mobility::carfollowing::krauss::{Krauss, KraussParams};
    let k = Krauss::new(KraussParams::default(), 1);
    let mut d = Krauss::class_profile(v2xw_mobility::VehicleClass::Passenger);
    // Model-wide sigma 0.5: a dawdle at eta 0.9.
    let free = k.next_speed(10.0, 30.0, None, &d, 0.1, 0.0);
    assert!(k.next_speed(10.0, 30.0, None, &d, 0.1, 0.9) < free);
    // The type's sigma 0: no dawdle.
    d.sigma = Some(0.0);
    assert_eq!(k.next_speed(10.0, 30.0, None, &d, 0.1, 0.9), free);
}

/// Attributes in `xml` outside comments, counted on the text.
fn attributes_in(xml: &str) -> u64 {
    let mut n = 0;
    let mut rest = xml;
    while let Some(i) = rest.find("<!--") {
        n += rest[..i].matches("=\"").count();
        rest = &rest[i..];
        let j = rest.find("-->").expect("closed comment");
        rest = &rest[j + 3..];
    }
    (n + rest.matches("=\"").count()) as u64
}

#[test]
fn the_lust_sample_imports_with_nothing_dropped_silently() {
    let path = root().join("worlds/sumo/lust-sample.rou.xml");
    let xml = std::fs::read_to_string(&path).unwrap();
    let f = RouteFile::read(&path).unwrap();
    let l = &f.ledger;
    assert_eq!(l.total(), attributes_in(&xml), "{l:#?}");
    // Six private types in a distribution and the bus.
    assert_eq!(f.vtypes.len(), 7);
    assert_eq!(f.vehicles.len(), 23);
    for k in ["accel", "decel", "sigma", "minGap", "maxSpeed", "speedDev", "probability"] {
        let key = format!("vType@{k}");
        let want = if k == "probability" { 6 } else { 7 };
        assert_eq!(l.applied.get(&key), Some(&want), "{key}: {l:#?}");
    }
    let p2a = &f.vtypes["passenger2a"];
    assert_eq!(p2a.accel_mps2, Some(3.0));
    assert_eq!(p2a.min_gap_m, Some(1.5));
    assert_eq!(p2a.speed_factor.map(|s| s.dev), Some(0.1));
    // What the engine does not model is said, by name.
    assert_eq!(l.not_modelled.get("vType@length"), Some(&7));
    assert_eq!(l.not_modelled.get("vehicle@arrivalPos"), Some(&23));
    assert_eq!(l.not_modelled.get("vehicle@arrivalSpeed"), Some(&20));
    assert!(l.elements_not_modelled.get("stop").copied().unwrap_or(0) > 0);
    assert_eq!(l.cosmetic.get("vType@guiShape"), Some(&7));
    // LuST's keywords are all supported: random positions, best lanes, numeric speeds.
    assert!(l.approximated.is_empty(), "{:?}", l.approximated);
    let first = f.vehicles.iter().find(|v| v.id == "randUni18368:1").unwrap();
    assert_eq!(first.vtype.as_deref(), Some("passenger2a"));
    assert!(matches!(
        first.depart_pos,
        v2xw_interop::routes::DepartPos::Random(_)
    ));
    let d = f.trip_driver(first);
    assert_eq!(d.max_accel_mps2, Some(3.0));
    assert_eq!(d.sigma, Some(0.5));
}
