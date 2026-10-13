//! Lane-change models — 04-models.md §2.2.
//!
//! `mobility/lane-change/mobil` ([`mobil`]) is the native medium tier. [`lc2013`] ports
//! SUMO LC2013's strategic look-ahead, cooperative, speed-gain and keep-right motivations;
//! the engine uses it in place of MOBIL's discretionary decision when
//! `EngineParams::lc2013` is set (MOBIL still supplies the safety criterion).

pub mod lc2013;
pub mod mobil;

pub use mobil::{Mobil, MobilParams, MobilPreset, smoothstep, transition_duration};
