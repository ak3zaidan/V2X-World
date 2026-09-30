//! `traffic_calibration` — runs a scenario's traffic headless and measures what a traffic
//! engineer measures at the kerb (`v2xw_mobility::calibration`): saturation headway and
//! start-up lost time at signalised stop lines, queue spacing, free-flow speed against the
//! posted limit, launch acceleration, stopping deceleration, turning speeds, travel speed
//! and pedestrian walking speed — each beside its published reference.
//!
//! ```text
//! cargo run -p v2xw-engine --example traffic_calibration -- <scenario.yaml> \
//!     [--rate VEH_PER_H] [--duration S] [--pedestrians N] [--cyclists N] [--json OUT]
//!     [--examples N]
//! ```
//!
//! The world, the mobility engine and the demand are built by the same
//! `v2xw_engine::wiring` functions the kernel uses, exactly as `traffic_audit` does, so the
//! traffic measured is the traffic the page shows. The auditor runs alongside, so every
//! calibration table is printed with the safety counts of the same run.

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
        .ok_or("usage: traffic_calibration <scenario.yaml> [--rate R] [--duration S]")?;
    let mut scenario = Scenario::load(path)?;
    if let Some(n) = value("--pedestrians") {
        scenario.actors.vru.pedestrians = n.parse()?;
    }
    if let Some(n) = value("--cyclists") {
        scenario.actors.vru.cyclists = n.parse()?;
    }
    if let Some(rate) = value("--rate") {
        scenario.actors.vehicles.demand.rate_veh_per_h = Some(rate.parse()?);
        if scenario.actors.vehicles.demand.kind == "mobility/demand/none" {
            scenario.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
        }
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
    let audit_params = AuditParams {
        right_turn_on_red: v2xw_mobility::rules::TrafficRules::of_highway_preset(
            scenario.world.highway_preset.map(|p| p.label()),
        )
        .right_turn_on_red,
        examples_per_check: value("--examples")
            .and_then(|n| n.parse().ok())
            .unwrap_or(3),
        ..AuditParams::default()
    };
    let mut auditor = TrafficAuditor::new(&world, audit_params);
    let step = scenario.time.mobility_step();
    let horizon = (scenario.time.duration_s * 1e9) as u64;
    let mut t = 0u64;
    while t < horizon {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = mobility.step(&mut ctx, step);
        let t1 = update.t;
        let actors = mobility.audit_actors(&world, t1);
        let people = mobility.audit_pedestrians(&world);
        harness.observe(&world, t, t1, &actors, &update.despawned, &people);
        auditor.observe_with_pedestrians(&world, t, t1, &actors, &update.despawned, &people);
        t = t1;
    }
    let report = harness.report();
    let audit = auditor.report();
    if let Some(out) = value("--json") {
        let json = serde_json::json!({
            "scenario": path,
            "rate_veh_per_h": scenario.actors.vehicles.demand.rate_veh_per_h,
            "duration_s": scenario.time.duration_s,
            "report": report,
            "comparisons": report.comparisons(),
            "audit": audit.counts,
        });
        std::fs::write(&out, serde_json::to_string_pretty(&json)?)?;
    }
    println!(
        "{:<44} {:>9} {:>7} {:>9} {:>17}  {}",
        "metric", "measured", "n", "ref", "band", "verdict"
    );
    for c in report.comparisons() {
        let verdict = match (c.tested, c.passes()) {
            (_, true) => "in band",
            (true, false) => "OUT",
            (false, false) => "outside (reported)",
        };
        println!(
            "{:<44} {:>9.3} {:>7} {:>9.3} {:>8.2}-{:<8.2}  {verdict}",
            c.metric, c.measured, c.samples, c.reference, c.band.0, c.band.1
        );
    }
    println!("\nheadway by queue position (s), mean [n]:");
    for (i, h) in report.headway_by_position_s.iter().enumerate() {
        print!("  {}: {:.2} [{}]", i + 1, h.mean, h.n);
    }
    println!();
    println!(
        "saturation headway all movements {:.3} s [{}]; queues {}; discharged {}; queue size \
         mean {:.1} max {:.0}",
        report.saturation_headway_all_movements_s.mean,
        report.saturation_headway_all_movements_s.n,
        report.queues,
        report.queued_vehicles_discharged,
        report.queue_size.mean,
        report.queue_size.max
    );
    println!(
        "free-flow ratio p15/p50/p85 {:.2}/{:.2}/{:.2}; launch accel p15/p85 {:.2}/{:.2}; \
         stop decel mean {:.2} max {:.2}",
        report.free_flow_speed_ratio.p15,
        report.free_flow_speed_ratio.p50,
        report.free_flow_speed_ratio.p85,
        report.launch_accel_mps2.p15,
        report.launch_accel_mps2.p85,
        report.stop_decel_mps2.mean,
        report.stop_decel_mps2.max
    );
    for (turn, s) in &report.turn_speed_mps {
        println!(
            "turn speed {turn:<8} mean {:.2} m/s ({:.1} km/h) p85 {:.2} [{}]",
            s.mean,
            s.mean * 3.6,
            s.p85,
            s.n
        );
    }
    println!(
        "permitted left lags: accepted mean {:.2} s p15 {:.2} [{}], rejected mean {:.2} s p85 \
         {:.2} [{}], Raff critical gap {:.2} s",
        report.permitted_left_lag_accepted_s.mean,
        report.permitted_left_lag_accepted_s.p15,
        report.permitted_left_lag_accepted_s.n,
        report.permitted_left_lag_rejected_s.mean,
        report.permitted_left_lag_rejected_s.p85,
        report.permitted_left_lag_rejected_s.n,
        report.permitted_left_critical_gap_s
    );
    println!(
        "pedestrian crossings {} (compliance {:.3}); approach volumes veh/h: avenues p15/p50/p85 \
         {:.0}/{:.0}/{:.0} [{}], streets {:.0}/{:.0}/{:.0} [{}]",
        report.pedestrian_crossings,
        report.pedestrian_compliance,
        report.avenue_volume_veh_per_h.p15,
        report.avenue_volume_veh_per_h.p50,
        report.avenue_volume_veh_per_h.p85,
        report.avenue_volume_veh_per_h.n,
        report.street_volume_veh_per_h.p15,
        report.street_volume_veh_per_h.p50,
        report.street_volume_veh_per_h.p85,
        report.street_volume_veh_per_h.n
    );
    println!(
        "trip speed mean {:.2} m/s [{}]; network speed {:.2} m/s ({:.1} mph)",
        report.trip_speed_mps.mean,
        report.trip_speed_mps.n,
        report.network_speed_mps,
        report.network_speed_mps / 0.44704
    );
    let nonzero: Vec<String> = Check::ALL
        .iter()
        .filter(|c| audit.count(**c) > 0)
        .map(|c| format!("{}={}", c.label(), audit.count(*c)))
        .collect();
    println!("audit (non-zero): {nonzero:?}");
    // `--examples N`: the auditor's examples of every class that fired, so one run
    // explains itself without a second audit run.
    for e in &audit.examples {
        println!(
            "  [{}] t={:.1}s actor={:?} other={:?} ({:.1}, {:.1}) lane={:?}: {}",
            e.check.label(),
            e.t_s,
            e.actor,
            e.other,
            e.x_m,
            e.y_m,
            e.lane,
            e.detail
        );
    }
    Ok(())
}
