//! NEMA phase numbers for a junction's movements, from its geometry (WP-17 P17.3).
//!
//! The numbering is the FHWA Signal Timing Manual 2nd ed. (NCHRP Report 812, 2015),
//! §5.1.1 and Exhibit 5-1 (pp. 5-2 to 5-3), on the dual-ring layout of §5.1.2 Exhibit 5-3:
//!
//! * the **main street** carries throughs 2 and 6 and lefts 1 and 5, the side street
//!   throughs 4 and 8 and lefts 3 and 7;
//! * ring 1 is {1, 2, 3, 4} and ring 2 {5, 6, 7, 8}; the barrier separates {1, 2, 5, 6}
//!   from {3, 4, 7, 8};
//! * a left turn takes the number that runs beside the opposing through, so the left from
//!   phase 2's approach is 5 and the left from phase 6's is 1 (and 7 beside 4, 3 beside 8):
//!   then 1+6 and 2+5 are a left with the through of its own approach, and 1+5 the two
//!   opposing lefts, which is what lets the rings time concurrently;
//! * a right turn runs with its approach's through;
//! * a pedestrian phase "is generally assigned the same phase number as the adjacent,
//!   parallel vehicular phase" (§5.1.1): a crosswalk walks parallel to one street and
//!   lies on the right-hand side of one of its throughs, whose number it takes.
//!
//! # Choices this build makes (the STM leaves them open)
//!
//! * **Main street**: the axis with the higher road class, then the more approach lanes
//!   (the build plan), then the more green in the junction's own plan (the timing the
//!   source gave the junction already says which street it favours), then the lower edge
//!   id, so the answer is total and deterministic.
//! * **Which through is 2 (or 4)**: the approach travelling more nearly north-east
//!   (larger `cos(heading − 45°)`), so eastbound and northbound are 2 and 4 on a compass
//!   grid. The STM numbers by convention only; any fixed rule serves.
//! * **Axes**: two approaches are one street when their headings are opposite within 45°.
//!   At most two streets may meet, at least 45° apart; anything else (a five-way, a Y, an
//!   offset T whose legs pair with nothing) is [`Unmapped`] and falls back to the
//!   single-ring controller, counted.
//! * **A left with no opposing through** (a one-way street, the stem of a T, a one-way
//!   pair) has no one to yield to, so it rides its own through's phase instead of taking a
//!   left phase.
//! * **A crosswalk across a two-approach street with no cross street** (a mid-block or
//!   one-axis signal) is served by a pedestrian-only side phase, 4 or 8 by the same
//!   right-hand-side rule against a virtual cross street.

use std::collections::BTreeMap;

use v2xw_core::ids::LaneId;
use v2xw_world::model::{Junction, LaneKind, RoadClass, TurnDirection};
use v2xw_world::{SignalPlan, SignalState, World};

/// What a controlled movement is, for the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementKind {
    /// Straight on (slight turns count as through).
    Through,
    /// A left turn or U-turn.
    Left,
    /// A right turn.
    Right,
    /// A pedestrian crosswalk.
    Crossing,
}

/// One controlled movement's NEMA phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignedMovement {
    /// The NEMA phase number, 1-8, that serves it.
    pub phase: u8,
    /// What it is.
    pub kind: MovementKind,
    /// For a left turn with its own phase: the through phase of the same approach, in
    /// whose green it may turn permissively (yielding to the opposing through). `None`
    /// for every other movement.
    pub permissive_with: Option<u8>,
}

/// A junction's NEMA phase assignment: one entry per [`SignalPlan::controlled`] movement.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// Parallel to the plan's `controlled` list.
    pub movements: Vec<AssignedMovement>,
}

/// Why a junction has no dual-ring assignment (it keeps the single-ring controller).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unmapped {
    /// The plan controls no vehicle approach.
    NoApproaches,
    /// More than four approaches.
    TooManyApproaches(usize),
    /// The approaches do not form at most two streets crossing at 45° or more.
    NotTwoStreets,
    /// A controlled movement has no approach lane and is not a crosswalk (its index).
    UnknownMovement(usize),
    /// A crosswalk parallel to neither street (its index).
    SkewedCrossing(usize),
}

impl Unmapped {
    /// A short label for counting.
    pub const fn label(self) -> &'static str {
        match self {
            Unmapped::NoApproaches => "no-approaches",
            Unmapped::TooManyApproaches(_) => "too-many-approaches",
            Unmapped::NotTwoStreets => "not-two-streets",
            Unmapped::UnknownMovement(_) => "unknown-movement",
            Unmapped::SkewedCrossing(_) => "skewed-crossing",
        }
    }
}

/// One approach (an incoming edge) of a junction, for [`assign_approaches`].
#[derive(Debug, Clone, PartialEq)]
pub struct Approach {
    /// A stable key, the approach edge's id: the last tie-break.
    pub key: u32,
    /// The direction of travel at the stop line, degrees clockwise from north.
    pub heading_deg: f64,
    /// The road's class (lower is a higher class, [`class_rank`]).
    pub class_rank: u8,
    /// Driving lanes on the approach.
    pub lanes: u32,
    /// Seconds of the plan's cycle in which some movement of this approach has a green.
    pub green_s: f64,
    /// Its movements: index into `controlled` and turn.
    pub movements: Vec<(usize, TurnDirection)>,
}

/// One controlled crosswalk, for [`assign_approaches`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrossingLeg {
    /// Its index into `controlled`.
    pub index: usize,
    /// The walking direction, degrees clockwise from north (either way along it).
    pub walk_heading_deg: f64,
    /// Its midpoint relative to the junction's centre, metres (east, north).
    pub offset_m: (f64, f64),
}

/// A road class's rank: 0 is the highest. OSM's `unclassified` is a minor through road,
/// above `residential` (OSM wiki, Key:highway).
pub const fn class_rank(c: RoadClass) -> u8 {
    match c {
        RoadClass::Motorway => 0,
        RoadClass::Trunk => 1,
        RoadClass::Primary => 2,
        RoadClass::Secondary => 3,
        RoadClass::Link | RoadClass::Tertiary => 4,
        RoadClass::Unclassified => 5,
        RoadClass::Residential => 6,
        RoadClass::Living => 7,
        RoadClass::Service => 8,
        RoadClass::Internal | RoadClass::Footway | RoadClass::Cycleway | RoadClass::Path => 9,
    }
}

/// Two approaches are one street when their headings are opposite within this, degrees.
const OPPOSITE_TOL_DEG: f64 = 45.0;

/// The ring of NEMA phase `p` (1 or 2).
pub const fn ring_of(p: u8) -> u8 {
    if p <= 4 { 1 } else { 2 }
}

/// Whether NEMA phase `p` is on the main-street side of the barrier ({1, 2, 5, 6}).
pub const fn main_side(p: u8) -> bool {
    matches!(p, 1 | 2 | 5 | 6)
}

/// Whether phases `p` and `q` may time together: the same phase, or one of each ring on
/// the same side of the barrier (STM 2nd ed. Exhibit 5-3).
pub const fn concurrent(p: u8, q: u8) -> bool {
    p == q || (ring_of(p) != ring_of(q) && main_side(p) == main_side(q))
}

/// The heading difference `a − b` folded into `[0, 360)`.
fn diff_deg(a: f64, b: f64) -> f64 {
    (a - b).rem_euclid(360.0)
}

/// How far two lines' directions are apart, `[0, 90]` degrees.
fn line_angle_deg(a: f64, b: f64) -> f64 {
    let d = diff_deg(a, b) % 180.0;
    d.min(180.0 - d)
}

/// How nearly north-east a heading points: larger is phase 2 (or 4).
fn ne_score(heading_deg: f64) -> f64 {
    (heading_deg - 45.0).to_radians().cos()
}

/// Whether a movement is through-like.
fn kind_of(t: TurnDirection) -> MovementKind {
    match t {
        TurnDirection::Straight | TurnDirection::SlightLeft | TurnDirection::SlightRight => {
            MovementKind::Through
        }
        TurnDirection::Left | TurnDirection::UTurn => MovementKind::Left,
        TurnDirection::Right => MovementKind::Right,
    }
}

/// One street: its approaches in slot order (slot 0 takes through 2 or 4, slot 1 takes
/// 6 or 8), either slot possibly empty.
#[derive(Debug, Clone, Copy)]
struct Street {
    slots: [Option<usize>; 2],
    /// The street's direction, degrees (a heading of one of its approaches).
    line_deg: f64,
}

/// The NEMA phase assignment of one junction from its approaches and crosswalks: the
/// pure core of [`assign_phases`], so generated, one-way and malformed layouts can be
/// tested without a world. `n` is the length of the plan's `controlled` list.
///
/// # Errors
///
/// [`Unmapped`], saying why the junction keeps the single-ring controller.
pub fn assign_approaches(
    n: usize,
    approaches: &[Approach],
    crossings: &[CrossingLeg],
) -> Result<Assignment, Unmapped> {
    if approaches.is_empty() {
        return Err(Unmapped::NoApproaches);
    }
    if approaches.len() > 4 {
        return Err(Unmapped::TooManyApproaches(approaches.len()));
    }
    // Pair opposite approaches, closest to exactly opposite first.
    let mut candidates: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..approaches.len() {
        for j in i + 1..approaches.len() {
            let dev =
                (diff_deg(approaches[i].heading_deg, approaches[j].heading_deg) - 180.0).abs();
            if dev <= OPPOSITE_TOL_DEG {
                candidates.push((dev, i, j));
            }
        }
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut used = vec![false; approaches.len()];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (_, i, j) in candidates {
        if !used[i] && !used[j] {
            used[i] = true;
            used[j] = true;
            groups.push(vec![i, j]);
        }
    }
    for (i, u) in used.iter().enumerate() {
        if !u {
            groups.push(vec![i]);
        }
    }
    if groups.len() > 2 {
        return Err(Unmapped::NotTwoStreets);
    }
    let line = |g: &[usize]| approaches[g[0]].heading_deg;
    if groups.len() == 2 && line_angle_deg(line(&groups[0]), line(&groups[1])) < 45.0 {
        return Err(Unmapped::NotTwoStreets);
    }
    // The main street: higher class, more lanes, more green, lower key.
    let rank = |g: &[usize]| {
        let best = g
            .iter()
            .map(|&i| approaches[i].class_rank)
            .min()
            .unwrap_or(u8::MAX);
        let lanes: u32 = g.iter().map(|&i| approaches[i].lanes).sum();
        let green: f64 = g.iter().map(|&i| approaches[i].green_s).sum();
        let key = g
            .iter()
            .map(|&i| approaches[i].key)
            .min()
            .unwrap_or(u32::MAX);
        (best, lanes, green, key)
    };
    if groups.len() == 2 {
        let (a, b) = (rank(&groups[0]), rank(&groups[1]));
        let b_is_main = b.0 < a.0
            || (b.0 == a.0
                && (b.1 > a.1
                    || (b.1 == a.1
                        && (b.2.total_cmp(&a.2).is_gt()
                            || (b.2.total_cmp(&a.2).is_eq() && b.3 < a.3)))));
        if b_is_main {
            groups.swap(0, 1);
        }
    }
    let street = |g: &[usize]| -> Street {
        let slots = match g {
            [i] => {
                if ne_score(approaches[*i].heading_deg) >= 0.0 {
                    [Some(*i), None]
                } else {
                    [None, Some(*i)]
                }
            }
            [i, j] => {
                if ne_score(approaches[*i].heading_deg) >= ne_score(approaches[*j].heading_deg) {
                    [Some(*i), Some(*j)]
                } else {
                    [Some(*j), Some(*i)]
                }
            }
            _ => [None, None],
        };
        Street {
            slots,
            line_deg: line(g),
        }
    };
    let main = street(&groups[0]);
    let side = groups.get(1).map(|g| street(g));
    // (through, left) numbers per slot: main 2/5 and 6/1, side 4/7 and 8/3.
    const MAIN: [(u8, u8); 2] = [(2, 5), (6, 1)];
    const SIDE: [(u8, u8); 2] = [(4, 7), (8, 3)];
    let mut out: Vec<Option<AssignedMovement>> = vec![None; n];
    let has_through = |i: usize| {
        approaches[i]
            .movements
            .iter()
            .any(|(_, t)| kind_of(*t) == MovementKind::Through)
    };
    for (st, numbers) in [(Some(main), MAIN), (side, SIDE)] {
        let Some(st) = st else { continue };
        for (slot, &(through, left)) in numbers.iter().enumerate() {
            let Some(a) = st.slots[slot] else { continue };
            let opposed = st.slots[1 - slot].is_some_and(has_through);
            for &(m, t) in &approaches[a].movements {
                let kind = kind_of(t);
                let (phase, permissive_with) = if kind == MovementKind::Left && opposed {
                    (left, Some(through))
                } else {
                    (through, None)
                };
                if let Some(slot) = out.get_mut(m) {
                    *slot = Some(AssignedMovement {
                        phase,
                        kind,
                        permissive_with,
                    });
                }
            }
        }
    }
    // Crosswalks take the parallel through on whose right-hand side they lie.
    for c in crossings {
        let parallel = [Some(main), side]
            .into_iter()
            .enumerate()
            .find_map(|(k, s)| {
                s.filter(|s| line_angle_deg(c.walk_heading_deg, s.line_deg) <= 45.0)
                    .map(|s| (k, s))
            });
        let (k, st) = match parallel {
            Some(found) => found,
            // One street and a crosswalk across it: a virtual cross street, both slots
            // pedestrian-only.
            None if side.is_none()
                && line_angle_deg(c.walk_heading_deg, main.line_deg + 90.0) <= 45.0 =>
            {
                (
                    1,
                    Street {
                        slots: [None, None],
                        line_deg: main.line_deg + 90.0,
                    },
                )
            }
            None => return Err(Unmapped::SkewedCrossing(c.index)),
        };
        let numbers = if k == 0 { MAIN } else { SIDE };
        let right_of = |h: f64| {
            let r = h.to_radians();
            c.offset_m.0 * r.cos() - c.offset_m.1 * r.sin()
        };
        // The two travel directions along the street, slot 0 the north-east one.
        let h0 = st.slots[0].map_or_else(
            || {
                let a = st.line_deg.rem_euclid(360.0);
                if ne_score(a) >= ne_score(a + 180.0) {
                    a
                } else {
                    a + 180.0
                }
            },
            |i| approaches[i].heading_deg,
        );
        let h1 = st.slots[1].map_or(h0 + 180.0, |i| approaches[i].heading_deg);
        let slot = match (st.slots[0].is_some(), st.slots[1].is_some()) {
            (true, false) => 0,
            (false, true) => 1,
            _ => usize::from(right_of(h1) > right_of(h0)),
        };
        if let Some(o) = out.get_mut(c.index) {
            *o = Some(AssignedMovement {
                phase: numbers[slot].0,
                kind: MovementKind::Crossing,
                permissive_with: None,
            });
        }
    }
    let mut movements = Vec::with_capacity(n);
    for (i, m) in out.into_iter().enumerate() {
        movements.push(m.ok_or(Unmapped::UnknownMovement(i))?);
    }
    Ok(Assignment { movements })
}

/// The compass heading of travel from `a` to `b`, degrees clockwise from north (world
/// frame: x east, y north).
fn heading_deg(dx: f64, dy: f64) -> f64 {
    dx.atan2(dy).to_degrees().rem_euclid(360.0)
}

/// The NEMA phase assignment of `plan`'s junction, from the world's geometry.
///
/// # Errors
///
/// [`Unmapped`]: the junction keeps the single-ring controller and is counted.
pub fn assign_phases(world: &World, plan: &SignalPlan) -> Result<Assignment, Unmapped> {
    let junction = world.roads.junction(plan.junction);
    // Each internal connector's approach lane and turn, from the junction's own
    // incoming lanes.
    let mut via: BTreeMap<LaneId, (LaneId, TurnDirection)> = BTreeMap::new();
    for &l in &junction.incoming {
        for c in world.roads.successors(l) {
            if let Some(v) = c.via {
                via.entry(v).or_insert((c.from_lane, c.direction));
            }
        }
    }
    let mut by_edge: BTreeMap<u32, Approach> = BTreeMap::new();
    let mut crossings = Vec::new();
    for (m, &l) in plan.controlled.iter().enumerate() {
        let Some(lane) = world.roads.try_lane(l) else {
            return Err(Unmapped::UnknownMovement(m));
        };
        if lane.kind == LaneKind::Crossing {
            let (Some(a), Some(b)) = (lane.centreline.first(), lane.centreline.last()) else {
                return Err(Unmapped::UnknownMovement(m));
            };
            crossings.push(CrossingLeg {
                index: m,
                walk_heading_deg: heading_deg(b.x - a.x, b.y - a.y),
                offset_m: (
                    0.5 * (a.x + b.x) - junction.position.x,
                    0.5 * (a.y + b.y) - junction.position.y,
                ),
            });
            continue;
        }
        let Some(&(from, turn)) = via.get(&l) else {
            return Err(Unmapped::UnknownMovement(m));
        };
        let from_lane = world.roads.lane(from);
        let edge = world.roads.edge(from_lane.edge);
        let entry = by_edge.entry(edge.id.index()).or_insert_with(|| {
            let pts = &from_lane.centreline;
            let (p, q) = if pts.len() >= 2 {
                (pts[pts.len() - 2], pts[pts.len() - 1])
            } else {
                (pts[0], pts[0])
            };
            Approach {
                key: edge.id.index(),
                heading_deg: heading_deg(q.x - p.x, q.y - p.y),
                class_rank: class_rank(edge.road_class),
                lanes: edge
                    .lanes
                    .iter()
                    .filter(|x| world.roads.lane(**x).kind == LaneKind::Driving)
                    .count() as u32,
                green_s: 0.0,
                movements: Vec::new(),
            }
        });
        entry.movements.push((m, turn));
    }
    let mut approaches: Vec<Approach> = by_edge.into_values().collect();
    for a in &mut approaches {
        a.green_s = plan
            .phases
            .iter()
            .filter(|p| {
                a.movements.iter().any(|(m, _)| {
                    matches!(
                        p.states.get(*m),
                        Some(SignalState::Green | SignalState::GreenYield)
                    )
                })
            })
            .map(|p| p.duration_s)
            .sum();
    }
    assign_approaches(plan.controlled.len(), &approaches, &crossings)
}

/// A pair of vehicle movements that the assignment gives protected greens at the same
/// time and the junction's conflict matrix calls foes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcurrentFoe {
    /// The two movements' indices into `controlled`.
    pub a: usize,
    /// The second.
    pub b: usize,
    /// Both are left turns with their own phases from opposite approaches of one street
    /// (1+5 or 3+7): Exhibit 5-3 runs them together, and the generated conflict matrices
    /// record their paths as a level conflict "because no highway code settles it"
    /// (`v2xw_world::procedural::graph::conflict_matrix`).
    pub opposing_lefts: bool,
}

impl Assignment {
    /// The NEMA phases that serve at least one movement, ascending.
    pub fn phases(&self) -> Vec<u8> {
        let mut p: Vec<u8> = self.movements.iter().map(|m| m.phase).collect();
        p.sort_unstable();
        p.dedup();
        p
    }

    /// The movements phase `p` serves.
    pub fn movements_of(&self, p: u8) -> Vec<usize> {
        (0..self.movements.len())
            .filter(|&i| self.movements[i].phase == p)
            .collect()
    }

    /// Every pair of vehicle movements in concurrent phases (protected in each) that
    /// `junction`'s conflict matrix calls foes. Crosswalks are not in the matrix.
    pub fn concurrent_foes(&self, plan: &SignalPlan, junction: &Junction) -> Vec<ConcurrentFoe> {
        let row: Vec<Option<usize>> = plan
            .controlled
            .iter()
            .map(|l| junction.internal.iter().position(|x| x == l))
            .collect();
        let mut out = Vec::new();
        for a in 0..self.movements.len() {
            for b in a + 1..self.movements.len() {
                let (ma, mb) = (self.movements[a], self.movements[b]);
                if ma.kind == MovementKind::Crossing || mb.kind == MovementKind::Crossing {
                    continue;
                }
                let (Some(ra), Some(rb)) = (row[a], row[b]) else {
                    continue;
                };
                if concurrent(ma.phase, mb.phase) && junction.conflicts.is_foe(ra, rb) {
                    let opposing_lefts = ma.phase != mb.phase
                        && ma.permissive_with.is_some()
                        && mb.permissive_with.is_some();
                    out.push(ConcurrentFoe {
                        a,
                        b,
                        opposing_lefts,
                    });
                }
            }
        }
        out
    }
}

/// What [`assign_all`] found over a world.
#[derive(Debug, Clone, Default)]
pub struct AssignReport {
    /// Plan index to its assignment.
    pub assigned: BTreeMap<usize, Assignment>,
    /// Plan index to why it keeps the single-ring controller.
    pub unmapped: BTreeMap<usize, Unmapped>,
}

impl AssignReport {
    /// How many plans fell back, by reason label.
    pub fn fallback_counts(&self) -> BTreeMap<&'static str, usize> {
        let mut c = BTreeMap::new();
        for u in self.unmapped.values() {
            *c.entry(u.label()).or_insert(0) += 1;
        }
        c
    }
}

/// [`assign_phases`] for every plan of `world`.
pub fn assign_all(world: &World) -> AssignReport {
    let mut r = AssignReport::default();
    for (i, plan) in world.signals.iter().enumerate() {
        match assign_phases(world, plan) {
            Ok(a) => {
                r.assigned.insert(i, a);
            }
            Err(u) => {
                r.unmapped.insert(i, u);
            }
        }
    }
    r
}

/// A world of one signalised junction with `arms` (unit direction, class, lanes per
/// direction), each arm 150 m long, from the shared graph builder. For tests.
#[cfg(test)]
pub(crate) fn test_junction(arms: &[((f64, f64), RoadClass, u32)]) -> World {
    use v2xw_core::geom::Vec3;
    use v2xw_world::ImportOptions;
    use v2xw_world::model::{GeoOrigin, WorldProvenance, WorldSourceKind};
    use v2xw_world::procedural::graph::{GraphEdge, GraphOptions, build_lane_graph, into_world};
    let c = Vec3::new_2d(300.0, 300.0);
    let mut nodes = vec![c];
    let mut edges = Vec::new();
    for (k, ((dx, dy), class, lanes)) in arms.iter().enumerate() {
        nodes.push(Vec3::new_2d(c.x + 150.0 * dx, c.y + 150.0 * dy));
        edges.push(
            GraphEdge::new(0, k + 1)
                .with_lanes(*lanes)
                .with_class(*class),
        );
    }
    let graph = build_lane_graph(&nodes, &edges, &GraphOptions::default().with_signals(true))
        .expect("graph");
    let opts = ImportOptions::default();
    let prov = WorldProvenance::new(
        WorldSourceKind::Procedural,
        "nema-test".to_string(),
        opts.imported_at.clone(),
        GeoOrigin::NULL_ISLAND,
    );
    into_world(graph, prov, &opts).expect("world")
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_world::ImportOptions;
    use v2xw_world::procedural::GridParams;

    fn junction(arms: &[((f64, f64), RoadClass, u32)]) -> World {
        test_junction(arms)
    }

    const E: (f64, f64) = (1.0, 0.0);
    const W: (f64, f64) = (-1.0, 0.0);
    const N: (f64, f64) = (0.0, 1.0);
    const S: (f64, f64) = (0.0, -1.0);

    type Table = BTreeMap<(u32, u8), Vec<u8>>;

    fn kind_code(k: MovementKind) -> u8 {
        match k {
            MovementKind::Through => 0,
            MovementKind::Left => 1,
            MovementKind::Right => 2,
            MovementKind::Crossing => 3,
        }
    }

    /// `(approach heading, kind) -> phases` over a world's only plan.
    fn table(world: &World) -> (Assignment, Table) {
        assert_eq!(world.signals.len(), 1, "one signalised junction");
        let plan = &world.signals[0];
        let a = assign_phases(world, plan).expect("maps");
        let junction = world.roads.junction(plan.junction);
        let mut via: BTreeMap<LaneId, LaneId> = BTreeMap::new();
        for &l in &junction.incoming {
            for c in world.roads.successors(l) {
                if let Some(v) = c.via {
                    via.insert(v, c.from_lane);
                }
            }
        }
        let mut t: Table = BTreeMap::new();
        for (m, &l) in plan.controlled.iter().enumerate() {
            let pts = &world.roads.lane(via[&l]).centreline;
            let (p, q) = (pts[pts.len() - 2], pts[pts.len() - 1]);
            let h = (heading_deg(q.x - p.x, q.y - p.y).round() as u32) % 360;
            let e = t.entry((h, kind_code(a.movements[m].kind))).or_default();
            e.push(a.movements[m].phase);
            e.sort_unstable();
            e.dedup();
        }
        (a, t)
    }

    fn only(t: &Table, h: u32, k: MovementKind) -> u8 {
        let v = t
            .get(&(h, kind_code(k)))
            .unwrap_or_else(|| panic!("{h} {k:?} in {t:?}"));
        assert_eq!(v.len(), 1, "{h} {k:?}: {v:?}");
        v[0]
    }

    /// STM Exhibit 5-1 on a crossroads: the primary street takes 2/6 and 1/5, the
    /// residential one 4/8 and 3/7; each left is numbered beside the opposing through
    /// and each right rides its through. Swapping the classes swaps the streets.
    #[test]
    fn a_crossroads_is_numbered_by_road_class() {
        use MovementKind::{Left, Right, Through};
        let ew_main = junction(&[
            (E, RoadClass::Primary, 1),
            (N, RoadClass::Residential, 1),
            (W, RoadClass::Primary, 1),
            (S, RoadClass::Residential, 1),
        ]);
        let (a, t) = table(&ew_main);
        // Headings of travel: eastbound 90, westbound 270, northbound 0, southbound 180.
        assert_eq!(only(&t, 90, Through), 2);
        assert_eq!(only(&t, 90, Left), 5);
        assert_eq!(only(&t, 90, Right), 2);
        assert_eq!(only(&t, 270, Through), 6);
        assert_eq!(only(&t, 270, Left), 1);
        assert_eq!(only(&t, 0, Through), 4);
        assert_eq!(only(&t, 0, Left), 7);
        assert_eq!(only(&t, 180, Through), 8);
        assert_eq!(only(&t, 180, Left), 3);
        assert_eq!(a.phases(), [1, 2, 3, 4, 5, 6, 7, 8]);
        // A left with its own phase may turn permissively beside its own through.
        for m in &a.movements {
            if m.kind == Left {
                let own = match m.phase {
                    5 => 2,
                    1 => 6,
                    7 => 4,
                    _ => 8,
                };
                assert_eq!(m.permissive_with, Some(own), "{m:?}");
            }
        }
        let ns_main = junction(&[
            (E, RoadClass::Residential, 1),
            (N, RoadClass::Primary, 1),
            (W, RoadClass::Residential, 1),
            (S, RoadClass::Primary, 1),
        ]);
        let (_, t) = table(&ns_main);
        assert_eq!(only(&t, 0, Through), 2);
        assert_eq!(only(&t, 180, Through), 6);
        assert_eq!(only(&t, 90, Through), 4);
        assert_eq!(only(&t, 270, Through), 8);
    }

    /// Equal classes: the street with more lanes is the main street.
    #[test]
    fn more_lanes_make_the_main_street_when_classes_tie() {
        let w = junction(&[
            (E, RoadClass::Secondary, 1),
            (N, RoadClass::Secondary, 2),
            (W, RoadClass::Secondary, 1),
            (S, RoadClass::Secondary, 2),
        ]);
        let (_, t) = table(&w);
        assert_eq!(only(&t, 0, MovementKind::Through), 2);
        assert_eq!(only(&t, 90, MovementKind::Through), 4);
    }

    /// A T: the through street keeps 2 and 6 and one left phase (the left into the
    /// stem, opposed by the other approach's through); the stem has no opposing approach,
    /// so its left and right both ride its through.
    #[test]
    fn a_tee_gives_the_stem_one_phase() {
        use MovementKind::{Left, Right, Through};
        let w = junction(&[
            (E, RoadClass::Secondary, 1),
            (W, RoadClass::Secondary, 1),
            (S, RoadClass::Residential, 1),
        ]);
        let (a, t) = table(&w);
        assert_eq!(only(&t, 90, Through), 2);
        assert_eq!(only(&t, 90, Right), 2);
        assert_eq!(only(&t, 270, Through), 6);
        assert_eq!(only(&t, 270, Left), 1);
        assert_eq!(only(&t, 0, Left), 4);
        assert_eq!(only(&t, 0, Right), 4);
        assert_eq!(a.phases(), [1, 2, 4, 6]);
    }

    fn approach(key: u32, heading: f64, rank: u8, moves: &[(usize, TurnDirection)]) -> Approach {
        Approach {
            key,
            heading_deg: heading,
            class_rank: rank,
            lanes: 1,
            green_s: 30.0,
            movements: moves.to_vec(),
        }
    }

    /// A one-way main street crossing a two-way side street: the one-way's left has no
    /// opposing through, so it rides phase 2 itself; the side street's lefts still get
    /// 7 and 3 (each is opposed).
    #[test]
    fn a_one_way_street_s_left_rides_its_through() {
        use TurnDirection::{Left, Straight};
        let a = assign_approaches(
            6,
            &[
                approach(0, 90.0, 2, &[(0, Straight), (1, Left)]),
                approach(1, 0.0, 6, &[(2, Straight), (3, Left)]),
                approach(2, 180.0, 6, &[(4, Straight), (5, Left)]),
            ],
            &[],
        )
        .expect("maps");
        let p: Vec<u8> = a.movements.iter().map(|m| m.phase).collect();
        assert_eq!(p, [2, 2, 4, 7, 8, 3]);
        assert_eq!(a.movements[1].permissive_with, None);
        // A westbound one-way numbers into ring 2.
        let a = assign_approaches(
            2,
            &[approach(0, 270.0, 2, &[(0, Straight), (1, Left)])],
            &[],
        )
        .expect("maps");
        assert_eq!(a.phases(), [6]);
    }

    /// Crosswalks take the parallel through on whose right-hand side they lie (STM
    /// §5.1.1): across the north leg of an east-west main street is beside westbound
    /// (6), the south leg beside eastbound (2), the east leg beside northbound (4).
    /// A crosswalk parallel to neither of two streets 60° apart falls back.
    #[test]
    fn crosswalks_take_the_adjacent_parallel_through() {
        use TurnDirection::Straight;
        let apps = [
            approach(0, 90.0, 2, &[(0, Straight)]),
            approach(1, 270.0, 2, &[(1, Straight)]),
            approach(2, 0.0, 6, &[(2, Straight)]),
            approach(3, 180.0, 6, &[(3, Straight)]),
        ];
        let cw = |index, walk, off| CrossingLeg {
            index,
            walk_heading_deg: walk,
            offset_m: off,
        };
        let a = assign_approaches(
            8,
            &apps,
            &[
                cw(4, 90.0, (0.0, 10.0)),
                cw(5, 270.0, (0.0, -10.0)),
                cw(6, 0.0, (10.0, 0.0)),
                cw(7, 180.0, (-10.0, 0.0)),
            ],
        )
        .expect("maps");
        let p: Vec<u8> = a.movements[4..].iter().map(|m| m.phase).collect();
        assert_eq!(p, [6, 2, 4, 8]);
        // A mid-block signal: one street, and a crosswalk across it gets a ped-only phase.
        let a = assign_approaches(3, &apps[..2], &[cw(2, 0.0, (0.0, 0.0))]).expect("maps");
        assert_eq!(a.movements[2].phase, 4);
        let skewed = [
            approach(0, 0.0, 2, &[(0, Straight)]),
            approach(1, 180.0, 2, &[(1, Straight)]),
            approach(2, 60.0, 6, &[(2, Straight)]),
            approach(3, 240.0, 6, &[(3, Straight)]),
        ];
        assert_eq!(
            assign_approaches(5, &skewed, &[cw(4, 120.0, (5.0, 5.0))]),
            Err(Unmapped::SkewedCrossing(4))
        );
    }

    /// Five approaches, or three that pair with nothing (a Y), keep the single-ring
    /// controller and say why.
    #[test]
    fn junctions_that_do_not_map_fall_back_and_say_why() {
        use TurnDirection::Straight;
        let five: Vec<Approach> = (0..5u32)
            .map(|k| approach(k, 72.0 * f64::from(k), 4, &[(k as usize, Straight)]))
            .collect();
        assert_eq!(
            assign_approaches(5, &five, &[]),
            Err(Unmapped::TooManyApproaches(5))
        );
        let y: Vec<Approach> = (0..3u32)
            .map(|k| approach(k, 120.0 * f64::from(k), 4, &[(k as usize, Straight)]))
            .collect();
        assert_eq!(assign_approaches(3, &y, &[]), Err(Unmapped::NotTwoStreets));
        // Two approaches from nearly the same side are not two streets.
        let near = [
            approach(0, 90.0, 4, &[(0, Straight)]),
            approach(1, 110.0, 4, &[(1, Straight)]),
        ];
        assert_eq!(
            assign_approaches(2, &near, &[]),
            Err(Unmapped::NotTwoStreets)
        );
        // A movement the approaches do not name is not guessed.
        let lone = [approach(0, 90.0, 4, &[(0, Straight)])];
        assert_eq!(
            assign_approaches(2, &lone, &[]),
            Err(Unmapped::UnknownMovement(1))
        );
    }

    /// The assignment never runs two foes on protected greens at once, against each
    /// junction's own conflict matrix, on generated crossroads, tees and the corner,
    /// edge and centre junctions of a 3x3 grid. The one level conflict Exhibit 5-3 runs
    /// together, two opposing lefts, is the only pair allowed and is named as such.
    #[test]
    fn concurrent_phases_hold_no_protected_foes() {
        let worlds = vec![
            junction(&[
                (E, RoadClass::Primary, 2),
                (N, RoadClass::Residential, 1),
                (W, RoadClass::Primary, 2),
                (S, RoadClass::Residential, 1),
            ]),
            junction(&[
                (E, RoadClass::Secondary, 1),
                (W, RoadClass::Secondary, 1),
                (S, RoadClass::Residential, 1),
            ]),
            junction(&[
                (E, RoadClass::Secondary, 1),
                (W, RoadClass::Secondary, 1),
                (N, RoadClass::Residential, 2),
            ]),
            v2xw_world::procedural::grid(
                &GridParams::legacy().with_size(3, 3).with_signals(true),
                &ImportOptions::default(),
            )
            .expect("grid"),
        ];
        let mut checked = 0;
        for w in &worlds {
            let report = assign_all(w);
            assert!(report.unmapped.is_empty(), "{:?}", report.fallback_counts());
            for (i, a) in &report.assigned {
                let plan = &w.signals[*i];
                let j = w.roads.junction(plan.junction);
                let bad: Vec<_> = a
                    .concurrent_foes(plan, j)
                    .into_iter()
                    .filter(|f| !f.opposing_lefts)
                    .collect();
                assert!(bad.is_empty(), "plan {i}: {bad:?} in {:?}", a.movements);
                checked += 1;
            }
        }
        assert!(checked >= 4, "{checked}");
    }
}
