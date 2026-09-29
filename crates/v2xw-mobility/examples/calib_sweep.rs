//! TEMPORARY sweep of the car-following driver set against the saturation-flow experiment.
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
    let env = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let base = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
    let driver = DriverProfile {
        time_headway_s: env("T", base.time_headway_s),
        max_accel_mps2: env("A", base.max_accel_mps2),
        comfort_decel_mps2: env("B", base.comfort_decel_mps2),
        min_gap_m: env("S0", base.min_gap_m),
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
        "T={} a={} b={} s0={} het={} enh={}: h_s={:.3} [{}] lost={:.2} spacing={:.2} launch={:.2} h={:?}",
        driver.time_headway_s,
        driver.max_accel_mps2,
        driver.comfort_decel_mps2,
        driver.min_gap_m,
        params.driver_heterogeneity,
        idm_params.enhanced,
        r.saturation_headway_s.mean,
        r.saturation_headway_s.n,
        r.startup_lost_time_s,
        r.queue_spacing_m.mean,
        r.launch_accel_mps2.mean,
        h
    );
}
