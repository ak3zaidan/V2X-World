//! The traffic validation suite.
//!
//! Every check here holds one part of the traffic model to something outside the code: the
//! equation and the figures of the paper the model comes from, a published field
//! measurement with a stated tolerance, a safety-surrogate distribution, or an invariant
//! that no real road breaks (no car enters on red, a closure never makes a car jump). Each
//! check is run twice or more: once as shipped, where its tested rows must sit inside their
//! bands, and once with a **fault injected** — a broken model, a broken rule, a broken
//! observation — where at least one tested row must leave its band. A row no fault ever
//! turned red is reported as *not proved*: a check that cannot fail is not a check.
//!
//! `cargo run -p v2xw-trafficval --bin traffic_validation` runs everything and writes
//! `docs/validation/traffic.md`: every check, its reference, its tolerance, the result and
//! the fault that proved it can fail.
//!
//! # Groups
//!
//! | Group | Module | What |
//! |---|---|---|
//! | model | [`model`] | IDM, MOBIL, gap acceptance, social force against their papers; the ring fundamental diagram; string stability; the Sugiyama ring |
//! | calibration | [`calib`] | saturation flow, start-up lost time, speeds and accelerations, turning, critical gaps, pedestrians, motorcycles against published measurements |
//! | safety | [`safety`] | time-to-collision and post-encroachment time, SSAM thresholds, conflicts per vehicle-kilometre |
//! | invariants | [`invariants`] | the auditor's classes at zero on every shipped scenario and at peak density; metamorphic properties |
//! | cities | [`cities`] | Manhattan, Portland and Berlin import and run with the auditor clean |
//!
//! # Determinism
//!
//! Every run is a pure function of its seed: the engine's keyed RNG streams, no wall clock
//! in what is measured. Wall-clock time is printed to the terminal for the operator and is
//! never written into the report, so two runs of the suite write the same file.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::any::Any;
use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

pub mod calib;
pub mod cities;
pub mod invariants;
pub mod model;
pub mod report;
pub mod run;
pub mod safety;
pub mod study;

/// The check groups, in report order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Group {
    /// The models against their papers.
    Model,
    /// The traffic against published field measurements.
    Calibration,
    /// Safety surrogates.
    Safety,
    /// Invariants and metamorphic properties.
    Invariants,
    /// Real cities.
    Cities,
}

impl Group {
    /// Every group, in report order.
    pub const ALL: [Group; 5] = [
        Group::Model,
        Group::Calibration,
        Group::Safety,
        Group::Invariants,
        Group::Cities,
    ];

    /// A stable label.
    pub const fn label(self) -> &'static str {
        match self {
            Group::Model => "model",
            Group::Calibration => "calibration",
            Group::Safety => "safety",
            Group::Invariants => "invariants",
            Group::Cities => "cities",
        }
    }

    /// The heading the report gives the group.
    pub const fn title(self) -> &'static str {
        match self {
            Group::Model => "1. The models against their papers",
            Group::Calibration => "2. Calibration against published measurements",
            Group::Safety => "3. Safety surrogates",
            Group::Invariants => "4. Invariants and metamorphic properties",
            Group::Cities => "5. Three real cities",
        }
    }
}

/// One measured figure of a check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// What was measured, with its unit.
    pub metric: String,
    /// The value.
    pub measured: f64,
    /// How many samples it rests on, where that means something.
    pub samples: Option<usize>,
    /// The published value or the rule, with its source.
    pub reference: String,
    /// The tolerance band, inclusive. `None` for a figure that is reported, not held.
    pub band: Option<(f64, f64)>,
}

impl Row {
    /// A held figure.
    pub fn held(metric: impl Into<String>, measured: f64, band: (f64, f64), reference: impl Into<String>) -> Self {
        Self {
            metric: metric.into(),
            measured,
            samples: None,
            reference: reference.into(),
            band: Some(band),
        }
    }

    /// A count that must be zero.
    pub fn zero(metric: impl Into<String>, count: u64, reference: impl Into<String>) -> Self {
        Self::held(metric, count as f64, (0.0, 0.0), reference)
    }

    /// A figure reported beside its reference but not held to a band.
    pub fn reported(metric: impl Into<String>, measured: f64, reference: impl Into<String>) -> Self {
        Self {
            metric: metric.into(),
            measured,
            samples: None,
            reference: reference.into(),
            band: None,
        }
    }

    /// With a sample count.
    pub fn n(mut self, samples: usize) -> Self {
        self.samples = Some(samples);
        self
    }

    /// Whether the figure is held to a band.
    pub fn tested(&self) -> bool {
        self.band.is_some()
    }

    /// Whether it sits inside its band (always true for a reported figure). A NaN never
    /// passes.
    pub fn passes(&self) -> bool {
        match self.band {
            None => true,
            Some((lo, hi)) => self.measured >= lo && self.measured <= hi,
        }
    }
}

/// What one run of a check produced.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Outcome {
    /// The figures.
    pub rows: Vec<Row>,
    /// Anything the reader needs to read them: sample sizes, examples of a violation.
    pub notes: Vec<String>,
}

impl Outcome {
    /// Whether every held figure is in its band.
    pub fn passes(&self) -> bool {
        self.rows.iter().all(Row::passes)
    }

    /// The row called `metric`.
    pub fn row(&self, metric: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.metric == metric)
    }
}

/// Which variant of a check to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Variant {
    /// The model as shipped.
    Shipped,
    /// The model with fault `k` of the check's list injected.
    Fault(usize),
}

impl Variant {
    /// True for a fault run.
    pub fn is_fault(self) -> bool {
        matches!(self, Variant::Fault(_))
    }
}

/// The suite's configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// The workspace root: scenarios and city extracts are found from here.
    pub root: PathBuf,
    /// Shorter runs and the grid scenarios only (no Manhattan-scale imports).
    pub quick: bool,
}

impl Config {
    /// The configuration for the workspace this crate was built in.
    pub fn new(quick: bool) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from("."));
        Self { root, quick }
    }

    /// Seconds of simulated time for a study: `full` normally, `quick` in quick mode.
    pub fn secs(&self, full: u64, quick: u64) -> u64 {
        if self.quick { quick } else { full }
    }
}

/// Where the checks run: the configuration and the studies already run, so two checks
/// reading the same simulation do not run it twice.
pub struct Lab {
    /// The configuration.
    pub cfg: Config,
    studies: BTreeMap<String, Box<dyn Any>>,
}

impl Lab {
    /// An empty lab.
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg,
            studies: BTreeMap::new(),
        }
    }

    /// The study called `key`, if it has run.
    pub fn get<T: Clone + 'static>(&self, key: &str) -> Option<T> {
        self.studies.get(key).and_then(|b| b.downcast_ref::<T>()).cloned()
    }

    /// The study called `key`, run with `f` the first time it is asked for.
    ///
    /// # Errors
    /// Whatever `f` returns; a failed study is not cached.
    pub fn study<T: Clone + 'static>(
        &mut self,
        key: &str,
        f: impl FnOnce(&Config) -> Result<T, String>,
    ) -> Result<T, String> {
        if let Some(v) = self.studies.get(key).and_then(|b| b.downcast_ref::<T>()) {
            return Ok(v.clone());
        }
        let started = std::time::Instant::now();
        eprintln!("  study {key} ...");
        let v = f(&self.cfg)?;
        eprintln!("  study {key} done in {:.1} s (wall clock, not reported)", started.elapsed().as_secs_f64());
        self.studies.insert(key.to_string(), Box::new(v.clone()));
        Ok(v)
    }
}

/// A check: what it holds, against what, and how to run it.
pub struct Check {
    /// A stable id, `group/name`.
    pub id: &'static str,
    /// Its group.
    pub group: Group,
    /// One line: what it proves.
    pub title: &'static str,
    /// How it is measured.
    pub procedure: &'static str,
    /// The faults that prove it can fail, one line each.
    pub faults: &'static [&'static str],
    /// The runner.
    pub run: fn(&mut Lab, Variant) -> Result<Outcome, String>,
}

/// Every check of the suite, in report order.
pub fn checks() -> Vec<Check> {
    let mut out = Vec::new();
    out.extend(model::checks());
    out.extend(calib::checks());
    out.extend(safety::checks());
    out.extend(invariants::checks());
    out.extend(cities::checks());
    out
}
