//! City scale (`v2xw_engine::scale`): the tiered traffic, the radio that follows it, and
//! the dormant radios beyond its margin — all of it identical at one, two and four threads.

use std::path::Path;

use v2xw_engine::scenario::schema::{Scale, ScaleMeso, ScaleRadio, ScaleTraffic};
use v2xw_engine::{Engine, MemoryRecorder, Scenario};
use v2xw_world::GeoBbox;

fn base() -> Scenario {
    let mut s = Scenario::load(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scenarios")
            .join("grid-traffic.yaml"),
    )
    .expect("loads");
    s.actors.vehicles.demand.rate_veh_per_h = Some(20_000.0);
    s.time.duration_s = 8.0;
    s
}

/// The grid-traffic scenario with its middle third driven microscopically, the radio
/// following it, and a zero margin so every queued vehicle's radio is off.
fn tiered() -> Scenario {
    let mut s = base();
    let world = Engine::build(base(), "").expect("builds").world().clone();
    let origin: v2xw_core::geo::GeoOrigin = world.origin.into();
    let b = world.bbox;
    let (w, h) = (b.max.x - b.min.x, b.max.y - b.min.y);
    let at = |x: f64, y: f64| origin.to_geodetic(v2xw_core::geom::Vec3::new(x, y, 0.0));
    let (lat0, lon0, _) = at(b.min.x + w / 3.0, b.min.y + h / 3.0);
    let (lat1, lon1, _) = at(b.min.x + 2.0 * w / 3.0, b.min.y + 2.0 * h / 3.0);
    s.scale = Some(Scale {
        traffic: ScaleTraffic {
            model: v2xw_mobility::meso::tiered::MODEL_ID.to_string(),
            region: Some(GeoBbox::new(lat0, lon0, lat1, lon1)),
            meso: ScaleMeso::default(),
        },
        radio: ScaleRadio {
            follows: true,
            margin_m: Some(0.0),
        },
    });
    s
}

fn run_on(threads: usize, scenario: Scenario) -> (String, v2xw_engine::RunReport) {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool");
    pool.install(|| {
        let mut engine = Engine::build(scenario, "").expect("builds");
        let mut recorder = MemoryRecorder::new();
        let report = engine.run(&mut recorder).expect("runs");
        (recorder.digest_hex(), report)
    })
}

#[test]
fn a_tiered_city_run_is_identical_on_one_two_and_four_threads() {
    let (one, r1) = run_on(1, tiered());
    let scale = r1.scale.expect("the scale block is reported");
    assert!(r1.reception_attempts > 0, "no radio ran: {r1:?}");
    assert!(
        scale.dormant_node_steps + scale.peak_deferred_nodes > 0,
        "no radio was ever off, so the gate was not exercised: {scale:?}"
    );
    assert!(scale.peak_awake_nodes > 0, "{scale:?}");
    let (two, r2) = run_on(2, tiered());
    let (four, r4) = run_on(4, tiered());
    assert_eq!(r1, r2);
    assert_eq!(r1, r4);
    assert_eq!(one, two, "two threads changed the record stream");
    assert_eq!(one, four, "four threads changed the record stream");
}

#[test]
fn a_scenario_without_scale_reports_none_and_a_tiered_one_differs() {
    let (plain, r) = run_on(2, base());
    assert!(r.scale.is_none());
    let (tiered, _) = run_on(2, tiered());
    assert_ne!(plain, tiered, "the scale section changed nothing");
}
