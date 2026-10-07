//! Demand and route choice for the V2X World Simulator.
//!
//! Where the traffic comes from and which way it goes: the layer SUMO covers with
//! `od2trips`, `duarouter`/`duaIterate`, `routeSampler`, `gtfs2pt` and `activitygen`, here
//! in one crate, deterministic to the bit, and plugged into the engine through the
//! [`v2xw_mobility::traits::Demand`] trait.
//!
//! | Module | What | Model id |
//! |---|---|---|
//! | [`assign`] | iterative dynamic traffic assignment to a stochastic user equilibrium, convergence by relative gap | `demand/assign/iterative-due` |
//! | [`choice`] | route choice: Gawron, C-logit, MSA | `demand/choice/*` |
//! | [`loader`] | network loading: BPR, point queue with signals, the microscopic engine | `demand/loader/*` |
//! | [`routefile`] | saved routes with alternatives (`v2xw/routes/1`), reused by runs | `mobility/demand/routes` |
//! | [`od`] | OD matrices (VISUM O/V, CSV), zones, gravity–Furness, matrix to trips | `mobility/demand/od-matrix` |
//! | [`counts`] | routes and trips from link counts, GEH check | `mobility/demand/counts` |
//! | [`transit`] | GTFS buses and street-level rail with stops and dwell | `mobility/demand/gtfs` |
//! | [`purpose`] | trip purposes, tours, departure-time profiles, multi-day | `mobility/demand/purposes` |
//! | [`models`] | trip lists and the composite of several demands | `mobility/demand/composite` |
//! | [`scenario`] | the scenario kinds and their parameters | |
//!
//! Every model carries a model card with its sources; every number that is not from a
//! source is marked `todo-calibrate` with a calibration plan.
//!
//! # Determinism
//!
//! All draws come from keyed streams ([`v2xw_core::rng::RngStream::derive`] with each
//! model's plug-in domain, keyed by the entity — trip, OD cell, count interval, tour, day —
//! and, in the assignment, the iteration); searches break ties by lane id; reductions are
//! id-ordered; no wall clock is read. Route and trip files are byte-identical across runs
//! and machines.
//!
//! # How to add a model
//!
//! Each family has a trait and a spec enum: [`choice::RouteChoice`] / [`choice::ChoiceSpec`],
//! [`loader::NetworkLoader`] / [`loader::LoaderSpec`], [`transit::dwell::DwellSpec`],
//! [`od::gravity::Deterrence`], [`purpose::DepartureProfile`], and a whole new demand kind
//! in [`scenario`]. Each module's docs end with a "How to add another" note.

pub mod assign;
pub mod choice;
pub mod costs;
pub mod counts;
pub mod error;
pub mod graph;
pub mod loader;
pub mod models;
pub mod od;
pub mod purpose;
pub mod routefile;
pub mod scenario;
pub mod tdsp;
pub mod transit;
pub mod trips;

pub use error::{DemandError, Result};

/// Every model card this crate defines at its default parameters, for the registry and
/// the documentation generator.
pub fn cards() -> Vec<v2xw_core::card::ModelCard> {
    use crate::choice::RouteChoice;
    vec![
        choice::gawron::Gawron::new(choice::gawron::default_beta(), choice::gawron::default_a())
            .card()
            .clone(),
        choice::logit::Logit::new(None, choice::logit::default_cv(), 1.0, 1.0)
            .card()
            .clone(),
        choice::msa::Msa::new().card().clone(),
        loader::LoaderSpec::Bpr {
            saturation_veh_per_h: loader::queue::default_saturation(),
            alpha: loader::bpr::default_alpha(),
            beta: loader::bpr::default_beta(),
        }
        .build()
        .card()
        .clone(),
        loader::LoaderSpec::default().build().card().clone(),
        loader::LoaderSpec::Micro {
            step_ms: loader::micro::default_step_ms(),
            seed: 0,
        }
        .build()
        .card()
        .clone(),
        od::gravity::card(&od::gravity::GravityParams::default()),
        counts::sampler::card(&counts::sampler::SamplerParams::default()),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_card_validates() {
        for c in super::cards() {
            c.validate().unwrap_or_else(|e| panic!("{}: {e}", c.id));
        }
    }
}
