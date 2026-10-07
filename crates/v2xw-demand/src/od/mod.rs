//! Origin-destination matrices: read, synthesise, and turn into trips.
//!
//! * [`OdMatrix::parse`] reads the PTV VISUM/VISSIM **O-format** (`$O…`, one
//!   `from to count` line per cell) and **V-format** (`$V…`, a full square matrix), the two
//!   formats SUMO's `od2trips` reads [SUMO "Importing O/D Matrices" documentation; PTV
//!   VISUM file-format manual], and a plain CSV (`from,to,count[,begin_s,end_s]`).
//! * [`gravity`] synthesises a matrix from trip ends with a doubly-constrained gravity
//!   model balanced by Furness's method, for a city with zone totals but no matrix.
//! * [`od2trips`] turns a matrix into individual trips: a cell's count is scaled, rounded
//!   stochastically (its whole part, plus one more with probability equal to the
//!   fraction), each trip's departure is drawn uniformly in the matrix interval or from a
//!   [`crate::purpose::DepartureProfile`], and its origin and destination lanes are drawn
//!   by the zones' lane weights. Every draw is keyed by the cell and the trip's index in
//!   it, so changing one cell changes no other cell's trips.
//!
//! # How to add another format
//!
//! Add a `parse_*` function returning an [`OdMatrix`] and a branch in
//! [`OdMatrix::parse`]'s detection; nothing downstream changes.

pub mod gravity;
pub mod zones;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use v2xw_core::ids::LaneId;
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_mobility::VehicleClass;
use v2xw_world::World;

use crate::error::{DemandError, Result};
use crate::purpose::{DepartureProfile, Purpose};
use crate::trips::Trip;
use zones::{Zone, ZoneSet};

/// The model id of matrix-to-trips conversion.
pub const MODEL_ID: &str = "mobility/demand/od-matrix";

/// A matrix: trips per cell over one interval.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct OdMatrix {
    /// Interval start, seconds of the day.
    pub begin_s: f64,
    /// Interval end, seconds of the day.
    pub end_s: f64,
    /// The file's global factor (already applied to the cells by the parsers; kept for
    /// the record).
    pub factor: f64,
    /// Trips per `(from, to)` cell over the interval.
    pub cells: BTreeMap<(String, String), f64>,
}

impl OdMatrix {
    /// An empty matrix over `[begin_s, end_s)`.
    pub fn new(begin_s: f64, end_s: f64) -> Self {
        Self {
            begin_s,
            end_s,
            factor: 1.0,
            cells: BTreeMap::new(),
        }
    }

    /// Adds `count` trips to a cell.
    pub fn add(&mut self, from: &str, to: &str, count: f64) {
        *self
            .cells
            .entry((from.to_string(), to.to_string()))
            .or_insert(0.0) += count;
    }

    /// The total number of trips.
    pub fn total(&self) -> f64 {
        v2xw_core::math::sum_ordered(self.cells.values().copied())
    }

    /// Reads a matrix file, detecting the format from its first non-blank character:
    /// `$O` O-format, `$V` V-format, anything else CSV.
    ///
    /// # Errors
    /// I/O and parse errors, with the line.
    pub fn load(path: &std::path::Path) -> Result<Vec<OdMatrix>> {
        let text = std::fs::read_to_string(path).map_err(|e| DemandError::io(path, e))?;
        Self::parse(&text).map_err(|e| match e {
            DemandError::Parse { message, .. } => DemandError::parse(path, message),
            other => other,
        })
    }

    /// Parses matrix text (see [`OdMatrix::load`]). A CSV may hold several intervals and
    /// so yields several matrices.
    ///
    /// # Errors
    /// Parse errors, with the line.
    pub fn parse(text: &str) -> Result<Vec<OdMatrix>> {
        let head = text.trim_start_matches('\u{feff}').trim_start();
        if head.starts_with("$O") {
            parse_o_format(head).map(|m| vec![m])
        } else if head.starts_with("$V") {
            parse_v_format(head).map(|m| vec![m])
        } else {
            parse_csv(head)
        }
    }
}

/// A VISUM time `H.MM` (hours, then minutes as two decimals) in seconds of the day.
fn visum_time(token: &str) -> Result<f64> {
    let v: f64 = token
        .parse()
        .map_err(|_| DemandError::inline(format!("'{token}' is not a time H.MM")))?;
    let h = v.trunc();
    let m = ((v - h) * 100.0).round();
    if !(0.0..60.0).contains(&m) {
        return Err(DemandError::inline(format!(
            "'{token}' has {m} minutes; VISUM times are H.MM"
        )));
    }
    Ok(h * 3600.0 + m * 60.0)
}

/// The data tokens of a VISUM file: comment lines (`*`) and the header line dropped.
fn visum_tokens(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .skip(1)
        .filter(|(_, l)| !l.trim_start().starts_with('*') && !l.trim().is_empty())
        .flat_map(|(i, l)| {
            l.split_whitespace()
                .map(move |t| (i + 1, t.to_string()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn number(tok: Option<&(usize, String)>, what: &str) -> Result<f64> {
    let (line, t) = tok.ok_or_else(|| DemandError::inline(format!("missing {what}")))?;
    t.parse::<f64>()
        .map_err(|_| DemandError::inline(format!("line {line}: '{t}' is not a number ({what})")))
}

/// Parses the O-format: header `$O…`, `[vehicle type if 'M' in the header]`, `from to`
/// times, factor, then `origin destination count` triples.
///
/// # Errors
/// Parse errors with the line.
pub fn parse_o_format(text: &str) -> Result<OdMatrix> {
    let header = text.lines().next().unwrap_or("");
    let toks = visum_tokens(text);
    let mut it = toks.iter();
    if header.contains('M') {
        it.next(); // vehicle type
    }
    let b = it.next().ok_or_else(|| DemandError::inline("missing begin time"))?;
    let e = it.next().ok_or_else(|| DemandError::inline("missing end time"))?;
    let begin_s = visum_time(&b.1)?;
    let end_s = visum_time(&e.1)?;
    let factor = number(it.next(), "factor")?;
    let mut m = OdMatrix::new(begin_s, end_s);
    m.factor = factor;
    let rest: Vec<&(usize, String)> = it.collect();
    if rest.len() % 3 != 0 {
        return Err(DemandError::inline(format!(
            "the O-format body has {} values, not a multiple of 3 (from to count)",
            rest.len()
        )));
    }
    for c in rest.chunks(3) {
        let count = number(Some(c[2]), "count")?;
        m.add(&c[0].1, &c[1].1, count * factor);
    }
    Ok(m)
}

/// Parses the V-format: header `$V…`, `[vehicle type if 'M']`, times, factor, the
/// number of zones `n`, their `n` names, then the `n × n` matrix row by row.
///
/// # Errors
/// Parse errors with the line.
pub fn parse_v_format(text: &str) -> Result<OdMatrix> {
    let header = text.lines().next().unwrap_or("");
    let toks = visum_tokens(text);
    let mut it = toks.iter();
    if header.contains('M') {
        it.next();
    }
    let b = it.next().ok_or_else(|| DemandError::inline("missing begin time"))?;
    let e = it.next().ok_or_else(|| DemandError::inline("missing end time"))?;
    let mut m = OdMatrix::new(visum_time(&b.1)?, visum_time(&e.1)?);
    let factor = number(it.next(), "factor")?;
    m.factor = factor;
    let n = number(it.next(), "zone count")? as usize;
    let names: Vec<String> = (0..n)
        .map(|_| it.next().map(|t| t.1.clone()))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| DemandError::inline("fewer zone names than the zone count"))?;
    for from in &names {
        for to in &names {
            let v = number(it.next(), "matrix value")?;
            if v != 0.0 {
                m.add(from, to, v * factor);
            }
        }
    }
    Ok(m)
}

/// Parses `from,to,count[,begin_s,end_s]` with an optional header line; rows without
/// times take `[0, 3600)`.
///
/// # Errors
/// Parse errors with the line.
pub fn parse_csv(text: &str) -> Result<Vec<OdMatrix>> {
    let mut by_interval: BTreeMap<(u64, u64), OdMatrix> = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() < 3 {
            return Err(DemandError::inline(format!(
                "line {}: expected from,to,count[,begin_s,end_s]",
                i + 1
            )));
        }
        let Ok(count) = f[2].parse::<f64>() else {
            if i == 0 {
                continue; // a header
            }
            return Err(DemandError::inline(format!(
                "line {}: '{}' is not a count",
                i + 1,
                f[2]
            )));
        };
        let (b, e) = if f.len() >= 5 {
            let b: f64 = f[3]
                .parse()
                .map_err(|_| DemandError::inline(format!("line {}: bad begin_s", i + 1)))?;
            let e: f64 = f[4]
                .parse()
                .map_err(|_| DemandError::inline(format!("line {}: bad end_s", i + 1)))?;
            (b, e)
        } else {
            (0.0, 3600.0)
        };
        by_interval
            .entry((b.to_bits(), e.to_bits()))
            .or_insert_with(|| OdMatrix::new(b, e))
            .add(f[0], f[1], count);
    }
    let mut out: Vec<OdMatrix> = by_interval.into_values().collect();
    out.sort_by(|a, b| a.begin_s.total_cmp(&b.begin_s));
    Ok(out)
}

/// How a matrix becomes trips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Od2TripsParams {
    /// Multiplies every cell (SUMO `od2trips --scale`).
    pub scale: f64,
    /// The time of day the run starts, seconds: a matrix for 07:00-08:00 and a run that
    /// starts at 07:00 departs its trips in the run's first hour.
    pub run_start_s_of_day: f64,
    /// The class of every trip, unless `fleet` is given.
    pub class: VehicleClass,
    /// Class shares, summing to 1.
    pub fleet: Vec<(VehicleClass, f64)>,
    /// The purpose recorded on the trips, if the matrix is for one.
    pub purpose: Option<Purpose>,
    /// The departure-time profile within the interval; `None` is uniform.
    pub profile: Option<DepartureProfile>,
    /// The seed.
    pub seed: u64,
}

impl Default for Od2TripsParams {
    fn default() -> Self {
        Self {
            scale: 1.0,
            run_start_s_of_day: 0.0,
            class: VehicleClass::Passenger,
            fleet: Vec::new(),
            purpose: None,
            profile: None,
            seed: 0x0D_2771,
        }
    }
}

/// Turns `matrix` into trips over `zones` (see the module docs). Departures before the
/// run start are dropped; ids are assigned in `(departure, cell, index)` order from 0.
///
/// # Errors
/// [`DemandError::Invalid`] if a cell names a zone the zone set does not have.
pub fn od2trips(
    world: &World,
    zones: &ZoneSet,
    matrix: &OdMatrix,
    p: &Od2TripsParams,
) -> Result<Vec<Trip>> {
    let _ = world;
    let mut raw: Vec<(f64, usize, u64, LaneId, LaneId, VehicleClass)> = Vec::new();
    for (cell_index, ((from, to), count)) in matrix.cells.iter().enumerate() {
        let zf = zones.get(from).ok_or_else(|| {
            DemandError::Invalid(format!("the matrix names zone '{from}', which is not defined"))
        })?;
        let zt = zones.get(to).ok_or_else(|| {
            DemandError::Invalid(format!("the matrix names zone '{to}', which is not defined"))
        })?;
        let cell_key = cell_key(from, to);
        let mut rng = RngStream::derive(
            p.seed,
            RngDomain::plugin(MODEL_ID),
            EntityRef::custom(MODEL_ID, cell_key),
        );
        let scaled = (count * p.scale).max(0.0);
        let n = scaled.floor() as u64 + u64::from(rng.bool(scaled - scaled.floor()));
        for k in 0..n {
            let mut r = RngStream::derive(
                p.seed,
                RngDomain::plugin(MODEL_ID),
                EntityRef::custom(MODEL_ID, cell_key ^ (k.wrapping_add(1) << 40)),
            );
            let t_day = match &p.profile {
                Some(prof) => prof.sample_between(&mut r, matrix.begin_s, matrix.end_s),
                None => r.uniform(matrix.begin_s, matrix.end_s),
            };
            let o = draw_lane(&mut r, zf, None);
            let d = draw_lane(&mut r, zt, Some(o));
            let class = if p.fleet.is_empty() {
                p.class
            } else {
                let w: Vec<f64> = p.fleet.iter().map(|f| f.1.max(0.0)).collect();
                p.fleet[r.choose_index(&w)].0
            };
            let depart = t_day - p.run_start_s_of_day;
            if depart >= 0.0 && o != d {
                raw.push((depart, cell_index, k, o, d, class));
            }
        }
    }
    raw.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let names: Vec<&(String, String)> = matrix.cells.keys().collect();
    Ok(raw
        .into_iter()
        .enumerate()
        .map(|(id, (depart, cell, _, o, d, class))| Trip {
            id: id as u64,
            depart_s: v2xw_core::math::quantize(depart, 3),
            origin: o,
            destination: d,
            class,
            purpose: p.purpose,
            tag: Some(format!("{}>{}", names[cell].0, names[cell].1)),
        })
        .collect())
}

/// A stable 64-bit key for a cell (FNV-1a over the names).
fn cell_key(from: &str, to: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in from.bytes().chain([0u8]).chain(to.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h & 0x0000_00FF_FFFF_FFFF
}

/// Draws a lane of `zone` by weight, avoiding `not` when the zone has another lane.
fn draw_lane(rng: &mut RngStream, zone: &Zone, not: Option<LaneId>) -> LaneId {
    let w: Vec<f64> = zone
        .lanes
        .iter()
        .map(|(l, w)| if Some(*l) == not && zone.lanes.len() > 1 { 0.0 } else { w.max(0.0) })
        .collect();
    if w.iter().all(|x| *x == 0.0) {
        return zone.lanes[0].0;
    }
    zone.lanes[rng.choose_index(&w)].0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_o_format_parses() {
        let text = "$OR;D2\n* From-Time  To-Time\n7.00 8.30\n* Factor\n2.00\n*\n 1 2 3.5\n 2 1 1\n";
        let m = &OdMatrix::parse(text).expect("parses")[0];
        assert_eq!(m.begin_s, 7.0 * 3600.0);
        assert_eq!(m.end_s, 8.5 * 3600.0);
        assert_eq!(m.cells[&("1".to_string(), "2".to_string())], 7.0);
        assert_eq!(m.total(), 9.0);
    }

    #[test]
    fn the_v_format_parses() {
        let text = "$VMR\n* vehicle type\n4\n* From-Time  To-Time\n0.00 1.00\n* Factor\n1.00\n\
                    * District number\n2\n* names\n 10 20\n* District 10\n 0 4\n* District 20\n 6 0\n";
        let m = &OdMatrix::parse(text).expect("parses")[0];
        assert_eq!(m.cells[&("10".to_string(), "20".to_string())], 4.0);
        assert_eq!(m.cells[&("20".to_string(), "10".to_string())], 6.0);
        assert_eq!(m.cells.len(), 2);
    }

    #[test]
    fn csv_intervals_split() {
        let text = "from,to,count,begin_s,end_s\na,b,5,0,900\na,b,7,900,1800\n";
        let ms = OdMatrix::parse(text).expect("parses");
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[1].total(), 7.0);
    }

    #[test]
    fn od2trips_matches_the_matrix_and_is_keyed_by_cell() {
        let w = crate::graph::tests::grid();
        let zones = ZoneSet::grid(&w, 2, 2, v2xw_world::ClassMask::CAR).expect("zones");
        let names: Vec<String> = zones.names().map(str::to_string).collect();
        let mut m = OdMatrix::new(0.0, 3600.0);
        m.add(&names[0], &names[1], 40.0);
        m.add(&names[1], &names[2], 25.5);
        let p = Od2TripsParams::default();
        let trips = od2trips(&w, &zones, &m, &p).expect("trips");
        assert!((65..=66).contains(&trips.len()), "{}", trips.len());
        // Adding a cell leaves the other cells' trips exactly as they were.
        let mut m2 = m.clone();
        m2.add(&names[2], &names[3], 10.0);
        let more = od2trips(&w, &zones, &m2, &p).expect("trips");
        let tag = format!("{}>{}", names[0], names[1]);
        let a: Vec<_> = trips.iter().filter(|t| t.tag.as_deref() == Some(&tag)).map(|t| (t.depart_s, t.origin, t.destination)).collect();
        let b: Vec<_> = more.iter().filter(|t| t.tag.as_deref() == Some(&tag)).map(|t| (t.depart_s, t.origin, t.destination)).collect();
        assert_eq!(a, b);
    }
}
