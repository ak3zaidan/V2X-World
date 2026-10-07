//! Does a V2X application change what the traffic burns? GLOSA on the connected
//! intersections, with drivers ignoring the advice and with every driver following it.
//!
//! The same scenario, seed and demand twice; only `apps.glosa_compliance` differs. The
//! energy layer is read two ways, and they must agree:
//!
//! * live — the run's own `metric.sample` stream from the `metrics/energy` provider the
//!   scenario's `energy:` section installs;
//! * re-scored — the recording's `gt.kinematics` fed afterwards through a fresh
//!   [`EnergyProvider`], which is what scoring a finished recording under another model
//!   or fleet looks like.
//!
//! The comparison that matters is over the vehicles GLOSA advised (`app.advice`), as the
//! published evaluations measure equipped vehicles approaching the signal: Treiber and
//! Kesting (2014, arXiv:1408.5498) report about 15 % fewer stops and 4 % on fuel at
//! 50 km/h, Tielert et al. (2012, ITST) up to 20 % battery energy, Katsaros et al. (2011,
//! IWCMC) a fuel and stop-time reduction growing with penetration.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use v2xw_core::ids::{ActorId, NodeId};
use v2xw_engine::{Engine, MemoryRecorder, Scenario};
use v2xw_energy::{EnergyProvider, EnergySpec, Totals};
use v2xw_metrics::MetricProvider;
use v2xw_metrics::channels::{GtKinematicsView, decode};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

fn scenario(compliance: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("connected-intersections.yaml"))
        .expect("the shipped scenario loads");
    s.time.duration_s = 90.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(6_000.0);
    s.metrics = vec![];
    s.apps.glosa_compliance = compliance;
    s.energy = Some(EnergySpec::default());
    s
}

struct Outcome {
    /// The live provider's last headline value of each run-so-far metric.
    live: BTreeMap<String, f64>,
    /// Per-actor totals, re-scored from the recording.
    rescored: BTreeMap<ActorId, Totals>,
    /// The actors GLOSA advised.
    advised: BTreeSet<ActorId>,
}

fn run(compliance: f64) -> Outcome {
    let s = scenario(compliance);
    let seed = s.seed;
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs");

    let mut live: BTreeMap<String, (u64, f64)> = BTreeMap::new();
    let mut node_actor: BTreeMap<NodeId, ActorId> = BTreeMap::new();
    let mut advised_nodes: BTreeSet<u64> = BTreeSet::new();
    let mut rescore = EnergyProvider::new(EnergySpec::default(), seed).expect("provider");
    for (_, r) in recorder.records() {
        match r.channel {
            "metric.sample" => {
                let v: serde_json::Value = serde_json::from_slice(&r.json).expect("json");
                let dims_empty = v["dims"].as_object().is_none_or(serde_json::Map::is_empty);
                let point = v["value"]["point"].as_f64();
                if let (true, Some(name), Some(t), Some(p)) =
                    (dims_empty, v["metric"].as_str(), v["t"].as_u64(), point)
                {
                    let e = live.entry(name.to_string()).or_insert((0, p));
                    if t >= e.0 {
                        *e = (t, p);
                    }
                }
            }
            "gt.kinematics" => {
                let k: GtKinematicsView = decode(r).expect("decodes");
                if let Some(n) = k.node {
                    node_actor.insert(n, k.actor);
                }
                rescore.on_event(r);
            }
            "app.advice" => {
                let v: serde_json::Value = serde_json::from_slice(&r.json).expect("json");
                if let Some(n) = v["node"].as_u64() {
                    advised_nodes.insert(n);
                }
            }
            _ => {}
        }
    }
    let advised = node_actor
        .iter()
        .filter(|(n, _)| advised_nodes.contains(&u64::from(n.index())))
        .map(|(_, a)| *a)
        .collect();
    let rescored = rescore
        .vehicles()
        .into_iter()
        .map(|v| (v.actor, v.totals))
        .collect();
    Outcome {
        live: live.into_iter().map(|(k, (_, v))| (k, v)).collect(),
        rescored,
        advised,
    }
}

fn sum(o: &Outcome, actors: &BTreeSet<ActorId>) -> Totals {
    let mut t = Totals::default();
    for a in actors {
        if let Some(x) = o.rescored.get(a) {
            t.co2_g += x.co2_g;
            t.fuel_l += x.fuel_l;
            t.km_ice += x.km_ice;
            t.km_all += x.km_all;
            t.stops += x.stops;
        }
    }
    t
}

#[test]
fn glosa_followed_reduces_stops_and_fuel_of_the_advised_vehicles() {
    let off = run(0.0);
    let on = run(1.0);

    // The live provider and the re-scored recording agree on the fleet's CO2 intensity.
    for o in [&off, &on] {
        let mut all = Totals::default();
        for t in o.rescored.values() {
            all.co2_g += t.co2_g;
            all.km_ice += t.km_ice;
            all.km_ev += t.km_ev;
        }
        let rescored = all.co2_g / (all.km_ice + all.km_ev);
        let live = o.live["co2_per_km"];
        assert!(
            (rescored - live).abs() <= 1e-3 * live.max(1.0),
            "live {live} g/km vs re-scored {rescored} g/km"
        );
    }

    // The vehicles advised in either run, present in both.
    let both: BTreeSet<ActorId> = off
        .advised
        .union(&on.advised)
        .filter(|a| off.rescored.contains_key(a) && on.rescored.contains_key(a))
        .copied()
        .collect();
    let (a, b) = (sum(&off, &both), sum(&on, &both));
    let per = |t: &Totals| {
        (
            t.fuel_l / t.km_ice * 100.0,
            t.co2_g / t.km_ice,
            t.stops as f64 / t.km_all,
        )
    };
    let ((fuel0, co20, stops0), (fuel1, co21, stops1)) = (per(&a), per(&b));
    eprintln!(
        "advised vehicles: {} | {:.2} km off, {:.2} km on",
        both.len(),
        a.km_all,
        b.km_all
    );
    eprintln!("GLOSA off: {fuel0:.2} L/100km, {co20:.1} g/km, {stops0:.3} stops/km");
    eprintln!("GLOSA on:  {fuel1:.2} L/100km, {co21:.1} g/km, {stops1:.3} stops/km");
    eprintln!(
        "change: fuel {:+.1} %, stops {:+.1} %",
        (fuel1 / fuel0 - 1.0) * 100.0,
        (stops1 / stops0 - 1.0) * 100.0
    );
    eprintln!(
        "fleet (live): off {:.1} g/km, {:.3} stops/km | on {:.1} g/km, {:.3} stops/km",
        off.live["co2_per_km"], off.live["stops_per_km"], on.live["co2_per_km"], on.live["stops_per_km"]
    );
    assert!(both.len() >= 20, "only {} advised vehicles to compare", both.len());
    assert!(stops1 < stops0, "GLOSA did not reduce stops: {stops0} -> {stops1}");
    assert!(fuel1 < fuel0, "GLOSA did not reduce fuel: {fuel0} -> {fuel1}");
}
