//! The two plug-in seams of this crate.
//!
//! * [`EmissionModel`] — a combustion vehicle's tailpipe emissions and fuel, family
//!   [`Family::Emission`](v2xw_core::card::Family::Emission).
//! * [`EvEnergyModel`] — a battery-electric vehicle's traction-battery power, family
//!   [`Family::Energy`](v2xw_core::card::Family::Energy).
//!
//! Both extend [`Model`], so every implementation carries a card whose sources name the
//! published model and the published coefficients, and both are pure: same state in, same
//! number out, on every platform (every transcendental goes through `v2xw_core::math`), no
//! random draws, no wall clock. The state that *does* accumulate — the battery's charge,
//! the run's totals — is the provider's ([`crate::provider`]), so a model can be swapped
//! without touching it.
//!
//! # How to add another model
//!
//! 1. Write a module with a struct holding its [`ModelCard`](v2xw_core::card::ModelCard)
//!    and its parameters; build the card in a `card()` function that cites the paper for
//!    the equations and the paper or dataset for every coefficient (`Source::new(Paper,
//!    "…")`), or marks an uncited value `todo-calibrate` with a calibration plan.
//! 2. Implement `Model` (return the card) and one of the two traits below.
//! 3. Add it to [`crate::registry`]'s two lists — the `id → constructor` match and
//!    [`crate::model_cards`] — and it is selectable as `energy.emission_model` or
//!    `energy.ev_model` in a scenario, listed in the scenario-editor's choice for that
//!    slot, and pinned in the run manifest.
//! 4. Add a reference-cycle test beside the others in `tests/models.rs`: run it over the
//!    EPA UDDS and HWFET cycles and check it lands in the cited band.

use v2xw_core::model::Model;

use crate::powertrain::Powertrain;
use crate::state::{DriveState, Rates};

/// The emission class a model resolved a powertrain to: an index into the model's own
/// class table, and the class's name as the model's source spells it (`PC_G_EU4`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EmissionClass {
    /// Index into the model's class table.
    pub index: usize,
    /// The class's name in the model's source.
    pub name: String,
}

/// Tailpipe emissions and fuel of a combustion-engined vehicle.
pub trait EmissionModel: Model {
    /// The class this model computes `p` with, or `None` when it has no class for it.
    ///
    /// A model may resolve to the nearest class it has (PHEMlight's published data has a
    /// Euro 4 passenger car only, so a Euro 6 car is computed as Euro 4) and must then say
    /// so on its card; `None` is the honest answer when nothing it has is near — the
    /// provider then counts the vehicle as unmodelled rather than inventing a number.
    /// Never called for an electric vehicle.
    fn resolve(&self, p: &Powertrain) -> Option<EmissionClass>;

    /// The rates over one interval, for a class [`EmissionModel::resolve`] returned.
    ///
    /// Must return [`Rates::is_physical`] rates for every finite state.
    fn rates(&self, class: &EmissionClass, s: &DriveState) -> Rates;
}

/// Traction-battery power of a battery-electric vehicle.
pub trait EvEnergyModel: Model {
    /// Mean power drawn from the battery over the interval, W: positive when it
    /// discharges, negative when regenerative braking charges it. Includes auxiliaries.
    fn battery_power_w(&self, s: &DriveState) -> f64;

    /// The usable battery capacity of the vehicle the parameters describe, Wh.
    fn capacity_wh(&self) -> f64;
}
