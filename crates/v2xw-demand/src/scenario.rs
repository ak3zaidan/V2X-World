//! The scenario kinds this crate provides, and how a scenario's `actors.vehicles.demand`
//! becomes one of them.
//!
//! | `kind` | `params` |
//! |---|---|
//! | `mobility/demand/routes` | `{ path }` — a `v2xw/routes/1` file (an assignment's or a count sampler's output) |
//! | `mobility/demand/trips` | `{ path, assign? }` — a `v2xw/trips/1` file, optionally assigned at load |
//! | `mobility/demand/od-matrix` | `{ matrix \| cells \| gravity, zones, od2trips?, assign? }` |
//! | `mobility/demand/counts` | `{ counts, sampler?, candidates? }` |
//! | `mobility/demand/gtfs` | [`crate::transit::TransitParams`] + `utc_offset_s` |
//! | `mobility/demand/purposes` | [`crate::purpose::PurposeParams`] + `utc_offset_s` |
//! | `mobility/demand/composite` | `{ parts: [{ kind, params, rate_veh_per_h? }], utc_offset_s? }` |
//!
//! File paths are read as given, relative to the working directory, as the scenario's
//! world `source.path` is. Times of day are local: `utc_offset_s` (e.g. `-28800` for
//! Pacific standard time) turns the scenario's UTC `t0` into the local start time that
//! timetables, matrices and departure profiles are written in. A part of a composite
//! inherits the composite's offset.
//!
//! # How to add another kind
//!
//! Write the demand model (anything implementing [`v2xw_mobility::traits::Demand`]), add
//! its id to [`KINDS`] and an arm to [`build`]; the engine's wiring forwards every kind in
//! [`KINDS`] here, so nothing in the engine changes.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{Source, SourceKind};
use v2xw_mobility::traits::Demand;
use v2xw_mobility::VehicleClass;
use v2xw_world::{ClassMask, World};

use crate::assign::{AssignParams, Assigner};
use crate::counts::sampler::{self, SamplerParams};
use crate::error::{DemandError, Result};
use crate::graph::LaneGraph;
use crate::models::{CompositeDemand, TripListDemand, trip_list_card};
use crate::od::gravity::{GravityParams, gravity};
use crate::od::zones::{ZoneSet, ZoneSpec};
use crate::od::{Od2TripsParams, OdMatrix, od2trips};
use crate::purpose::{PurposeDemand, PurposeModel, PurposeParams};
use crate::routefile::RouteFile;
use crate::transit::{TransitDemand, TransitParams};
use crate::trips::{Trip, TripSet};

/// Every scenario kind this crate builds.
pub const KINDS: [&str; 7] = [
    crate::routefile::MODEL_ID,
    "mobility/demand/trips",
    crate::od::MODEL_ID,
    sampler::MODEL_ID,
    crate::transit::MODEL_ID,
    crate::purpose::MODEL_ID,
    crate::models::COMPOSITE_ID,
];

/// What the scenario says that every kind may need.
#[derive(Clone)]
pub struct BuildContext<'w> {
    /// The world.
    pub world: &'w World,
    /// The scenario's master seed.
    pub seed: u64,
    /// The scenario's `time.t0`, RFC 3339 (UTC).
    pub t0: String,
    /// The run's length, seconds.
    pub duration_s: f64,
    /// The scenario's fleet shares (`actors.vehicles.classes`).
    pub fleet: Vec<(VehicleClass, f64)>,
    /// Builds a demand kind this crate does not own — the engine's built-in Poisson or
    /// 3GPP drop — as a composite part: `(kind, params, rate_veh_per_h)`.
    pub other: Option<&'w OtherKinds<'w>>,
}

/// The engine's builder for its own demand kinds, handed to a composite.
pub type OtherKinds<'a> =
    dyn Fn(&str, &serde_json::Value, Option<f64>) -> std::result::Result<Box<dyn Demand>, String> + 'a;

/// `(seconds of the day, YYYYMMDD)` of an RFC 3339 UTC instant shifted by `offset_s`.
///
/// # Errors
/// A `t0` that is not `YYYY-MM-DDTHH:MM:SS…`.
pub fn local_start(t0: &str, offset_s: f64) -> Result<(f64, u32)> {
    let bad = || DemandError::Invalid(format!("time.t0 '{t0}' is not YYYY-MM-DDTHH:MM:SSZ"));
    let num = |a: usize, b: usize| -> Result<i64> {
        t0.get(a..b).and_then(|s| s.parse().ok()).ok_or_else(bad)
    };
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hh, mm, ss) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let mut days = days_from_civil(y, m, d);
    let mut sod = (hh * 3600 + mm * 60 + ss) as f64 + offset_s;
    while sod < 0.0 {
        sod += 86_400.0;
        days -= 1;
    }
    while sod >= 86_400.0 {
        sod -= 86_400.0;
        days += 1;
    }
    let (y, m, d) = civil_from_days(days);
    Ok((sod, (y * 10_000 + m * 100 + d) as u32))
}

/// Days since 1970-01-01 of a civil date (H. Hinnant, "chrono-Compatible Low-Level Date
/// Algorithms").
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn take_offset(params: &mut serde_json::Value, inherited: f64) -> f64 {
    params
        .as_object_mut()
        .and_then(|m| m.remove("utc_offset_s"))
        .and_then(|v| v.as_f64())
        .unwrap_or(inherited)
}

fn de<T: for<'de> Deserialize<'de>>(kind: &str, v: serde_json::Value) -> Result<T> {
    serde_json::from_value(v)
        .map_err(|e| DemandError::Invalid(format!("{kind}: the parameters do not fit: {e}")))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PathParams {
    path: PathBuf,
    #[serde(default)]
    assign: Option<AssignParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cell {
    from: String,
    to: String,
    count: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OdParams {
    #[serde(default)]
    matrix: Option<PathBuf>,
    #[serde(default)]
    cells: Option<Vec<Cell>>,
    #[serde(default)]
    begin_s_of_day: Option<f64>,
    #[serde(default)]
    end_s_of_day: Option<f64>,
    #[serde(default)]
    gravity: Option<GravityParams>,
    #[serde(default)]
    zones: ZoneSpec,
    #[serde(default)]
    od2trips: Od2TripsParams,
    #[serde(default)]
    assign: Option<AssignParams>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CountParams {
    counts: PathBuf,
    #[serde(default)]
    sampler: SamplerParams,
    #[serde(default)]
    candidates: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Part {
    kind: String,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(default)]
    rate_veh_per_h: Option<f64>,
}

/// Builds the demand model a scenario names.
///
/// # Errors
/// [`DemandError`] naming what is wrong with the parameters or the files they name.
pub fn build(kind: &str, params: &serde_json::Value, ctx: &BuildContext) -> Result<Box<dyn Demand>> {
    build_with_offset(kind, params, ctx, 0.0)
}

fn routed(
    world: &World,
    trips: Vec<Trip>,
    assign: Option<AssignParams>,
    card_id: &str,
    purpose: &str,
    sources: Vec<Source>,
) -> Result<TripListDemand> {
    let mut card = trip_list_card(card_id, purpose, sources);
    let Some(p) = assign else {
        return Ok(TripListDemand::new(trips, Vec::new(), card));
    };
    crate::assign::validate(&p)?;
    let a = Assigner::new(world, p).run(&trips, None, |_| {})?;
    card.assumptions.push(format!(
        "routes from {} (converged: {}, final relative gap {:.4})",
        crate::assign::MODEL_ID,
        a.converged,
        a.routes.log.last().map_or(f64::NAN, |l| l.relative_gap)
    ));
    let routes: std::collections::BTreeMap<u64, Vec<v2xw_core::ids::LaneId>> = a
        .routes
        .trips
        .iter()
        .filter_map(|r| r.route().map(|l| (r.trip.id, l.to_vec())))
        .collect();
    // Unroutable trips stay in the demand without a route; the engine's router tries them.
    let (trips, routes) = crate::models::pair_routes(trips, &routes);
    Ok(TripListDemand::new(trips, routes, card))
}

fn build_with_offset(
    kind: &str,
    params: &serde_json::Value,
    ctx: &BuildContext,
    inherited_offset: f64,
) -> Result<Box<dyn Demand>> {
    let mut params = if params.is_null() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        params.clone()
    };
    let offset = take_offset(&mut params, inherited_offset);
    let (start_sod, date) = local_start(&ctx.t0, offset)?;
    let world = ctx.world;
    match kind {
        k if k == crate::routefile::MODEL_ID => {
            let p: PathParams = de(k, params)?;
            let f = RouteFile::load(&p.path)?;
            Ok(Box::new(f.demand(world)?))
        }
        "mobility/demand/trips" => {
            let p: PathParams = de(kind, params)?;
            let set = TripSet::load(&p.path)?;
            set.check_world(world)?;
            Ok(Box::new(routed(
                world,
                set.trips,
                p.assign,
                kind,
                "Trips from a v2xw/trips/1 file.",
                Vec::new(),
            )?))
        }
        k if k == crate::od::MODEL_ID => {
            let p: OdParams = de(k, params)?;
            let zones = ZoneSet::build(world, &p.zones, ClassMask::CAR)?;
            let mut matrices: Vec<OdMatrix> = Vec::new();
            if let Some(path) = &p.matrix {
                matrices.extend(OdMatrix::load(path)?);
            }
            if let Some(cells) = &p.cells {
                let mut m = OdMatrix::new(
                    p.begin_s_of_day.unwrap_or(start_sod),
                    p.end_s_of_day.unwrap_or(start_sod + 3600.0),
                );
                for c in cells {
                    m.add(&c.from, &c.to, c.count);
                }
                matrices.push(m);
            }
            if let Some(g) = &p.gravity {
                matrices.push(gravity(world, &zones, g)?.0);
            }
            if matrices.is_empty() {
                return Err(DemandError::Invalid(format!(
                    "{k}: give a matrix file, inline cells or a gravity model"
                )));
            }
            let mut o2t = p.od2trips.clone();
            o2t.run_start_s_of_day = start_sod;
            if o2t.seed == Od2TripsParams::default().seed {
                o2t.seed ^= ctx.seed;
            }
            if o2t.fleet.is_empty() {
                o2t.fleet = ctx.fleet.clone();
            }
            let mut trips: Vec<Trip> = Vec::new();
            for m in &matrices {
                let mut part = od2trips(world, &zones, m, &o2t)?;
                let base = trips.len() as u64;
                for t in &mut part {
                    t.id += base;
                }
                trips.extend(part);
            }
            trips.retain(|t| t.depart_s < ctx.duration_s);
            Ok(Box::new(routed(
                world,
                trips,
                p.assign,
                k,
                "Trips from an origin-destination matrix over traffic analysis zones.",
                vec![Source::new(
                    SourceKind::Code,
                    "SUMO od2trips (sumo.dlr.de/docs/od2trips.html); PTV VISUM O- and V-format",
                )],
            )?))
        }
        k if k == sampler::MODEL_ID => {
            let p: CountParams = de(k, params)?;
            let text = std::fs::read_to_string(&p.counts).map_err(|e| DemandError::Io {
                path: p.counts.display().to_string(),
                message: e.to_string(),
            })?;
            let counts = crate::counts::parse_counts(&text)?;
            let graph = LaneGraph::new(world, ClassMask::CAR);
            let placed = crate::counts::place(world, &graph, &counts)?;
            let mut sp = p.sampler.clone();
            if sp.seed == SamplerParams::default().seed {
                sp.seed ^= ctx.seed;
            }
            let cands = match &p.candidates {
                Some(path) => RouteFile::load(path)?
                    .trips
                    .iter()
                    .flat_map(|r| r.alternatives.iter().filter_map(|a| graph.nodes_of(&a.lanes)))
                    .collect(),
                None => sampler::candidates(&graph, &placed, &sp),
            };
            let s = sampler::sample(&graph, &placed, &cands, &sp);
            let routes = s.routes.iter().map(|r| Some(graph.lanes_of(r))).collect();
            let mut card = sampler::card(&sp);
            card.assumptions.push(format!(
                "{} trips on {} candidate routes; uncovered counts: {:?}",
                s.trips.len(),
                cands.len(),
                s.uncovered
            ));
            Ok(Box::new(TripListDemand::new(s.trips, routes, card)))
        }
        k if k == crate::transit::MODEL_ID => {
            let mut p: TransitParams = de(k, params)?;
            p.start_s_of_day = start_sod;
            p.duration_s = ctx.duration_s;
            if p.seed == TransitParams::default().seed {
                p.seed ^= ctx.seed;
            }
            let d = p.date.unwrap_or(date);
            Ok(Box::new(TransitDemand::from_feed(world, p, d)?))
        }
        k if k == crate::purpose::MODEL_ID => {
            let explicit = params.get("run_start_s_of_day").is_some();
            let mut p: PurposeParams = de(k, params)?;
            if !explicit {
                p.run_start_s_of_day = start_sod;
            }
            if p.seed == PurposeParams::default().seed {
                p.seed ^= ctx.seed;
            }
            Ok(Box::new(PurposeDemand::new(PurposeModel::new(world, p)?)))
        }
        k if k == crate::models::COMPOSITE_ID => {
            let parts: Vec<Part> = de(
                k,
                params
                    .get("parts")
                    .cloned()
                    .ok_or_else(|| DemandError::Invalid(format!("{k}: 'parts' is required")))?,
            )?;
            let mut built: Vec<Box<dyn Demand>> = Vec::new();
            for part in parts {
                if part.kind == crate::models::COMPOSITE_ID {
                    return Err(DemandError::Invalid(format!("{k}: composites do not nest")));
                }
                if KINDS.contains(&part.kind.as_str()) {
                    built.push(build_with_offset(&part.kind, &part.params, ctx, offset)?);
                } else if let Some(other) = ctx.other {
                    built.push(
                        other(&part.kind, &part.params, part.rate_veh_per_h)
                            .map_err(|e| DemandError::Invalid(format!("{k}: part '{}': {e}", part.kind)))?,
                    );
                } else {
                    return Err(DemandError::Invalid(format!(
                        "{k}: part kind '{}' is not one of {:?}",
                        part.kind,
                        &KINDS[..6]
                    )));
                }
            }
            Ok(Box::new(CompositeDemand::new(built)))
        }
        other => Err(DemandError::Invalid(format!(
            "'{other}' is not a demand kind of this crate; use one of {KINDS:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_time_crosses_midnight() {
        assert_eq!(local_start("2027-03-04T08:00:00Z", 0.0).unwrap(), (28_800.0, 20270304));
        // 03:00 UTC is 19:00 the previous day in Pacific standard time.
        assert_eq!(local_start("2027-03-04T03:00:00Z", -28_800.0).unwrap(), (68_400.0, 20270303));
        assert_eq!(local_start("2024-12-31T23:30:00Z", 3600.0).unwrap(), (1_800.0, 20250101));
    }
}
