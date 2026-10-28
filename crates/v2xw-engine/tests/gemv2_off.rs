//! WP-18's GEMV²-style channel terms, end to end (P18.5).
//!
//! - Off, nothing moves: `radio.channel_model.foliage` and `density_sigma` set to false
//!   give the run the unset keys give, record for record.
//! - On a forest road (a self-written OSM fixture: a road through a 110 m wood), the
//!   foliage term costs receptions across the wood.
//! - The density σ reaches the run: it changes what is received on the Midtown grid.

use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The procedural Midtown grid, `secs` long, at `rate` vehicles an hour, with building
/// obstruction off so links reach along and across the streets.
fn grid(rate: f64, secs: f64) -> Scenario {
    let mut s =
        Scenario::load(scenarios().join("phase1-grid.yaml")).expect("the shipped scenario loads");
    s.time.duration_s = secs;
    s.actors.vehicles.demand.rate_veh_per_h = Some(rate);
    s.world.buildings.enabled = false;
    s
}

/// A digest of every record, and the report.
fn run(scenario: Scenario) -> (String, RunReport) {
    let errors = v2xw_engine::scenario::validate(&scenario);
    assert!(errors.is_empty(), "the scenario does not load: {errors:?}");
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    let digest = v2xw_core::hash::sha256_hex(format!("{:?}", recorder.records()).as_bytes());
    (digest, report)
}

fn tuned(mut s: Scenario, foliage: Option<bool>, density: Option<bool>) -> Scenario {
    s.radio.channel_model.foliage = foliage;
    s.radio.channel_model.density_sigma = density;
    s
}

#[test]
fn off_is_byte_identical() {
    let base = grid(3000.0, 20.0);
    let (unset, report) = run(base.clone());
    assert!(
        report.receptions_ok > 100,
        "{} receptions",
        report.receptions_ok
    );
    let (off, _) = run(tuned(base, Some(false), Some(false)));
    assert_eq!(unset, off, "the terms set to false moved the run");
}

#[test]
fn the_density_sigma_reaches_the_run() {
    // GEMV²'s σ replaces the preset's on every link: the shadowing, and so what is
    // received, changes.
    let base = grid(3000.0, 20.0);
    let (off, _) = run(base.clone());
    let (on, report) = run(tuned(base, None, Some(true)));
    assert!(report.receptions_ok > 100);
    assert_ne!(off, on, "the density sigma reached nothing");
}

/// The self-written forest-road fixture: a straight two-lane road 890 m long on the
/// equator with a wood 110 m deep across its middle.
fn forest_road() -> PathBuf {
    scenarios().join("sensitivity/forest-road.osm.xml")
}

fn forest(foliage: Option<bool>) -> Scenario {
    let mut s = grid(1800.0, 40.0);
    s.world.source = v2xw_world::WorldSourceSpec::OsmXml {
        path: forest_road().to_string_lossy().into_owned(),
        bbox: None,
    };
    s.world.highway_preset = Some(v2xw_world::osm::HighwayPreset::UrbanUsPortland);
    s.world.buildings.enabled = false;
    s.world.vegetation = true;
    tuned(s, foliage, None)
}

#[test]
fn trees_on_a_forest_road_cost_receptions() {
    // Across 110 m of wood a link loses up to the canopy diffraction (about 30 dB at
    // these spacings), so fewer frames arrive; with the term off the trees are
    // radio-transparent.
    let (_, off) = run(forest(None));
    let (_, on) = run(forest(Some(true)));
    assert!(off.receptions_ok > 100, "{} receptions", off.receptions_ok);
    let (a, b) = (off.receptions_ok as f64, on.receptions_ok as f64);
    assert!(b < 0.97 * a, "receptions {b} with foliage, {a} without");
}

#[test]
fn the_gemv2_rows_hold() {
    // The sweep reads world paths from the workspace root, as the CLI does; nothing else
    // in this binary reads a relative path.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    std::env::set_current_dir(&root).expect("to the workspace root");
    use v2xw_engine::sensitivity::{expect, sweep};
    let path = root.join("scenarios/sensitivity/catalogue.yaml");
    let fam = sweep::Families::load(&path).expect("families file loads");
    let mut exp = sweep::load_expectations(&[root.join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    exp.retain(|e| {
        e.setting == "radio.channel_model.foliage"
            || e.setting == "radio.channel_model.density_sigma"
    });
    assert_eq!(exp.len(), 2, "the foliage and the density rows");
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
