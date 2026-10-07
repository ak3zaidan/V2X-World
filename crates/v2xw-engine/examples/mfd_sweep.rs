//! `mfd_sweep` — the macroscopic fundamental diagram of a scenario's network: the same
//! world run at several demand levels, each run's network density, flow and speed
//! measured with Edie's definitions every minute (`v2xw_mobility::network`), and the
//! highest flow set beside the method-of-cuts bound `s·g/C` (Daganzo & Geroliminis 2008).
//!
//! ```text
//! cargo run -p v2xw-engine --example mfd_sweep -- <scenario.yaml> \
//!     [--rates 1200,2400,4800,9600] [--duration S] [--pedestrians N] [--json OUT]
//! ```
//!
//! A real network's points fall on one curve that rises, flattens and — past the critical
//! density — falls (Geroliminis & Daganzo 2008, Yokohama). Each line printed is one
//! interval of one run; the summary line per rate gives the run's mean speed, stops per
//! kilometre and the auditor's safety counts, so a congested run that is only congested
//! because vehicles overlap or gridlock shows it.

use v2xw_core::rng::RngRegistry;
use v2xw_engine::Scenario;
use v2xw_mobility::audit::{AuditParams, Check, TrafficAuditor};
use v2xw_mobility::calibration::CalibrationObserver;
use v2xw_mobility::{Mobility, MobilityCtx};

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
        .ok_or("usage: mfd_sweep <scenario.yaml> [--rates R1,R2,...] [--duration S]")?;
    let rates: Vec<f64> = value("--rates")
        .unwrap_or_else(|| "1200,2400,4800,9600".to_string())
        .split(',')
        .map(|r| r.trim().parse())
        .collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    for rate in rates {
        let mut scenario = Scenario::load(path)?;
        scenario.actors.vru.pedestrians = value("--pedestrians")
            .map(|n| n.parse())
            .transpose()?
            .unwrap_or(0);
        scenario.actors.vru.cyclists = 0;
        scenario.actors.vehicles.demand.rate_veh_per_h = Some(rate);
        if scenario.actors.vehicles.demand.kind == "mobility/demand/none" {
            scenario.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
        }
        if let Some(d) = value("--duration") {
            scenario.time.duration_s = d.parse()?;
        }
        let world = v2xw_engine::wiring::build_world(&scenario)?;
        let rng = RngRegistry::new(scenario.seed);
        let mut mobility = v2xw_engine::wiring::native_mobility(&scenario);
        {
            let demand = v2xw_engine::wiring::build_demand(&scenario, &world)?;
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            mobility.init(&mut ctx, demand)?;
        }
        let mut harness = CalibrationObserver::new(&world);
        let mut auditor = TrafficAuditor::new(&world, AuditParams::default());
        let step = scenario.time.mobility_step();
        let horizon = (scenario.time.duration_s * 1e9) as u64;
        let mut t = 0u64;
        while t < horizon {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            let update = mobility.step(&mut ctx, step);
            let actors = mobility.audit_actors(&world, update.t);
            let people = mobility.audit_pedestrians(&world);
            harness.observe(&world, t, update.t, &actors, &update.despawned, &people);
            auditor.observe(&world, t, update.t, &actors, &update.despawned);
            t = update.t;
        }
        let report = harness.report();
        let audit = auditor.report();
        let n = &report.network;
        for m in &n.mfd {
            println!(
                "rate {rate:>6.0}  t {:>5.0}s  k {:>6.2} veh/km/ln  q {:>6.1} veh/h/ln  v {:>5.2} m/s",
                m.t_s, m.density_veh_per_km, m.flow_veh_per_h, m.speed_mps
            );
        }
        let nonzero: Vec<String> = Check::ALL
            .iter()
            .filter(|c| audit.count(**c) > 0)
            .map(|c| format!("{}={}", c.label(), audit.count(*c)))
            .collect();
        println!(
            "rate {rate:>6.0}  SUMMARY speed {:.2} m/s ({:.1} min/km), {:.2} stops/veh-km, max \
             q {:.0} vs bound {:.0} veh/h/ln, box blocking {:.1} veh-s, spillback {:.0} \
             lane-s/h, kerbside {:?}, audit {nonzero:?}",
            n.network_speed_mps,
            n.travel_time_min_per_km,
            n.stops_per_km,
            n.mfd_max_flow_veh_per_h,
            n.capacity_bound_veh_per_h,
            n.box_blocking_s,
            n.spillback_lane_s_per_h,
            n.kerbside_dwell_s
                .iter()
                .map(|(k, s)| format!("{k}:{}x{:.0}s", s.n, s.mean))
                .collect::<Vec<_>>()
        );
        out.push(serde_json::json!({
            "rate_veh_per_h": rate,
            "network": n,
            "audit": audit.counts,
        }));
    }
    if let Some(path) = value("--json") {
        std::fs::write(path, serde_json::to_string_pretty(&out)?)?;
    }
    Ok(())
}
