//! Dwell time at a stop.
//!
//! * `scheduled` — the timetable's own dwell, departure − arrival, with a floor of the
//!   door cycle (a timetable that gives one instant for both still opens the doors).
//! * `tcqsm` — the Transit Capacity and Quality of Service Manual's dwell for a single
//!   loading door, `t_d = P_b t_b + P_a t_a + t_oc`, with boardings `P_b` and alightings
//!   `P_a` drawn from Poisson distributions [TCRP Report 165, *Transit Capacity and Quality
//!   of Service Manual*, 3rd ed., Transportation Research Board 2013, ch. 6]. The manual's
//!   ranges are 2–5 s for the door opening and closing time `t_oc`, roughly 2.5–4 s per
//!   boarding by fare medium and 1.5–3 s per alighting; the defaults (3.5 s, 2.5 s, 4 s)
//!   sit inside them. The mean passenger counts per stop are a study input with no
//!   general value and are marked `todo-calibrate`.
//!
//! # How to add another
//!
//! Add a variant to [`DwellSpec`] and its arm in [`DwellSpec::dwell_s`] (a passenger-load
//! model fed by automatic passenger counts is the obvious next one), and cite it.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{Parameter, Source, SourceKind};
use v2xw_core::rng::RngStream;

/// The dwell model.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DwellSpec {
    /// The timetable's dwell, floored at `min_s`.
    Scheduled {
        /// The floor, seconds.
        #[serde(default = "default_door_s")]
        min_s: f64,
    },
    /// TCQSM single-door dwell with Poisson passengers.
    Tcqsm {
        /// Mean boardings per stop.
        #[serde(default = "default_boardings")]
        mean_boardings: f64,
        /// Mean alightings per stop.
        #[serde(default = "default_alightings")]
        mean_alightings: f64,
        /// Seconds per boarding.
        #[serde(default = "default_tb")]
        boarding_s: f64,
        /// Seconds per alighting.
        #[serde(default = "default_ta")]
        alighting_s: f64,
        /// Door opening and closing, seconds.
        #[serde(default = "default_door_s")]
        door_s: f64,
    },
}

fn default_door_s() -> f64 {
    4.0
}
fn default_boardings() -> f64 {
    2.0
}
fn default_alightings() -> f64 {
    2.0
}
fn default_tb() -> f64 {
    3.5
}
fn default_ta() -> f64 {
    2.5
}

impl Default for DwellSpec {
    fn default() -> Self {
        DwellSpec::Scheduled {
            min_s: default_door_s(),
        }
    }
}

/// A Poisson draw by inversion (exact; fine for the small means of passengers per stop).
pub fn poisson(rng: &mut RngStream, mean: f64) -> u32 {
    if !(mean > 0.0) {
        return 0;
    }
    let limit = v2xw_core::math::exp(-mean.min(500.0));
    let mut p = 1.0;
    let mut k = 0u32;
    loop {
        p *= rng.f64();
        if p <= limit || k > 10_000 {
            return k;
        }
        k += 1;
    }
}

impl DwellSpec {
    /// The dwell at a call whose timetable gives `scheduled_s` (departure − arrival).
    pub fn dwell_s(&self, scheduled_s: f64, rng: &mut RngStream) -> f64 {
        match *self {
            DwellSpec::Scheduled { min_s } => scheduled_s.max(min_s),
            DwellSpec::Tcqsm {
                mean_boardings,
                mean_alightings,
                boarding_s,
                alighting_s,
                door_s,
            } => {
                let b = poisson(rng, mean_boardings);
                let a = poisson(rng, mean_alightings);
                f64::from(b) * boarding_s + f64::from(a) * alighting_s + door_s
            }
        }
    }

    /// The card parameters.
    pub fn parameters(&self) -> Vec<Parameter> {
        let tcqsm = Source::new(
            SourceKind::Standard,
            "TCRP Report 165, Transit Capacity and Quality of Service Manual, 3rd ed., TRB \
             2013, ch. 6 (dwell time: door 2-5 s, boarding and alighting times by fare medium \
             and door use)",
        );
        match *self {
            DwellSpec::Scheduled { min_s } => vec![Parameter::new(
                "dwell_min_s",
                "s",
                serde_json::json!(min_s),
                tcqsm,
            )],
            DwellSpec::Tcqsm {
                mean_boardings,
                mean_alightings,
                boarding_s,
                alighting_s,
                door_s,
            } => {
                let mut pax = Parameter::new(
                    "mean_passengers",
                    "pax/stop",
                    serde_json::json!([mean_boardings, mean_alightings]),
                    Source::todo_calibrate("boardings and alightings per stop"),
                );
                pax.calibration = Some(
                    "the agency's automatic passenger counter (APC) data by stop and hour"
                        .to_string(),
                );
                vec![
                    pax,
                    Parameter::new("boarding_s", "s", serde_json::json!(boarding_s), tcqsm.clone()),
                    Parameter::new("alighting_s", "s", serde_json::json!(alighting_s), tcqsm.clone()),
                    Parameter::new("door_s", "s", serde_json::json!(door_s), tcqsm),
                ]
            }
        }
    }
}
