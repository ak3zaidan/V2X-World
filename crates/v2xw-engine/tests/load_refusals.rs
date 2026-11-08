//! Scenario-fuzzing regressions: each test is the minimal scenario a fuzz case reduced to,
//! pinned at the loader or builder rule that now refuses or accepts it.

use v2xw_engine::scenario::{Scenario, validate};

fn fields(text: &str) -> Vec<String> {
    let s: Scenario = serde_yml::from_str(text).expect("the document deserialises");
    validate(&s)
        .iter()
        .filter_map(|e| e.field().map(str::to_string))
        .collect()
}

const HEAD: &str = r#"
schema: v2xw/scenario/1
world:
  source: {kind: procedural, generator: world/source/procedural-grid, params: {}}
"#;

/// `demand.kind: mobility/demand/sumo-rou` over the
/// base's Poisson parameters (`max_total_vehicles`) passed `v2xw validate` and was then
/// refused by `v2xw run` (wiring::build_demand). The loader now parses the parameters the
/// way the run does, for each built-in kind.
#[test]
fn demand_params_that_do_not_fit_their_kind_are_refused_at_load() {
    let rou = fields(&format!(
        "{HEAD}actors: {{vehicles: {{demand: {{kind: mobility/demand/sumo-rou, \
         params: {{max_total_vehicles: 40}}}}}}}}\n"
    ));
    assert!(
        rou.iter().any(|f| f == "actors.vehicles.demand.params"),
        "a route-file import with the Poisson model's parameters loads: {rou:?}"
    );
    let flows = fields(&format!(
        "{HEAD}actors: {{vehicles: {{demand: {{kind: mobility/demand/sumo-rou, \
         params: {{path: x.rou.xml, probability_flows: poisson}}}}}}}}\n"
    ));
    assert!(
        flows.iter().any(|f| f == "actors.vehicles.demand.params"),
        "an unknown flow law loads: {flows:?}"
    );
    let poisson = fields(&format!(
        "{HEAD}actors: {{vehicles: {{demand: {{kind: mobility/demand/poisson, \
         params: {{path: x.rou.xml}}}}}}}}\n"
    ));
    assert!(
        poisson.iter().any(|f| f == "actors.vehicles.demand.params"),
        "a Poisson model with a route file's parameters loads: {poisson:?}"
    );
    // the same parameters on the kinds they belong to load
    let ok_rou = fields(&format!(
        "{HEAD}actors: {{vehicles: {{demand: {{kind: mobility/demand/sumo-rou, \
         params: {{path: x.rou.xml}}}}}}}}\n"
    ));
    assert!(
        !ok_rou.iter().any(|f| f == "actors.vehicles.demand.params"),
        "{ok_rou:?}"
    );
    let ok_poisson = fields(&format!(
        "{HEAD}actors: {{vehicles: {{demand: {{kind: mobility/demand/poisson, \
         params: {{max_total_vehicles: 40}}}}}}}}\n"
    ));
    assert!(
        !ok_poisson
            .iter()
            .any(|f| f == "actors.vehicles.demand.params"),
        "{ok_poisson:?}"
    );
}

/// `demand.kind: mobility/demand/trips` (a
/// `v2xw-demand` kind) without its `path` passed `v2xw validate` and was refused by the
/// run's build ("the parameters do not fit: missing field `path`"). The loader now runs
/// that crate's own parse (`v2xw_demand::scenario::check_params`).
#[test]
fn a_demand_crate_kind_without_its_parameters_is_refused_at_load() {
    let bare = fields(&format!(
        "{HEAD}actors: {{vehicles: {{demand: {{kind: mobility/demand/trips, \
         params: {{max_total_vehicles: 40}}}}}}}}\n"
    ));
    assert!(
        bare.iter().any(|f| f == "actors.vehicles.demand.params"),
        "a trips demand with the Poisson model's parameters loads: {bare:?}"
    );
    // A GTFS demand naming no feed reads `stops.txt` from the working
    // directory at build; an OD demand with no source was refused only at build
    for kind in ["mobility/demand/gtfs", "mobility/demand/od-matrix"] {
        let empty = fields(&format!(
            "{HEAD}actors: {{vehicles: {{demand: {{kind: {kind}}}}}}}\n"
        ));
        assert!(
            empty.iter().any(|f| f == "actors.vehicles.demand.params"),
            "{kind} with nothing to read loads: {empty:?}"
        );
    }
    let ok = fields(&format!(
        "{HEAD}actors: {{vehicles: {{demand: {{kind: mobility/demand/trips, \
         params: {{path: x.trips.json}}}}}}}}\n"
    ));
    assert!(
        !ok.iter().any(|f| f == "actors.vehicles.demand.params"),
        "{ok:?}"
    );
}

/// An experiment sweep over `radio.mac.cw_min_vo` on the DSRC grid was
/// refused ("does not name a field of this scenario") because the setting is optional and
/// unset, so the serialised document does not show it; the same was true of every
/// published optional leaf the base leaves unset (549 of the 693 published rows carry no
/// default). A published leaf is now written like a missing section, and an invented key
/// is still refused.
#[test]
fn a_sweep_over_an_unset_published_setting_is_accepted_and_written() {
    use v2xw_engine::scenario::{fill_default_section, names_a_default_section, resolve_path};
    let sweep = |path: &str, values: &str| {
        fields(&format!(
            "{HEAD}experiment: {{sweep: {{{path}: {values}}}, seeds: [1]}}\n"
        ))
    };
    let mac = sweep("radio.mac.cw_min_vo", "[3, 15]");
    assert!(
        !mac.iter().any(|f| f.starts_with("experiment.sweep")),
        "a sweep over a published optional setting is refused: {mac:?}"
    );
    let junk = sweep("radio.mac.cw_min_zz", "[3, 15]");
    assert!(
        junk.iter()
            .any(|f| f == "experiment.sweep.radio.mac.cw_min_zz"),
        "an invented key is accepted: {junk:?}"
    );
    let free = sweep("world.source.params.invented", "[1, 2]");
    assert!(
        free.iter()
            .any(|f| f == "experiment.sweep.world.source.params.invented"),
        "an invented key in a free-form map is accepted: {free:?}"
    );
    // the runner's fill writes the leaf, so the sweep then replaces it
    let s: Scenario = serde_yml::from_str(HEAD).expect("the document deserialises");
    let mut doc = serde_json::to_value(&s).expect("serialises");
    let v = [serde_json::json!(3), serde_json::json!(15)];
    assert!(resolve_path(&doc, "radio.mac.cw_min_vo").is_none());
    assert!(names_a_default_section(&doc, "radio.mac.cw_min_vo", &v));
    assert!(fill_default_section(&mut doc, "radio.mac.cw_min_vo", &v));
    assert!(resolve_path(&doc, "radio.mac.cw_min_vo").is_some(), "{doc}");
}

fn scenarios() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// `actors.rsu_placement.count: 4` on the DSRC sensitivity grid
/// (no signals) ran with no roadside unit at all, because units are placed only at
/// signalised junctions (city::place_rsus), and the run's resolved scenario then failed to
/// reload (its `radio.devices.rsu` set the radio of units that did not exist). The build
/// now refuses a placement that stands none; on a signalised grid it places them.
#[test]
fn an_rsu_placement_that_places_nothing_is_refused_at_build() {
    use v2xw_engine::{Engine, EngineError};
    let mut s = Scenario::load(scenarios().join("sensitivity/dsrc-grid.yaml"))
        .expect("the shipped scenario loads");
    s.time.duration_s = 0.5;
    s.metrics = vec![];
    let mut placed = s.clone();
    s.actors.rsu_placement =
        serde_json::from_value(serde_json::json!({"count": 4})).expect("a placement deserialises");
    match Engine::build(s, "") {
        Err(EngineError::Scenario(e)) => {
            assert_eq!(e.field(), Some("actors.rsu_placement.count"), "{e}");
        }
        Err(other) => panic!("refused, but not by the placement: {other}"),
        Ok(_) => panic!("a placement of 4 on a grid without signals builds (with no unit)"),
    }
    // the same grid with signals places them
    match &mut placed.world.source {
        v2xw_world::WorldSourceSpec::Procedural { params, .. } => {
            params["signalised"] = serde_json::json!(true);
        }
        other => panic!("the DSRC grid is procedural: {other:?}"),
    }
    placed.actors.rsu_placement =
        serde_json::from_value(serde_json::json!({"count": 4})).expect("a placement deserialises");
    let engine = Engine::build(placed, "").expect("a signalised grid takes the placement");
    assert!(
        !engine.scenario().actors.rsus.is_empty(),
        "no unit placed on a signalised grid"
    );
}

/// `world.terrain.drape: true` without a DEM was a
/// preflight note (`v2xw validate` exit 0, "`v2xw run` will refuse it from this
/// directory") although it depends on no file: the run refused it. It is now a rule of
/// the document, and a drape over a named DEM still loads.
#[test]
fn a_drape_without_a_dem_is_refused_at_load() {
    let doc = |terrain: &str| {
        format!(
            "schema: v2xw/scenario/1\nworld:\n  source: {{kind: procedural, generator: \
             world/source/procedural-grid, params: {{}}}}\n  terrain: {terrain}\n"
        )
    };
    let bare = fields(&doc("{drape: true}"));
    assert!(
        bare.iter().any(|f| f == "world.terrain.drape"),
        "a drape over no terrain loads: {bare:?}"
    );
    let dem = fields(&doc("{dem: x.asc, drape: true}"));
    assert!(!dem.iter().any(|f| f == "world.terrain.drape"), "{dem:?}");
}

/// The same placement, refused by the loader where the document alone says the world has
/// no signals: a procedural grid with `signalised: false`.
#[test]
fn an_rsu_placement_on_a_grid_without_signals_is_refused_at_load() {
    let doc = |signalised: bool| {
        format!(
            "schema: v2xw/scenario/1\nworld:\n  source: {{kind: procedural, generator: \
             world/source/procedural-grid, params: {{signalised: {signalised}}}}}\n\
             actors: {{rsu_placement: {{count: 4}}}}\n"
        )
    };
    let off = fields(&doc(false));
    assert!(
        off.iter().any(|f| f == "actors.rsu_placement.count"),
        "a placement on a grid without signals loads: {off:?}"
    );
    let on = fields(&doc(true));
    assert!(
        !on.iter().any(|f| f == "actors.rsu_placement.count"),
        "{on:?}"
    );
}

/// With `scale.radio.follows` the run writes the
/// normalised radio tiers as its `scenario.resolved.yaml`, and the loader refused them
/// (abstract propagation under a medium PHY; a medium focus "not above" a medium MAC), so
/// check-traffic, `v2xw compare` and `v2xw resume` failed on every such run. The document
/// the engine runs must be one it reads back, and reading it back changes nothing more.
#[test]
fn a_normalised_city_scale_scenario_reloads() {
    let text = format!(
        "{HEAD}scale:\n  traffic:\n    model: mobility/tiered/meso-micro\n    region: \
         {{min_lat_deg: 40.0, min_lon_deg: -74.0, max_lat_deg: 40.01, max_lon_deg: -73.99}}\n\
         radio:\n  tiers: {{propagation: medium, phy: medium, mac: medium}}\n  \
         channel_model: {{fading: rice}}\n"
    );
    let s: Scenario = serde_yml::from_str(&text).expect("the document deserialises");
    let before: Vec<String> = validate(&s).iter().map(ToString::to_string).collect();
    let run = v2xw_engine::scale::normalise(s);
    let focus = run
        .radio
        .tiers
        .focus
        .as_ref()
        .expect("the radio follows the box");
    assert_eq!(focus.tier, v2xw_core::card::Tier::Medium);
    let new: Vec<String> = validate(&run)
        .iter()
        .map(ToString::to_string)
        .filter(|e| !before.contains(e))
        .collect();
    assert!(
        new.is_empty(),
        "the normalised scenario is refused: {new:?}"
    );
    assert_eq!(
        v2xw_engine::scale::normalise(run.clone()),
        run,
        "normalising twice moves it"
    );
}

/// The fuzz's directed batch: menu values no scenario can run, offered as available. PSM and
/// VAM are sent by VRU devices and refused in `messages.sets`; a focus at `abstract` is
/// below nothing. Each is now marked unavailable, with the reason, and the refusal names
/// what to do instead.
#[test]
fn menu_values_no_scenario_can_run_are_not_offered() {
    let fields = v2xw_engine::scenario::publish::fields();
    let available = |path: &str, value: &str| {
        fields
            .iter()
            .find(|f| f["x-path"] == path)
            .and_then(|f| f["x-options"].as_array())
            .and_then(|o| o.iter().find(|r| r["value"] == value))
            .map(|r| r["available"].as_bool())
    };
    assert_eq!(available("messages.sets[]", "psm"), Some(Some(false)));
    assert_eq!(available("messages.sets[]", "vam"), Some(Some(false)));
    assert_eq!(available("messages.sets[]", "bsm"), Some(Some(true)));
    assert_eq!(
        available("radio.tiers.focus.tier", "abstract"),
        Some(Some(false))
    );
    assert_eq!(
        available("radio.tiers.focus.tier", "medium"),
        Some(Some(true))
    );
    let s: Scenario = serde_yml::from_str(&format!("{HEAD}messages: {{sets: [bsm, psm]}}\n"))
        .expect("the document deserialises");
    let why: Vec<String> = validate(&s).iter().map(ToString::to_string).collect();
    assert!(
        why.iter().any(|w| w.contains("actors.vru.device_fraction")),
        "the refusal does not say what to do: {why:?}"
    );
}
