//! `traffic.vehicle_dynamics` reaches the road (WP-22, P22.3;
//! docs/design/research/grade-dynamics.md).
//!
//! - With only the grade on, a flat world is byte-identical: a level lane chord applies
//!   nothing, so not a single state changes.
//! - The power limit slows tractor-trailers on a freeway: the design truck's acceleration
//!   is power-limited above about 15 m/s.
//! - The expectation rows of `tests/expectations/traffic.yaml` for the two keys hold on
//!   the `freight` family (`scenarios/sensitivity/catalogue.yaml`).

use std::path::{Path, PathBuf};

use v2xw_core::rng::RngRegistry;
use v2xw_engine::Scenario;
use v2xw_engine::scenario::schema::VehicleDynamicsSettings;
use v2xw_engine::sensitivity::{expect, sweep};
use v2xw_mobility::{Mobility, MobilityCtx, VehicleClass};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A signalised procedural grid with steady Poisson demand, `trailers` of it
/// tractor-trailers.
fn grid_scenario(trailers: f64) -> Scenario {
    scenario(
        serde_json::json!({
            "cols": 4, "rows": 4, "block_x_m": 200.0, "block_y_m": 150.0,
            "lanes_per_direction": 2, "speed_limit_mps": 15.6, "signalised": true,
            "corner_radius_m": 4.5
        }),
        trailers,
    )
}

/// The six-lane 65 mph road of `highway-congestion.yaml` (two 1.5 km blocks, no
/// signals), `trailers` of the fleet tractor-trailers.
fn freeway_scenario(trailers: f64) -> Scenario {
    scenario(
        serde_json::json!({
            "cols": 2, "rows": 2, "block_x_m": 1500.0, "block_y_m": 60.0,
            "lanes_per_direction": 3, "lane_width_m": 3.66, "sidewalk_m": 0.0,
            "speed_limit_mps": 29.06, "signalised": false, "crossings": false,
            "block_buildings": false, "rsu_at_junctions": false, "corner_radius_m": 15.0
        }),
        trailers,
    )
}

fn scenario(params: serde_json::Value, trailers: f64) -> Scenario {
    let mut s = Scenario::minimal();
    s.world.source =
        v2xw_world::WorldSourceSpec::procedural("world/source/procedural-grid", params);
    s.time.duration_s = 120.0;
    s.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
    s.actors.vehicles.demand.rate_veh_per_h = Some(2400.0);
    if trailers > 0.0 {
        s.actors.vehicles.classes = serde_json::from_value(serde_json::json!({
            "passenger": {"fraction": 1.0 - trailers},
            "trailer": {"fraction": trailers}
        }))
        .expect("a class mix");
    }
    s
}

/// The run's mobility: a digest of every published state, and the mean speed of the
/// trailers and of everything, over every vehicle-step.
fn run(s: &Scenario) -> (String, f64, f64) {
    s.validate().expect("the scenario validates");
    let world = v2xw_engine::wiring::build_world(s).expect("world");
    let rng = RngRegistry::new(s.seed);
    let mut mobility = v2xw_engine::wiring::native_mobility(s);
    {
        let demand = v2xw_engine::wiring::build_demand(s, &world).expect("demand");
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        mobility.init(&mut ctx, demand).expect("init");
    }
    let step = s.time.mobility_step();
    let mut trace = String::new();
    let mut trailers = std::collections::BTreeSet::new();
    let (mut sum, mut n, mut tsum, mut tn) = (0.0, 0u64, 0.0, 0u64);
    let mut t = 0u64;
    while t < (s.time.duration_s * 1e9) as u64 {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = mobility.step(&mut ctx, step);
        for sp in &update.spawned {
            if sp.class == VehicleClass::Trailer {
                trailers.insert(sp.actor);
            }
        }
        trace.push_str(&format!("{:?}\n", update.states));
        for (id, k) in &update.states {
            let v = k.ground_speed_mps();
            sum += v;
            n += 1;
            if trailers.contains(id) {
                tsum += v;
                tn += 1;
            }
        }
        t = update.t;
    }
    assert!(n > 1000, "{n} vehicle-steps");
    (
        v2xw_core::hash::sha256_hex(trace.as_bytes()),
        tsum / tn.max(1) as f64,
        sum / n as f64,
    )
}

fn with(mut s: Scenario, grade: bool, power_limit: bool) -> Scenario {
    s.traffic
        .get_or_insert_with(Default::default)
        .vehicle_dynamics = Some(VehicleDynamicsSettings {
        grade: Some(grade),
        power_limit: Some(power_limit),
    });
    s
}

#[test]
fn flat_grade_only_is_byte_identical() {
    let base = grid_scenario(0.2);
    let (off, _, _) = run(&with(base.clone(), false, false));
    let (on, _, _) = run(&with(base, true, false));
    assert_eq!(off, on, "the grade alone moved a flat world");
}

#[test]
fn the_power_limit_slows_tractor_trailers_on_a_freeway() {
    // Power = force × speed: the 120 kg/kW design truck's tractive force falls below
    // what the IDM asks of it on the way up to its 23.6 m/s above about 15 m/s, so on a
    // freeway it takes longer to reach speed. In a 35 mph city it never binds (the IDM
    // asks less than the power gives below 13 m/s), which is the realistic answer.
    let base = freeway_scenario(0.3);
    let (d_off, trailer_off, _) = run(&with(base.clone(), false, false));
    let (d_on, trailer_on, _) = run(&with(base, false, true));
    assert_ne!(d_off, d_on, "the power limit reached nothing");
    assert!(
        trailer_on < trailer_off * 0.995,
        "trailers {trailer_on} m/s on, {trailer_off} m/s off"
    );
}

#[test]
fn the_vehicle_dynamics_rows_hold() {
    let path = root().join("scenarios/sensitivity/catalogue.yaml");
    let fam = sweep::Families::load(&path).expect("families file loads");
    let mut exp = sweep::load_expectations(&[root().join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    exp.retain(|e| e.setting.starts_with("traffic.vehicle_dynamics"));
    assert_eq!(exp.len(), 2, "the grade and the power-limit rows");
    let (settings, unplanned) = expect::expect_plan(&fam, &exp);
    assert!(unplanned.is_empty(), "{unplanned:?}");
    let bases = fam.bases(&path).expect("bases load");
    let result =
        sweep::run_cached(settings, vec![], &bases, &exp, 2, None, |_, _, _| {}).expect("runs");
    let failed: Vec<String> = result
        .verdicts
        .iter()
        .filter(|v| !v.pass)
        .map(|v| {
            format!(
                "{} {}: {:?} -> {:?} ({})",
                v.expectation.setting, v.expectation.metric, v.base, v.value, v.note
            )
        })
        .collect();
    assert_eq!(result.verdicts.len(), 2);
    assert!(failed.is_empty(), "{failed:#?}");
}
