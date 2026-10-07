//! Trips: the unit every demand source here produces and every assignment routes.
//!
//! A trip is a departure instant, an origin lane, a destination lane and a class — what
//! SUMO calls a `<trip>` [SUMO `od2trips` and `duarouter` documentation] — plus the purpose
//! that made it, when one did. A [`TripSet`] is serialised as `v2xw/trips/1` JSON, which
//! is what `v2xw-demand od2trips` and `v2xw-demand purposes` write and what the
//! `mobility/demand/trips` scenario kind reads.

use serde::{Deserialize, Serialize};
use v2xw_core::ids::LaneId;
use v2xw_mobility::VehicleClass;

use crate::error::{DemandError, Result};
use crate::purpose::Purpose;

/// The schema tag of a trip file.
pub const TRIPS_SCHEMA: &str = "v2xw/trips/1";

/// One trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trip {
    /// A stable id, unique within its set.
    pub id: u64,
    /// Departure, seconds from the run start.
    pub depart_s: f64,
    /// The lane it starts on.
    pub origin: LaneId,
    /// The lane it ends on.
    pub destination: LaneId,
    /// The vehicle class.
    #[serde(default)]
    pub class: VehicleClass,
    /// Why it is made, when the source knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<Purpose>,
    /// The OD-matrix cell or count interval it came from, for the reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

/// A set of trips, ordered by `(depart_s, id)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TripSet {
    /// [`TRIPS_SCHEMA`].
    pub schema: String,
    /// The world the lane ids refer to: its content hash, hex.
    pub world_hash: String,
    /// What made the set (model id and parameters), for provenance.
    #[serde(default)]
    pub source: serde_json::Value,
    /// The trips.
    pub trips: Vec<Trip>,
}

impl TripSet {
    /// A set over `world`, sorted.
    pub fn new(world_hash: String, source: serde_json::Value, mut trips: Vec<Trip>) -> Self {
        sort_trips(&mut trips);
        Self {
            schema: TRIPS_SCHEMA.to_string(),
            world_hash,
            source,
            trips,
        }
    }

    /// Reads a trip file.
    ///
    /// # Errors
    /// I/O and parse errors, and a schema tag that is not [`TRIPS_SCHEMA`].
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| DemandError::io(path, e))?;
        let mut set: TripSet =
            serde_json::from_str(&text).map_err(|e| DemandError::parse(path, e.to_string()))?;
        if set.schema != TRIPS_SCHEMA {
            return Err(DemandError::parse(
                path,
                format!("schema '{}' is not {TRIPS_SCHEMA}", set.schema),
            ));
        }
        sort_trips(&mut set.trips);
        Ok(set)
    }

    /// Writes the set as pretty JSON.
    ///
    /// # Errors
    /// I/O errors.
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let text = serde_json::to_string(self).map_err(|e| DemandError::parse(path, e.to_string()))?;
        std::fs::write(path, text).map_err(|e| DemandError::io(path, e))
    }

    /// Checks the set was made for `world`.
    ///
    /// # Errors
    /// [`DemandError::WorldMismatch`] when the hashes differ.
    pub fn check_world(&self, world: &v2xw_world::World) -> Result<()> {
        check_world_hash(&self.world_hash, world)
    }
}

/// Sorts trips into stream order: departure, then id.
pub fn sort_trips(trips: &mut [Trip]) {
    trips.sort_by(|a, b| a.depart_s.total_cmp(&b.depart_s).then(a.id.cmp(&b.id)));
}

/// The world's content hash, hex: what route and trip files record.
pub fn world_hash(world: &v2xw_world::World) -> String {
    world.content_hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// Checks a recorded world hash against `world`. An empty recorded hash is accepted (a
/// hand-written file).
///
/// # Errors
/// [`DemandError::WorldMismatch`] when the hashes differ.
pub fn check_world_hash(recorded: &str, world: &v2xw_world::World) -> Result<()> {
    let actual = world_hash(world);
    if recorded.is_empty() || recorded == actual {
        Ok(())
    } else {
        Err(DemandError::WorldMismatch {
            recorded: recorded.to_string(),
            actual,
        })
    }
}
