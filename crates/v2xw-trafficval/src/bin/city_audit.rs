//! `city_audit` — imports cities of the library and runs one scenario's traffic on each,
//! briefly, under the traffic auditor.
//!
//! ```text
//! cargo run -p v2xw-trafficval --bin city_audit -- <scenario.yaml> \
//!     [--only id,id,...] [--seconds S] [--json OUT.jsonl]
//! ```
//!
//! For each city (every one in `worlds/cities.yaml`, or `--only` these) the scenario is put
//! on the city exactly as `v2xw run --city` puts it (`Scenario::with_city`), the world is
//! imported, the scenario fitted to it (`v2xw_engine::city::adapt_to_world`: the demand
//! per lane-kilometre becomes a rate, the placed roadside units find their intersections),
//! and its traffic is stepped with the auditor watching, as `traffic_audit` does. One line
//! per city says whether it imported, how big its network is, how many signals and units
//! it has, and whether every safety invariant held; `--json` appends the full record.
//!
//! A city passes when it imports, puts at least one vehicle through a trip, places at least
//! one roadside unit when the scenario asks for them, and shows no safety-class violation
//! and no vehicle standing past the standstill limit — the bar `cities/manhattan`,
//! `cities/portland` and `cities/berlin` hold in the validation suite.

use std::io::Write as _;
use std::time::Instant;

use serde_json::json;
use v2xw_core::rng::RngRegistry;
use v2xw_engine::Scenario;
use v2xw_mobility::audit::{AuditParams, Check as Class, TrafficAuditor};
use v2xw_mobility::{Mobility, MobilityCtx};
use v2xw_trafficval::invariants::{RATE, SAFETY};
use v2xw_world::model::JunctionControl;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let path = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or("usage: city_audit <scenario.yaml> [--only id,id] [--seconds S] [--json OUT]")?
        .clone();
    let seconds: f64 = value("--seconds").map_or(Ok(60.0), |s| s.parse())?;
    let only: Option<Vec<String>> =
        value("--only").map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
    let mut out = match value("--json") {
        Some(p) => Some(std::fs::OpenOptions::new().create(true).append(true).open(p)?),
        None => None,
    };
    let registry = v2xw_engine::city::registry();
    let mut failed = 0usize;
    for city in &registry.cities {
        if only.as_ref().is_some_and(|o| !o.contains(&city.id)) {
            continue;
        }
        let record = audit_city(&path, &city.id, seconds);
        let pass = record["pass"].as_bool().unwrap_or(false);
        if !pass {
            failed += 1;
        }
        println!(
            "{:<16} {:<5} import {:>5.1}s lane-km {:>6.1} signals {:>3} rsus {:>2} rate {:>5} \
             peak {:>3} trips {:>4} dropped {:>3} safety {:>3} standstill {:>2} {}",
            city.id,
            if pass { "PASS" } else { "FAIL" },
            record["import_s"].as_f64().unwrap_or(0.0),
            record["drive_lane_km"].as_f64().unwrap_or(0.0),
            record["signals"].as_u64().unwrap_or(0),
            record["rsus"].as_u64().unwrap_or(0),
            record["rate_veh_per_h"].as_f64().unwrap_or(0.0),
            record["peak_vehicles"].as_u64().unwrap_or(0),
            record["trips_completed"].as_u64().unwrap_or(0),
            record["dropped_trips"].as_u64().unwrap_or(0),
            record["safety_violations"].as_u64().unwrap_or(0),
            record["standstill"].as_u64().unwrap_or(0),
            record["why"].as_str().unwrap_or("")
        );
        if let Some(f) = out.as_mut() {
            writeln!(f, "{}", serde_json::to_string(&record)?)?;
        }
    }
    if failed > 0 {
        eprintln!("{failed} city/cities failed");
        std::process::exit(1);
    }
    Ok(())
}

fn audit_city(path: &str, id: &str, seconds: f64) -> serde_json::Value {
    let fail = |why: String| json!({"city": id, "pass": false, "why": why});
    let Some(city) = v2xw_engine::city::find(id) else {
        return fail("not in the registry".into());
    };
    if !city.extract_present() {
        return fail(format!("extract missing: worlds/fetch.sh {id}"));
    }
    let scenario = match Scenario::load_unvalidated(path).and_then(|s| {
        s.with_city(id).map_err(v2xw_engine::EngineError::Scenario)
    }) {
        Ok(mut s) => {
            s.time.duration_s = seconds;
            s
        }
        Err(e) => return fail(format!("scenario: {e}")),
    };
    if let Err(e) = scenario.validate() {
        return fail(format!("validate: {e}"));
    }
    let asked_rsus = scenario.actors.rsu_placement.as_ref().map_or(0, |p| p.count);
    let t = Instant::now();
    let world = match v2xw_engine::wiring::build_world(&scenario) {
        Ok(w) => w,
        Err(e) => return fail(format!("import: {e}")),
    };
    let import_s = t.elapsed().as_secs_f64();
    // `CITY_AUDIT_OD=1`: how many of the connected OD law's pairs the router can drive.
    if std::env::var("CITY_AUDIT_OD").is_ok() {
        use v2xw_mobility::demand::{OdModel, OdParams};
        use v2xw_mobility::routing::{Dijkstra, DijkstraParams, FreeFlowCost};
        let od = OdModel::build(&world, OdParams { connected_only: true, ..OdParams::default() });
        if let Ok(od) = od {
            let router = Dijkstra::new(DijkstraParams::default());
            let cost = FreeFlowCost::new(&world);
            let (o, d) = (od.origins(), od.destinations());
            let mut fail = 0;
            for i in 0..300usize {
                let a = o[(i * 7919) % o.len()];
                let b = d[(i * 104_729 + 13) % d.len()];
                if router.search(&world, a, b, 0, &cost).is_none() {
                    fail += 1;
                    if fail <= 3 {
                        let (la, lb) = (world.lane(a), world.lane(b));
                        eprintln!(
                            "{id}: no route {} ({:?} {:.1} m) -> {} ({:?} {:.1} m, allowed {:?})",
                            a.index(), la.kind, la.length_m, b.index(), lb.kind, lb.length_m, lb.allowed
                        );
                    }
                }
            }
            eprintln!("{id}: {} origins, {} destinations, {fail}/300 sampled pairs unroutable", o.len(), d.len());
        }
    }
    // `CITY_AUDIT_LANES=id,id`: print those lanes' geometry, to read an example off the map.
    if let Ok(list) = std::env::var("CITY_AUDIT_LANES") {
        for id in list.split(',').filter_map(|x| x.trim().parse::<u32>().ok()) {
            if let Some(lane) = world.try_lane(v2xw_core::ids::LaneId::new(id)) {
                let pts: Vec<String> = lane
                    .centreline
                    .iter()
                    .map(|p| format!("({:.2},{:.2})", p.x, p.y))
                    .collect();
                eprintln!(
                    "{id_city} lane {id}: {:?} len {:.2} junction {:?} pts {}",
                    lane.kind,
                    lane.length_m,
                    lane.junction.map(|j| j.index()),
                    pts.join(" "),
                    id_city = id
                );
            }
        }
    }
    let scenario = v2xw_engine::city::adapt_to_world(scenario, &world);
    let drive_lane_km = v2xw_engine::city::drive_lane_km(&world);
    let signals = world
        .roads
        .junctions()
        .iter()
        .filter(|j| matches!(j.control, JunctionControl::Signalised { .. }))
        .count();
    let rsus = scenario.actors.rsus.len();
    let rng = RngRegistry::new(scenario.seed);
    let mut mobility = v2xw_engine::wiring::native_mobility(&scenario);
    let demand = match v2xw_engine::wiring::build_demand(&scenario, &world) {
        Ok(d) => d,
        Err(e) => return fail(format!("demand: {e}")),
    };
    {
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        if let Err(e) = mobility.init(&mut ctx, demand) {
            return fail(format!("mobility: {e}"));
        }
    }
    let mut params = AuditParams::default();
    params.right_turn_on_red = v2xw_mobility::rules::TrafficRules::of_highway_preset(
        scenario.world.highway_preset.map(|p| p.label()),
    )
    .right_turn_on_red;
    let mut auditor = TrafficAuditor::new(&world, params);
    auditor.audit_world(&world);
    let step = scenario.time.mobility_step();
    let horizon = (scenario.time.duration_s * 1e9) as u64;
    let started = Instant::now();
    let mut t = 0u64;
    while t < horizon {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = mobility.step(&mut ctx, step);
        let t1 = update.t;
        let actors = mobility.audit_actors(&world, t1);
        let people = mobility.audit_pedestrians(&world);
        auditor.observe_with_pedestrians(&world, t, t1, &actors, &update.despawned, &people);
        t = t1;
    }
    let run_s = started.elapsed().as_secs_f64();
    let report = auditor.report();
    let safety: u64 = SAFETY.iter().map(|c| report.count(*c)).sum();
    let rate: u64 = RATE.iter().map(|c| report.count(*c)).sum();
    let standstill = report.count(Class::Standstill);
    let nonzero: Vec<String> = SAFETY
        .iter()
        .filter(|c| report.count(**c) > 0)
        .map(|c| format!("{} {}", c.label(), report.count(*c)))
        .collect();
    let examples: Vec<String> = report
        .examples
        .iter()
        .filter(|e| SAFETY.contains(&e.check) || e.check == Class::Standstill)
        .take(4)
        .map(|e| {
            format!(
                "{} t={:.1}s actor={:?} lane={:?} ({:.1},{:.1}): {}",
                e.check.label(),
                e.t_s,
                e.actor,
                e.lane,
                e.x_m,
                e.y_m,
                e.detail
            )
        })
        .collect();
    let trips = report.stats.trips_completed;
    let mut why = Vec::new();
    if trips == 0 {
        why.push("no trip completed".to_string());
    }
    if asked_rsus > 0 && rsus == 0 {
        why.push("no signalised intersection for a roadside unit".to_string());
    }
    if safety > 0 {
        why.push(nonzero.join(", "));
    }
    if standstill > 0 {
        why.push(format!("standstill {standstill}"));
    }
    json!({
        "city": id,
        "name": city.name,
        "state": city.state,
        "pass": why.is_empty(),
        "why": why.join("; "),
        "import_s": (import_s * 10.0).round() / 10.0,
        "run_s": (run_s * 10.0).round() / 10.0,
        "simulated_s": seconds,
        "lanes": world.roads.lanes().len(),
        "junctions": world.roads.junctions().len(),
        "buildings": world.buildings.len(),
        "drive_lane_km": (drive_lane_km * 10.0).round() / 10.0,
        "signals": signals,
        "rsus": rsus,
        "rsus_asked": asked_rsus,
        "rate_veh_per_h": scenario.actors.vehicles.demand.rate_veh_per_h,
        "peak_vehicles": report.stats.peak_vehicles,
        "vehicles": report.stats.vehicles,
        "trips_completed": trips,
        "dropped_trips": mobility.dropped_trips(),
        "unroutable_trips": mobility.unroutable_trips(),
        "od_connected": scenario.actors.vehicles.demand.params.pointer("/od/connected_only").cloned(),
        "mean_speed_mps": report.stats.mean_speed_mps,
        "safety_violations": safety,
        "rate_exceedances": rate,
        "standstill": standstill,
        "examples": examples,
    })
}
