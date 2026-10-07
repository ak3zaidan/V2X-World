//! `v2xw-energy` — what the traffic burns and emits: tailpipe emission and fuel models,
//! battery-electric energy models, and the metrics and health bands over them.
//!
//! # Where to look
//!
//! | Concern | Module |
//! |---|---|
//! | The two plug-in seams, and how to add a model | [`traits`] |
//! | What a model is given (one interval) and returns | [`state`] |
//! | Segment, fuel, Euro class; the keyed fleet draw | [`powertrain`] |
//! | HBEFA 3.1 polynomial fit (Krajzewicz et al. 2015; SUMO's `HBEFA3`) | [`hbefa3`] |
//! | PHEMlight power-based model (Hausberger et al.; SUMO's `PHEMlight`) | [`phemlight`] |
//! | US EPA MOVES operating modes with MOVESTAR rates | [`moves`] |
//! | EV energy: Kurczveil et al. 2014, VT-CPEM (Fiori et al. 2016); the battery | [`ev`] |
//! | Model selection by id | [`registry`] |
//! | The scenario's `energy:` section | [`spec`] |
//! | The metric provider over `gt.kinematics` | [`provider`] |
//! | Health bands | [`bands`] |
//!
//! # What this adds beyond SUMO's emission device
//!
//! SUMO computes HBEFA, PHEMlight and its battery model inside the simulation loop. This
//! crate computes the same published models from the recorded ground-truth stream, which
//! buys three things: a finished recording can be re-scored under another model or fleet
//! without re-running it; the result is bit-identical across machines and thread counts
//! (keyed fleet draw, `v2xw_core::math`, actor-ordered sums); and every number carries a
//! model card with its paper, its coefficients' source, what it ignores and a band it is
//! checked against. It also adds the MOVES operating-mode method, which SUMO does not ship,
//! and VT-CPEM's deceleration-dependent regeneration as a selectable model rather than a
//! single parameter.
//!
//! # Determinism
//!
//! Models are pure functions of the interval; every transcendental goes through
//! [`v2xw_core::math`]; the only random draw is each vehicle's powertrain, from a stream
//! keyed by its actor id; reductions run over `BTreeMap`s in key order.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod bands;
pub mod cite;
pub mod ev;
pub mod hbefa3;
pub mod moves;
pub mod phemlight;
pub mod powertrain;
pub mod provider;
pub mod registry;
pub mod spec;
pub mod state;
pub mod traits;

pub use bands::{Band, Health, Verdict, assess};
pub use powertrain::{FleetExt, FleetSpec, Fuel, Powertrain, Segment, SegmentMix};
pub use provider::{EnergyProvider, Totals, VehicleEnergy};
pub use spec::{EnergySpec, EvSpec};
pub use state::{DriveState, Rates};
pub use traits::{EmissionClass, EmissionModel, EvEnergyModel};

/// Every model card this crate registers, sorted by id: the five selectable models and the
/// provider (with the default section's models).
#[must_use]
pub fn model_cards() -> Vec<v2xw_core::card::ModelCard> {
    let spec = EnergySpec::default();
    let mut cards = vec![
        hbefa3::card(),
        phemlight::card(),
        moves::card(),
        ev::kurczveil_card(),
        ev::vt_cpem_card(),
        provider::card(&spec, hbefa3::MODEL_ID, ev::KURCZVEIL_ID),
    ];
    cards.sort_by(|a, b| a.id.cmp(&b.id));
    cards
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_card_validates_and_every_uncited_default_has_a_plan() {
        let cards = super::model_cards();
        assert_eq!(cards.len(), 6);
        for c in &cards {
            c.validate().unwrap_or_else(|e| panic!("{}: {e}", c.id));
            assert!(!c.sources.is_empty(), "{} cites nothing", c.id);
            assert!(!c.ignores.is_empty(), "{} ignores nothing?", c.id);
        }
    }
}
