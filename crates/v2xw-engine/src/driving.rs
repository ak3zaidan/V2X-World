//! The scenario's driving-model block (`actors.vehicles.driving`) turned into the native
//! engine's car-following and lateral models.
//!
//! The library itself lives in `v2xw_mobility::carfollowing::library` and
//! `v2xw_mobility::lateral`; this module only builds what a scenario names, refuses what it
//! cannot build (at validation, so the page hears about a bad name at Apply), and leaves
//! a scenario without the block exactly as it was.

use v2xw_mobility::MobilPreset;
use v2xw_mobility::carfollowing::library;
use v2xw_mobility::rules::TrafficRules;
use v2xw_mobility::{EngineParams, NativeMobility};

use crate::error::ScenarioError;
use crate::scenario::Scenario;

/// The native engine with the scenario's driving models.
///
/// No `driving` block: [`NativeMobility::new`], byte for byte the engine a scenario got
/// before the library existed. A block the validator would refuse never reaches here (the
/// loader runs [`validate`] first); should one arrive anyway, the default engine is built
/// rather than panicking mid-run.
pub fn native_with_driving(
    scenario: &Scenario,
    params: EngineParams,
    rules: &TrafficRules,
) -> NativeMobility {
    let spec = &scenario.actors.vehicles.driving;
    let d = &scenario.actors.vehicles.driver;
    let lane_change_set = d.politeness.is_some() || d.lane_change_threshold_mps2.is_some();
    let mut mobility = match library::build(spec, scenario.seed) {
        Ok(Some(cf)) if !lane_change_set => {
            NativeMobility::with_models(params, cf, MobilPreset::Kesting2007)
        }
        Ok(Some(cf)) => NativeMobility::with_lane_change_params(
            params,
            cf,
            MobilPreset::Kesting2007,
            mobil_of(d),
        ),
        Ok(None) | Err(_) if lane_change_set => {
            // `NativeMobility::new`'s own car-following model, with these MOBIL numbers.
            let idm: std::sync::Arc<dyn v2xw_mobility::traits::CarFollowing + Send + Sync> =
                std::sync::Arc::new(v2xw_mobility::carfollowing::idm::Idm::new(
                    v2xw_mobility::carfollowing::idm::IdmPreset::UrbanHcm,
                ));
            NativeMobility::with_lane_change_params(
                params,
                idm,
                MobilPreset::Kesting2007,
                mobil_of(d),
            )
        }
        Ok(None) | Err(_) => NativeMobility::new(params),
    };
    if d.max_accel_mps2.is_some()
        || d.comfort_decel_mps2.is_some()
        || d.time_headway_s.is_some()
        || d.min_gap_m.is_some()
    {
        mobility = mobility.with_driver_override(v2xw_mobility::DriverOverride {
            max_accel_mps2: d.max_accel_mps2,
            comfort_decel_mps2: d.comfort_decel_mps2,
            time_headway_s: d.time_headway_s,
            min_gap_m: d.min_gap_m,
        });
    }
    if let Some(lateral) = &spec.lateral {
        mobility = mobility.with_lateral(lateral.build(rules));
    }
    mobility
}

/// Kesting 2007's MOBIL parameters with `actors.vehicles.driver`'s numbers over them.
fn mobil_of(
    d: &crate::scenario::schema::DriverSettings,
) -> v2xw_mobility::lanechange::mobil::MobilParams {
    let mut p = MobilPreset::Kesting2007.params();
    if let Some(v) = d.politeness {
        p.politeness = v;
    }
    if let Some(v) = d.lane_change_threshold_mps2 {
        p.threshold_mps2 = v;
    }
    p
}

/// The block's errors, each naming its scenario key.
pub fn validate(s: &Scenario) -> Vec<ScenarioError> {
    let spec = &s.actors.vehicles.driving;
    let mut e = Vec::new();
    if let Err(why) = library::build(spec, s.seed) {
        let (field, msg) = why.split_once(": ").map_or(
            ("actors.vehicles.driving".to_string(), why.clone()),
            |(f, m)| (f.to_string(), m.to_string()),
        );
        e.push(ScenarioError::conflict(field, msg));
    }
    let c = &spec.cooperative;
    if !(c.max_age_s.is_finite() && (0.0..=5.0).contains(&c.max_age_s)) {
        e.push(ScenarioError::conflict(
            "actors.vehicles.driving.cooperative.max_age_s",
            format!("{} is outside 0..=5 s", c.max_age_s),
        ));
    }
    if !(c.association_gate_m.is_finite() && (0.5..=50.0).contains(&c.association_gate_m)) {
        e.push(ScenarioError::conflict(
            "actors.vehicles.driving.cooperative.association_gate_m",
            format!("{} is outside 0.5..=50 m", c.association_gate_m),
        ));
    }
    if let Some(l) = &spec.lateral {
        let p = &l.params;
        if !(p.resolution_m > 0.0 && p.max_speed_lat_mps > 0.0) {
            e.push(ScenarioError::conflict(
                "actors.vehicles.driving.lateral.params",
                "resolution_m and max_speed_lat_mps must be positive",
            ));
        }
    }
    e
}
