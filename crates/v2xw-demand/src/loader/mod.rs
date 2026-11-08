//! Network loading: drive the chosen routes and measure what they cost.
//!
//! An assignment alternates route choice with a loading of the network by the chosen
//! routes; the loading's measured, time-dependent travel times are what the next round of
//! choice responds to. Three loaders, from fastest to most faithful:
//!
//! * [`bpr`] — `demand/loader/bpr`: static link performance, `t = t₀ (1 + α (v/c)^β)`
//!   [Bureau of Public Roads, *Traffic Assignment Manual*, 1964]. One interval, no queues;
//!   the classical static assignment, for sketch planning and for checking the algorithm
//!   against Wardrop's conditions.
//! * [`queue`] — `demand/loader/queue`: an event-driven point-queue network with signal
//!   gating [Vickrey 1969; Gawron 1998's queue model without spill-back]. Seconds for a
//!   city-hour; the default.
//! * [`micro`] — `demand/loader/micro`: the engine itself ([`v2xw_mobility::NativeMobility`])
//!   driving the routes, as SUMO's `duaIterate` runs `sumo` each iteration. The most
//!   faithful and by far the slowest.
//!
//! # How to add another
//!
//! Implement [`NetworkLoader`] in a new file here (a cell-transmission or link-transmission
//! model is the obvious next one), give it a model card citing its paper, add a variant to
//! [`LoaderSpec`] and a line in [`LoaderSpec::build`]. It is then selectable as
//! `params.assign.loader.model` in a scenario and with `v2xw-demand assign --loader`.

pub mod bpr;
pub mod micro;
pub mod queue;

use serde::{Deserialize, Serialize};
use v2xw_core::card::ModelCard;
use v2xw_world::World;

use crate::costs::TravelTimes;
use crate::error::Result;
use crate::graph::{LaneGraph, Node};
use crate::trips::Trip;

/// The time window a loading measures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    /// Start, seconds from the run start.
    pub begin_s: f64,
    /// End, seconds; trips still driving then are counted as not arrived.
    pub end_s: f64,
    /// The aggregation interval of the travel-time table, seconds.
    pub bin_s: f64,
}

/// What a loading measured.
#[derive(Debug, Clone)]
pub struct Loading {
    /// Time-dependent lane travel times.
    pub times: TravelTimes,
    /// Each trip's experienced travel time, seconds; `NaN` if it did not arrive.
    pub experienced_s: Vec<f64>,
    /// How many trips arrived.
    pub arrived: usize,
}

/// A network loader.
pub trait NetworkLoader {
    /// Its card.
    fn card(&self) -> &ModelCard;

    /// Loads the network with `trips[i]` driving `routes[i]`.
    ///
    /// # Errors
    /// A loader that cannot run on this world says why.
    fn load(
        &mut self,
        world: &World,
        graph: &LaneGraph,
        trips: &[Trip],
        routes: &[Vec<Node>],
        window: Window,
    ) -> Result<Loading>;
}

/// The loader a scenario or the CLI names.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "kebab-case", deny_unknown_fields)]
pub enum LoaderSpec {
    /// [`bpr::Bpr`].
    Bpr {
        /// Saturation flow per lane, veh/h.
        #[serde(default = "queue::default_saturation")]
        saturation_veh_per_h: f64,
        /// BPR α.
        #[serde(default = "bpr::default_alpha")]
        alpha: f64,
        /// BPR β.
        #[serde(default = "bpr::default_beta")]
        beta: f64,
    },
    /// [`queue::PointQueue`].
    Queue {
        /// Saturation flow per lane, veh/h.
        #[serde(default = "queue::default_saturation")]
        saturation_veh_per_h: f64,
    },
    /// [`micro::Micro`].
    Micro {
        /// The engine's step, milliseconds.
        #[serde(default = "micro::default_step_ms")]
        step_ms: u64,
        /// The master seed of the engine run.
        #[serde(default)]
        seed: u64,
    },
}

impl Default for LoaderSpec {
    fn default() -> Self {
        LoaderSpec::Queue {
            saturation_veh_per_h: queue::default_saturation(),
        }
    }
}

impl LoaderSpec {
    /// The loader.
    pub fn build(self) -> Box<dyn NetworkLoader> {
        match self {
            LoaderSpec::Bpr {
                saturation_veh_per_h,
                alpha,
                beta,
            } => Box::new(bpr::Bpr::new(saturation_veh_per_h, alpha, beta)),
            LoaderSpec::Queue {
                saturation_veh_per_h,
            } => Box::new(queue::PointQueue::new(saturation_veh_per_h)),
            LoaderSpec::Micro { step_ms, seed } => Box::new(micro::Micro::new(step_ms, seed)),
        }
    }

    /// The loader's label.
    pub fn label(&self) -> &'static str {
        match self {
            LoaderSpec::Bpr { .. } => "bpr",
            LoaderSpec::Queue { .. } => "queue",
            LoaderSpec::Micro { .. } => "micro",
        }
    }
}
