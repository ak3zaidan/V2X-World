//! Mid-block crossings ("jaywalking"): where a pedestrian can cross a street away from any
//! crosswalk, the gap they need to do it, and the band their path cuts across each lane.
//!
//! # What is modelled
//!
//! * **Where.** [`MidblockIndex::build`] casts a ray across the street from points along
//!   every sidewalk lane, away from the junctions at its ends ([`MidblockParams::end_margin_m`]),
//!   and keeps the points where the ray crosses one or more driven lanes (general, bus or
//!   cycle; a parking lane is crossed but not counted) and then reaches a sidewalk on the far
//!   side within [`MidblockParams::max_crossing_m`]. A ray that meets a junction connector or
//!   a crosswalk first is not a mid-block crossing and is dropped.
//! * **How often.** A pedestrian walking an eligible stretch decides to cross it with a
//!   constant hazard per metre walked ([`MidblockParams::rate_per_100m`]), multiplied by
//!   [`MidblockParams::stopped_traffic_factor`] where the traffic beside them stands still.
//!   A constant hazard per metre makes the chance of crossing a block grow with the block's
//!   length — the observed pattern that pedestrians cross mid-block more where blocks are
//!   long and the detour to the corner is large — but the rate itself is **this crate's
//!   choice**, not a measurement: no observed mid-block crossing rate per metre of sidewalk
//!   could be read. It is a parameter, and the run reports the share of crossings made
//!   mid-block so it can be compared with a local count.
//! * **Where to.** Straight across, or diagonally forward by an angle drawn uniformly up to
//!   [`MidblockParams::diagonal_max_deg`]: pedestrians who cross mid-block often angle
//!   towards where they are going.
//! * **When.** The HCM's pedestrian critical headway, `t_c = L/S_p + t_s` (*Highway Capacity
//!   Manual*, two-way stop-controlled pedestrian mode), applied lane by lane — the rolling
//!   gap: a vehicle approaching lane `i` is accepted if it arrives after the pedestrian has
//!   cleared that lane (`d_far,i / v + t_s`), or if its rear will have passed before the
//!   pedestrian reaches the lane (`d_near,i / v`, less `t_s` as a margin). `S_p` is the
//!   pedestrian's own walking speed, so a slower walker needs a longer gap: the critical gap
//!   is a function of both the approaching vehicle's speed and its distance. On top of that,
//!   no pedestrian steps in front of a vehicle that could not stop for them
//!   ([`crate::vru::crosswalk`], UVC §11-502(b)'s rule applied everywhere).
//! * **Drivers.** A driver who sees a pedestrian waiting at the kerb mid-block and could stop
//!   comfortably yields with [`MidblockParams::driver_yield_probability`]; one who sees a
//!   pedestrian *in* their lane, or about to enter it, always stops if they can (the duty of
//!   due care, UVC §11-504). Away from a crosswalk the pedestrian must yield (UVC §11-503(a)),
//!   and 72 % of drivers know it (Ragland & Mitman 2007, unmarked mid-block crossings), so the
//!   yield probability is low; its value is **a choice**, since no measured yield rate for a
//!   mid-block jaywalker could be read.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::LaneId;
use v2xw_core::math;
use v2xw_world::walk::CrosswalkConflict;
use v2xw_world::{ClassMask, Lane, LaneKind, World};

/// Crosswalk numbers at and above this one are mid-block paths, not painted crosswalks: the
/// pedestrian's slot is added to it.
pub const DYNAMIC_BASE: usize = 1 << 24;

/// The mid-block crossing parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MidblockParams {
    /// Crossing decisions per 100 m of eligible sidewalk walked, where traffic moves.
    /// **A choice** (see the module documentation). Zero turns mid-block crossing off.
    pub rate_per_100m: f64,
    /// The multiplier on that rate beside traffic that stands still (a queue): crossing
    /// between stopped cars is the commonest jaywalk in a congested grid. **A choice.**
    pub stopped_traffic_factor: f64,
    /// A sidewalk lane shorter than this has no mid-block crossing points, metres.
    pub min_block_m: f64,
    /// How far from either end of a sidewalk lane a crossing point must be, metres: inside
    /// it the corner's crosswalk is the crossing.
    pub end_margin_m: f64,
    /// The widest street a pedestrian crosses mid-block, kerb to far sidewalk, metres.
    pub max_crossing_m: f64,
    /// Spacing of the candidate crossing points along a sidewalk, metres.
    pub spacing_m: f64,
    /// The largest forward angle of a diagonal crossing, degrees from perpendicular.
    pub diagonal_max_deg: f64,
    /// The chance that a driver who could stop comfortably yields to a pedestrian waiting
    /// at the kerb mid-block. **A choice.**
    pub driver_yield_probability: f64,
    /// How long a pedestrian waits for a gap before walking on, seconds. **A choice.**
    pub max_wait_s: f64,
    /// The width of the strip a crossing pedestrian sweeps, metres: a body (0.48 m,
    /// SUMO's pedestrian vType) plus a quarter metre either side.
    pub corridor_width_m: f64,
}

impl Default for MidblockParams {
    fn default() -> Self {
        Self {
            rate_per_100m: 0.0,
            stopped_traffic_factor: 4.0,
            min_block_m: 50.0,
            end_margin_m: 12.0,
            max_crossing_m: 32.0,
            spacing_m: 4.0,
            diagonal_max_deg: 35.0,
            driver_yield_probability: 0.1,
            max_wait_s: 40.0,
            corridor_width_m: 1.0,
        }
    }
}

impl MidblockParams {
    /// The defaults with mid-block crossing switched on at the rate a dense downtown grid
    /// uses here: on average one crossing decision per 400 m of eligible sidewalk where
    /// traffic moves (four times as many beside a queue). **A choice**, see the module
    /// documentation.
    pub fn urban() -> Self {
        Self {
            rate_per_100m: 0.25,
            ..Self::default()
        }
    }
}

/// One place a pedestrian on a sidewalk can cross the street.
#[derive(Debug, Clone, PartialEq)]
pub struct Site {
    /// Arc length on the sidewalk lane, metres.
    pub s_m: f64,
    /// Which side of the sidewalk lane the street is: +1 left of its direction, −1 right.
    pub side: f64,
    /// The sidewalk lane on the far side.
    pub far_lane: LaneId,
    /// Arc length on it, straight across.
    pub far_s_m: f64,
    /// The distance from the near sidewalk's centreline to the far one's, straight across,
    /// metres.
    pub width_m: f64,
    /// The driven lanes the straight path crosses, nearest first.
    pub crossed: Vec<LaneId>,
}

/// Every mid-block crossing point of a world, by sidewalk lane.
#[derive(Debug, Clone, Default)]
pub struct MidblockIndex {
    sites: BTreeMap<LaneId, Vec<Site>>,
    grid: Grid,
    margin_m: f64,
}

/// Lane segments by grid cell, for the ray casts.
#[derive(Debug, Clone, Default)]
struct Grid {
    cell: f64,
    cells: BTreeMap<(i64, i64), Vec<(u32, u32)>>,
}

impl Grid {
    fn key(&self, x: f64, y: f64) -> (i64, i64) {
        ((x / self.cell).floor() as i64, (y / self.cell).floor() as i64)
    }
}

impl MidblockIndex {
    /// Finds the crossing points of every sidewalk lane of `world`.
    pub fn build(world: &World, params: &MidblockParams) -> Self {
        let lanes = world.roads.lanes();
        let mut grid = Grid {
            cell: 24.0,
            cells: BTreeMap::new(),
        };
        for lane in lanes {
            for (i, w) in lane.centreline.windows(2).enumerate() {
                let (a, b) = grid.key(w[0].x.min(w[1].x), w[0].y.min(w[1].y));
                let (c, d) = grid.key(w[0].x.max(w[1].x), w[0].y.max(w[1].y));
                for x in a..=c {
                    for y in b..=d {
                        grid.cells
                            .entry((x, y))
                            .or_default()
                            .push((lane.id.index(), i as u32));
                    }
                }
            }
        }
        let mut sites: BTreeMap<LaneId, Vec<Site>> = BTreeMap::new();
        let spacing = params.spacing_m.max(1.0);
        for lane in lanes {
            if lane.kind != LaneKind::Sidewalk
                || !lane.admits(ClassMask::PEDESTRIAN)
                || lane.length_m < params.min_block_m
            {
                continue;
            }
            let mut s = params.end_margin_m;
            while s <= lane.length_m - params.end_margin_m {
                for side in [1.0, -1.0] {
                    if let Some(site) = cast(world, &grid, lane, s, side, params) {
                        sites.entry(lane.id).or_default().push(site);
                    }
                }
                s += spacing;
            }
        }
        Self {
            sites,
            grid,
            margin_m: params.end_margin_m,
        }
    }

    /// The driven lanes the straight path `a→b` crosses, nearest first — or `None` if it
    /// meets a junction connector or a crosswalk on the way, so is no mid-block crossing.
    /// A diagonal path can cut lanes the straight-across ray of its site did not, a turn
    /// pocket near the corner among them; the bands are cut on these.
    pub fn path_lanes(&self, world: &World, a: Vec3, b: Vec3) -> Option<Vec<LaneId>> {
        let grid = &self.grid;
        let (x0, y0) = grid.key(a.x.min(b.x), a.y.min(b.y));
        let (x1, y1) = grid.key(a.x.max(b.x), a.y.max(b.y));
        let mut seen: Vec<(u32, u32)> = Vec::new();
        for x in x0..=x1 {
            for y in y0..=y1 {
                if let Some(v) = grid.cells.get(&(x, y)) {
                    seen.extend_from_slice(v);
                }
            }
        }
        seen.sort_unstable();
        seen.dedup();
        let mut hits: Vec<(f64, LaneId)> = Vec::new();
        for (l, i) in seen {
            let lane = world.lane(LaneId::new(l));
            let (u0, u1) = (lane.centreline[i as usize], lane.centreline[i as usize + 1]);
            if let Some((t, _)) = seg_intersect(a, b, u0, u1) {
                match lane.kind {
                    LaneKind::Internal | LaneKind::Crossing => return None,
                    _ if v2xw_world::walk::is_driven(lane) => hits.push((t, lane.id)),
                    _ => {}
                }
            }
        }
        hits.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        let mut out: Vec<LaneId> = Vec::new();
        for (_, id) in hits {
            if !out.contains(&id) {
                out.push(id);
            }
        }
        Some(out)
    }

    /// The end margin the index was built with, metres: how far from a sidewalk lane's
    /// ends a crossing may land.
    pub fn margin_m(&self) -> f64 {
        self.margin_m
    }

    /// The crossing points of one sidewalk lane, by arc length.
    pub fn sites_on(&self, lane: LaneId) -> &[Site] {
        self.sites.get(&lane).map_or(&[], Vec::as_slice)
    }

    /// How many crossing points there are.
    pub fn len(&self) -> usize {
        self.sites.values().map(Vec::len).sum()
    }

    /// True if there are none.
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }
}

/// One hit of a ray: distance along it, the lane, and the arc length on the lane.
type Hit = (f64, LaneId, f64);

/// Casts a ray from the sidewalk at `s` towards `side` and returns the site it finds.
fn cast(
    world: &World,
    grid: &Grid,
    lane: &Lane,
    s: f64,
    side: f64,
    params: &MidblockParams,
) -> Option<Site> {
    let p = lane.point_at(s);
    let h = lane.heading_at(s);
    let (sn, cs) = math::sin_cos(h);
    let n = Vec3::new_2d(-sn * side, cs * side);
    let reach = params.max_crossing_m + 2.0;
    let q = Vec3::new_2d(p.x + n.x * reach, p.y + n.y * reach);
    let (a, b) = grid.key(p.x.min(q.x), p.y.min(q.y));
    let (c, d) = grid.key(p.x.max(q.x), p.y.max(q.y));
    let mut seen: Vec<(u32, u32)> = Vec::new();
    for x in a..=c {
        for y in b..=d {
            if let Some(v) = grid.cells.get(&(x, y)) {
                seen.extend_from_slice(v);
            }
        }
    }
    seen.sort_unstable();
    seen.dedup();
    let mut hits: Vec<Hit> = Vec::new();
    for (l, i) in seen {
        if l == lane.id.index() {
            continue;
        }
        let other = world.lane(LaneId::new(l));
        let (u0, u1) = (other.centreline[i as usize], other.centreline[i as usize + 1]);
        if let Some((t, u)) = seg_intersect(p, q, u0, u1) {
            let seg = math::hypot(u1.x - u0.x, u1.y - u0.y);
            hits.push((t * reach, other.id, other.cumulative[i as usize] + u * seg));
        }
    }
    hits.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let mut crossed: Vec<LaneId> = Vec::new();
    let mut k = 0;
    while k < hits.len() {
        let (t, id, _) = hits[k];
        let other = world.lane(id);
        match other.kind {
            LaneKind::Internal | LaneKind::Crossing => return None,
            LaneKind::Sidewalk => {
                if crossed.is_empty() {
                    if t < 3.0 {
                        // The same sidewalk's other walking direction, beside this one.
                        k += 1;
                        continue;
                    }
                    return None;
                }
                if t > params.max_crossing_m {
                    return None;
                }
                // The far sidewalk: of the walking lanes there, the one heading the way
                // this pedestrian walks.
                let mut best: Option<(f64, LaneId, f64)> = None;
                for (t2, id2, s2) in hits[k..].iter().copied() {
                    if t2 > t + 1.5 {
                        break;
                    }
                    let l2 = world.lane(id2);
                    if l2.kind != LaneKind::Sidewalk || !l2.admits(ClassMask::PEDESTRIAN) {
                        continue;
                    }
                    let along = math::cos(l2.heading_at(s2) - h);
                    if best.is_none_or(|(b, _, _)| along > b) {
                        best = Some((along, id2, s2));
                    }
                }
                let (_, far_lane, far_s_m) = best?;
                return Some(Site {
                    s_m: s,
                    side,
                    far_lane,
                    far_s_m,
                    width_m: t,
                    crossed,
                });
            }
            LaneKind::Parking => {}
            _ => {
                if v2xw_world::walk::is_driven(other) {
                    if !crossed.contains(&id) {
                        crossed.push(id);
                    }
                }
            }
        }
        k += 1;
    }
    None
}

/// Where segment `p→q` meets segment `a→b`: `(t on p→q, u on a→b)`, both in `[0, 1]`.
fn seg_intersect(p: Vec3, q: Vec3, a: Vec3, b: Vec3) -> Option<(f64, f64)> {
    let r = (q.x - p.x, q.y - p.y);
    let s = (b.x - a.x, b.y - a.y);
    let denom = r.0 * s.1 - r.1 * s.0;
    if denom.abs() < 1e-12 {
        return None;
    }
    let ap = (a.x - p.x, a.y - p.y);
    let t = (ap.0 * s.1 - ap.1 * s.0) / denom;
    let u = (ap.0 * r.1 - ap.1 * r.0) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

/// One band a mid-block path cuts across a driven lane, with where along the path the lane
/// begins and ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathBand {
    /// The band, as the crosswalk rules read it; `crosswalk` is [`DYNAMIC_BASE`] plus the
    /// path's number.
    pub conflict: CrosswalkConflict,
    /// Distance along the path from its start to where the lane's near edge is, metres.
    pub d_near_m: f64,
    /// … and to its far edge.
    pub d_far_m: f64,
}

/// The bands the straight path `a→b` cuts across the lanes in `crossed`, numbered `id`.
pub fn path_bands(
    world: &World,
    crossed: &[LaneId],
    a: Vec3,
    b: Vec3,
    corridor_width_m: f64,
    id: usize,
) -> Vec<PathBand> {
    let len = math::hypot(b.x - a.x, b.y - a.y).max(1e-6);
    let mut out = Vec::new();
    for lane_id in crossed {
        let Some(lane) = world.try_lane(*lane_id) else {
            continue;
        };
        for (i, w) in lane.centreline.windows(2).enumerate() {
            let Some((t, u)) = seg_intersect(a, b, w[0], w[1]) else {
                continue;
            };
            let seg = math::hypot(w[1].x - w[0].x, w[1].y - w[0].y).max(1e-9);
            let (rx, ry) = ((b.x - a.x) / len, (b.y - a.y) / len);
            let (lx, ly) = ((w[1].x - w[0].x) / seg, (w[1].y - w[0].y) / seg);
            let sin = (rx * ly - ry * lx).abs().max(0.25);
            let cos = (rx * lx + ry * ly).abs();
            let half = 0.5 * corridor_width_m / sin + 0.5 * lane.width_m * cos / sin;
            let d = t * len;
            // How far along the path the lane's edges are: half its width over the sine.
            let half_across = 0.5 * lane.width_m / sin;
            out.push(PathBand {
                conflict: CrosswalkConflict {
                    crosswalk: DYNAMIC_BASE + id,
                    lane: lane.id,
                    s_m: lane.cumulative[i] + u * seg,
                    half_extent_m: half,
                    s_on_crossing_m: d,
                },
                d_near_m: (d - half_across).max(0.0),
                d_far_m: (d + half_across).min(len),
            });
            break;
        }
    }
    out.sort_by(|x, y| x.d_near_m.total_cmp(&y.d_near_m));
    out
}
