//! WP-18's GEMV²-style channel terms, end to end (P18.5).
//!
//! - Off, nothing moves: `radio.channel_model.foliage` and `density_sigma` set to false
//!   give the run the unset keys give, record for record.
//! - On a forest road (a self-written OSM fixture: a road through a 110 m wood), the
//!   foliage term costs receptions across the wood.
//! - The density σ reaches the run: it changes what is received on the Midtown grid.
//! - WP-27: the R-tree of footprint centroids gives the scan's density, bit for bit.
//! - WP-29: single-bounce wall reflection off is byte-identical, and on it lifts PRR on
//!   downtown Portland's real footprints.

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

/// SplitMix64: the test's own keyed stream, so the links are the same on every run.
fn splitmix(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

#[test]
fn density_index_equals_the_scan() {
    // WP-27: the R-tree of footprint centroids gives the density the scan of every
    // footprint gives, bit for bit, on 2,000 keyed-random links over Midtown Manhattan.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut s = Scenario::load(scenarios().join("phase2-manhattan.yaml")).expect("loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source {
        *path = root.join(path.as_str()).to_string_lossy().into_owned();
    }
    let world = v2xw_engine::wiring::build_world(&s).expect("the Manhattan world builds");
    assert!(
        world.buildings.len() > 1_000,
        "{} buildings",
        world.buildings.len()
    );
    let term = v2xw_engine::wiring::DensityTerm::new(&world);
    assert_eq!(term.footprints(), world.buildings.len());
    let anchors: Vec<v2xw_core::geom::Vec3> = world
        .buildings
        .iter()
        .filter_map(|b| b.footprint.first().copied())
        .collect();
    let classes = [
        v2xw_radio::LosClass::Los,
        v2xw_radio::LosClass::NlosV,
        v2xw_radio::LosClass::NlosB,
    ];
    let mut state = 0x5EED_0027_u64;
    let (mut built, mut differ) = (0usize, 0usize);
    let (mut t_scan, mut t_index) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    for i in 0..2_000 {
        let a = anchors[(splitmix(&mut state) * anchors.len() as f64) as usize];
        let tx = v2xw_core::geom::Vec3::new(a.x + 20.0 * splitmix(&mut state), a.y, 0.0);
        let (d, th) = (
            700.0 * splitmix(&mut state),
            std::f64::consts::TAU * splitmix(&mut state),
        );
        let rx = v2xw_core::geom::Vec3::new(tx.x + d * th.cos(), tx.y + d * th.sin(), 0.0);
        let class = classes[i % 3];
        let vehicles = [tx.lerp(rx, 0.25), tx.lerp(rx, 0.5)];
        let t = std::time::Instant::now();
        let scan = term.link_density_by_scan(class, tx, rx, vehicles);
        t_scan += t.elapsed();
        let t = std::time::Instant::now();
        let index = term.link_density(class, tx, rx, vehicles);
        t_index += t.elapsed();
        if scan.static_share > 0.0 {
            built += 1;
        }
        let same = scan.static_share.to_bits() == index.static_share.to_bits()
            && scan.vehicles_per_km2.to_bits() == index.vehicles_per_km2.to_bits();
        if !same {
            differ += 1;
            eprintln!("link {i}: scan {scan:?}, index {index:?}");
        }
    }
    eprintln!(
        "WP-27 note: {} footprints, 2,000 links: scan {t_scan:?}, R-tree {t_index:?}",
        term.footprints()
    );
    assert!(
        built > 1_000,
        "only {built} links saw a building: the check would not bite"
    );
    assert_eq!(differ, 0, "{differ} links differ from the scan");
}

/// The Midtown grid with its block buildings on (the medium tier's log-distance law with
/// the Sommer term), `secs` long at `rate` vehicles an hour.
fn built_grid(rate: f64, secs: f64, reflection: Option<bool>) -> Scenario {
    let mut s = grid(rate, secs);
    s.world.buildings.enabled = true;
    s.radio.channel_model.reflection = reflection;
    s
}

#[test]
fn reflection_off_is_byte_identical() {
    // WP-29: `radio.channel_model.reflection: false` gives the run the unset key gives,
    // record for record, on a grid whose buildings block links.
    let (unset, report) = run(built_grid(3000.0, 20.0, None));
    assert!(report.receptions_ok > 100, "{}", report.receptions_ok);
    let (off, _) = run(built_grid(3000.0, 20.0, Some(false)));
    assert_eq!(unset, off, "reflection set to false moved the run");
}

/// Downtown Portland (OSM footprints, which are building by building, not block by
/// block) at the medium tier, whose log-distance law leaves building-blocked links to the
/// Sommer term, `secs` long.
fn canyon(secs: f64, reflection: Option<bool>) -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut s = Scenario::load(scenarios().join("portland-downtown.yaml")).expect("loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source {
        *path = root.join(path.as_str()).to_string_lossy().into_owned();
    }
    s.time.duration_s = secs;
    s.actors.vehicles.demand.rate_veh_per_h = Some(1500.0);
    s.radio.tiers.propagation = v2xw_core::card::Tier::Medium;
    s.radio.channel_model.reflection = reflection;
    s
}

#[test]
fn reflections_carry_links_round_the_blocks() {
    // WP-29: with single-bounce reflections a building-blocked link also receives the
    // strongest wall ray, power-summed with its shadowed path, so its loss can only fall
    // and more frames arrive. On a procedural grid of kerb-to-kerb blocks no single
    // bounce turns a corner (the leg out of the junction always cuts the next block), so
    // the run is a real map's.
    let (off, a) = run(canyon(30.0, None));
    let (on, b) = run(canyon(30.0, Some(true)));
    assert_ne!(off, on, "the reflection reached nothing");
    assert!(
        b.receptions_ok > a.receptions_ok,
        "receptions {} with reflections, {} without",
        b.receptions_ok,
        a.receptions_ok
    );
}

#[test]
fn the_reflection_rows_hold() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    std::env::set_current_dir(&root).expect("to the workspace root");
    use v2xw_engine::sensitivity::{expect, sweep};
    let path = root.join("scenarios/sensitivity/catalogue.yaml");
    let fam = sweep::Families::load(&path).expect("families file loads");
    let mut exp = sweep::load_expectations(&[root.join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    exp.retain(|e| e.setting == "radio.channel_model.reflection");
    assert_eq!(exp.len(), 2, "the PRR and the below-sensitivity rows");
    let (settings, unplanned) = expect::expect_plan(&fam, &exp);
    assert!(unplanned.is_empty(), "{unplanned:?}");
    let bases = fam.bases(&path).expect("bases load");
    let result =
        sweep::run_cached(settings, vec![], &bases, &exp, 2, None, |_, _, _| {}).expect("runs");
    for v in &result.verdicts {
        eprintln!(
            "reflection: {} {}: {:?} -> {:?} pass {} ({})",
            v.expectation.setting, v.expectation.metric, v.base, v.value, v.pass, v.note
        );
    }
    let failed: Vec<&str> = result
        .verdicts
        .iter()
        .filter(|v| !v.pass)
        .map(|v| v.expectation.metric.as_str())
        .collect();
    assert_eq!(result.verdicts.len(), 2);
    assert!(failed.is_empty(), "{failed:?}");
}
