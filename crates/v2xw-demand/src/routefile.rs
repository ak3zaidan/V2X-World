//! Saved routes: `v2xw/routes/1`.
//!
//! What an assignment writes and a run reuses — the equivalent of SUMO's `.rou.alt.xml`
//! [SUMO `duarouter` documentation, route alternatives]: every trip with its known routes,
//! each route's cost and probability, and the one chosen. The file also records the world
//! it was made for (its content hash), the assignment's parameters and model versions,
//! and the iteration log, so a reviewer can see how far it converged.
//!
//! A route file is reused three ways: the `mobility/demand/routes` scenario kind runs it
//! ([`RouteFile::demand`]); `v2xw-demand assign --warm` continues it; and
//! `v2xw-demand check-counts` compares it with observed counts.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{Source, SourceKind};
use v2xw_core::ids::LaneId;

use crate::assign::IterationLog;
use crate::error::{DemandError, Result};
use crate::models::{TripListDemand, trip_list_card};
use crate::trips::{Trip, check_world_hash};

/// The schema tag.
pub const ROUTES_SCHEMA: &str = "v2xw/routes/1";

/// The scenario kind and model id of a saved-route demand.
pub const MODEL_ID: &str = "mobility/demand/routes";

/// One remembered route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedAlternative {
    /// Its lanes.
    pub lanes: Vec<LaneId>,
    /// Its cost when last measured, seconds.
    pub cost_s: f64,
    /// Its choice probability.
    pub probability: f64,
}

/// A trip with its routes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutedTrip {
    /// The trip.
    #[serde(flatten)]
    pub trip: Trip,
    /// The index of the route it drives.
    pub chosen: usize,
    /// Its known routes.
    pub alternatives: Vec<SavedAlternative>,
}

impl RoutedTrip {
    /// The chosen route's lanes.
    pub fn route(&self) -> Option<&[LaneId]> {
        self.alternatives
            .get(self.chosen)
            .map(|a| a.lanes.as_slice())
    }
}

/// A route file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteFile {
    /// [`ROUTES_SCHEMA`].
    pub schema: String,
    /// The world's content hash, hex.
    pub world_hash: String,
    /// What made it.
    #[serde(default)]
    pub source: serde_json::Value,
    /// The iteration log of the assignment, when one made it.
    #[serde(default)]
    pub log: Vec<IterationLog>,
    /// Whether the assignment converged.
    #[serde(default)]
    pub converged: bool,
    /// The trips.
    pub trips: Vec<RoutedTrip>,
}

impl RouteFile {
    /// A route file.
    pub fn new(
        world_hash: String,
        source: serde_json::Value,
        log: Vec<IterationLog>,
        converged: bool,
        mut trips: Vec<RoutedTrip>,
    ) -> Self {
        trips.sort_by(|a, b| {
            a.trip
                .depart_s
                .total_cmp(&b.trip.depart_s)
                .then(a.trip.id.cmp(&b.trip.id))
        });
        Self {
            schema: ROUTES_SCHEMA.to_string(),
            world_hash,
            source,
            log,
            converged,
            trips,
        }
    }

    /// Reads a route file.
    ///
    /// # Errors
    /// I/O, parse and schema errors.
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| DemandError::io(path, e))?;
        let f: RouteFile =
            serde_json::from_str(&text).map_err(|e| DemandError::parse(path, e.to_string()))?;
        if f.schema != ROUTES_SCHEMA {
            return Err(DemandError::parse(
                path,
                format!("schema '{}' is not {ROUTES_SCHEMA}", f.schema),
            ));
        }
        Ok(f)
    }

    /// Writes it as JSON.
    ///
    /// # Errors
    /// I/O errors.
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let text =
            serde_json::to_string(self).map_err(|e| DemandError::parse(path, e.to_string()))?;
        std::fs::write(path, text).map_err(|e| DemandError::io(path, e))
    }

    /// The demand model that runs these trips on their chosen routes.
    ///
    /// # Errors
    /// [`DemandError::WorldMismatch`] if the file was made for another world.
    pub fn demand(&self, world: &v2xw_world::World) -> Result<TripListDemand> {
        check_world_hash(&self.world_hash, world)?;
        let trips: Vec<Trip> = self.trips.iter().map(|r| r.trip.clone()).collect();
        let routes = self
            .trips
            .iter()
            .map(|r| r.route().map(<[LaneId]>::to_vec))
            .collect();
        let mut card = trip_list_card(
            MODEL_ID,
            "Trips on saved routes: the routes an assignment or a count sampler chose, \
             reused verbatim.",
            vec![Source::new(
                SourceKind::Code,
                "SUMO duarouter route-alternative files (.rou.alt.xml) as the reuse model",
            )],
        );
        card.assumptions.push(format!(
            "made by {} (converged: {})",
            self.source
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("an unknown source"),
            self.converged
        ));
        Ok(TripListDemand::new(trips, routes, card))
    }
}
