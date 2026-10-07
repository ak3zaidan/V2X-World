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
//!   or fleet looks like. It is also re-scored under PHEMlight, so the model's share of
//!   the answer is visible beside the application's.
//!
//! The comparison that matters is over the vehicles GLOSA advised (`app.advice`), as the
//! published evaluations measure equipped vehicles approaching the signal: Treiber and
//! Kesting (2014, arXiv:1408.5498) report about 15 % fewer stops and 4 % on fuel at
//! 50 km/h, Tielert et al. (2012, ITST) up to 20 % battery energy, Katsaros et al. (2011,
//! IWCMC) a fuel and stop-time reduction growing with penetration and shrinking as density
//! rises, because a queue the vehicle joins anyway is a stop no advice can remove.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use v2xw_core::ids::{ActorId, NodeId};
use v2xw_energy::{EnergyProvider, EnergySpec, Totals};
use v2xw_engine::{Engine, MemoryRecorder, Scenario};
use v2xw_metrics::MetricProvider;
use v2xw_metrics::channels::{GtKinematicsView, decode};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

fn scenario(compliance: f64, rate_veh_per_h: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("connected-intersections.yaml"))
        .expect("the shipped scenario loads");
    s.time.duration_s = 90.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(rate_veh_per_h);
    s.metrics = vec![];
    s.apps.glosa_compliance = compliance;
    s.energy = Some(EnergySpec::default());
    s
}

struct Outcome {
    /// The live provider's last headline value of each run-so-far metric.
    live: BTreeMap<String, f64>,
    /// Per-actor totals, re-scored from the recording (HBEFA 3, the run's own model).
    rescored: BTreeMap<ActorId, Totals>,
    /// The same recording re-scored under PHEMlight.
    phem: BTreeMap<ActorId, Totals>,
    /// The actors GLOSA advised.
    advised: BTreeSet<ActorId>,
    /// The fleet's CO2 per km re-scored from the records up to the live provider's last
    /// flush, which is what the live headline covers.
    rescored_at_last_flush: f64,
}

fn totals_of(p: &EnergyProvider) -> BTreeMap<ActorId, Totals> {
    p.vehicles().into_iter().map(|v| (v.actor, v.totals)).collect()
}

/// A signalised arterial as the published single-route evaluations build one: a 7 × 3
/// grid of 300 m blocks at 50 km/h on a 60 s cycle, whose middle row's five four-way
/// junctions each carry a SPaT/MAP roadside unit, so an advised vehicle along the arterial
/// meets advice at every signal 300 m apart (a MAP describes lanes up to about 327 m,
/// J2735 Node-XY-32b) rather than at four of the connected grid's
/// thirty-six junctions 80 m apart.
fn corridor(compliance: f64, rate_veh_per_h: f64) -> Scenario {
    let mut s = scenario(compliance, rate_veh_per_h);
    s.time.duration_s = 180.0;
    let mut doc = serde_json::to_value(&s).expect("serialises");
    let p = &mut doc["world"]["source"]["params"];
    p["cols"] = serde_json::json!(7);
    p["rows"] = serde_json::json!(3);
    p["block_x_m"] = serde_json::json!(300.0);
    p["block_y_m"] = serde_json::json!(300.0);
    p["speed_limit_mps"] = serde_json::json!(13.89);
    // The middle row's five interior four-way junctions (site = row * cols + col).
    let rsus: Vec<serde_json::Value> = (8..13)
        .map(|site| {
            serde_json::json!({"site": site, "roles": ["spat", "map"],
                               "profile": "rsu/commsignia-its-rs4"})
        })
        .collect();
    doc["actors"]["rsus"] = serde_json::Value::Array(rsus);
    Scenario::from_document(doc).expect("the corridor deserialises")
}

fn run(s: Scenario) -> Outcome {
    let seed = s.seed;
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs");

    let mut live: BTreeMap<String, (u64, f64)> = BTreeMap::new();
    let mut node_actor: BTreeMap<NodeId, ActorId> = BTreeMap::new();
    let mut advised_nodes: BTreeSet<u64> = BTreeSet::new();
    let mut rescore = EnergyProvider::new(EnergySpec::default(), seed).expect("provider");
    let phem_spec = EnergySpec {
        emission_model: "phemlight".to_string(),
        ..EnergySpec::default()
    };
    let mut phem = EnergyProvider::new(phem_spec, seed).expect("provider");
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
                phem.on_event(r);
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
    // The live headline covers the records up to its last flush; re-score exactly those.
    let t_last = live.get("co2_per_km").map_or(0, |(t, _)| *t);
    let mut upto = EnergyProvider::new(EnergySpec::default(), seed).expect("provider");
    for (t, r) in recorder.records() {
        if r.channel == "gt.kinematics" && *t <= t_last {
            upto.on_event(r);
        }
    }
    let u = upto.run_totals();
    let advised = node_actor
        .iter()
        .filter(|(n, _)| advised_nodes.contains(&u64::from(n.index())))
        .map(|(_, a)| *a)
        .collect();
    Outcome {
        live: live.into_iter().map(|(k, (_, v))| (k, v)).collect(),
        rescored: totals_of(&rescore),
        phem: totals_of(&phem),
        advised,
        rescored_at_last_flush: u.co2_g / (u.km_ice + u.km_ev),
    }
}

fn sum(by: &BTreeMap<ActorId, Totals>, actors: &BTreeSet<ActorId>) -> Totals {
    let mut t = Totals::default();
    for a in actors {
        if let Some(x) = by.get(a) {
            t.co2_g += x.co2_g;
            t.fuel_l += x.fuel_l;
            t.km_ice += x.km_ice;
            t.km_all += x.km_all;
            t.stops += x.stops;
        }
    }
    t
}

/// GLOSA off against GLOSA followed by every driver, at one demand.
#[derive(Debug, Clone, Copy)]
struct Comparison {
    advised: usize,
    /// HBEFA 3 (the run's model): L/100 km, g CO2/km and stops/km, off then on.
    off: (f64, f64, f64),
    on: (f64, f64, f64),
    /// PHEMlight re-score: L/100 km off, on.
    phem: (f64, f64),
}

fn compare(rate: f64, make: fn(f64, f64) -> Scenario) -> Comparison {
    let off = run(make(0.0, rate));
    let on = run(make(1.0, rate));
    // The live provider and the re-scored recording agree on the fleet's CO2 intensity.
    for o in [&off, &on] {
        let rescored = o.rescored_at_last_flush;
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
    let per = |t: &Totals| {
        (
            t.fuel_l / t.km_ice * 100.0,
            t.co2_g / t.km_ice,
            t.stops as f64 / t.km_all,
        )
    };
    let (a, b) = (sum(&off.rescored, &both), sum(&on.rescored, &both));
    let (pa, pb) = (sum(&off.phem, &both), sum(&on.phem, &both));
    let c = Comparison {
        advised: both.len(),
        off: per(&a),
        on: per(&b),
        phem: (per(&pa).0, per(&pb).0),
    };
    let pct = |x: f64, y: f64| (y / x - 1.0) * 100.0;
    eprintln!(
        "{rate:>6} veh/h | {} advised, {:.1} km | stops/km {:.3} -> {:.3} ({:+.1} %) | \
         HBEFA3 L/100km {:.2} -> {:.2} ({:+.1} %) | PHEMlight {:.2} -> {:.2} ({:+.1} %) | \
         fleet g/km {:.1} -> {:.1}, stops/km {:.3} -> {:.3}",
        c.advised,
        a.km_all,
        c.off.2,
        c.on.2,
        pct(c.off.2, c.on.2),
        c.off.0,
        c.on.0,
        pct(c.off.0, c.on.0),
        c.phem.0,
        c.phem.1,
        pct(c.phem.0, c.phem.1),
        off.live["co2_per_km"],
        on.live["co2_per_km"],
        off.live["stops_per_km"],
        on.live["stops_per_km"],
    );
    c
}

/// The check: on the arterial at 2,400 veh/h, every driver following GLOSA cuts the
/// advised vehicles' stops per km by at least 5 % and burns no more fuel per km, under the
/// run's own model and re-scored under PHEMlight.
///
/// Measured in this build (2026-10-07): stops −15.2 % (0.896 → 0.759 per km), fuel −0.8 %
/// under HBEFA 3 and −0.3 % under PHEMlight, 28 advised vehicles over 21 km; the fleet's
/// stops −7.7 %. The stop reduction is in line with Treiber and Kesting's ~15 %; the fuel
/// saving is well below their ~4 %, because a follower is held at the advised speed (at
/// least 20 km/h) for the whole approach and only part of each trip is advised.
///
/// It can fail: with the same compliance in both runs the two results are bit-identical
/// (the engine is deterministic), so a 5 % stop reduction is impossible.
#[test]
fn glosa_reduces_stops_on_an_arterial_without_costing_fuel() {
    let c = compare(2_400.0, corridor);
    assert!(c.advised >= 20, "only {} advised vehicles", c.advised);
    assert!(
        c.on.2 <= 0.95 * c.off.2,
        "stops/km {} -> {}: under a 5 % reduction",
        c.off.2,
        c.on.2
    );
    assert!(c.on.0 <= c.off.0, "HBEFA 3 fuel rose: {} -> {}", c.off.0, c.on.0);
    assert!(c.phem.1 <= c.phem.0, "PHEMlight fuel rose: {} -> {}", c.phem.0, c.phem.1);
}

/// A measurement, not a check: GLOSA's effect across demand. Run it with
/// `cargo test -p v2xw-engine --test energy_glosa -- --ignored --nocapture`.
///
/// Measured in this build (2026-10-07) on the connected grid, advised vehicles, HBEFA 3:
/// 1,500 veh/h stops +0.1 %, fuel +0.1 %; 3,000 veh/h stops −5.1 %, fuel +2.5 %
/// (PHEMlight +2.6 %); 6,000 veh/h stops +0.9 %, fuel +5.5 % (PHEMlight +2.2 %). Junctions
/// 80 m apart, advice at four of thirty-six and queues that discharge at the green the
/// advice aims for leave nothing for GLOSA to remove, and a capped approach costs fuel:
/// the density limit Katsaros et al. (2011) report, and a reason not to quote a GLOSA
/// saving without the network it was measured on.
#[test]
#[ignore = "a measurement over six runs; minutes in a debug build"]
fn glosa_effect_across_demand() {
    eprintln!("connected-intersections grid (4 of 36 junctions advise):");
    for rate in [1_500.0, 3_000.0, 6_000.0] {
        let c = compare(rate, scenario);
        assert!(c.advised > 0, "no vehicle was advised at {rate} veh/h");
    }
}

/// The same measurement on a signalised arterial where every junction advises.
#[test]
#[ignore = "a measurement over four runs; minutes in a debug build"]
fn glosa_effect_on_an_arterial() {
    eprintln!("arterial corridor (every junction advises):");
    for rate in [1_200.0, 2_400.0] {
        let c = compare(rate, corridor);
        assert!(c.advised > 0, "no vehicle was advised at {rate} veh/h");
    }
}
