//! The traffic-audit settings (docs/guides/config-inventory-traffic.md): one same-seed test
//! per group exposed by that audit, each running a baseline and the setting side by side
//! so a setting that never reached the model fails its assertion, plus the loader's
//! refusals of the combinations where a setting would do nothing.

use std::path::{Path, PathBuf};

use v2xw_engine::scenario::schema::{ClassDriver, GnssSettings, KerbsideSettings, Traffic};
use v2xw_engine::{Engine, MemoryRecorder, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// A small signalised procedural grid, traffic only (no radios), `secs` long, with a
/// steady Poisson demand: cheap, needs no map, and every vehicle meets junctions.
fn grid(secs: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("loads");
    s.time.duration_s = secs;
    s.metrics = vec![];
    s.actors.vehicles.equipped_fraction = 0.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(1500.0);
    s.world.buildings.enabled = false;
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = serde_json::json!(4);
        params["rows"] = serde_json::json!(6);
    }
    s
}

fn run(s: Scenario) -> MemoryRecorder {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "the scenario should load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    rec
}

/// Mean ground-truth speed over every actor sample, and the number of samples.
fn speeds(rec: &MemoryRecorder) -> (f64, usize) {
    let v: Vec<f64> = rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "gt.kinematics")
        .filter_map(|(_, r)| {
            let v: serde_json::Value = serde_json::from_slice(&r.json).ok()?;
            v.get("speed_mps")?.as_f64()
        })
        .collect();
    assert!(!v.is_empty(), "no ground-truth kinematics were recorded");
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    eprintln!("audit-speed {mean:.3} m/s over {} samples", v.len());
    (mean, v.len())
}

fn refused(s: &Scenario, field: &str) -> bool {
    v2xw_engine::scenario::validate::validate(s)
        .iter()
        .any(|e| e.to_string().contains(field))
}

/// Drivers' speed and reaction: drivers who choose 70 % of the limit and are slow off
/// the line average a lower speed than the default population on the same seed.
#[test]
fn the_driver_speed_and_reaction_numbers_reach_the_drivers() {
    let base = grid(60.0);
    let (v0, _) = speeds(&run(base.clone()));
    let mut slow = base;
    slow.actors.vehicles.drivers.speed_factor = Some(0.7);
    slow.actors.vehicles.drivers.speed_spread = Some(0.0);
    slow.actors.vehicles.drivers.start_reaction_s = Some(2.5);
    slow.actors.vehicles.drivers.queue_start_delay_s = Some(2.0);
    let (v1, _) = speeds(&run(slow));
    assert!(
        v1 < 0.9 * v0,
        "slow drivers averaged {v1:.2} m/s against the default {v0:.2} m/s"
    );
}

/// Per-class drivers: an all-truck fleet with sluggish truck numbers is slower than the
/// same trucks on their own calibration.
#[test]
fn the_per_class_numbers_reach_that_class() {
    let mut base = grid(60.0);
    base.actors.vehicles.classes = [(
        "truck".to_string(),
        v2xw_engine::scenario::schema::VehicleClassSpec {
            fraction: 1.0,
            obu: None,
        },
    )]
    .into_iter()
    .collect();
    let (v0, _) = speeds(&run(base.clone()));
    let mut sluggish = base;
    sluggish.actors.vehicles.drivers.truck = Some(ClassDriver {
        max_accel_mps2: Some(0.4),
        time_headway_s: Some(3.5),
        min_gap_m: Some(6.0),
        ..ClassDriver::default()
    });
    let (v1, _) = speeds(&run(sluggish));
    assert!(
        v1 < 0.9 * v0,
        "sluggish trucks averaged {v1:.2} m/s against {v0:.2} m/s"
    );
}

/// Junctions: on an unsignalised grid, drivers who wait for twice the HCM gaps and take
/// corners gently are slower than drivers who take half the gaps.
#[test]
fn the_junction_numbers_reach_the_junction_models() {
    let mut base = grid(60.0);
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut base.world.source {
        params["signalised"] = serde_json::json!(false);
    }
    let with = |factor: f64, turn: f64| {
        let mut s = base.clone();
        s.traffic = Some(Traffic {
            critical_gap_factor: Some(factor),
            turn_lateral_accel_mps2: Some(turn),
            ..Traffic::default()
        });
        s
    };
    let (bold, _) = speeds(&run(with(0.5, 6.0)));
    let (timid, _) = speeds(&run(with(2.0, 1.0)));
    assert!(
        timid < 0.95 * bold,
        "timid junction drivers averaged {timid:.2} m/s against bold ones' {bold:.2} m/s"
    );
}

/// Kerbside stops: an all-bus fleet stopping every 100 m for two minutes is slower than
/// the same buses with kerbside stops off.
#[test]
fn the_kerbside_numbers_reach_the_stops() {
    let mut base = grid(90.0);
    base.actors.vehicles.classes = [(
        "bus".to_string(),
        v2xw_engine::scenario::schema::VehicleClassSpec {
            fraction: 1.0,
            obu: None,
        },
    )]
    .into_iter()
    .collect();
    let with = |k: KerbsideSettings| {
        let mut s = base.clone();
        s.traffic = Some(Traffic {
            kerbside: Some(k),
            ..Traffic::default()
        });
        s
    };
    let (off, _) = speeds(&run(with(KerbsideSettings {
        enabled: Some(false),
        ..KerbsideSettings::default()
    })));
    let (stops, _) = speeds(&run(with(KerbsideSettings {
        bus_stop_spacing_m: Some(100.0),
        bus_dwell_mean_s: Some(120.0),
        ..KerbsideSettings::default()
    })));
    assert!(
        stops < 0.9 * off,
        "buses stopping every 100 m averaged {stops:.2} m/s against {off:.2} m/s with no stops"
    );
}

/// GNSS: the model the run builds makes smaller errors with RTK receivers, and outliers
/// switched off remove the long tail, on the same seed and nodes.
#[test]
fn the_gnss_settings_reach_the_error_model() {
    use v2xw_core::geom::Vec3;
    use v2xw_core::rng::RngRegistry;
    let mut s = grid(10.0);
    s.actors.vehicles.equipped_fraction = 1.0;
    let world = v2xw_engine::wiring::build_world(&s).expect("the grid builds");
    let errors = |s: &Scenario| {
        let mut m = v2xw_engine::wiring::build_gnss(s);
        let rng = RngRegistry::new(s.seed);
        let gt = v2xw_core::Kinematics::at_rest(0, Vec3::new(100.0, 100.0, 0.0));
        // 200 receivers, one fix a second for 10 s: the event processes (outliers,
        // bursts) are per second of elapsed time, so a cold first fix carries none.
        let mut sum = 0.0;
        for step in 0..10u64 {
            for id in 0..200u32 {
                let mut ctx = v2xw_mobility::MobilityCtx::new(step * 1_000_000_000, &world, &rng);
                let e = m.estimate(
                    &mut ctx,
                    v2xw_core::NodeId::new(id),
                    &gt,
                    &v2xw_mobility::GnssEnv::OPEN_SKY,
                );
                sum += e.pos.distance_2d(gt.pos);
            }
        }
        sum / 2000.0
    };
    let standard = errors(&s);
    let mut rtk = s.clone();
    rtk.actors.vehicles.gnss = Some(GnssSettings {
        rtk: Some(true),
        ..GnssSettings::default()
    });
    let rtk_err = errors(&rtk);
    assert!(
        rtk_err < 0.8 * standard,
        "RTK mean error {rtk_err:.2} m against standard {standard:.2} m"
    );
    let mut wild = s;
    wild.actors.vehicles.gnss = Some(GnssSettings {
        outlier_rate_hz: Some(0.5),
        outlier_magnitude_m: Some(100.0),
        ..GnssSettings::default()
    });
    assert!(
        errors(&wild) > 2.0 * standard,
        "frequent 100 m outliers did not raise the mean error"
    );
}

/// World signals: the cycle length reaches the plans an imported world is given. Needs
/// the Manhattan extract (`ln -sf …/worlds/cache/manhattan.osm.xml worlds/cache/`); the
/// test says so and passes vacuously only when the extract is absent.
#[test]
fn the_signal_timing_reaches_the_imported_plans() {
    let mut s = Scenario::load(scenarios().join("scale/base.yaml")).expect("loads");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source {
        let abs = root.join(&*path);
        if !abs.exists() {
            eprintln!("skipped: {} is not present", abs.display());
            return;
        }
        *path = abs.to_string_lossy().into_owned();
    }
    s.world.buildings.enabled = false;
    s.world.cache = None;
    let cycles = |s: &Scenario| -> Vec<f64> {
        let w = v2xw_engine::wiring::build_world(s).expect("the extract imports");
        w.signals.iter().map(|p| p.cycle_s).collect()
    };
    let base = cycles(&s);
    eprintln!("audit-signals {} plans", base.len());
    let mut long = s.clone();
    long.world.signals.cycle_s = Some(150.0);
    let longer = cycles(&long);
    assert!(!base.is_empty(), "the extract has no signals");
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    assert!(
        mean(&longer) > mean(&base) + 30.0,
        "a 150 s cycle gave plans of {:.1} s mean against {:.1} s",
        mean(&longer),
        mean(&base)
    );
}

/// The loader refuses each combination where a traffic-audit setting would do nothing.
#[test]
fn the_loader_refuses_settings_that_would_do_nothing() {
    let base = grid(10.0);

    let mut s = base.clone();
    s.actors.vehicles.drivers.realism = v2xw_engine::scenario::schema::DriverRealism::Identical;
    s.actors.vehicles.drivers.speed_factor = Some(1.1);
    assert!(refused(&s, "actors.vehicles.drivers.speed_factor"));

    let mut s = base.clone();
    s.actors.vehicles.drivers.bicycle = Some(ClassDriver {
        max_accel_mps2: Some(1.0),
        ..ClassDriver::default()
    });
    assert!(refused(&s, "actors.vehicles.drivers.bicycle"));

    let mut s = base.clone();
    s.traffic = Some(Traffic {
        kerbside: Some(KerbsideSettings {
            enabled: Some(false),
            taxi_share: Some(0.2),
            ..KerbsideSettings::default()
        }),
        ..Traffic::default()
    });
    assert!(refused(&s, "traffic.kerbside.taxi_share"));

    let mut s = base.clone();
    s.world.signals.cycle_s = Some(120.0);
    assert!(refused(&s, "world.signals.cycle_s"));

    let mut s = base.clone();
    s.actors.vehicles.gnss = Some(GnssSettings {
        rtk: Some(true),
        ..GnssSettings::default()
    });
    assert!(refused(&s, "actors.vehicles.gnss"));

    let mut s = base;
    s.traffic = Some(Traffic {
        critical_gap_factor: Some(5.0),
        ..Traffic::default()
    });
    assert!(refused(&s, "traffic.critical_gap_factor"));
}

/// Defaults change nothing: a scenario that sets no traffic-audit field serialises without
/// any of them, so its digest is the one it had before the audit.
#[test]
fn the_defaults_leave_the_scenario_digest_alone() {
    let s = grid(10.0);
    let text = serde_json::to_string(&s).expect("serialises");
    for key in [
        "speed_factor",
        "critical_gap_factor",
        "kerbside",
        "gnss",
        "pedestrian_walk_s",
        "\"truck\"",
    ] {
        assert!(
            !text.contains(key),
            "{key} serialised for a default scenario"
        );
    }
    let mut t = s.clone();
    t.traffic = Some(Traffic::default());
    let text = serde_json::to_string(&t).expect("serialises");
    assert!(!text.contains("permitted_left_gap_s"));
}
