//! Group 2: calibration against published measurements.
//!
//! The measuring is `v2xw_mobility::calibration` (the HCM field procedures, Raff's method,
//! Knoblauch's walking speeds); this module runs it on two studies and holds each figure
//! to its published band, with a fault per check that pushes the figure out:
//!
//! * the **saturation-flow field study** (`SaturationExperiment`): one signalised
//!   crossroads whose approaches never run dry;
//! * the **Midtown-shaped grid**: 5 × 8 blocks of 274 × 80 m, two lanes each way, 25 mph,
//!   90 s signals with crosswalks, buildings, 120 pedestrians and 0.8 veh/s of Poisson
//!   demand for 300 s (150 s in quick mode). The same run feeds groups 3's safety
//!   surrogates.

use std::sync::Arc;

use v2xw_core::card::ModelCard;
use v2xw_core::model::Model as _;
use v2xw_core::time::Duration;
use v2xw_core::weather::WeatherState;
use v2xw_mobility::audit::AuditParams;
use v2xw_mobility::calibration::{CalibrationReport, SaturationExperiment};
use v2xw_mobility::carfollowing::idm::{Idm, IdmPreset};
use v2xw_mobility::demand::{FleetMix, OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::engine::VruPopulation;
use v2xw_mobility::lanechange::mobil::MobilPreset;
use v2xw_mobility::traits::CarFollowing;
use v2xw_mobility::views::{DriverProfile, LaneView, LeaderView, VehicleView};
use v2xw_mobility::vru::social_force::{SocialForce, SocialForceParams};
use v2xw_mobility::{EngineParams, IntersectionMode, NativeMobility, VehicleClass};
use v2xw_world::procedural::GridParams;
use v2xw_world::{ImportOptions, World};

use crate::study::{self, StudyResult, Tamper};
use crate::{Check, Group, Lab, Outcome, Row, Variant};

/// The Midtown-shaped grid.
pub fn midtown_grid() -> Result<World, String> {
    let params = GridParams {
        cols: 5,
        rows: 8,
        block_x_m: 274.0,
        block_y_m: 80.0,
        lanes_per_direction: 2,
        lane_width_m: 3.35,
        sidewalk_m: 2.5,
        speed_limit_mps: 11.176,
        signalised: true,
        cycle_s: 90.0,
        amber_s: 3.0,
        crossings: true,
        block_buildings: true,
        corner_radius_m: 4.5,
        ..GridParams::legacy()
    };
    v2xw_world::procedural::grid(&params, &ImportOptions::default()).map_err(|e| e.to_string())
}

/// Which engine the grid study runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridVariant {
    /// As shipped.
    Shipped,
    /// Broken drivers and pedestrians: a = 0.6, b = 0.8, cruising at 0.8 × the limit, no
    /// heterogeneity, pedestrians wanting 0.9 m/s.
    BrokenDrivers,
    /// The observation stream tampered with (see the check).
    Tampered,
    /// No junction control.
    NoJunctionControl,
    /// Turning at 6 m/s² of lateral acceleration.
    FastTurns,
    /// Pedestrians cross against the signal with this probability (per mille).
    Jaywalk(u32),
    /// 15 % motorcycles in the fleet.
    Motorcycles,
    /// 15 % motorcycles that drive with the car's parameters.
    MotorcyclesAsCars,
}

/// A car-following model that delegates to the city IDM but hands out broken drivers.
struct BrokenDrivers {
    inner: Idm,
    /// Cruise at this fraction of the limit.
    limit_factor: f64,
    /// Override of a and b, if any.
    ab: Option<(f64, f64)>,
    /// Give a motorcycle the car's driver.
    motorcycle_as_car: bool,
}

impl v2xw_core::model::Model for BrokenDrivers {
    fn card(&self) -> &ModelCard {
        self.inner.card()
    }
}

impl CarFollowing for BrokenDrivers {
    fn accel(&self, ego: &VehicleView, leader: Option<&LeaderView>, lane: &LaneView, w: &WeatherState) -> f64 {
        let mut l = *lane;
        l.speed_limit_mps *= self.limit_factor;
        self.inner.accel(ego, leader, &l, w)
    }

    fn profile(&self, class: VehicleClass) -> DriverProfile {
        let class = if self.motorcycle_as_car && class == VehicleClass::Motorcycle {
            VehicleClass::Passenger
        } else {
            class
        };
        let mut p = self.inner.profile(class);
        if let Some((a, b)) = self.ab {
            p.max_accel_mps2 = a;
            p.comfort_decel_mps2 = b;
        }
        p
    }
}

/// Runs (or recalls) the Midtown-grid study for `variant`.
///
/// # Errors
/// The world generator's or the engine's.
pub fn grid_study(lab: &mut Lab, variant: GridVariant, tamper: Option<Tamper>) -> Result<StudyResult, String> {
    let key = format!("midtown-grid-{variant:?}");
    lab.study(&key, |cfg| {
        let world = midtown_grid()?;
        let seconds = cfg.secs(300, 150);
        let mut params = EngineParams::default();
        let mut sf = SocialForceParams::default();
        let mut fleet = FleetMix::CarsOnly;
        let cf: Arc<dyn CarFollowing + Send + Sync> = match variant {
            GridVariant::BrokenDrivers => {
                params.driver_heterogeneity = false;
                sf.desired_speed_mean_mps = 0.9;
                Arc::new(BrokenDrivers {
                    inner: Idm::new(IdmPreset::UrbanHcm),
                    limit_factor: 0.8,
                    ab: Some((0.6, 0.8)),
                    motorcycle_as_car: false,
                })
            }
            GridVariant::MotorcyclesAsCars => {
                fleet = FleetMix::from_shares(&[(VehicleClass::Passenger, 0.85), (VehicleClass::Motorcycle, 0.15)]);
                Arc::new(BrokenDrivers {
                    inner: Idm::new(IdmPreset::UrbanHcm),
                    limit_factor: 1.0,
                    ab: None,
                    motorcycle_as_car: true,
                })
            }
            _ => Arc::new(Idm::new(IdmPreset::UrbanHcm)),
        };
        match variant {
            GridVariant::NoJunctionControl => {
                params.intersections = IntersectionMode::None;
                params.junction_clearance = false;
                params.crosswalk_yield = false;
            }
            GridVariant::FastTurns => params.turn_lateral_accel_mps2 = 6.0,
            GridVariant::Jaywalk(pm) => sf.jaywalk_probability = f64::from(pm) / 1000.0,
            GridVariant::Motorcycles => {
                fleet = FleetMix::from_shares(&[(VehicleClass::Passenger, 0.85), (VehicleClass::Motorcycle, 0.15)]);
            }
            _ => {}
        }
        let engine = NativeMobility::with_models(params, cf, MobilPreset::Kesting2007)
            .with_vru(SocialForce::new(sf))
            .with_vru_population(VruPopulation {
                pedestrians: 120,
                cyclists: 0,
            });
        let demand = PoissonDemand::new(
            &world,
            PoissonParams {
                arrival_rate_per_s: 0.8,
                duration: Duration::from_secs(seconds),
                fleet,
                ..PoissonParams::default()
            },
            OdParams::default(),
        )
        .map_err(|e| e.to_string())?;
        study::run(
            &world,
            engine,
            Box::new(demand),
            0xCA11_B8A7,
            params.step,
            seconds as f64,
            AuditParams::default(),
            &[],
            if variant == GridVariant::Tampered { tamper } else { None },
            |_, _| {},
        )
    })
}

/// The harness's comparisons named `names`, as rows.
fn rows_from(report: &CalibrationReport, names: &[&str]) -> Vec<Row> {
    let all = report.comparisons();
    names
        .iter()
        .filter_map(|n| all.iter().find(|c| c.metric == *n))
        .map(|c| {
            let measured = if c.samples == 0 { f64::NAN } else { c.measured };
            Row {
                metric: c.metric.to_string(),
                measured,
                samples: Some(c.samples),
                reference: c.source.to_string(),
                band: c.tested.then_some(c.band),
            }
        })
        .collect()
}

/// The checks of this group.
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "calibration/saturation-flow",
            group: Group::Calibration,
            title: "A saturated stop line discharges at the HCM's rate, with its start-up lost time and queue spacing",
            procedure: "`SaturationExperiment`: a 3 × 3 grid of 274 m blocks, two lanes each \
                way, 25 mph, a 90 s fixed-time plan with an ITE all-red, no crosswalks (the \
                HCM's base conditions), every approach to the centre junction kept loaded \
                for 600 s. The HCM field method: each queue recorded at green onset, each \
                queued vehicle's rear timed over the stop line; saturation headway from the \
                fifth vehicle on, through movements; start-up lost time over the first four.",
            faults: &["the Kesting 2010 freeway drivers (T 1.5 s, a 1.4 m/s²) instead of the city drivers"],
            run: saturation,
        },
        Check {
            id: "calibration/midtown-drivers",
            group: Group::Calibration,
            title: "Free-flow speeds, launches, stops and walking speeds on a Midtown-shaped grid match published figures",
            procedure: "The Midtown-grid study (module docs): free-flow speed over the posted \
                limit mid-block with no leader in reach; launch acceleration from standstill \
                to 8 m/s; the peak deceleration of each stop from above 5 m/s; the speed of \
                every walking pedestrian step.",
            faults: &["drivers with a = 0.6 m/s², b = 0.8 m/s², no heterogeneity, cruising at 0.8 × the limit; pedestrians wanting 0.9 m/s"],
            run: midtown_drivers,
        },
        Check {
            id: "calibration/turning",
            group: Group::Calibration,
            title: "Cars turn at corners no faster than AASHTO's side friction allows",
            procedure: "The Midtown-grid study: for every traversal of a junction connector \
                above 2 m/s, the peak lateral acceleration `v·dψ/dt` from the published \
                heading; its 85th percentile against the side-friction factor AASHTO's \
                Green Book uses for low-speed intersection curves. Turning speeds by turn \
                are reported (no single published figure fits every corner radius).",
            faults: &["drivers who accept 6 m/s² of lateral acceleration in a turn"],
            run: turning,
        },
        Check {
            id: "calibration/permitted-left-gap",
            group: Group::Calibration,
            title: "Permitted left turners accept gaps like the HCM's field drivers (critical headway 4.5 s)",
            procedure: "The Midtown-grid study: each lag offered to a left turner waiting at \
                the stop line on a permissive green, timed to the opposing car's arrival at \
                its stop line, accepted or rejected; Raff's critical gap from the two \
                samples (Raff & Hart 1950).",
            faults: &[],
            run: permitted_left,
        },
        Check {
            id: "calibration/pedestrian-compliance",
            group: Group::Calibration,
            title: "With jaywalking switched on at Manhattan's rate, pedestrians cross on Walk as often as Manhattan's do",
            procedure: "The Midtown-grid study with `jaywalk_probability` = 0.107 (Basch et \
                al. 2015's 10.7 % crossing against the signal): the share of kerb \
                departures onto a signalised crossing made on Walk. The shipped default is \
                0 (every pedestrian complies), reported beside it.",
            faults: &["jaywalk probability 0.5"],
            run: pedestrian_compliance,
        },
        Check {
            id: "calibration/motorcycles",
            group: Group::Calibration,
            title: "Motorcycles launch harder than cars",
            procedure: "The Midtown-grid study with 15 % motorcycles in the fleet: mean launch \
                acceleration 0–8 m/s of each class. Powered two-wheelers out-accelerate cars \
                from a stop, which is why they lead a queue's discharge (Minh, Sano & \
                Matsumoto 2005, Proc. EASTS 5, and the PTW literature; qualitative: no \
                urban field mean was read, so the figures are reported and the ordering is \
                held).",
            faults: &["motorcycles given the car's driver parameters"],
            run: motorcycles,
        },
    ]
}

fn saturation(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let key = format!("saturation-{}", variant.is_fault());
    let r = lab.study(&key, |cfg| {
        let engine = if variant.is_fault() {
            NativeMobility::with_models(
                EngineParams::default(),
                Arc::new(Idm::new(IdmPreset::Kesting2010)),
                MobilPreset::Kesting2007,
            )
        } else {
            NativeMobility::new(EngineParams::default())
        };
        SaturationExperiment {
            seconds: cfg.secs(600, 400),
            ..SaturationExperiment::default()
        }
        .run(engine)
        .map_err(|e| e.to_string())
    })?;
    let mut rows = rows_from(&r, &["saturation headway, through, s", "start-up lost time, s", "queue spacing, front to front, m"]);
    let h = &r.headway_by_position_s;
    if h.len() >= 6 {
        rows.push(Row::held(
            "first discharge headway minus the sixth, s",
            h[0].mean - h[5].mean,
            (0.0, f64::INFINITY),
            "FHWA Signal Timing Manual 2008 §3.3.1 Fig. 3-2: headways shorten and stabilise after about the fourth vehicle",
        ));
    }
    let g = [3.8, 3.1, 2.7, 2.4, 2.2];
    for (i, want) in g.iter().enumerate() {
        if let Some(s) = h.get(i) {
            rows.push(
                Row::reported(
                    format!("headway of queue position {}, s", i + 1),
                    s.mean,
                    format!("Greenshields 1947: {want} s (secondary, via Mannering & Washburn)"),
                )
                .n(s.n),
            );
        }
    }
    Ok(Outcome {
        rows,
        notes: vec![format!("{} queues recorded, {} queued vehicles timed.", r.queues, r.queued_vehicles_discharged)],
    })
}

fn midtown_drivers(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let gv = if variant.is_fault() { GridVariant::BrokenDrivers } else { GridVariant::Shipped };
    let s = grid_study(lab, gv, None)?;
    let mut rows = rows_from(
        &s.calib,
        &[
            "free-flow speed / limit, mean",
            "free-flow speed / limit, spread",
            "launch acceleration 0-8 m/s, mean, m/s²",
            "stop deceleration, 85th percentile, m/s²",
            "pedestrian walking speed, mean, m/s",
            "pedestrian walking speed, 15th percentile, m/s",
            "network travel speed, m/s",
        ],
    );
    rows.push(Row::reported(
        "acceleration, 95th percentile of vehicle-steps, m/s²",
        s.motion.accel_p95,
        "reported: no urban field distribution of every-step acceleration was read",
    ));
    rows.push(Row::reported(
        "acceleration, 5th percentile of vehicle-steps, m/s²",
        s.motion.accel_p05,
        "reported",
    ));
    Ok(Outcome {
        rows,
        notes: vec![format!(
            "{} vehicles at peak, {} trips completed, {} trips the router could not place.",
            s.audit.stats.peak_vehicles, s.audit.stats.trips_completed, s.dropped_trips
        )],
    })
}

/// AASHTO's side-friction factor for a low-speed intersection curve at 10 mph (the top of
/// the range), times g.
pub const AASHTO_TURN_LATERAL_MPS2: f64 = 0.38 * 9.81;

fn turning(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let gv = if variant.is_fault() { GridVariant::FastTurns } else { GridVariant::Shipped };
    let s = grid_study(lab, gv, None)?;
    let lat = &s.motion.turn_peak_lateral_mps2;
    let mut rows = vec![Row::held(
        "peak lateral acceleration in a turn, 85th percentile, m/s²",
        lat.p85,
        (0.0, AASHTO_TURN_LATERAL_MPS2),
        "AASHTO Green Book 2018 Ch. 3/9: side friction 0.32–0.38 for intersection curves at 15–10 mph (secondary), × g",
    )
    .n(lat.n)];
    rows.push(Row::reported("peak lateral acceleration in a turn, mean, m/s²", lat.mean, "—"));
    for (turn, sm) in &s.calib.turn_speed_mps {
        rows.push(
            Row::reported(format!("{turn} turn: lowest speed through the connector, mean, km/h"), sm.mean * 3.6, "reported: depends on the corner radius")
                .n(sm.n),
        );
    }
    Ok(Outcome { rows, notes: vec![] })
}

fn permitted_left(lab: &mut Lab, _variant: Variant) -> Result<Outcome, String> {
    let s = grid_study(lab, GridVariant::Shipped, None)?;
    let mut rows = rows_from(&s.calib, &["permitted left critical gap (Raff), s"]);
    let a = &s.calib.permitted_left_lag_accepted_s;
    let r = &s.calib.permitted_left_lag_rejected_s;
    rows.push(Row::reported("lags accepted, mean, s", a.mean, "—").n(a.n));
    rows.push(Row::reported("lags rejected, mean, s", r.mean, "—").n(r.n));
    Ok(Outcome {
        rows,
        notes: vec!["No fault is listed: a fault only proves a check can fail, and this one has failed as shipped (see the verdict).".to_string()],
    })
}

fn pedestrian_compliance(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let pm = if variant.is_fault() { 500 } else { 107 };
    let s = grid_study(lab, GridVariant::Jaywalk(pm), None)?;
    let mut rows = rows_from(&s.calib, &["pedestrian signal compliance, share"]);
    for r in &mut rows {
        // Held here: the harness reports it untested because its default run has jaywalking off.
        r.band = Some((0.85, 0.95));
    }
    let shipped = grid_study(lab, GridVariant::Shipped, None)?;
    rows.push(
        Row::reported(
            "compliance with the shipped default (jaywalking off), share",
            shipped.calib.pedestrian_compliance,
            "the shipped default; Manhattan's is about 0.89",
        )
        .n(shipped.calib.pedestrian_crossings),
    );
    Ok(Outcome {
        rows,
        notes: vec![format!("{} crossings started at signalised crosswalks.", s.calib.pedestrian_crossings)],
    })
}

fn motorcycles(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let gv = if variant.is_fault() { GridVariant::MotorcyclesAsCars } else { GridVariant::Motorcycles };
    let s = grid_study(lab, gv, None)?;
    let car = s.motion.launch_by_class.get(VehicleClass::Passenger.as_str()).cloned().unwrap_or_default();
    let moto = s.motion.launch_by_class.get(VehicleClass::Motorcycle.as_str()).cloned().unwrap_or_default();
    let ratio = if car.n > 0 && moto.n > 0 { moto.mean / car.mean } else { f64::NAN };
    Ok(Outcome {
        rows: vec![
            Row::held(
                "motorcycle launch acceleration / car launch acceleration",
                ratio,
                (1.05, f64::INFINITY),
                "PTWs out-accelerate cars from a stop (Minh, Sano & Matsumoto 2005; qualitative)",
            ),
            Row::reported("motorcycle launch acceleration 0-8 m/s, mean, m/s²", moto.mean, "reported").n(moto.n),
            Row::reported("car launch acceleration 0-8 m/s, mean, m/s²", car.mean, "reported").n(car.n),
        ],
        notes: vec![],
    })
}
