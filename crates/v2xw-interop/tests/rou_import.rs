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

/// Runs `xml` on the grid for `steps` of 100 ms: every spawn, and the speed of trip `seq`
/// at each step it is on the road.
fn run_grid(
    xml: &str,
    steps: usize,
    seq: u64,
) -> (Vec<v2xw_mobility::views::ActorSpawn>, Vec<f64>) {
    let opts = SumoOptions::default().imported_at("1970-01-01T00:00:00Z");
    let (world, _) = import_sumo_net(root().join("worlds/sumo/grid3.net.xml"), &opts).unwrap();
    let file = RouteFile::parse(xml).unwrap();
    let rng = RngRegistry::new(3);
    let mut engine = NativeMobility::new(EngineParams::default());
    let mut ctx = MobilityCtx::new(0, &world, &rng);
    engine
        .init(&mut ctx, Box::new(RouteFileDemand::new(file)))
        .unwrap();
    let mut spawned = Vec::new();
    let mut speeds = Vec::new();
    let mut t = 0;
    let mut actor = None;
    for _ in 0..steps {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let u = engine.step(&mut ctx, Duration::from_millis(100));
        t = u.t;
        if let Some(s) = u.spawned.iter().find(|s| s.seq == seq) {
            actor = Some(s.actor);
        }
        spawned.extend(u.spawned);
        if let Some(a) = actor
            && let Some((_, k)) = u.states.iter().find(|(x, _)| *x == a)
        {
            speeds.push(k.vel.x.hypot(k.vel.y));
        }
    }
    (spawned, speeds)
}

/// WP-15 follow-up: a vType's `carFollowModel` is the model its vehicles drive. On a free
/// road from standstill, SUMO's Krauss (sigma 0) speeds up at exactly `accel` until its
/// maximum, `v(t) = accel·t` (Krauss 1998, the `v + a·Δt` term of the desired speed),
/// while the IDM's free-road term `a·(1 − (v/v0)⁴)` (Treiber et al. 2000) falls short of
/// it as `v` nears `v0`: the same vType without a model drives the engine's IDM.
#[test]
fn a_vtypes_car_following_model_drives_its_vehicles() {
    let xml = |model: &str| {
        format!(
            r#"<routes>
  <vType id="t" vClass="passenger" accel="1.5" decel="4.5" sigma="0" maxSpeed="12" speedFactor="1" {model}/>
  <vehicle id="a" type="t" depart="0" departLane="0"><route edges="A1B1 B1C1"/></vehicle>
</routes>"#
        )
    };
    let (_, krauss) = run_grid(&xml(r#"carFollowModel="Krauss""#), 61, 0);
    let (_, idm) = run_grid(&xml(""), 61, 0);
    assert!(krauss.len() > 50 && idm.len() > 50);
    let (vk, vi) = (krauss[krauss.len() - 1], idm[idm.len() - 1]);
    let t = (krauss.len() - 1) as f64 * 0.1;
    eprintln!("after {t:.1} s: Krauss {vk:.3} m/s, IDM {vi:.3} m/s, a·t {:.3}", 1.5 * t);
    // The slope over the last second, not v at the end: the engine holds a vehicle that
    // starts from standstill for its start-up delay first (8.25 m/s at 6.0 s on the
    // 2026-10-27 merged tree is 1.5 m/s² from 0.5 s), whatever its car-following model.
    let slope = |v: &[f64]| v[v.len() - 1] - v[v.len() - 11];
    let (sk, si) = (slope(&krauss), slope(&idm));
    eprintln!("last-second slope: Krauss {sk:.3}, IDM {si:.3} m/s²");
    assert!((sk - 1.5).abs() < 0.02, "Krauss speeds up at accel: {sk} against 1.5");
    assert!(vk <= 1.5 * t + 0.05, "Krauss above a·t: {vk} against {}", 1.5 * t);
    assert!(si < 1.4, "the IDM's free-road term falls short of accel: {si}");
    assert!(vi < vk - 0.3, "the IDM falls short of a·t: {vi} against {vk}");
}

/// WP-15 follow-up: a stated `departSpeed` above the safe insertion speed delays the
/// insertion, as SUMO documents ("If that speed is unsafe, departure is delayed"), and the
/// vehicle then enters at exactly the stated speed; `max` instead enters at once, slower.
#[test]
fn an_unsafe_stated_depart_speed_delays_the_insertion() {
    let xml = |speed: &str| {
        format!(
            r#"<routes>
  <vType id="t" vClass="passenger" sigma="0"/>
  <vehicle id="l" type="t" depart="0" departLane="0" departSpeed="0"><route edges="A1B1 B1C1"/></vehicle>
  <vehicle id="f" type="t" depart="1" departLane="0" departSpeed="{speed}"><route edges="A1B1 B1C1"/></vehicle>
</routes>"#
        )
    };
    let at = |speed: &str| {
        let (spawned, _) = run_grid(&xml(speed), 200, 1);
        let f = spawned
            .iter()
            .find(|s| s.seq == 1)
            .expect("the follower enters");
        let v = f.kinematics.vel.x.hypot(f.kinematics.vel.y);
        (f.t as f64 / 1e9, v)
    };
    let (t_num, v_num) = at("13");
    let (t_max, v_max) = at("max");
    eprintln!(
        "departSpeed 13: in at {t_num:.1} s at {v_num:.2} m/s; max: {t_max:.1} s at {v_max:.2} m/s"
    );
    assert!((v_num - 13.0).abs() < 1e-9, "enters at the stated speed: {v_num}");
    assert!(t_num > t_max + 1.0, "delayed: {t_num} against {t_max}");
    assert!(v_max < 13.0, "max is lowered to the safe speed: {v_max}");
}
