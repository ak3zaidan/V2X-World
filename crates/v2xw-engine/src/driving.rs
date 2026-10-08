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
    let mut mobility = match library::build(spec, scenario.seed) {
        Ok(Some(cf)) => NativeMobility::with_models(params, cf, MobilPreset::Kesting2007),
        Ok(None) | Err(_) => NativeMobility::new(params),
    };
    // `actors.vehicles.drivers.motorcycle_filtering` overrides the jurisdiction's rule;
    // filtering needs the sub-lane model, so allowing it turns that model on.
    let mut rules = *rules;
    let mut lateral = spec.lateral.clone();
    match scenario.actors.vehicles.drivers.motorcycle_filtering {
        Some(true) => {
            rules.motorcycle_filtering = v2xw_mobility::rules::Filtering::BetweenStopped;
            let l = lateral.get_or_insert_with(Default::default);
            l.params.filter.motorcycles_share = true;
        }
        Some(false) => rules.motorcycle_filtering = v2xw_mobility::rules::Filtering::Prohibited,
        None => {}
    }
    if let Some(lateral) = &lateral {
        mobility = mobility.with_lateral(lateral.build(&rules));
    }
    mobility
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

#[cfg(test)]
mod drivers_tests {
    use crate::scenario::Scenario;
    use crate::scenario::schema::DriverRealism;

    #[test]
    fn the_drivers_block_reaches_the_engine() {
        let mut s = Scenario::minimal();
        let base = crate::wiring::native_mobility(&s);
        assert_eq!(base.params().human, Default::default());
        assert!(base.params().driver_heterogeneity);

        s.actors.vehicles.drivers.realism = DriverRealism::Human;
        s.actors.vehicles.drivers.reaction_time_s = Some(1.1);
        s.actors.vehicles.drivers.truck_lane_restriction = Some(true);
        let m = crate::wiring::native_mobility(&s);
        let h = m.params().human;
        assert_eq!(h.reaction_time_s, 1.1);
        assert!(h.parameter_spread > 0.0 && h.distance_error > 0.0 && h.truck_lane_restriction);

        s.actors.vehicles.drivers = Default::default();
        s.actors.vehicles.drivers.realism = DriverRealism::Identical;
        let m = crate::wiring::native_mobility(&s);
        assert!(!m.params().driver_heterogeneity && !m.params().driver_styles);
    }
}
