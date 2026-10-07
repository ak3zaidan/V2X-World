//! The demand kinds reach a run through the engine's own wiring: a scenario names them,
//! validates, builds, and its vehicles drive.

use v2xw_core::rng::RngRegistry;
use v2xw_engine::scenario::Scenario;
use v2xw_mobility::{Mobility, MobilityCtx};

fn scenario(kind: &str, params: serde_json::Value) -> Scenario {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/vru-grid.yaml");
    let mut s = Scenario::load_unvalidated(root).expect("vru-grid loads");
    s.time.duration_s = 120.0;
    s.actors.vehicles.demand.kind = kind.to_string();
    s.actors.vehicles.demand.params = params;
    s.actors.vehicles.demand.rate_veh_per_h = None;
    s
}

/// Runs the scenario's mobility for its duration; returns (vehicles spawned, card id).
fn drive(s: &Scenario) -> (usize, String) {
    s.validate().expect("the scenario validates with the new kind");
    let world = v2xw_engine::wiring::build_world(s).expect("world");
    let demand = v2xw_engine::wiring::build_demand(s, &world).expect("demand builds");
    let id = v2xw_core::model::Model::card(demand.as_ref()).id.clone();
    let rng = RngRegistry::new(s.seed);
    let mut m = v2xw_engine::wiring::build_mobility(s);
    m.init(&mut MobilityCtx::new(0, &world, &rng), demand).expect("init");
    let step = v2xw_core::time::Duration::from_millis(s.time.mobility_step_ms);
    let mut t = 0;
    let mut spawned = 0;
    while t < v2xw_core::time::secs_to_ns(s.time.duration_s) {
        let u = m.step(&mut MobilityCtx::new(t, &world, &rng), step);
        spawned += u.spawned.len();
        t = u.t;
    }
    (spawned, id)
}

#[test]
fn a_composite_of_poisson_an_assigned_od_matrix_and_purposes_runs() {
    let s = scenario(
        "mobility/demand/composite",
        serde_json::json!({
            "parts": [
                { "kind": "mobility/demand/poisson", "rate_veh_per_h": 600.0 },
                { "kind": "mobility/demand/od-matrix", "params": {
                    "zones": { "kind": "grid", "cols": 2, "rows": 2 },
                    "cells": [
                        { "from": "r0c0", "to": "r1c1", "count": 120 },
                        { "from": "r1c1", "to": "r0c0", "count": 120 }
                    ],
                    "assign": { "max_iterations": 4 }
                }},
                { "kind": "mobility/demand/purposes", "params": { "daily_trips": 20000 } }
            ]
        }),
    );
    let (spawned, id) = drive(&s);
    assert_eq!(id, "mobility/demand/composite");
    assert!(spawned >= 20, "only {spawned} vehicles in two minutes");
}

#[test]
fn an_unknown_part_is_refused_by_name() {
    let s = scenario(
        "mobility/demand/composite",
        serde_json::json!({ "parts": [ { "kind": "mobility/demand/nonesuch" } ] }),
    );
    let world = v2xw_engine::wiring::build_world(&s).expect("world");
    let err = v2xw_engine::wiring::build_demand(&s, &world).err().expect("refused");
    assert!(err.to_string().contains("nonesuch"), "{err}");
}
