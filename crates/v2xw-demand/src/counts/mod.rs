//! Observed link counts: read them, place them on the network, compare a demand with them.
//!
//! A count is a number of vehicles past a place in an interval. The place is a world
//! edge (every lane of it), one lane, or a **coordinate** — a real count station's
//! latitude and longitude, optionally with the direction of travel — which is placed on
//! the nearest road whose heading matches. Counts are read from CSV:
//!
//! ```text
//! id,kind,ref,begin_s,end_s,count
//! 5th-ave-n,point,45.5190 -122.6765 0,0,3600,612     # lat lon heading-deg (0 = north)
//! e17,edge,17,0,900,140
//! ```
//!
//! [`sampler`] builds routes and trips that reproduce counts; [`check`] measures how well
//! any set of routed trips reproduces them, with the GEH statistic and the UK guidance's
//! acceptance criterion (GEH < 5 on at least 85 % of counts) [UK Department for Transport,
//! *TAG Unit M3.1 Highway Assignment Modelling*, 2020, Table 2; the statistic is due to
//! G. Havers' work for the Greater London Council in the 1970s].

pub mod sampler;

use serde::{Deserialize, Serialize};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::LaneId;
use v2xw_world::{LaneKind, World};

use crate::costs::TimeCost;
use crate::error::{DemandError, Result};
use crate::graph::{LaneGraph, Node};
use crate::trips::Trip;

/// Where a count was taken.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Location {
    /// Every lane of a world edge.
    Edge {
        /// The edge id.
        edge: u32,
    },
    /// One lane.
    Lane {
        /// The lane id.
        lane: u32,
    },
    /// A coordinate, placed on the nearest road heading within 45° of `heading_deg`
    /// (clockwise from north) when one is given.
    Point {
        /// Latitude, degrees.
        lat: f64,
        /// Longitude, degrees.
        lon: f64,
        /// Direction of travel, degrees clockwise from north.
        heading_deg: Option<f64>,
    },
}

/// One observed count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Count {
    /// Its id.
    pub id: String,
    /// Where.
    pub location: Location,
    /// Interval start, seconds from the run start.
    pub begin_s: f64,
    /// Interval end, seconds.
    pub end_s: f64,
    /// Vehicles counted.
    pub count: f64,
}

/// A count placed on the graph.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedCount {
    /// The count.
    pub count: Count,
    /// The graph nodes it observes (sorted).
    pub nodes: Vec<Node>,
}

impl PlacedCount {
    /// True if `node` is observed by this count.
    pub fn sees(&self, node: Node) -> bool {
        self.nodes.binary_search(&node).is_ok()
    }
}

/// Parses the count CSV (see the module docs).
///
/// # Errors
/// Parse errors with the line.
pub fn parse_counts(text: &str) -> Result<Vec<Count>> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let l = line.split('#').next().unwrap_or("").trim();
        if l.is_empty() {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() < 6 {
            return Err(DemandError::inline(format!(
                "line {}: expected id,kind,ref,begin_s,end_s,count",
                i + 1
            )));
        }
        let num = |s: &str, what: &str| -> Result<f64> {
            s.parse::<f64>()
                .map_err(|_| DemandError::inline(format!("line {}: '{s}' is not {what}", i + 1)))
        };
        let Ok(begin_s) = f[3].parse::<f64>() else {
            if i == 0 {
                continue; // header
            }
            return Err(DemandError::inline(format!("line {}: bad begin_s", i + 1)));
        };
        let location = match f[1] {
            "edge" => Location::Edge {
                edge: num(f[2], "an edge id")? as u32,
            },
            "lane" => Location::Lane {
                lane: num(f[2], "a lane id")? as u32,
            },
            "point" => {
                let p: Vec<&str> = f[2].split_whitespace().collect();
                if p.len() < 2 {
                    return Err(DemandError::inline(format!(
                        "line {}: a point is 'lat lon [heading]'",
                        i + 1
                    )));
                }
                Location::Point {
                    lat: num(p[0], "a latitude")?,
                    lon: num(p[1], "a longitude")?,
                    heading_deg: p.get(2).map(|h| num(h, "a heading")).transpose()?,
                }
            }
            other => {
                return Err(DemandError::inline(format!(
                    "line {}: kind '{other}' is not edge, lane or point",
                    i + 1
                )));
            }
        };
        out.push(Count {
            id: f[0].to_string(),
            location,
            begin_s,
            end_s: num(f[4], "end_s")?,
            count: num(f[5], "a count")?,
        });
    }
    Ok(out)
}

/// Places counts on `graph`.
///
/// # Errors
/// [`DemandError::Invalid`] naming a count whose place is not on a road the graph's
/// classes may use.
pub fn place(world: &World, graph: &LaneGraph, counts: &[Count]) -> Result<Vec<PlacedCount>> {
    counts
        .iter()
        .map(|c| {
            let lanes: Vec<LaneId> = match &c.location {
                Location::Edge { edge } => world
                    .roads
                    .edges()
                    .get(*edge as usize)
                    .map(|e| e.lanes.clone())
                    .unwrap_or_default(),
                Location::Lane { lane } => vec![LaneId::new(*lane)],
                Location::Point {
                    lat,
                    lon,
                    heading_deg,
                } => locate(world, graph, *lat, *lon, *heading_deg)
                    .map(|e| world.edge(world.lane(e).edge).lanes.clone())
                    .unwrap_or_default(),
            };
            let mut nodes: Vec<Node> = lanes.iter().filter_map(|l| graph.node(*l)).collect();
            nodes.sort_unstable();
            nodes.dedup();
            if nodes.is_empty() {
                return Err(DemandError::Invalid(format!(
                    "count '{}' is not on a road the counted classes may drive",
                    c.id
                )));
            }
            Ok(PlacedCount {
                count: c.clone(),
                nodes,
            })
        })
        .collect()
}

/// The non-internal graph lane nearest a coordinate whose heading (clockwise from north)
/// is within 45° of `heading_deg`, within 60 m.
fn locate(
    world: &World,
    graph: &LaneGraph,
    lat: f64,
    lon: f64,
    heading_deg: Option<f64>,
) -> Option<LaneId> {
    let (x, y) = world.projection().to_enu(lat, lon);
    let p = Vec3::new(x, y, 0.0);
    let mut best: Option<(f64, LaneId)> = None;
    for n in 0..graph.len() as Node {
        if graph.is_internal(n) {
            continue;
        }
        let lane = world.lane(graph.lane(n));
        if lane.kind == LaneKind::Internal {
            continue;
        }
        let proj = lane.project_point(p);
        if proj.distance_m > 60.0 {
            continue;
        }
        if let Some(h) = heading_deg {
            // World heading is counter-clockwise from east; a compass bearing is clockwise
            // from north.
            let bearing = (90.0 - lane.heading_at(proj.s_m).to_degrees()).rem_euclid(360.0);
            let diff = ((bearing - h).rem_euclid(360.0) + 180.0).rem_euclid(360.0) - 180.0;
            if diff.abs() > 45.0 {
                continue;
            }
        }
        let better = match best {
            None => true,
            Some((d, l)) => proj.distance_m.total_cmp(&d).then(lane.id.cmp(&l)).is_lt(),
        };
        if better {
            best = Some((proj.distance_m, lane.id));
        }
    }
    best.map(|b| b.1)
}

/// The GEH statistic of a modelled and an observed hourly flow:
/// `√(2 (M − C)² / (M + C))`.
pub fn geh(m_per_h: f64, c_per_h: f64) -> f64 {
    let s = m_per_h + c_per_h;
    if s <= 0.0 {
        return 0.0;
    }
    v2xw_core::math::sqrt(2.0 * (m_per_h - c_per_h) * (m_per_h - c_per_h) / s)
}

/// One count's comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CountRow {
    /// The count id.
    pub id: String,
    /// Interval start, s.
    pub begin_s: f64,
    /// Observed vehicles.
    pub observed: f64,
    /// Modelled vehicles.
    pub modelled: f64,
    /// GEH on hourly flows.
    pub geh: f64,
}

/// How well routed trips reproduce counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CountCheck {
    /// Every count.
    pub rows: Vec<CountRow>,
    /// Share of counts with GEH < 5.
    pub share_geh_below_5: f64,
    /// The TAG M3.1 criterion: at least 85 % of counts with GEH < 5.
    pub passes_tag_m3_1: bool,
    /// Root-mean-square error as a percentage of the mean observed count.
    pub rmse_percent: f64,
    /// Coefficient of determination of modelled on observed.
    pub r_squared: f64,
}

/// Compares `trips` driving `routes` with `counts`, each trip's passing time at a counted
/// lane found by walking its route with `costs` from its departure: a vehicle is counted
/// when it **enters** an observed lane within the count's interval.
pub fn check(
    counts: &[PlacedCount],
    trips: &[Trip],
    routes: &[Vec<Node>],
    costs: &dyn TimeCost,
) -> CountCheck {
    let mut modelled = vec![0.0; counts.len()];
    for (trip, route) in trips.iter().zip(routes) {
        let mut t = trip.depart_s;
        for n in route {
            for (i, c) in counts.iter().enumerate() {
                if c.sees(*n) && t >= c.count.begin_s && t < c.count.end_s {
                    modelled[i] += 1.0;
                }
            }
            t += costs.cost_s(*n, t);
        }
    }
    summarise(counts, &modelled)
}

/// The check's statistics from modelled values parallel to `counts`.
pub fn summarise(counts: &[PlacedCount], modelled: &[f64]) -> CountCheck {
    let rows: Vec<CountRow> = counts
        .iter()
        .zip(modelled)
        .map(|(c, m)| {
            let hours = ((c.count.end_s - c.count.begin_s) / 3600.0).max(1e-9);
            CountRow {
                id: c.count.id.clone(),
                begin_s: c.count.begin_s,
                observed: c.count.count,
                modelled: *m,
                geh: geh(m / hours, c.count.count / hours),
            }
        })
        .collect();
    let n = rows.len().max(1) as f64;
    let below = rows.iter().filter(|r| r.geh < 5.0).count() as f64 / n;
    let mean_obs = rows.iter().map(|r| r.observed).sum::<f64>() / n;
    let mse = rows
        .iter()
        .map(|r| (r.modelled - r.observed) * (r.modelled - r.observed))
        .sum::<f64>()
        / n;
    let ss_tot: f64 = rows
        .iter()
        .map(|r| (r.observed - mean_obs) * (r.observed - mean_obs))
        .sum();
    let ss_res: f64 = rows
        .iter()
        .map(|r| (r.modelled - r.observed) * (r.modelled - r.observed))
        .sum();
    CountCheck {
        share_geh_below_5: below,
        passes_tag_m3_1: below >= 0.85,
        rmse_percent: if mean_obs > 0.0 {
            100.0 * v2xw_core::math::sqrt(mse) / mean_obs
        } else {
            0.0
        },
        r_squared: if ss_tot > 0.0 {
            1.0 - ss_res / ss_tot
        } else {
            1.0
        },
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geh_matches_its_definition() {
        // M = 120, C = 100: sqrt(2*400/220) = 1.907
        assert!((geh(120.0, 100.0) - 1.906_925_4).abs() < 1e-6);
        assert_eq!(geh(0.0, 0.0), 0.0);
    }

    #[test]
    fn counts_parse() {
        let text = "id,kind,ref,begin_s,end_s,count\na,edge,3,0,900,40\nb,point,45.5 -122.6 90,0,3600,100 # east\n";
        let c = parse_counts(text).expect("parses");
        assert_eq!(c.len(), 2);
        assert!(matches!(c[1].location, Location::Point { heading_deg: Some(h), .. } if h == 90.0));
    }
}
