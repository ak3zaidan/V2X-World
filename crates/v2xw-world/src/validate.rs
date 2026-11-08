//! World validation: does the imported road network look like the street it came from, and
//! can a car, a cyclist and a pedestrian actually use it?
//!
//! # Why
//!
//! The traffic auditor (`v2xw_mobility::audit`) finds defects by driving vehicles through a
//! world and watching them misbehave: a pedestrian overlapped by a car, a heading that turns
//! faster than a car can steer. Almost every such defect it found was the *importer's*: a
//! sidewalk laid on the roadway, a 1 m stub lane between two junction connectors. This
//! module finds those defects in the world itself, before anything drives on it, and names
//! the lane and the OSM way responsible. It also compares what the importer made of each
//! way with what the source tags say, so a regression in lane counts, one-way handling,
//! turn lanes, bus lanes, cycle lanes, parking lanes, widths or speed limits is caught by
//! a count rather than by eye.
//!
//! # The checks
//!
//! Geometry, on any [`World`]:
//!
//! | Check | What it counts |
//! |---|---|
//! | `sidewalk-on-roadway` | sidewalk lanes whose band overlaps a driving or bus lane's band by more than [`ValidationParams::overlap_tolerance_m`] (lanes; the overlapping length is in `metres`) |
//! | `roadway-overlap` | driving lanes of different roads whose bands overlap, away from their ends — two one-way ways mapped closer together than their lanes are wide |
//! | `sidewalk-in-vehicle-envelope` | sidewalk lanes whose centreline comes within half a car's width plus a pedestrian's radius of a driving lane's centreline — where a walker and a car on their own lanes *touch* |
//! | `cycle-on-roadway` | the same as `sidewalk-on-roadway`, for cycle lanes |
//! | `sidewalk-on-cycle-lane` | sidewalk lanes laid along a cycle lane or track, their bands overlapping by more than the tolerance |
//! | `path-turns-faster-than-a-car` | junction connectors on which a car's body (rear axle to front bumper, [`ValidationParams::body_length_m`]) would turn faster than the AASHTO passenger-car minimum path radius allows, over the connector and the lanes either side of it — the geometric twin of the auditor's `heading-jump` |
//! | `uturn-tighter-than-a-car` | U-turn connectors whose path is tighter than the same radius |
//! | `dead-end-uturn` | U-turn connectors at a junction with one road arm |
//! | `stub-driving-lane` | driving lanes shorter than [`ValidationParams::stub_lane_m`] |
//! | `vehicle-envelope-in-building` | driving lanes along which a car's body (centreline ± half its width) enters a building, outside any mapped passage |
//! | `signal-movement-never-green` | movements of a signal plan (vehicle connectors and crosswalk lanes alike) that no phase shows green or permissive green: a lane whose movement a plan holds on red all cycle |
//! | `signal-group-never-green` | signal head groups ([`World::group_signals`]) that show no green in the whole cycle: a head a driver or a pedestrian would wait at for ever |
//!
//! Source fidelity, when the OSM file and the importer's edge-to-way table are supplied
//! ([`SourceLink`]):
//!
//! | Check | Expected (from the tags) against imported |
//! |---|---|
//! | `lanes-mismatch` | `lanes`, `lanes:forward`, `lanes:backward` against driving + bus lanes per direction |
//! | `oneway-mismatch` | `oneway` against which directions have lanes |
//! | `turn-lanes-violated` | an approach lane with a movement its `turn:lanes` entry does not allow |
//! | `bus-lanes-mismatch` | `lanes:bus`, `bus:lanes`, `busway` against bus lanes per direction |
//! | `cycle-lane-missing` | `cycleway*=lane|track` against a cycle lane on that side |
//! | `parking-lane-missing` | `parking*=lane|…`, `parking:lane:*` against a parking lane on that side |
//! | `width-mismatch` | the `width` tag against the imported carriageway width, beyond 15 % |
//! | `speed-mismatch` | `maxspeed` against the driving lanes' limit |
//! | `crossing-node-without-crosswalk` | a `highway=crossing` node on a road with no crosswalk lane within [`ValidationParams::crossing_node_radius_m`] |
//!
//! Every check reports how many things failed, out of how many were examined, and a few
//! examples with world coordinates and OSM ids. A [`Baseline`] holds the most failures each
//! check may report on one city; [`ValidationReport::regressions`] lists the checks above
//! their ceiling, which is what the `world_report` example turns into a non-zero exit.
//!
//! Everything here is a pure function of the world (and the source), iterated in id order,
//! with transcendentals from [`v2xw_core::math`], so two runs report the same numbers.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::{EdgeId, JunctionId, LaneId};
use v2xw_core::math;

use crate::model::{
    ClassMask, Lane, LaneKind, TurnDirection, World, normalise_angle, road_meets_building,
};
use crate::osm::{OsmFile, Tags};

/// The tunable thresholds of the checks.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ValidationParams {
    /// How far two lane bands may overlap before it counts, metres. 0.3 m, the tolerance the
    /// 2026-09-24 QA used when it counted 811 sidewalks on the roadway.
    pub overlap_tolerance_m: f64,
    /// Half a passenger car's width, metres: 0.9 (a 1.8 m car, the auditor's).
    pub car_half_width_m: f64,
    /// A pedestrian's body radius, metres: 0.25, as the social-force model's.
    pub pedestrian_radius_m: f64,
    /// The smallest path radius a passenger car can drive, metres: 5.42, the auditor's
    /// bound for the AASHTO P design vehicle.
    pub min_path_radius_m: f64,
    /// Rear axle to front bumper, metres: the chord the engine's published heading is
    /// measured along. 4.5, the passenger car's length.
    pub body_length_m: f64,
    /// Heading slack per step, radians: 0.02, the auditor's.
    pub heading_slack_rad: f64,
    /// A driving lane shorter than this is a stub, metres.
    pub stub_lane_m: f64,
    /// A crossing node needs a crosswalk lane within this distance, metres.
    pub crossing_node_radius_m: f64,
    /// How many examples each check keeps.
    pub examples: usize,
}

impl Default for ValidationParams {
    fn default() -> Self {
        Self {
            overlap_tolerance_m: 0.3,
            car_half_width_m: 0.9,
            pedestrian_radius_m: 0.25,
            min_path_radius_m: 5.42,
            body_length_m: 4.5,
            heading_slack_rad: 0.02,
            stub_lane_m: 2.0,
            crossing_node_radius_m: 8.0,
            examples: 6,
        }
    }
}

/// One check's outcome.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CheckResult {
    /// How many things failed.
    pub count: u64,
    /// How many things were examined.
    pub of: u64,
    /// The length involved, metres, for the checks that measure one (0 otherwise).
    pub metres: f64,
    /// A few failures, for a human.
    pub examples: Vec<String>,
}

/// Every check's outcome, by check name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// By check name, in name order.
    pub checks: BTreeMap<String, CheckResult>,
    /// Descriptive figures that are not pass/fail — how many lanes of each kind, how many
    /// signal plans at each cycle length.
    pub figures: BTreeMap<String, f64>,
}

/// The most failures each check may report on one world: a regression gate.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// What the baseline is for, e.g. the city and the options.
    #[serde(default)]
    pub description: String,
    /// Check name to its ceiling. A check absent from the map is not gated.
    pub max: BTreeMap<String, u64>,
}

impl ValidationReport {
    /// The failure count of `check`, zero when it did not run.
    pub fn count(&self, check: &str) -> u64 {
        self.checks.get(check).map_or(0, |c| c.count)
    }

    /// The checks whose count exceeds the baseline's ceiling, one line each.
    pub fn regressions(&self, baseline: &Baseline) -> Vec<String> {
        let mut out = Vec::new();
        for (name, max) in &baseline.max {
            let count = self.count(name);
            if count > *max {
                out.push(format!("{name}: {count} > baseline {max}"));
            }
        }
        out
    }

    /// A baseline that holds every check at exactly what this report found.
    pub fn as_baseline(&self, description: &str) -> Baseline {
        Baseline {
            description: description.to_string(),
            max: self
                .checks
                .iter()
                .map(|(k, v)| (k.clone(), v.count))
                .collect(),
        }
    }

    /// A plain-text rendering, one check per line, examples indented under it.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for (name, c) in &self.checks {
            if c.metres > 0.0 {
                s.push_str(&format!(
                    "{name:<36} {:>7} of {:<7} ({:.0} m)\n",
                    c.count, c.of, c.metres
                ));
            } else {
                s.push_str(&format!("{name:<36} {:>7} of {:<7}\n", c.count, c.of));
            }
            for e in &c.examples {
                s.push_str(&format!("    {e}\n"));
            }
        }
        if !self.figures.is_empty() {
            s.push_str("figures:\n");
            for (k, v) in &self.figures {
                s.push_str(&format!("    {k:<40} {v}\n"));
            }
        }
        s
    }

    fn entry(&mut self, name: &str) -> &mut CheckResult {
        self.checks.entry(name.to_string()).or_default()
    }

    fn fail(&mut self, name: &str, keep: usize, example: impl FnOnce() -> String) {
        let c = self.entry(name);
        c.count += 1;
        if c.examples.len() < keep {
            c.examples.push(example());
        }
    }
}

/// Where each edge of an imported world came from: what [`crate::osm::ImportReport`]
/// records in `edge_sources`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeSource {
    /// The OSM way whose plan the edge carries (the lowest-numbered piece of the segment).
    pub way: i64,
    /// The OSM way that reaches the edge's `from` junction.
    pub near_way: i64,
    /// The OSM way that reaches the edge's `to` junction — whose `turn:lanes` describes the
    /// approach this edge makes.
    pub far_way: i64,
    /// What the edge is, relative to that way: its carriageway, or a side lane
    /// (sidewalk, cycle, parking) the way's tags produced.
    pub role: EdgeRole,
}

/// What an edge is to the way it came from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeRole {
    /// The way's own lanes, in one direction.
    #[default]
    Carriageway,
    /// A lane the way's tags placed beside its carriageway: a tagged sidewalk, cycle lane
    /// or parking lane.
    Side,
}

/// The source a world was imported from, and which way each of its edges came from.
#[derive(Debug, Clone, Copy)]
pub struct SourceLink<'a> {
    /// The parsed extract.
    pub file: &'a OsmFile,
    /// Indexed by edge id; `None` for an edge with no way (a junction's internal edge).
    pub edges: &'a [Option<EdgeSource>],
}

/// Runs every check on `world`, and the source-fidelity checks too when `source` is given.
pub fn validate(
    world: &World,
    source: Option<SourceLink<'_>>,
    params: &ValidationParams,
) -> ValidationReport {
    let mut report = ValidationReport::default();
    let index = SegmentIndex::new(world, 20.0);
    check_band_overlaps(world, &index, params, &mut report);
    check_roadway_overlaps(world, &index, params, &mut report);
    check_paths(world, params, &mut report);
    check_stubs_and_buildings(world, params, &mut report);
    check_signals(world, params, &mut report);
    figures(world, &mut report);
    if let Some(link) = source {
        check_source(world, link, params, &mut report);
    }
    report
}

// ---------------------------------------------------------------------------------------
// A grid over lane segments
// ---------------------------------------------------------------------------------------

/// Lane segments by grid cell, for "which lanes pass near this point".
struct SegmentIndex {
    cell: f64,
    cells: BTreeMap<(i64, i64), Vec<(LaneId, u32)>>,
}

impl SegmentIndex {
    fn new(world: &World, cell: f64) -> Self {
        let mut cells: BTreeMap<(i64, i64), Vec<(LaneId, u32)>> = BTreeMap::new();
        for lane in world.roads.lanes() {
            for (i, w) in lane.centreline.windows(2).enumerate() {
                let (x0, x1) = (w[0].x.min(w[1].x), w[0].x.max(w[1].x));
                let (y0, y1) = (w[0].y.min(w[1].y), w[0].y.max(w[1].y));
                for cx in cell_of(x0, cell)..=cell_of(x1, cell) {
                    for cy in cell_of(y0, cell)..=cell_of(y1, cell) {
                        cells.entry((cx, cy)).or_default().push((lane.id, i as u32));
                    }
                }
            }
        }
        Self { cell, cells }
    }

    /// Every `(lane, segment)` whose cell is within `radius` of `p`, sorted and deduplicated.
    fn near(&self, p: Vec3, radius: f64) -> Vec<(LaneId, u32)> {
        let mut out = Vec::new();
        for cx in cell_of(p.x - radius, self.cell)..=cell_of(p.x + radius, self.cell) {
            for cy in cell_of(p.y - radius, self.cell)..=cell_of(p.y + radius, self.cell) {
                if let Some(v) = self.cells.get(&(cx, cy)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

fn cell_of(v: f64, cell: f64) -> i64 {
    (v / cell).floor() as i64
}

/// Horizontal distance from `p` to the segment `a → b`.
fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let u = if len2 <= 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (a.x + u * dx, a.y + u * dy);
    math::hypot(p.x - qx, p.y - qy)
}

/// True for the lanes a car drives along between junctions.
fn is_carriageway(lane: &Lane) -> bool {
    matches!(lane.kind, LaneKind::Driving | LaneKind::Bus)
        && lane.admits(ClassMask::MOTOR_TRAFFIC.union(ClassMask::BUS))
}

// ---------------------------------------------------------------------------------------
// Bands that overlap
// ---------------------------------------------------------------------------------------

fn check_band_overlaps(
    world: &World,
    index: &SegmentIndex,
    params: &ValidationParams,
    report: &mut ValidationReport,
) {
    let keep = params.examples;
    for kind in [LaneKind::Sidewalk, LaneKind::Cycle] {
        let name = if kind == LaneKind::Sidewalk {
            "sidewalk-on-roadway"
        } else {
            "cycle-on-roadway"
        };
        report.entry(name);
        if kind == LaneKind::Sidewalk {
            report.entry("sidewalk-in-vehicle-envelope");
            report.entry("sidewalk-on-cycle-lane");
        }
        for lane in world.roads.lanes().iter().filter(|l| l.kind == kind) {
            report.entry(name).of += 1;
            if kind == LaneKind::Sidewalk {
                report.entry("sidewalk-in-vehicle-envelope").of += 1;
                report.entry("sidewalk-on-cycle-lane").of += 1;
            }
            let mut on_cycle: Option<(f64, LaneId, Vec3)> = None;
            let mut on_cycle_m = 0.0;
            let samples = (lane.length_m / 1.0).ceil().max(1.0) as usize;
            let step = lane.length_m / samples as f64;
            let mut worst: Option<(f64, LaneId, Vec3)> = None;
            let mut overlap_m = 0.0;
            let mut touch: Option<(f64, LaneId, Vec3)> = None;
            let mut crosses: Option<LaneId> = None;
            for k in 0..samples {
                let p = lane.point_at((k as f64 + 0.5) * step);
                let h = lane.heading_at((k as f64 + 0.5) * step);
                let mut best_overlap = f64::NEG_INFINITY;
                let mut best_lane = LaneId::new(0);
                let mut nearest = f64::INFINITY;
                let mut nearest_lane = LaneId::new(0);
                let mut cycle_overlap = f64::NEG_INFINITY;
                let mut cycle_lane = LaneId::new(0);
                for (id, seg) in index.near(p, 12.0) {
                    let other = world.lane(id);
                    // A pavement laid on a cycle lane or track: walkers and riders in one
                    // band, which the auditor sees as bicycles through pedestrians.
                    if kind == LaneKind::Sidewalk && other.kind == LaneKind::Cycle {
                        let (a, b) = (
                            other.centreline[seg as usize],
                            other.centreline[seg as usize + 1],
                        );
                        let parallel =
                            math::sin(h - math::atan2(b.y - a.y, b.x - a.x)).abs() <= 0.5;
                        let o = 0.5 * (other.width_m + lane.width_m) - segment_distance(p, a, b);
                        if parallel && ((a.z + b.z) * 0.5 - p.z).abs() <= 3.0 && o > cycle_overlap {
                            cycle_overlap = o;
                            cycle_lane = id;
                        }
                        continue;
                    }
                    if !is_carriageway(other) {
                        continue;
                    }
                    // A lane on another level — a tunnel under the pavement, a bridge
                    // over it — is not on the same surface.
                    let (a, b) = (
                        other.centreline[seg as usize],
                        other.centreline[seg as usize + 1],
                    );
                    if ((a.z + b.z) * 0.5 - p.z).abs() > 3.0 {
                        continue;
                    }
                    let d = segment_distance(p, a, b);
                    let overlap = 0.5 * (other.width_m + lane.width_m) - d;
                    // A path that crosses the road — a pavement across a driveway, a
                    // cycle track across a side street — meets it at an angle; that is a
                    // crossing, counted apart, not a pavement laid on the roadway.
                    let across = math::sin(h - math::atan2(b.y - a.y, b.x - a.x)).abs() > 0.5;
                    if across {
                        if overlap > params.overlap_tolerance_m {
                            crosses = Some(id);
                        }
                        continue;
                    }
                    if overlap > best_overlap {
                        best_overlap = overlap;
                        best_lane = id;
                    }
                    if d < nearest {
                        nearest = d;
                        nearest_lane = id;
                    }
                }
                if best_overlap > params.overlap_tolerance_m {
                    overlap_m += step;
                    if worst.is_none_or(|w| best_overlap > w.0) {
                        worst = Some((best_overlap, best_lane, p));
                    }
                }
                if cycle_overlap > params.overlap_tolerance_m {
                    on_cycle_m += step;
                    if on_cycle.is_none_or(|w| cycle_overlap > w.0) {
                        on_cycle = Some((cycle_overlap, cycle_lane, p));
                    }
                }
                let envelope = params.car_half_width_m + params.pedestrian_radius_m;
                if kind == LaneKind::Sidewalk
                    && nearest < envelope
                    && touch.is_none_or(|t| nearest < t.0)
                {
                    touch = Some((nearest, nearest_lane, p));
                }
            }
            if let Some((o, other, p)) = worst {
                report.entry(name).metres += overlap_m;
                report.fail(name, keep, || {
                    format!(
                        "lane {} overlaps lane {} by {o:.2} m over {overlap_m:.0} m, at ({:.1}, {:.1})",
                        lane.id.index(),
                        other.index(),
                        p.x,
                        p.y
                    )
                });
            }
            if let Some((o, other, p)) = on_cycle {
                report.entry("sidewalk-on-cycle-lane").metres += on_cycle_m;
                report.fail("sidewalk-on-cycle-lane", keep, || {
                    format!(
                        "lane {} overlaps cycle lane {} by {o:.2} m over {on_cycle_m:.0} m, at ({:.1}, {:.1})",
                        lane.id.index(),
                        other.index(),
                        p.x,
                        p.y
                    )
                });
            }
            let across_name = if kind == LaneKind::Sidewalk {
                "sidewalk-crosses-roadway"
            } else {
                "cycle-crosses-roadway"
            };
            report.entry(across_name).of += 1;
            if let Some(other) = crosses {
                report.fail(across_name, keep, || {
                    format!(
                        "lane {} crosses driving lane {} with no crosswalk",
                        lane.id.index(),
                        other.index()
                    )
                });
            }
            if let Some((d, other, p)) = touch {
                report.fail("sidewalk-in-vehicle-envelope", keep, || {
                    format!(
                        "lane {} passes {d:.2} m from driving lane {}, at ({:.1}, {:.1})",
                        lane.id.index(),
                        other.index(),
                        p.x,
                        p.y
                    )
                });
            }
        }
    }
}

/// Driving lanes of different roads laid over each other: two one-way ways mapped closer
/// together than their lanes are wide, or a road drawn through another. Adjacent lanes of
/// one road are exactly a lane apart and never count; the ends of a lane, where roads
/// meet at a junction, are skipped.
fn check_roadway_overlaps(
    world: &World,
    index: &SegmentIndex,
    params: &ValidationParams,
    report: &mut ValidationReport,
) {
    let name = "roadway-overlap";
    report.entry(name);
    // The two directions of one road share their junction pair.
    let pair = |e: EdgeId| {
        let edge = world.edge(e);
        (edge.from.min(edge.to), edge.from.max(edge.to))
    };
    for lane in world.roads.lanes().iter().filter(|l| is_carriageway(l)) {
        report.entry(name).of += 1;
        let own = pair(lane.edge);
        let samples = (lane.length_m / 1.0).floor() as usize;
        let mut worst: Option<(f64, LaneId, Vec3)> = None;
        let mut metres = 0.0;
        for k in 0..samples {
            let s = k as f64 + 0.5;
            if s < 3.0 || s > lane.length_m - 3.0 {
                continue;
            }
            let p = lane.point_at(s);
            let h = lane.heading_at(s);
            let mut best: Option<(f64, LaneId)> = None;
            for (id, seg) in index.near(p, 8.0) {
                let other = world.lane(id);
                if !is_carriageway(other) || other.edge == lane.edge || pair(other.edge) == own {
                    continue;
                }
                let (a, b) = (
                    other.centreline[seg as usize],
                    other.centreline[seg as usize + 1],
                );
                if ((a.z + b.z) * 0.5 - p.z).abs() > 3.0 {
                    continue;
                }
                if math::sin(h - math::atan2(b.y - a.y, b.x - a.x)).abs() > 0.5 {
                    continue;
                }
                let overlap = 0.5 * (other.width_m + lane.width_m) - segment_distance(p, a, b);
                if best.is_none_or(|x| overlap > x.0) {
                    best = Some((overlap, id));
                }
            }
            if let Some((o, id)) = best {
                if o > params.overlap_tolerance_m {
                    metres += 1.0;
                    if worst.is_none_or(|w| o > w.0) {
                        worst = Some((o, id, p));
                    }
                }
            }
        }
        if let Some((o, other, p)) = worst {
            report.entry(name).metres += metres;
            report.fail(name, params.examples, || {
                format!(
                    "lane {} overlaps lane {} by {o:.2} m over {metres:.0} m, at ({:.1}, {:.1})",
                    lane.id.index(),
                    other.index(),
                    p.x,
                    p.y
                )
            });
        }
    }
}

// ---------------------------------------------------------------------------------------
// Paths a car cannot drive
// ---------------------------------------------------------------------------------------

/// A path as a list of `(lane, s_from, s_to)` pieces.
type Piece = (LaneId, f64, f64);

fn check_paths(world: &World, params: &ValidationParams, report: &mut ValidationReport) {
    let keep = params.examples;
    let lead = params.body_length_m + 4.0;
    // The road arms at each junction, to recognise a dead end.
    let mut arms: BTreeMap<JunctionId, BTreeSet<(JunctionId, JunctionId)>> = BTreeMap::new();
    for edge in world.roads.edges() {
        if edge.from == edge.to {
            continue;
        }
        let motor = edge
            .lanes
            .iter()
            .any(|l| world.lane(*l).admits(ClassMask::MOTOR_TRAFFIC));
        if !motor {
            continue;
        }
        let key = (edge.from.min(edge.to), edge.from.max(edge.to));
        arms.entry(edge.from).or_default().insert(key);
        arms.entry(edge.to).or_default().insert(key);
    }
    report.entry("path-turns-faster-than-a-car");
    report.entry("uturn-tighter-than-a-car");
    report.entry("dead-end-uturn");
    let mut flagged: BTreeSet<LaneId> = BTreeSet::new();
    for c in world.roads.connections() {
        let Some(via) = c.via else { continue };
        let connector = world.lane(via);
        if !connector.admits(ClassMask::CAR) {
            continue;
        }
        let from = world.lane(c.from_lane);
        let to = world.lane(c.to_lane);
        let uturn = c.direction == TurnDirection::UTurn;
        let name = if uturn {
            "uturn-tighter-than-a-car"
        } else {
            "path-turns-faster-than-a-car"
        };
        report.entry(name).of += 1;
        if uturn {
            report.entry("dead-end-uturn").of += 1;
            let j = connector.junction.unwrap_or(JunctionId::new(0));
            if arms.get(&j).is_some_and(|a| a.len() <= 1) {
                report.fail("dead-end-uturn", keep, || {
                    let p = connector.start();
                    format!(
                        "connector {} at junction {}, ({:.1}, {:.1})",
                        via.index(),
                        j.index(),
                        p.x,
                        p.y
                    )
                });
            }
        }
        // Behind: the approach lane's last `lead` metres, and its predecessor's if it is
        // shorter than that. Ahead: the departure lane and whatever follows it, until the
        // path has run `lead` metres past the connector.
        let mut behind: Vec<Piece> =
            vec![(from.id, (from.length_m - lead).max(0.0), from.length_m)];
        if from.length_m < lead {
            if let Some(p) = world.roads.connections().iter().find(|p| {
                p.to_lane == from.id
                    && p.via.is_none()
                    && world.lane(p.from_lane).kind == LaneKind::Internal
            }) {
                let l = world.lane(p.from_lane);
                behind.insert(
                    0,
                    (
                        l.id,
                        (l.length_m - (lead - from.length_m)).max(0.0),
                        l.length_m,
                    ),
                );
            }
        }
        let mut heads: Vec<Vec<Piece>> = Vec::new();
        extend_ahead(
            world,
            vec![
                (via, 0.0, connector.length_m),
                (to.id, 0.0, to.length_m.min(lead)),
            ],
            to.length_m,
            lead,
            &mut heads,
        );
        let mut worst: Option<(f64, f64)> = None;
        for head in heads {
            let mut path = behind.clone();
            path.extend(head);
            if let Some(excess) = worst_turn_excess(world, &path, params) {
                if worst.is_none_or(|w| excess.0 > w.0) {
                    worst = Some(excess);
                }
            }
        }
        if let Some((excess, at_s)) = worst {
            if excess > 0.0 && flagged.insert(via) {
                report.fail(name, keep, || {
                    let p = connector.start();
                    format!(
                        "connector {} (lane {} -> {}, {:?}, {:.1} m) turns {:.1}° too fast {:.1} m along; ({:.1}, {:.1})",
                        via.index(),
                        from.id.index(),
                        to.id.index(),
                        c.direction,
                        connector.length_m,
                        excess.to_degrees(),
                        at_s,
                        p.x,
                        p.y
                    )
                });
            }
        }
    }
}

/// Every continuation of `path` until it has run `lead` metres past the connector:
/// `run` is how far past it the path already reaches.
fn extend_ahead(world: &World, path: Vec<Piece>, run: f64, lead: f64, out: &mut Vec<Vec<Piece>>) {
    if run >= lead || out.len() > 32 {
        out.push(path);
        return;
    }
    let last = path.last().map(|p| p.0).expect("a path has a piece");
    let next: Vec<LaneId> = world
        .successors(last)
        .iter()
        .filter(|c| c.permitted)
        .map(|c| c.via.unwrap_or(c.to_lane))
        .filter(|l| world.lane(*l).admits(ClassMask::CAR))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if next.is_empty() {
        out.push(path);
        return;
    }
    for n in next {
        let lane = world.lane(n);
        let take = lane.length_m.min(lead - run);
        let mut p = path.clone();
        p.push((n, 0.0, take));
        extend_ahead(world, p, run + take, lead, out);
    }
}

/// The largest amount, radians, by which a car's body heading along `path` turns faster
/// than [`ValidationParams::min_path_radius_m`] allows, and where (metres along the path);
/// `None` when the path is too short to hold the body.
fn worst_turn_excess(
    world: &World,
    path: &[Piece],
    params: &ValidationParams,
) -> Option<(f64, f64)> {
    let total: f64 = path.iter().map(|p| p.2 - p.1).sum();
    if total < params.body_length_m + 1.0 {
        return None;
    }
    let point = |s: f64| -> Vec3 {
        let mut left = s;
        for (lane, a, b) in path {
            let len = b - a;
            if left <= len {
                return world.lane(*lane).point_at(a + left);
            }
            left -= len;
        }
        let (lane, _, b) = path[path.len() - 1];
        world.lane(lane).point_at(b)
    };
    let heading = |front: f64| -> f64 {
        let f = point(front);
        let r = point(front - params.body_length_m);
        math::atan2(f.y - r.y, f.x - r.x)
    };
    let mut worst: Option<(f64, f64)> = None;
    for step in [0.5, 1.0] {
        let bound = step / params.min_path_radius_m + params.heading_slack_rad;
        let mut s = params.body_length_m;
        let mut h = heading(s);
        while s + step <= total {
            let h2 = heading(s + step);
            let excess = normalise_angle(h2 - h).abs() - bound;
            if worst.is_none_or(|w| excess > w.0) {
                worst = Some((excess, s));
            }
            s += 0.25;
            h = heading(s);
        }
    }
    worst
}

// ---------------------------------------------------------------------------------------
// Stubs, and cars in buildings
// ---------------------------------------------------------------------------------------

fn check_stubs_and_buildings(
    world: &World,
    params: &ValidationParams,
    report: &mut ValidationReport,
) {
    let keep = params.examples;
    report.entry("stub-driving-lane");
    report.entry("vehicle-envelope-in-building");
    for lane in world.roads.lanes() {
        if !is_carriageway(lane) {
            continue;
        }
        report.entry("stub-driving-lane").of += 1;
        report.entry("vehicle-envelope-in-building").of += 1;
        if lane.length_m < params.stub_lane_m {
            report.fail("stub-driving-lane", keep, || {
                let p = lane.start();
                format!(
                    "lane {} is {:.2} m long, at ({:.1}, {:.1})",
                    lane.id.index(),
                    lane.length_m,
                    p.x,
                    p.y
                )
            });
        }
        let samples = (lane.length_m / 1.0).ceil().max(1.0) as usize;
        let mut hit: Option<(u32, Vec3)> = None;
        let mut metres = 0.0;
        for k in 0..=samples {
            let s = lane.length_m * k as f64 / samples as f64;
            let mut inside = false;
            for d in [-params.car_half_width_m, 0.0, params.car_half_width_m] {
                let p = lane.offset_point(s, d);
                for b in world.buildings_in_bbox(v2xw_core::geom::Bbox::new(p, p)) {
                    if world.is_passage(lane.id, b) {
                        continue;
                    }
                    if world
                        .building(b)
                        .is_some_and(|bl| bl.contains_2d(p) && road_meets_building(bl, p.z))
                    {
                        inside = true;
                        if hit.is_none() {
                            hit = Some((b.index(), p));
                        }
                    }
                }
            }
            if inside {
                metres += lane.length_m / samples as f64;
            }
        }
        if let Some((b, p)) = hit {
            report.entry("vehicle-envelope-in-building").metres += metres;
            report.fail("vehicle-envelope-in-building", keep, || {
                format!(
                    "lane {} enters building {b} over {metres:.1} m, at ({:.1}, {:.1})",
                    lane.id.index(),
                    p.x,
                    p.y
                )
            });
        }
    }
}

/// `signal-movement-never-green` and `signal-group-never-green`: every movement a signal
/// plan controls, and every head group it shows, gets a green somewhere in the cycle.
fn check_signals(world: &World, params: &ValidationParams, report: &mut ValidationReport) {
    use crate::model::SignalState;
    let keep = params.examples;
    report.entry("signal-movement-never-green");
    report.entry("signal-group-never-green");
    let go = |s: SignalState| matches!(s, SignalState::Green | SignalState::GreenYield);
    for plan in &world.signals {
        for (i, lane) in plan.controlled.iter().enumerate() {
            report.entry("signal-movement-never-green").of += 1;
            if !plan
                .phases
                .iter()
                .any(|p| p.states.get(i).copied().is_some_and(go))
            {
                report.fail("signal-movement-never-green", keep, || {
                    let kind = world.try_lane(*lane).map_or("?", |l| l.kind.wire_name());
                    format!(
                        "plan {} (junction {}) movement {i}: {kind} lane {} is red all cycle",
                        plan.id.index(),
                        plan.junction.index(),
                        lane.index()
                    )
                });
            }
        }
    }
    for g in world.group_signals() {
        report.entry("signal-group-never-green").of += 1;
        if !g.timeline.iter().any(|(s, _)| go(*s)) {
            report.fail("signal-group-never-green", keep, || {
                format!(
                    "group {} (plan {}, group {}) shows {:?} all cycle",
                    g.wire_id,
                    (g.wire_id >> 16).wrapping_sub(1),
                    g.wire_id & 0xFFFF,
                    g.timeline.iter().map(|(s, _)| *s).collect::<Vec<_>>()
                )
            });
        }
    }
}

fn figures(world: &World, report: &mut ValidationReport) {
    for kind in LaneKind::ALL {
        let (n, m) = world
            .roads
            .lanes()
            .iter()
            .filter(|l| l.kind == kind)
            .fold((0u64, 0.0f64), |(n, m), l| (n + 1, m + l.length_m));
        report
            .figures
            .insert(format!("lanes.{}.count", kind.wire_name()), n as f64);
        report.figures.insert(
            format!("lanes.{}.km", kind.wire_name()),
            (m / 1000.0 * 100.0).round() / 100.0,
        );
    }
    let mut cycles: BTreeMap<u64, u64> = BTreeMap::new();
    for plan in &world.signals {
        *cycles.entry(plan.cycle_s.round() as u64).or_default() += 1;
    }
    for (c, n) in cycles {
        report
            .figures
            .insert(format!("signals.cycle_{c:03}s"), n as f64);
    }
    // Coordination and the major phase's share of the cycle (its first green).
    let offset = world.signals.iter().filter(|p| p.offset_s != 0.0).count();
    report
        .figures
        .insert("signals.offset_nonzero".to_string(), offset as f64);
    let shares: Vec<f64> = world
        .signals
        .iter()
        .filter(|p| p.cycle_s > 0.0)
        .filter_map(|p| p.phases.first().map(|g| g.duration_s / p.cycle_s))
        .collect();
    if !shares.is_empty() {
        let mean = shares.iter().sum::<f64>() / shares.len() as f64;
        report.figures.insert(
            "signals.first_green_share_mean".to_string(),
            (mean * 1000.0).round() / 1000.0,
        );
    }
    report.figures.insert(
        "buildings.with_holes".to_string(),
        world
            .buildings
            .iter()
            .filter(|b| !b.holes.is_empty())
            .count() as f64,
    );
}

// ---------------------------------------------------------------------------------------
// Source fidelity
// ---------------------------------------------------------------------------------------

/// What one direction of a way should carry, read from its tags.
#[derive(Debug, Clone, Default, PartialEq)]
struct Expected {
    /// Driving + bus lanes, when the tags say.
    lanes: Option<u32>,
    /// Bus lanes, when the tags say.
    bus: Option<u32>,
}

fn is_yes(v: Option<&str>) -> bool {
    matches!(v, Some("yes" | "true" | "1"))
}

fn count_of(tags: &Tags, key: &str) -> Option<u32> {
    tags.get(key).and_then(|v| v.trim().parse::<u32>().ok())
}

/// `oneway` read independently of the importer: `Some(true)` forward only, `Some(false)`
/// backward only, `None` both ways.
fn oneway_of(tags: &Tags) -> Option<bool> {
    match tags.get("oneway") {
        Some("yes" | "true" | "1" | "reversible" | "alternating") => Some(true),
        Some("-1" | "reverse") => Some(false),
        Some(_) => None,
        None => {
            let h = tags.get("highway").unwrap_or("");
            let roundabout = matches!(tags.get("junction"), Some("roundabout" | "circular"));
            (h == "motorway" || roundabout).then_some(true)
        }
    }
}

/// How many entries of a `|`-separated per-lane list say `designated`.
fn designated_in(list: &str) -> u32 {
    list.split('|').filter(|t| t.trim() == "designated").count() as u32
}

fn expected(tags: &Tags, forward: bool) -> Expected {
    let dir = if forward { "forward" } else { "backward" };
    let oneway = oneway_of(tags);
    let total = count_of(tags, "lanes");
    let lanes = match oneway {
        Some(f) if f == forward => count_of(tags, &format!("lanes:{dir}")).or(total),
        Some(_) => Some(0),
        None => count_of(tags, &format!("lanes:{dir}")).or_else(|| {
            let other = if forward { "backward" } else { "forward" };
            match (total, count_of(tags, &format!("lanes:{other}"))) {
                (Some(t), Some(o)) => Some(t.saturating_sub(o)),
                (Some(t), None) => Some(if forward { t.div_ceil(2) } else { t / 2 }.max(1)),
                _ => None,
            }
        }),
    };
    let bus = if oneway.is_some_and(|f| f != forward) {
        Some(0)
    } else {
        count_of(tags, &format!("lanes:bus:{dir}"))
            .or_else(|| count_of(tags, &format!("lanes:psv:{dir}")))
            .or_else(|| tags.get(&format!("bus:lanes:{dir}")).map(designated_in))
            .or_else(|| tags.get(&format!("psv:lanes:{dir}")).map(designated_in))
            .or_else(|| {
                if oneway.is_some() {
                    count_of(tags, "lanes:bus")
                        .or_else(|| count_of(tags, "lanes:psv"))
                        .or_else(|| tags.get("bus:lanes").map(designated_in))
                        .or_else(|| tags.get("psv:lanes").map(designated_in))
                } else {
                    // A two-way road's undirected count: half each way, when it halves.
                    count_of(tags, "lanes:bus")
                        .or_else(|| count_of(tags, "lanes:psv"))
                        .filter(|t| t % 2 == 0)
                        .map(|t| t / 2)
                }
            })
    };
    // A per-lane bus list longer than `lanes` by exactly its `designated` entries means
    // `lanes` counted the general lanes only (Portland's Transit Mall): the list is the
    // street, as the importer reads it.
    let lanes = lanes.map(|n| {
        let keys: Vec<String> = if oneway.is_some() {
            vec![
                "bus:lanes".into(),
                "psv:lanes".into(),
                format!("bus:lanes:{dir}"),
                format!("psv:lanes:{dir}"),
            ]
        } else {
            vec![format!("bus:lanes:{dir}"), format!("psv:lanes:{dir}")]
        };
        if n == 0 {
            return 0;
        }
        for key in &keys {
            if let Some(raw) = tags.get(key) {
                let len = raw.split('|').count() as u32;
                if len > n && len - n == designated_in(raw) {
                    return len;
                }
            }
        }
        n
    });
    Expected { lanes, bus }
}

/// The sides (`left`, `right`) on which a tag family places a lane, for a way whose
/// travel direction the caller gives.
fn tagged_sides(tags: &Tags, family: &str, values: &[&str]) -> BTreeSet<&'static str> {
    let mut out = BTreeSet::new();
    let hit = |k: String| tags.get(&k).is_some_and(|v| values.contains(&v));
    if hit(family.to_string()) || hit(format!("{family}:both")) {
        out.insert("left");
        out.insert("right");
    }
    if hit(format!("{family}:left")) {
        out.insert("left");
    }
    if hit(format!("{family}:right")) {
        out.insert("right");
    }
    out
}

/// The heading of `way` near `p`, radians, from its projected nodes.
fn way_heading_near(world: &World, file: &OsmFile, way: i64, p: Vec3) -> Option<f64> {
    let w = file.way(way)?;
    let proj = world.projection();
    let pts: Vec<Vec3> = w
        .nodes
        .iter()
        .filter_map(|n| file.node(*n))
        .map(|n| proj.to_enu_vec3(n.lat, n.lon, 0.0))
        .collect();
    let mut best: Option<(f64, f64)> = None;
    for s in pts.windows(2) {
        if s[0].distance_2d(s[1]) < 1e-6 {
            continue;
        }
        let d = segment_distance(p, s[0], s[1]);
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, math::atan2(s[1].y - s[0].y, s[1].x - s[0].x)));
        }
    }
    best.map(|b| b.1)
}

/// The way's projected polyline.
fn way_points(world: &World, file: &OsmFile, way: i64) -> Vec<Vec3> {
    let proj = world.projection();
    file.way(way)
        .map(|w| {
            w.nodes
                .iter()
                .filter_map(|n| file.node(*n))
                .map(|n| proj.to_enu_vec3(n.lat, n.lon, 0.0))
                .collect()
        })
        .unwrap_or_default()
}

/// Signed lateral offset of `p` from the polyline, positive to the left of its direction.
fn signed_offset(points: &[Vec3], p: Vec3) -> Option<f64> {
    let mut best: Option<(f64, f64)> = None;
    for s in points.windows(2) {
        let (dx, dy) = (s[1].x - s[0].x, s[1].y - s[0].y);
        if dx * dx + dy * dy < 1e-12 {
            continue;
        }
        let d = segment_distance(p, s[0], s[1]);
        let cross = dx * (p.y - s[0].y) - dy * (p.x - s[0].x);
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, if cross >= 0.0 { d } else { -d }));
        }
    }
    best.map(|b| b.1)
}

fn parse_speed_mps(v: &str) -> Option<f64> {
    let v = v.trim();
    if let Some(mph) = v.strip_suffix("mph") {
        return mph.trim().parse::<f64>().ok().map(|x| x * 0.44704);
    }
    if let Some(kmh) = v.strip_suffix("km/h") {
        return kmh.trim().parse::<f64>().ok().map(|x| x / 3.6);
    }
    v.parse::<f64>().ok().map(|x| x / 3.6)
}

fn parse_metres(v: &str) -> Option<f64> {
    let v = v.trim();
    if let Some((ft, rest)) = v.split_once('\'') {
        let inches = rest.trim_end_matches('"').trim();
        let inches = if inches.is_empty() {
            0.0
        } else {
            inches.parse::<f64>().ok()?
        };
        return Some((ft.trim().parse::<f64>().ok()? * 12.0 + inches) * 0.0254);
    }
    let v = v.trim_end_matches('m').trim();
    v.parse::<f64>().ok()
}

#[allow(clippy::too_many_lines)]
fn check_source(
    world: &World,
    link: SourceLink<'_>,
    params: &ValidationParams,
    report: &mut ValidationReport,
) {
    let keep = params.examples;
    let file = link.file;
    for name in [
        "lanes-mismatch",
        "oneway-mismatch",
        "turn-lanes-violated",
        "bus-lanes-mismatch",
        "cycle-lane-missing",
        "parking-lane-missing",
        "width-mismatch",
        "speed-mismatch",
        "crossing-node-without-crosswalk",
    ] {
        report.entry(name);
    }
    // Carriageway edges per way, with the direction each runs relative to the way's nodes,
    // and the side lanes per way.
    let mut carriageways: BTreeMap<i64, Vec<(EdgeId, bool)>> = BTreeMap::new();
    let mut sides: BTreeMap<i64, Vec<EdgeId>> = BTreeMap::new();
    for edge in world.roads.edges() {
        let Some(Some(src)) = link.edges.get(edge.id.as_usize()) else {
            continue;
        };
        if edge.lanes.is_empty() {
            continue;
        }
        match src.role {
            EdgeRole::Side => sides.entry(src.way).or_default().push(edge.id),
            EdgeRole::Carriageway => {
                // Every lane of the way, for the side checks; the direction checks only
                // look at edges that carry traffic (not a contraflow cycle lane's own edge).
                sides.entry(src.way).or_default().push(edge.id);
                let Some(lane) = edge
                    .lanes
                    .iter()
                    .map(|l| world.lane(*l))
                    .find(|l| is_carriageway(l))
                else {
                    continue;
                };
                let mid = lane.point_at(0.5 * lane.length_m);
                let h = lane.heading_at(0.5 * lane.length_m);
                let Some(wh) = way_heading_near(world, file, src.way, mid) else {
                    continue;
                };
                let forward = normalise_angle(h - wh).abs() < core::f64::consts::FRAC_PI_2;
                carriageways
                    .entry(src.way)
                    .or_default()
                    .push((edge.id, forward));
            }
        }
    }
    for (way_id, edges) in &carriageways {
        let Some(way) = file.way(*way_id) else {
            continue;
        };
        let tags = &way.tags;
        let motor_lanes = |e: EdgeId| -> Vec<&Lane> {
            world
                .edge(e)
                .lanes
                .iter()
                .map(|l| world.lane(*l))
                .filter(|l| is_carriageway(l))
                .collect()
        };
        // One-way.
        let has_fwd = edges.iter().any(|(_, f)| *f);
        let has_bwd = edges.iter().any(|(_, f)| !*f);
        report.entry("oneway-mismatch").of += 1;
        let ok = match oneway_of(tags) {
            Some(true) => has_fwd && !has_bwd,
            Some(false) => has_bwd && !has_fwd,
            None => has_fwd && has_bwd,
        };
        if !ok {
            report.fail("oneway-mismatch", keep, || {
                format!(
                    "way {way_id} oneway={:?}: forward edges {has_fwd}, backward {has_bwd}",
                    tags.get("oneway")
                )
            });
        }
        // Lanes and bus lanes, per direction, on every edge of the way.
        for (e, forward) in edges {
            let exp = expected(tags, *forward);
            let lanes = motor_lanes(*e);
            if let Some(n) = exp.lanes {
                report.entry("lanes-mismatch").of += 1;
                // A two-way street keeps a lane each way whatever `lanes=1` says; the
                // importer's documented rule, so not a mismatch.
                let n = if oneway_of(tags).is_none() {
                    n.max(1)
                } else {
                    n
                };
                if lanes.len() as u32 != n.min(8) {
                    report.fail("lanes-mismatch", keep, || {
                        format!(
                            "way {way_id} {}: tags say {n}, edge {} has {}",
                            if *forward { "forward" } else { "backward" },
                            e.index(),
                            lanes.len()
                        )
                    });
                }
            }
            let bus = lanes.iter().filter(|l| l.kind == LaneKind::Bus).count() as u32;
            let busway = tags.get("busway").is_some_and(|v| v == "lane")
                || tags.get("busway:right").is_some_and(|v| v == "lane");
            let want_bus = exp.bus.or(busway.then_some(1));
            if want_bus.is_some() || bus > 0 {
                report.entry("bus-lanes-mismatch").of += 1;
                if want_bus.unwrap_or(0) != bus {
                    report.fail("bus-lanes-mismatch", keep, || {
                        format!(
                            "way {way_id} {}: tags say {:?}, edge {} has {bus}",
                            if *forward { "forward" } else { "backward" },
                            want_bus,
                            e.index()
                        )
                    });
                }
            }
            // Speed.
            if let Some(v) = tags.get("maxspeed").and_then(parse_speed_mps) {
                report.entry("speed-mismatch").of += 1;
                if lanes.iter().any(|l| (l.speed_limit_mps - v).abs() > 0.05) {
                    report.fail("speed-mismatch", keep, || {
                        format!(
                            "way {way_id}: maxspeed {v:.2} m/s, lane limit {:.2}",
                            lanes[0].speed_limit_mps
                        )
                    });
                }
            }
            // Turn lanes: every movement from lane k must be one its entry allows.
            let key = if oneway_of(tags).is_some() {
                "turn:lanes"
            } else if *forward {
                "turn:lanes:forward"
            } else {
                "turn:lanes:backward"
            };
            let far = link
                .edges
                .get(e.as_usize())
                .and_then(|s| *s)
                .map_or(*way_id, |s| s.far_way);
            let far_tags = file.way(far).map(|w| &w.tags);
            let raw = far_tags.and_then(|t| {
                t.get(key).or_else(|| {
                    if oneway_of(t) == Some(*forward) {
                        t.get(if *forward {
                            "turn:lanes:forward"
                        } else {
                            "turn:lanes:backward"
                        })
                    } else {
                        None
                    }
                })
            });
            if let Some(raw) = raw {
                let mut entries: Vec<&str> = raw.split('|').collect();
                entries.reverse(); // rightmost first, as lane index 0 is
                // Only the lanes the tag describes: bus lanes are in `turn:lanes` too.
                let all: Vec<&Lane> = world
                    .edge(*e)
                    .lanes
                    .iter()
                    .map(|l| world.lane(*l))
                    .filter(|l| l.kind.is_motorised() && l.kind != LaneKind::Parking)
                    .collect();
                if entries.len() == all.len() {
                    for (k, lane) in all.iter().enumerate() {
                        let allowed: Vec<&str> = entries[k].split(';').map(str::trim).collect();
                        if allowed.iter().all(|t| t.is_empty() || *t == "none") {
                            continue;
                        }
                        report.entry("turn-lanes-violated").of += 1;
                        for c in world.successors(lane.id) {
                            if !c.permitted || c.via.is_none() {
                                continue;
                            }
                            let ok = allowed.iter().any(|t| match *t {
                                "through" | "merge_to_left" | "merge_to_right" | "" | "none" => {
                                    matches!(
                                        c.direction,
                                        TurnDirection::Straight
                                            | TurnDirection::SlightLeft
                                            | TurnDirection::SlightRight
                                    )
                                }
                                "left" | "sharp_left" | "slight_left" => matches!(
                                    c.direction,
                                    TurnDirection::Left | TurnDirection::SlightLeft
                                ),
                                "right" | "sharp_right" | "slight_right" => matches!(
                                    c.direction,
                                    TurnDirection::Right | TurnDirection::SlightRight
                                ),
                                "reverse" => c.direction == TurnDirection::UTurn,
                                _ => true,
                            });
                            if !ok {
                                report.fail("turn-lanes-violated", keep, || {
                                    format!(
                                        "way {far} lane {} ({k} from the right) tagged {:?} makes a {:?}",
                                        lane.id.index(),
                                        entries[k],
                                        c.direction
                                    )
                                });
                                break;
                            }
                        }
                    }
                }
            }
        }
        // Width: the tag is the carriageway, kerb to kerb.
        if let Some(w) = tags.get("width").and_then(parse_metres) {
            if w > 2.0 && w < 60.0 {
                report.entry("width-mismatch").of += 1;
                // Kerb to kerb: general, bus and parking lanes and painted cycle lanes; a
                // cycle track is beyond the kerb.
                let pts = way_points(world, file, *way_id);
                let tracks = tagged_sides(tags, "cycleway", &["track", "opposite_track"]);
                // Per piece of the way (the edges between one pair of junctions): a way split
                // at three junctions has three pieces, and summing all of them tripled it.
                let mut pieces: BTreeMap<(JunctionId, JunctionId), f64> = BTreeMap::new();
                for e in sides.get(way_id).into_iter().flatten() {
                    let edge = world.edge(*e);
                    let key = (edge.from.min(edge.to), edge.from.max(edge.to));
                    let built = pieces.entry(key).or_insert(0.0);
                    for l in &edge.lanes {
                        let l = world.lane(*l);
                        let on = match l.kind {
                            LaneKind::Driving | LaneKind::Bus | LaneKind::Parking => true,
                            LaneKind::Cycle => signed_offset(&pts, l.point_at(0.5 * l.length_m))
                                .is_some_and(|off| {
                                    !tracks.contains(if off >= 0.0 { "left" } else { "right" })
                                }),
                            _ => false,
                        };
                        if on {
                            *built += l.width_m;
                        }
                    }
                }
                let built = pieces.values().copied().fold(0.0f64, f64::max);
                if (built - w).abs() > 0.15 * w {
                    report.fail("width-mismatch", keep, || {
                        format!("way {way_id}: width tag {w:.1} m, built {built:.1} m")
                    });
                }
            }
        }
        // Cycle and parking lanes, by side.
        let pts = way_points(world, file, *way_id);
        let side_kinds = |kind: LaneKind| -> BTreeSet<&'static str> {
            let mut out = BTreeSet::new();
            for e in sides.get(way_id).into_iter().flatten() {
                for l in &world.edge(*e).lanes {
                    let lane = world.lane(*l);
                    if lane.kind != kind {
                        continue;
                    }
                    if let Some(off) = signed_offset(&pts, lane.point_at(0.5 * lane.length_m)) {
                        out.insert(if off >= 0.0 { "left" } else { "right" });
                    }
                }
            }
            out
        };
        let cycle_values = ["lane", "track", "opposite_lane", "opposite_track"];
        let mut want_cycle = tagged_sides(tags, "cycleway", &cycle_values);
        // A bare `cycleway=lane` on a one-way street names one lane and no side (OSM wiki,
        // Key:cycleway): one on either side satisfies it.
        let sided = ["cycleway:left", "cycleway:right", "cycleway:both"]
            .iter()
            .any(|k| tags.get(k).is_some_and(|v| cycle_values.contains(&v)));
        let either = oneway_of(tags).is_some() && !sided && !want_cycle.is_empty();
        if either {
            want_cycle = BTreeSet::from(["either"]);
        }
        if !want_cycle.is_empty() {
            let mut have = side_kinds(LaneKind::Cycle);
            if either && !have.is_empty() {
                have.insert("either");
            }
            for side in &want_cycle {
                report.entry("cycle-lane-missing").of += 1;
                if !have.contains(side) {
                    report.fail("cycle-lane-missing", keep, || {
                        format!("way {way_id}: cycle lane tagged on the {side}, none built")
                    });
                }
            }
        }
        let mut want_parking = tagged_sides(
            tags,
            "parking",
            &[
                "lane",
                "yes",
                "street_side",
                "on_street",
                "half_on_kerb",
                "on_kerb",
                "parallel",
                "diagonal",
                "perpendicular",
            ],
        );
        want_parking.extend(tagged_sides(
            tags,
            "parking:lane",
            &["parallel", "diagonal", "perpendicular", "marked"],
        ));
        if !want_parking.is_empty() {
            let have = side_kinds(LaneKind::Parking);
            for side in &want_parking {
                report.entry("parking-lane-missing").of += 1;
                if !have.contains(side) {
                    report.fail("parking-lane-missing", keep, || {
                        format!("way {way_id}: parking tagged on the {side}, none built")
                    });
                }
            }
        }
    }
    // Crossing nodes on roads.
    let mut on_road: BTreeSet<i64> = BTreeSet::new();
    for way in &file.ways {
        let motor = way.tags.get("highway").is_some_and(|h| {
            !matches!(
                h,
                "footway"
                    | "pedestrian"
                    | "path"
                    | "cycleway"
                    | "steps"
                    | "corridor"
                    | "track"
                    | "bridleway"
                    | "construction"
                    | "proposed"
                    | "platform"
            )
        });
        if motor {
            on_road.extend(way.nodes.iter().copied());
        }
    }
    let crossings: Vec<&Lane> = world
        .roads
        .lanes()
        .iter()
        .filter(|l| l.kind == LaneKind::Crossing)
        .collect();
    let proj = world.projection();
    let bbox = world.bbox;
    for (id, tags) in &file.node_tags {
        if !(tags.is("highway", "crossing") || tags.has("crossing")) || !on_road.contains(id) {
            continue;
        }
        if tags.is("crossing", "no") || tags.is("crossing", "impassable") {
            continue;
        }
        let Some(n) = file.node(*id) else { continue };
        let p = proj.to_enu_vec3(n.lat, n.lon, 0.0);
        if !bbox.contains_2d(p) {
            continue;
        }
        report.entry("crossing-node-without-crosswalk").of += 1;
        let near = crossings.iter().any(|l| {
            l.centreline
                .windows(2)
                .any(|s| segment_distance(p, s[0], s[1]) < params.crossing_node_radius_m)
        });
        if !near {
            report.fail("crossing-node-without-crosswalk", keep, || {
                format!("node {id} at ({:.1}, {:.1})", p.x, p.y)
            });
        }
    }
    let _ = is_yes;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tag_reader_splits_lanes_by_direction() {
        let t = Tags::from_pairs([
            ("highway", "secondary"),
            ("lanes", "5"),
            ("lanes:bus", "1"),
            ("oneway", "yes"),
        ]);
        assert_eq!(expected(&t, true).lanes, Some(5));
        assert_eq!(expected(&t, true).bus, Some(1));
        assert_eq!(expected(&t, false).lanes, Some(0));
        let two = Tags::from_pairs([("highway", "residential"), ("lanes", "3")]);
        assert_eq!(expected(&two, true).lanes, Some(2));
        assert_eq!(expected(&two, false).lanes, Some(1));
        let bl = Tags::from_pairs([("oneway", "yes"), ("bus:lanes", "yes|yes|designated")]);
        assert_eq!(expected(&bl, true).bus, Some(1));
    }

    #[test]
    fn imperial_widths_and_speeds_parse() {
        assert!((parse_metres("32'0\"").unwrap() - 9.7536).abs() < 1e-9);
        assert!((parse_metres("11'").unwrap() - 3.3528).abs() < 1e-9);
        assert!((parse_metres("21.3").unwrap() - 21.3).abs() < 1e-12);
        assert!((parse_speed_mps("25 mph").unwrap() - 11.176).abs() < 1e-9);
        assert!((parse_speed_mps("50").unwrap() - 13.888_888_888_888_89).abs() < 1e-9);
    }
}
