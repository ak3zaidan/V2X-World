//! Traffic analysis zones: named sets of lanes where trips start and end.
//!
//! SUMO calls these TAZ (`<taz>` with source and sink edges) [SUMO "Traffic Analysis
//! Zones" documentation]. Here a zone is a set of lanes with weights; a trip from the zone
//! starts on one of them drawn by weight. Zones come from:
//!
//! * [`ZoneSpec::Grid`] — tiles over the world box, for a city that has no zone system;
//! * [`ZoneSpec::Boxes`] — named latitude/longitude boxes (a census tract's extent, a
//!   district);
//! * [`ZoneSpec::Lanes`] — explicit lane ids, as a TAZ file lists edges.
//!
//! **Weights.** By default a lane's weight is the gross floor area of the buildings whose
//! centroid is nearest to it (within 80 m) plus its own length in square metres — the
//! floor-area basis of trip generation [Institute of Transportation Engineers, *Trip
//! Generation Manual*, 11th ed., 2021: trip rates per 1,000 ft² of gross floor area] with
//! the length as a floor so a street with no buildings still sees some traffic. Storeys are
//! the building height over 3.5 m, rounded down, at least one (a typical commercial storey
//! height; the world records height, not storeys). `ZoneWeight::Length` weights by length
//! alone.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::LaneId;
use v2xw_world::{ClassMask, World};

use crate::error::{DemandError, Result};
use crate::graph::LaneGraph;

/// The shortest lane a trip may start or end on, metres: a bus (12 m) plus room.
pub const MIN_ENDPOINT_LANE_M: f64 = 20.0;

/// How lanes in a zone are weighted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ZoneWeight {
    /// Nearby building floor area plus lane length.
    #[default]
    FloorArea,
    /// Lane length.
    Length,
}

/// How zones are defined.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ZoneSpec {
    /// `cols × rows` tiles over the world box, named `r{row}c{col}` from the south-west.
    Grid {
        /// Columns.
        cols: u32,
        /// Rows.
        rows: u32,
        /// Lane weights.
        #[serde(default)]
        weight: ZoneWeight,
    },
    /// Named boxes `[south, west, north, east]`, degrees.
    Boxes {
        /// The boxes.
        zones: BTreeMap<String, [f64; 4]>,
        /// Lane weights.
        #[serde(default)]
        weight: ZoneWeight,
    },
    /// Explicit lane ids.
    Lanes {
        /// The lanes of each zone.
        zones: BTreeMap<String, Vec<u32>>,
    },
}

impl Default for ZoneSpec {
    fn default() -> Self {
        ZoneSpec::Grid {
            cols: 3,
            rows: 3,
            weight: ZoneWeight::FloorArea,
        }
    }
}

/// One zone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    /// Its name.
    pub name: String,
    /// Its lanes and their weights, in lane-id order.
    pub lanes: Vec<(LaneId, f64)>,
}

impl Zone {
    /// The zone's total weight.
    pub fn weight(&self) -> f64 {
        v2xw_core::math::sum_ordered(self.lanes.iter().map(|l| l.1))
    }
}

/// A zone system.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ZoneSet {
    zones: BTreeMap<String, Zone>,
}

impl ZoneSet {
    /// Builds the zones `spec` describes on `world` for `classes`.
    ///
    /// # Errors
    /// [`DemandError::Invalid`] if a zone ends up with no usable lane.
    pub fn build(world: &World, spec: &ZoneSpec, classes: ClassMask) -> Result<Self> {
        let graph = LaneGraph::new(world, classes);
        let ends: Vec<LaneId> = graph
            .endpoints(MIN_ENDPOINT_LANE_M)
            .into_iter()
            .map(|n| graph.lane(n))
            .collect();
        let mut zones: BTreeMap<String, Zone> = BTreeMap::new();
        match spec {
            ZoneSpec::Grid { cols, rows, weight } => {
                let (cols, rows) = ((*cols).max(1), (*rows).max(1));
                let w = weights(world, &ends, *weight);
                let b = world.bbox;
                let dx = (b.max.x - b.min.x).max(1e-9) / f64::from(cols);
                let dy = (b.max.y - b.min.y).max(1e-9) / f64::from(rows);
                for (lane, weight) in ends.iter().zip(&w) {
                    let m = mid(world, *lane);
                    let c = (((m.x - b.min.x) / dx).floor().max(0.0) as u32).min(cols - 1);
                    let r = (((m.y - b.min.y) / dy).floor().max(0.0) as u32).min(rows - 1);
                    let name = format!("r{r}c{c}");
                    zones
                        .entry(name.clone())
                        .or_insert_with(|| Zone {
                            name,
                            lanes: Vec::new(),
                        })
                        .lanes
                        .push((*lane, *weight));
                }
            }
            ZoneSpec::Boxes { zones: boxes, weight } => {
                let w = weights(world, &ends, *weight);
                let proj = world.projection();
                for (name, [s, west, n, e]) in boxes {
                    let (x0, y0) = proj.to_enu(*s, *west);
                    let (x1, y1) = proj.to_enu(*n, *e);
                    let lanes: Vec<(LaneId, f64)> = ends
                        .iter()
                        .zip(&w)
                        .filter(|(l, _)| {
                            let m = mid(world, **l);
                            m.x >= x0.min(x1) && m.x <= x0.max(x1) && m.y >= y0.min(y1) && m.y <= y0.max(y1)
                        })
                        .map(|(l, w)| (*l, *w))
                        .collect();
                    if lanes.is_empty() {
                        return Err(DemandError::Invalid(format!(
                            "zone '{name}' contains no lane a trip can start on"
                        )));
                    }
                    zones.insert(
                        name.clone(),
                        Zone {
                            name: name.clone(),
                            lanes,
                        },
                    );
                }
            }
            ZoneSpec::Lanes { zones: given } => {
                for (name, ids) in given {
                    let lanes: Vec<(LaneId, f64)> = ids
                        .iter()
                        .map(|i| LaneId::new(*i))
                        .filter(|l| graph.node(*l).is_some())
                        .map(|l| (l, world.lane(l).length_m))
                        .collect();
                    if lanes.is_empty() {
                        return Err(DemandError::Invalid(format!(
                            "zone '{name}' names no lane the classes may drive"
                        )));
                    }
                    zones.insert(
                        name.clone(),
                        Zone {
                            name: name.clone(),
                            lanes,
                        },
                    );
                }
            }
        }
        for z in zones.values_mut() {
            z.lanes.sort_by_key(|l| l.0);
        }
        if zones.is_empty() {
            return Err(DemandError::Invalid(
                "the zone system has no zone with a usable lane".to_string(),
            ));
        }
        Ok(Self { zones })
    }

    /// A `cols × rows` grid with floor-area weights.
    ///
    /// # Errors
    /// As [`ZoneSet::build`].
    pub fn grid(world: &World, cols: u32, rows: u32, classes: ClassMask) -> Result<Self> {
        Self::build(
            world,
            &ZoneSpec::Grid {
                cols,
                rows,
                weight: ZoneWeight::FloorArea,
            },
            classes,
        )
    }

    /// The zone names, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.zones.keys().map(String::as_str)
    }

    /// The zone called `name`.
    pub fn get(&self, name: &str) -> Option<&Zone> {
        self.zones.get(name)
    }

    /// Every zone, by name.
    pub fn zones(&self) -> impl Iterator<Item = &Zone> {
        self.zones.values()
    }

    /// The number of zones.
    pub fn len(&self) -> usize {
        self.zones.len()
    }

    /// True if there are none.
    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
    }
}

/// A lane's midpoint.
pub fn mid(world: &World, lane: LaneId) -> Vec3 {
    let l = world.lane(lane);
    l.point_at(l.length_m / 2.0)
}

/// The weight of each of `lanes` under `rule`.
pub fn weights(world: &World, lanes: &[LaneId], rule: ZoneWeight) -> Vec<f64> {
    let mut w: Vec<f64> = lanes.iter().map(|l| world.lane(*l).length_m).collect();
    if rule == ZoneWeight::Length || lanes.is_empty() {
        return w;
    }
    let index: BTreeMap<LaneId, usize> = lanes.iter().enumerate().map(|(i, l)| (*l, i)).collect();
    for b in &world.buildings {
        let n = b.footprint.len();
        if n < 3 {
            continue;
        }
        // Shoelace area and centroid of the footprint.
        let mut a2 = 0.0;
        let (mut cx, mut cy) = (0.0, 0.0);
        for i in 0..n {
            let p = b.footprint[i];
            let q = b.footprint[(i + 1) % n];
            let cross = p.x * q.y - q.x * p.y;
            a2 += cross;
            cx += (p.x + q.x) * cross;
            cy += (p.y + q.y) * cross;
        }
        let area = (a2 / 2.0).abs();
        if area < 1.0 || a2 == 0.0 {
            continue;
        }
        let centroid = Vec3::new(cx / (3.0 * a2), cy / (3.0 * a2), 0.0);
        let storeys = ((b.height_m - b.min_height_m) / 3.5).floor().max(1.0);
        let Some(m) = world.nearest_lane_within(centroid, 80.0, Some(ClassMask::CAR)) else {
            continue;
        };
        if let Some(i) = index.get(&m.pos.lane) {
            w[*i] += area * storeys;
        }
    }
    w
}
