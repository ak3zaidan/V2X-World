//! `v2xw` — the command-line control surface.
//!
//! Five commands, and no model logic in any of them (02-architecture.md §2, ADR 0010):
//!
//! | Command | What it does |
//! |---|---|
//! | [`run`] | runs a scenario and writes its recording, metrics and manifest |
//! | [`validate`] | loads and checks a scenario without building its world |
//! | [`import`] | imports an OpenStreetMap extract into the three world formats |
//! | [`info`] | prints a recording's manifest, channels and verification report |
//! | [`experiment`] | expands a scenario's sweep, runs it, aggregates it, resumes it |
//! | [`sigcompare`] | `v2xw compare-signatures`: one scenario under several signature schemes, side by side |
//! | [`agent`] | `v2xw agent "<prompt>"`: the agent harness end to end; `v2xw serve`: the engine with the agent's endpoints |
//!
//! Everything the tool does is a call into a library crate. The one thing it adds is the
//! clock: [`v2xw_engine::Engine::build`] takes its manifest timestamp as an argument and
//! [`v2xw_world::ImportOptions`] takes its import date as one, precisely so that no engine
//! code reads one, and [`wall`] is where this crate supplies them. No other module here
//! may read a clock, and that rule is checkable by reading one file.
//!
//! Every command is a plain function over an options struct that returns a serialisable
//! outcome, and [`fmt`] renders one. `--json` prints the same struct the function returned,
//! which is why the tests assert on the struct rather than on parsed output.

#![deny(missing_docs)]
// `deny` rather than `forbid`: `progress::phys_footprint_bytes` reads the memory footprint
// through one `proc_pid_rusage` call, allowed there by name and nowhere else.
#![deny(unsafe_code)]

pub mod agent;
pub mod agent_host;
pub mod check_traffic;
pub mod checkpoint;
pub mod cli;
pub mod energy;
pub mod error;
pub mod experiment;
pub mod export_sumo;
pub mod fmt;
pub mod import;
pub mod info;
pub mod progress;
pub mod run;
pub mod sigcompare;
pub mod signal;
pub mod stream;
pub mod tally;
pub mod validate;
pub mod wall;

pub use error::{CliError, Result};
