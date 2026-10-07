//! Car-following models — 04-models.md §2.1, and the driving-model library.
//!
//! | Model | Id | Module | Reads V2X |
//! |---|---|---|---|
//! | Intelligent Driver Model (the engine's default) | `mobility/car-following/idm` | [`idm`] | no |
//! | Extended IDM (improved IDM + CAH + jerk bound) | `mobility/car-following/eidm` | [`eidm`] | no |
//! | Krauß (SUMO's default) | `mobility/car-following/krauss` | [`krauss`] | no |
//! | Gipps (Aimsun's) | `mobility/car-following/gipps` | [`gipps`] | no |
//! | Wiedemann 99 (Vissim's) | `mobility/car-following/w99` | [`w99`] | no |
//! | ACC (PATH, Milanés & Shladover 2014) | `mobility/car-following/acc` | [`acc`] | no |
//! | CACC (Milanés & Shladover 2014) | `mobility/car-following/cacc-milanes` | [`cacc`] | predecessor |
//! | CACC (Ploeg et al. 2011) | `mobility/car-following/cacc-ploeg` | [`cacc`] | predecessor |
//! | Platoon (PATH, Rajamani 2012) | `mobility/car-following/cacc-path` | [`cacc`] | predecessor + leader |
//!
//! [`library`] selects them per class from the scenario; [`platoon`] measures how the
//! cooperative ones respond to packet loss and latency. `docs/models/driving-model-library.md`
//! is the guide to adding another.
//!
//! # History
//!
//! Until the library, §2.1's Krauss, Wiedemann 99 and Gipps were left out because their
//! constants were not in the research sheets. They are in now, each with its source on
//! its card and the parts that are secondary-sourced marked so; the §2.1 calibration plans
//! still apply to every one of them.

pub mod acc;
pub mod cacc;
pub mod common;
pub mod eidm;
pub mod gipps;
pub mod idm;
pub mod krauss;
pub mod library;
pub mod platoon;
pub mod w99;

#[cfg(test)]
pub(crate) mod testkit;

pub use idm::{Idm, IdmParams, IdmPreset};
