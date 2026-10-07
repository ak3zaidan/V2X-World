//! The scenario's `energy:` section.
//!
//! ```yaml
//! energy:
//!   emission_model: energy/emission/hbefa3      # or hbefa3 | phemlight | moves-opmode
//!   emission_model_by_segment:                  # optional: another model per segment
//!     passenger-car: phemlight
//!   ev_model: energy/ev/kurczveil               # or kurczveil | vt-cpem
//!   fleet:
//!     passenger-car: { ev_share: 0.2, diesel_share: 0.3, euro: 6 }
//!     urban-bus:     { ev_share: 0.5 }
//!   ev:
//!     initial_soc: 0.8
//!     params: { mass_kg: 1600 }                 # the EV model's card parameter names
//! ```
//!
//! Every field is optional; `energy: {}` runs HBEFA 3 over a Euro 4 petrol fleet (diesel
//! for trucks, buses and coaches) with no electric vehicles. A scenario without the
//! section computes no energy metrics, so every scenario written before this crate keeps
//! its output — and its digests — exactly.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::powertrain::{FleetExt, Segment, SegmentMix};
use crate::registry;

/// The `energy:` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnergySpec {
    /// The combustion-vehicle emission model, by id or short name.
    #[serde(default = "EnergySpec::default_emission")]
    pub emission_model: String,
    /// A different emission model for some segments, by id or short name — PHEMlight's
    /// measured curves for passenger cars, say, and HBEFA 3 for everything else.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub emission_model_by_segment: BTreeMap<Segment, String>,
    /// The battery-electric energy model, by id or short name.
    #[serde(default = "EnergySpec::default_ev")]
    pub ev_model: String,
    /// The fleet's powertrains per segment.
    #[serde(default)]
    pub fleet: BTreeMap<Segment, SegmentMix>,
    /// Battery-electric settings.
    #[serde(default)]
    pub ev: EvSpec,
    /// The speed below which a vehicle counts as stopped, m/s.
    #[serde(default = "EnergySpec::default_stop_speed")]
    pub stop_speed_mps: f64,
}

/// Battery-electric settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvSpec {
    /// State of charge every electric vehicle starts with, `0..=1`.
    #[serde(default = "EvSpec::default_soc")]
    pub initial_soc: f64,
    /// Overrides of the EV model's card parameters, by name.
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
}

impl EvSpec {
    /// 0.8: an implementer's choice, carried on the provider's card as `todo-calibrate`
    /// (`ev.initial_soc`) because the right value is the study's charging assumption.
    const fn default_soc() -> f64 {
        0.8
    }
}

impl Default for EvSpec {
    fn default() -> Self {
        Self {
            initial_soc: Self::default_soc(),
            params: BTreeMap::new(),
        }
    }
}

impl EnergySpec {
    fn default_emission() -> String {
        crate::hbefa3::MODEL_ID.to_string()
    }
    fn default_ev() -> String {
        crate::ev::KURCZVEIL_ID.to_string()
    }
    /// SUMO's halting threshold: a vehicle below 0.1 m/s is halting (`--waiting-time`
    /// and the `halting` counts of SUMO's detectors; SUMO documentation, "Simulation/
    /// Output"). Cited on the provider's card.
    const fn default_stop_speed() -> f64 {
        0.1
    }

    /// Checks the section: both models exist, the EV overrides are the EV model's
    /// parameters, every share is a fraction and the stop speed is positive.
    ///
    /// # Errors
    /// A message naming the field.
    pub fn validate(&self) -> Result<(), String> {
        registry::emission_model(&self.emission_model)
            .map_err(|e| format!("energy.emission_model: {e}"))?;
        for (seg, m) in &self.emission_model_by_segment {
            registry::emission_model(m).map_err(|e| {
                format!("energy.emission_model_by_segment.{}: {e}", seg.as_str())
            })?;
        }
        registry::ev_model(&self.ev_model, &self.ev.params)
            .map_err(|e| format!("energy.ev: {e}"))?;
        self.fleet.check()?;
        if !(0.0..=1.0).contains(&self.ev.initial_soc) {
            return Err(format!(
                "energy.ev.initial_soc must be in [0, 1], got {}",
                self.ev.initial_soc
            ));
        }
        if !(self.stop_speed_mps > 0.0 && self.stop_speed_mps <= 5.0) {
            return Err(format!(
                "energy.stop_speed_mps must be in (0, 5] m/s, got {}",
                self.stop_speed_mps
            ));
        }
        Ok(())
    }
}

impl Default for EnergySpec {
    fn default() -> Self {
        Self {
            emission_model: Self::default_emission(),
            emission_model_by_segment: BTreeMap::new(),
            ev_model: Self::default_ev(),
            fleet: BTreeMap::new(),
            ev: EvSpec::default(),
            stop_speed_mps: Self::default_stop_speed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_section_is_the_default_and_validates() {
        let s: EnergySpec = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(s, EnergySpec::default());
        assert!(s.validate().is_ok());
    }

    #[test]
    fn short_names_unknown_models_and_unknown_params_are_handled() {
        let s: EnergySpec = serde_json::from_value(serde_json::json!({
            "emission_model": "phemlight", "ev_model": "vt-cpem",
            "ev": {"params": {"mass_kg": 1600.0}}
        }))
        .unwrap();
        assert!(s.validate().is_ok());
        let mut bad = s.clone();
        bad.emission_model = "copert".to_string();
        assert!(bad.validate().unwrap_err().contains("emission_model"));
        let mut bad = s;
        bad.ev.params.insert("wings".to_string(), 2.0);
        assert!(bad.validate().unwrap_err().contains("wings"));
        assert!(
            serde_json::from_value::<EnergySpec>(serde_json::json!({"fleet": {"tank": {}}}))
                .is_err()
        );
    }
}
