//! The car-following sweep behind `IdmPreset::UrbanHcm`: runs the saturation-flow experiment
//! (`calibration::SaturationExperiment`) for each driver set given as `T,A,B,S0,FOLLOW`
//! (time gap s, max acceleration, comfortable deceleration, standstill gap, queue start-up
//! delay median s) and prints the HCM figures; `trace` prints a loaded green onset step by
//! step.
//!
//! ```text
//! cargo run -p v2xw-mobility --example calib_sweep -- 1.0,1.7,2,2,0.5 1.5,1.4,2,2,0.5
//! ```
use std::sync::Arc;

use v2xw_core::card::ModelCard;
use v2xw_core::weather::WeatherState;
use v2xw_mobility::calibration::SaturationExperiment;
use v2xw_mobility::lanechange::MobilPreset;
use v2xw_mobility::views::{DriverProfile, LaneView, LeaderView, VehicleView};
use v2xw_mobility::{CarFollowing, EngineParams, Idm, IdmPreset, NativeMobility, VehicleClass};

struct Swept {
    idm: Idm,
    driver: DriverProfile,
}

impl v2xw_core::model::Model for Swept {
    fn card(&self) -> &ModelCard {
        v2xw_core::model::Model::card(&self.idm)
    }
}

impl CarFollowing for Swept {
    fn accel(&self, e: &VehicleView, l: Option<&LeaderView>, lane: &LaneView, w: &WeatherState) -> f64 {
        self.idm.accel(e, l, lane, w)
    }
    fn profile(&self, _class: VehicleClass) -> DriverProfile {
        self.driver
    }
}

fn main() {
    // Each argument is one configuration, `T,A,B,S0,FOLLOW` (FOLLOW: the queue start-up
    // delay median, s); with none, the defaults.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("trace") {
        trace();
        return;
    }
    let specs = if args.is_empty() {
        vec![String::new()]
    } else {
        args
    };
    for spec in specs {
        run(&spec);
    }
}

/// Three cars queue at the experiment's centre junction; every step from a second before
/// the green to 12 s after it is printed.
fn trace() {
    use v2xw_core::ids::LaneId;
    use v2xw_mobility::{Mobility, MobilityCommand, MobilityCtx, TripRequest};
    use v2xw_world::{LaneKind, TurnDirection};
    let exp = SaturationExperiment::default();
    let world = exp.world().expect("world");
    let rng = v2xw_core::rng::RngRegistry::new(exp.seed);
    let params = EngineParams::default();
    let mut engine = NativeMobility::new(params);
    {
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        engine
            .init(&mut ctx, Box::new(v2xw_mobility::NoDemand::new()))
            .expect("init");
    }
    let centre = world
        .roads
        .junctions()
        .iter()
        .filter(|j| matches!(j.control, v2xw_world::JunctionControl::Signalised { .. }))
        .max_by_key(|j| (j.incoming.len(), core::cmp::Reverse(j.id)))
        .expect("centre")
        .id;
    let (origin, exit, via): (LaneId, LaneId, LaneId) = world
        .roads
        .lanes()
        .iter()
        .filter(|l| l.kind == LaneKind::Driving && world.edge(l.edge).to == centre)
        .find_map(|l| {
            world
                .successors(l.id)
                .iter()
                .find(|c| c.via.is_some() && c.direction == TurnDirection::Straight)
                .map(|c| (l.id, c.to_lane, c.via.expect("via")))
        })
        .expect("an approach");
    let len = world.lane(origin).length_m;
    let mut t = 0u64;
    let step = params.step;
    let mut prev_state = None;
    let mut green_at: Option<f64> = None;
    let _ = exit;
    // Every approach of the centre, loaded as the experiment loads it.
    let feeds: Vec<(LaneId, LaneId)> = world
        .roads
        .lanes()
        .iter()
        .filter(|l| l.kind == LaneKind::Driving && world.edge(l.edge).to == centre)
        .filter_map(|l| {
            world
                .successors(l.id)
                .iter()
                .find(|c| c.via.is_some() && c.direction == TurnDirection::Straight)
                .map(|c| (l.id, c.to_lane))
        })
        .collect();
    let mut seq = 0u64;
    let mut greens = 0;
    while t < 400 * v2xw_core::time::NS_PER_S {
        let ts = v2xw_core::time::ns_to_secs(t);
        let state = v2xw_mobility::audit::movement_state(&world, via, t);
        let mut nearest: std::collections::BTreeMap<LaneId, f64> = Default::default();
        for (_, lane, s, _) in engine.longitudinal_states() {
            let e = nearest.entry(lane).or_insert(f64::INFINITY);
            *e = e.min(s);
        }
        {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            for (o, x) in &feeds {
                if nearest.get(o).is_some_and(|f| *f < 14.0) {
                    continue;
                }
                seq += 1;
                let class = VehicleClass::Passenger;
                engine.command(
                    &mut ctx,
                    MobilityCommand::Spawn(TripRequest {
                        seq,
                        t,
                        origin: *o,
                        origin_s_m: class.spec().length_m,
                        destination: *x,
                        class,
                        desired_speed_mps: class.spec().desired_speed_mps(),
                    }),
                );
            }
        }
        if prev_state.is_some() && prev_state != state {
            println!("t={ts:.1} signal {prev_state:?} -> {state:?}");
            if matches!(state, Some(v2xw_world::SignalState::Green)) {
                greens += 1;
                if greens >= 2 {
                    green_at = Some(ts);
                }
            }
        }
        prev_state = state;
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, step);
        if let Some(g) = green_at
            && ts >= g - 1.0
            && ts <= g + 8.0
        {
            let rows: Vec<String> = engine
                .audit_actors(&world, update.t)
                .iter()
                .filter(|a| {
                    (a.lane == origin && len - a.s_m < 16.0)
                        || world.lane(a.lane).junction == Some(centre)
                })
                .map(|a| {
                    let to_line = if a.lane == origin { len - a.s_m } else { -a.s_m };
                    format!(
                        "#{}@{} line {:6.2} v {:5.2} a {:5.2}",
                        a.actor.index(),
                        a.lane.index(),
                        to_line,
                        a.speed_mps,
                        a.accel_mps2
                    )
                })
                .collect();
            println!("t={:.1} (+{:.1}) {}", v2xw_core::time::ns_to_secs(update.t), v2xw_core::time::ns_to_secs(update.t) - g, rows.join(" | "));
        }
        if green_at.is_some_and(|g| ts > g + 8.0) {
            break;
        }
        t = update.t;
    }
}

fn run(spec: &str) {
    let v: Vec<f64> = spec.split(',').filter_map(|x| x.parse().ok()).collect();
    let at = |i: usize, d: f64| v.get(i).copied().unwrap_or(d);
    let env = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let base = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
    let driver = DriverProfile {
        time_headway_s: at(0, base.time_headway_s),
        max_accel_mps2: at(1, base.max_accel_mps2),
        comfort_decel_mps2: at(2, base.comfort_decel_mps2),
        min_gap_m: at(3, base.min_gap_m),
        ..base
    };
    let mut idm_params = *Idm::new(IdmPreset::Kesting2010).params();
    idm_params.enhanced = env("ENH", 0.0) > 0.5;
    let cf: Arc<dyn CarFollowing + Send + Sync> = Arc::new(Swept {
        idm: Idm::with_params(IdmPreset::Kesting2010, idm_params),
        driver,
    });
    let params = EngineParams {
        driver_heterogeneity: env("HET", 1.0) > 0.5,
        queue_start_delay_median_s: at(4, EngineParams::default().queue_start_delay_median_s),
        ..EngineParams::default()
    };
    let engine = NativeMobility::with_models(params, cf, MobilPreset::Kesting2007);
    let exp = SaturationExperiment {
        seconds: env("SECS", 450.0) as u64,
        ..SaturationExperiment::default()
    };
    let r = exp.run(engine).expect("run");
    let h: Vec<String> = r
        .headway_by_position_s
        .iter()
        .map(|h| format!("{:.2}", h.mean))
        .collect();
    println!(
        "T={} a={} b={} s0={} follow={} het={} enh={}: h_s={:.3} [{}] lost={:.2} spacing={:.2} launch={:.2} h={:?}",
        driver.time_headway_s,
        driver.max_accel_mps2,
        driver.comfort_decel_mps2,
        driver.min_gap_m,
        params.queue_start_delay_median_s,
        params.driver_heterogeneity,
        idm_params.enhanced,
        r.saturation_headway_s.mean,
        r.saturation_headway_s.n,
        r.startup_lost_time_s,
        r.queue_spacing_m.mean,
        r.launch_accel_mps2.mean,
        h
    );
    let h1 = &r.headway_by_position_s[0];
    println!(
        "    h1 p15/p50/p85 {:.2}/{:.2}/{:.2} [{}]; head gap mean {:.2} p85 {:.2} max {:.2}; queues {} size mean {:.1}",
        h1.p15,
        h1.p50,
        h1.p85,
        h1.n,
        r.queue_head_gap_m.mean,
        r.queue_head_gap_m.p85,
        r.queue_head_gap_m.max,
        r.queues,
        r.queue_size.mean
    );
}
