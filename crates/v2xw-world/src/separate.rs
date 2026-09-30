//! Lanes of different roads laid over each other, and moving them apart.
//!
//! # Why
//!
//! An OpenStreetMap way is a line; a lane is a band. The importer lays each road's lanes
//! side by side about its way, `lanes × lane width` wide, and that width is a class default
//! wherever the way does not state one. Two things the map draws separately can then end up
//! on top of each other:
//!
//! * **A pavement on the roadway.** A `footway=sidewalk` way is drawn where the pavement
//!   is; the carriageway beside it is as wide as its lane count implies. Where the street's
//!   centreline is off-centre, or its lanes narrower than the default, the pavement lane
//!   runs inside the drive lanes: 811 of 7,741 pavement lanes on the Midtown extract, and
//!   the traffic auditor counted 15 cars driving through pedestrians because of them.
//! * **Two carriageways too close.** The two branches of a fork or a merge are mapped from
//!   one node at a shallow angle, and the two directions of a ramp as two one-way ways a
//!   couple of metres apart. Each way's lanes, centred on it, overlap the other's for tens
//!   of metres, and cars on them drove through each other (67 overlaps on dense Manhattan
//!   in the first run after the junction join).
//!
//! # What this does
//!
//! It measures, every metre along each lane, how far the lane's band overlaps the nearest
//! parallel drive lane of *another* road, and moves the lane sideways by exactly that much
//! plus [`MARGIN_M`]: a pavement or cycle lane all the way (the carriageway is what the lane
//! count describes; the pavement is placed "to the kerb from the carriageway width the map
//! implies"), two carriageways half each. A carriageway moves as a whole — every lane of
//! the road by the same amount — so its lanes stay adjacent. The shift is feathered out
//! over [`TAPER_M`] on either side of where it is needed, so the moved lane stays smooth,
//! and a pavement's shift is also feathered to zero over its last [`END_TAPER_M`] at each
//! end, so it still meets the crossing or pavement it connects to there.
//!
//! A lane that would have to move further than [`MAX_SOFT_SHIFT_M`] (a pavement) or
//! [`MAX_CARRIAGEWAY_SHIFT_M`] (a carriageway) is not a placement error but a mapping one —
//! a road drawn through another — and is left where it is and counted, so the validator
//! still reports it.
//!
//! Arithmetic, `sqrt` and [`v2xw_core::math`] only, in lane-id order, so the result is
//! bit-identical on every platform.

use std::collections::BTreeMap;

use v2xw_core::geom::Vec3;
use v2xw_core::math;

use crate::model::{Lane, LaneKind};

/// Clearance left between two bands once they are moved apart, metres.
pub const MARGIN_M: f64 = 0.1;

/// Over how far a shift is feathered in and out along the lane, metres.
///
/// The feather is a smoothstep, whose slope is zero where it starts and ends, so the moved
/// lane has no kink: a 2 m shift over 20 m turns the lane by at most 8.6° at a radius of
/// no less than 33 m — gentler than any lane change. A linear feather put an 11° corner at
/// each end of the shift, and cars' headings jumped there.
pub const TAPER_M: f64 = 20.0;

/// Over how far a pavement's shift is feathered to zero at each of its ends, metres. A
/// carriageway's is feathered over [`TAPER_M`], for the same reason its shift is.
pub const END_TAPER_M: f64 = 4.0;

/// The furthest a pavement or cycle lane is moved, metres.
pub const MAX_SOFT_SHIFT_M: f64 = 3.0;

/// The furthest a carriageway is moved, metres (each of the two, so they separate by up to
/// twice this).
pub const MAX_CARRIAGEWAY_SHIFT_M: f64 = 2.0;

/// How a lane takes part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// A road's drive lane: moved as part of its road, and what everything else is kept
    /// clear of.
    Carriageway,
    /// A pavement or cycle lane: moved clear of every carriageway.
    Soft,
    /// Neither measured nor moved (a crossing, which is meant to cross the road).
    Fixed,
}

/// One lane's part in the separation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LaneInfo {
    /// How it takes part.
    pub role: Role,
    /// Its road in one direction: the lanes that move together.
    pub group: usize,
    /// Its road in both directions: two groups of one road never push each other.
    pub road: usize,
}

/// What [`separate`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Separation {
    /// Drive lanes moved.
    pub carriageway_lanes_moved: u64,
    /// Pavement and cycle lanes moved.
    pub soft_lanes_moved: u64,
    /// Lanes that would have had to move further than the cap, left where they are.
    pub unresolved: u64,
}

/// Carriageway lane segments by grid cell.
struct Grid {
    cells: BTreeMap<(i64, i64), Vec<(usize, usize)>>,
}

const CELL_M: f64 = 10.0;

fn cell(v: f64) -> i64 {
    (v / CELL_M).floor() as i64
}

impl Grid {
    fn new(lanes: &[Lane], info: &[LaneInfo]) -> Self {
        let mut cells: BTreeMap<(i64, i64), Vec<(usize, usize)>> = BTreeMap::new();
        for (i, lane) in lanes.iter().enumerate() {
            if info[i].role != Role::Carriageway {
                continue;
            }
            for (k, w) in lane.centreline.windows(2).enumerate() {
                for cx in cell(w[0].x.min(w[1].x))..=cell(w[0].x.max(w[1].x)) {
                    for cy in cell(w[0].y.min(w[1].y))..=cell(w[0].y.max(w[1].y)) {
                        cells.entry((cx, cy)).or_default().push((i, k));
                    }
                }
            }
        }
        Self { cells }
    }

    fn near(&self, p: Vec3, r: f64) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for cx in cell(p.x - r)..=cell(p.x + r) {
            for cy in cell(p.y - r)..=cell(p.y + r) {
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

/// The closest point to `p` on segment `a → b`, and the distance to it.
fn closest(p: Vec3, a: Vec3, b: Vec3) -> (Vec3, f64) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let u = if len2 <= 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    let q = Vec3::new(a.x + u * dx, a.y + u * dy, a.z + u * (b.z - a.z));
    let (ex, ey) = (p.x - q.x, p.y - q.y);
    (q, math::sqrt(ex * ex + ey * ey))
}

/// The signed shift, metres left of travel, lane `i` needs at each metre sample along it:
/// `(s, shift)` for every sample that needs one, or `None` for a sample squeezed from both
/// sides.
fn needs(lanes: &[Lane], info: &[LaneInfo], grid: &Grid, i: usize) -> Vec<(f64, Option<f64>)> {
    let lane = &lanes[i];
    let me = info[i];
    let share = if me.role == Role::Soft { 1.0 } else { 0.5 };
    let samples = lane.length_m.floor() as usize;
    let mut out = Vec::new();
    for k in 0..samples {
        let s = k as f64 + 0.5;
        let p = lane.point_at(s);
        let h = lane.heading_at(s);
        let (sin_h, cos_h) = math::sin_cos(h);
        let mut left = 0.0f64; // the most this sample must move left
        let mut right = 0.0f64; // ... and right
        for (j, seg) in grid.near(p, 8.0) {
            let other = info[j];
            if j == i || other.group == me.group || other.road == me.road {
                continue;
            }
            let o = &lanes[j];
            let (a, b) = (o.centreline[seg], o.centreline[seg + 1]);
            let (q, d) = closest(p, a, b);
            if (q.z - p.z).abs() > 3.0 {
                continue;
            }
            // Parallel only: a road that crosses this one is a junction or a bridge, not
            // a band laid on this one.
            let hb = math::atan2(b.y - a.y, b.x - a.x);
            if math::sin(h - hb).abs() > 0.5 {
                continue;
            }
            let need = 0.5 * (lane.width_m + o.width_m) + MARGIN_M - d;
            if need <= 0.0 {
                continue;
            }
            // Which side the other band is on: move away from it.
            let side = cos_h * (q.y - p.y) - sin_h * (q.x - p.x);
            if side >= 0.0 {
                right = right.max(need * share);
            } else {
                left = left.max(need * share);
            }
        }
        match (left > 0.0, right > 0.0) {
            (false, false) => {}
            (true, false) => out.push((s, Some(left))),
            (false, true) => out.push((s, Some(-right))),
            (true, true) => out.push((s, None)),
        }
    }
    out
}

/// A shift profile along a lane, from the samples that need one: each sample's shift
/// feathered out over [`TAPER_M`] either side ([`smoothstep`]), the largest magnitude
/// winning where they overlap.
fn profile(samples: &[(f64, f64)], s: f64) -> f64 {
    let mut best = 0.0f64;
    for (at, shift) in samples {
        let x = 1.0 - (s - at).abs() / TAPER_M;
        if x <= 0.0 {
            continue;
        }
        let v = shift * smoothstep(x);
        if v.abs() > best.abs() {
            best = v;
        }
    }
    best
}

/// `3x² − 2x³`: 0 at 0, 1 at 1, with zero slope at both.
fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Lane `lane` resampled at one metre or less and moved sideways by `shift(s)`.
fn displaced(lane: &Lane, shift: &dyn Fn(f64) -> f64) -> Vec<Vec3> {
    let pts = &lane.centreline;
    let mut dense: Vec<(Vec3, f64)> = Vec::new();
    let mut s0 = 0.0;
    for w in pts.windows(2) {
        let len = w[0].distance_2d(w[1]);
        let n = (len / 1.0).ceil().max(1.0) as usize;
        for k in 0..n {
            let f = k as f64 / n as f64;
            dense.push((w[0].lerp(w[1], f), s0 + f * len));
        }
        s0 += len;
    }
    dense.push((pts[pts.len() - 1], s0));
    let m = dense.len();
    let mut out = Vec::with_capacity(m);
    for i in 0..m {
        let a = dense[i.saturating_sub(1)].0;
        let b = dense[(i + 1).min(m - 1)].0;
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = math::sqrt(dx * dx + dy * dy);
        let (p, s) = dense[i];
        if len < 1e-9 {
            out.push(p);
            continue;
        }
        let d = shift(s);
        out.push(Vec3::new(p.x - dy / len * d, p.y + dx / len * d, p.z));
    }
    out
}

/// Rebuilds lane `i` along `points`, keeping everything else; the old lane stays when the
/// new geometry is refused.
fn rebuild(lanes: &mut [Lane], i: usize, points: Vec<Vec3>) -> bool {
    let lane = &lanes[i];
    match Lane::new(
        lane.id,
        lane.edge,
        lane.junction,
        lane.index,
        lane.kind,
        points,
        lane.width_m,
        lane.speed_limit_mps,
        lane.allowed,
    ) {
        Ok(new) => {
            lanes[i] = new;
            true
        }
        Err(_) => false,
    }
}

/// Moves overlapping lanes apart; see the module documentation.
pub(crate) fn separate(lanes: &mut [Lane], info: &[LaneInfo]) -> Separation {
    let mut report = Separation::default();
    let mut moved_carriageway = vec![false; lanes.len()];
    // Carriageways first, a road at a time, in three passes: moving one road can bring it
    // near a third.
    for _pass in 0..3 {
        let grid = Grid::new(lanes, info);
        // Group the carriageway lanes by road direction.
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, li) in info.iter().enumerate() {
            if li.role == Role::Carriageway {
                groups.entry(li.group).or_default().push(i);
            }
        }
        let mut plans: Vec<(Vec<usize>, Vec<(f64, f64)>, f64)> = Vec::new();
        for members in groups.values() {
            // Every member's needs, on the normalised arc length of the group.
            let mut merged: Vec<(f64, Option<f64>)> = Vec::new();
            for &i in members {
                let len = lanes[i].length_m.max(1e-9);
                for (s, v) in needs(lanes, info, &grid, i) {
                    merged.push((s / len, v));
                }
            }
            if merged.is_empty() {
                continue;
            }
            if merged.iter().any(|(_, v)| v.is_none()) {
                report.unresolved += members.len() as u64;
                continue;
            }
            let worst = merged
                .iter()
                .filter_map(|(_, v)| *v)
                .fold(0.0f64, |m, v| if v.abs() > m.abs() { v } else { m });
            let lefts = merged
                .iter()
                .filter(|(_, v)| v.is_some_and(|x| x > 0.0))
                .count();
            let rights = merged.len() - lefts;
            if lefts > 0 && rights > 0 {
                // Pushed both ways along its length: a road squeezed between two others.
                report.unresolved += members.len() as u64;
                continue;
            }
            if worst.abs() > MAX_CARRIAGEWAY_SHIFT_M {
                report.unresolved += members.len() as u64;
                continue;
            }
            let samples: Vec<(f64, f64)> =
                merged.iter().map(|(t, v)| (*t, v.unwrap_or(0.0))).collect();
            plans.push((members.clone(), samples, 0.0));
        }
        if plans.is_empty() {
            break;
        }
        for (members, samples, _) in plans {
            for i in members {
                let len = lanes[i].length_m;
                let at: Vec<(f64, f64)> = samples.iter().map(|(t, v)| (t * len, *v)).collect();
                // Feathered to nothing at both ends as well: the road continues through
                // its junctions into lanes this pass does not move, and a connector a
                // couple of metres long cannot absorb a sideways step without a heading
                // jump.
                let feather = |s: f64| smoothstep((s / TAPER_M).min((len - s) / TAPER_M));
                let pts = displaced(&lanes[i], &|s| profile(&at, s) * feather(s));
                if rebuild(lanes, i, pts) {
                    moved_carriageway[i] = true;
                }
            }
        }
    }
    report.carriageway_lanes_moved = moved_carriageway.iter().filter(|m| **m).count() as u64;

    // Then the pavements and cycle lanes, clear of the carriageways where they now are.
    let grid = Grid::new(lanes, info);
    for i in 0..lanes.len() {
        if info[i].role != Role::Soft {
            continue;
        }
        let n = needs(lanes, info, &grid, i);
        if n.is_empty() {
            continue;
        }
        if n.iter().any(|(_, v)| v.is_none()) {
            report.unresolved += 1;
            continue;
        }
        let samples: Vec<(f64, f64)> = n.iter().map(|(s, v)| (*s, v.unwrap_or(0.0))).collect();
        if samples.iter().any(|(_, v)| v.abs() > MAX_SOFT_SHIFT_M) {
            report.unresolved += 1;
            continue;
        }
        let len = lanes[i].length_m;
        let feather = |s: f64| smoothstep((s / END_TAPER_M).min((len - s) / END_TAPER_M));
        let pts = displaced(&lanes[i], &|s| profile(&samples, s) * feather(s));
        if rebuild(lanes, i, pts) {
            report.soft_lanes_moved += 1;
        }
    }
    report
}

/// The lane kinds a pavement-or-cycle [`Role::Soft`] applies to.
pub(crate) fn is_soft(kind: LaneKind) -> bool {
    matches!(kind, LaneKind::Sidewalk | LaneKind::Cycle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::ids::{EdgeId, LaneId};

    use crate::model::ClassMask;

    fn lane(id: u32, edge: u32, kind: LaneKind, y: f64, width: f64) -> Lane {
        Lane::new(
            LaneId::new(id),
            EdgeId::new(edge),
            None,
            0,
            kind,
            vec![Vec3::new_2d(0.0, y), Vec3::new_2d(100.0, y)],
            width,
            10.0,
            ClassMask::ALL,
        )
        .expect("lane")
    }

    #[test]
    fn a_pavement_on_the_roadway_is_moved_to_the_kerb() {
        // A 3.35 m drive lane on y = 0 and a 2 m pavement 1.5 m from its centre: the bands
        // overlap by 1.175 m.
        let mut lanes = vec![
            lane(0, 0, LaneKind::Driving, 0.0, 3.35),
            lane(1, 1, LaneKind::Sidewalk, 1.5, 2.0),
        ];
        let info = [
            LaneInfo {
                role: Role::Carriageway,
                group: 0,
                road: 0,
            },
            LaneInfo {
                role: Role::Soft,
                group: 1,
                road: 1,
            },
        ];
        let r = separate(&mut lanes, &info);
        assert_eq!(r.soft_lanes_moved, 1);
        // Mid-block it is now clear of the drive lane by the margin.
        let mid = lanes[1].point_at(50.0);
        assert!(
            (mid.y - (0.5 * (3.35 + 2.0) + MARGIN_M)).abs() < 1e-2,
            "{mid:?}"
        );
        // Its ends are where they were, so it still meets what it connects to.
        assert!((lanes[1].start().y - 1.5).abs() < 1e-9);
        assert!((lanes[1].end().y - 1.5).abs() < 1e-9);
        // The drive lane did not move.
        assert!((lanes[0].point_at(50.0).y).abs() < 1e-9);
    }

    #[test]
    fn two_carriageways_too_close_move_apart_equally() {
        let mut lanes = vec![
            lane(0, 0, LaneKind::Driving, 0.0, 3.35),
            lane(1, 1, LaneKind::Driving, 2.35, 3.35),
        ];
        let info = [
            LaneInfo {
                role: Role::Carriageway,
                group: 0,
                road: 0,
            },
            LaneInfo {
                role: Role::Carriageway,
                group: 1,
                road: 1,
            },
        ];
        let r = separate(&mut lanes, &info);
        assert_eq!(r.carriageway_lanes_moved, 2);
        let gap = lanes[1].point_at(50.0).y - lanes[0].point_at(50.0).y;
        assert!(gap >= 3.35 + MARGIN_M - 1e-3, "gap {gap}");
    }

    #[test]
    fn the_two_directions_of_one_road_are_left_alone() {
        let mut lanes = vec![
            lane(0, 0, LaneKind::Driving, 0.0, 3.35),
            lane(1, 1, LaneKind::Driving, 3.35, 3.35),
        ];
        let info = [
            LaneInfo {
                role: Role::Carriageway,
                group: 0,
                road: 7,
            },
            LaneInfo {
                role: Role::Carriageway,
                group: 1,
                road: 7,
            },
        ];
        let before = lanes.clone();
        let r = separate(&mut lanes, &info);
        assert_eq!(r, Separation::default());
        assert_eq!(lanes, before);
    }
}
