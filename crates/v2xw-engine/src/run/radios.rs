//! The equipped actors at the last published state, in a grid: the receivers a frame can
//! have.
//!
//! A frame's candidate receivers and its reception census used to come from
//! [`ActorSnapshot::actors_within`], which returns every *actor* in range, and were then
//! filtered to the ones carrying a radio by a map lookup each. At Midtown's evening peak
//! (`scenarios/midtown-peak-load.yaml`) three actors in four are pedestrians and cyclists
//! with no handset, and the line-of-sight reach covers the whole box, so every frame walked
//! about 2,300 actors through two ordered-map lookups and a sort to keep about 600: the
//! query was 12 % of the engine thread and the filter most of `start_frame`'s own time
//! (`sample` of the release server, stabilise track, 2026-10-06).
//!
//! This index holds only the actors with a node, with the same snapshot positions the
//! actor query tests, so [`RadioIndex::within`] answers exactly the equipped subset of
//! `actors_within(p, r)` — the same distance test on the same numbers — in actor-id order.
//! Nothing downstream depends on more than that set (the candidate lists are sorted by node
//! and the census counts), so the run's records are unchanged; the content digests the
//! engine's tests hold are the check.

use v2xw_core::geom::Vec3;
use v2xw_core::ids::{ActorId, NodeId};

/// The equipped actors, gridded.
#[derive(Debug, Default)]
pub(super) struct RadioIndex {
    /// In actor-id order, so sorted indices are id order.
    entries: Vec<(ActorId, NodeId, Vec3)>,
    origin: (f64, f64),
    cols: i64,
    rows: i64,
    /// Row-major cells: `start[c]..start[c + 1]` indexes `items`.
    start: Vec<u32>,
    items: Vec<u32>,
}

impl RadioIndex {
    /// The grid's cell size, metres: a few hundred metres is the census radius and a
    /// fraction of a capped candidate range, so a query reads a handful of cells.
    const CELL_M: f64 = 128.0;

    /// The index of `entries`, which must be in actor-id order (the snapshot's order).
    pub fn build(entries: Vec<(ActorId, NodeId, Vec3)>) -> Self {
        debug_assert!(entries.windows(2).all(|w| w[0].0 < w[1].0));
        if entries.is_empty() {
            return Self::default();
        }
        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for (_, _, p) in &entries {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        let origin = (min_x, min_y);
        let cols = (((max_x - min_x) / Self::CELL_M).floor() as i64 + 1).max(1);
        let rows = (((max_y - min_y) / Self::CELL_M).floor() as i64 + 1).max(1);
        let cell_of = |p: Vec3| -> usize {
            let cx = (((p.x - origin.0) / Self::CELL_M).floor() as i64).clamp(0, cols - 1);
            let cy = (((p.y - origin.1) / Self::CELL_M).floor() as i64).clamp(0, rows - 1);
            (cy * cols + cx) as usize
        };
        let n_cells = (cols * rows) as usize;
        let mut count = vec![0u32; n_cells + 1];
        for (_, _, p) in &entries {
            count[cell_of(*p) + 1] += 1;
        }
        for c in 0..n_cells {
            count[c + 1] += count[c];
        }
        let start = count.clone();
        let mut fill = count;
        let mut items = vec![0u32; entries.len()];
        for (i, (_, _, p)) in entries.iter().enumerate() {
            let c = cell_of(*p);
            items[fill[c] as usize] = i as u32;
            fill[c] += 1;
        }
        Self {
            entries,
            origin,
            cols,
            rows,
            start,
            items,
        }
    }

    /// Every equipped actor whose position is within `radius_m` of `p` in the plane
    /// (`distance_2d`, as [`v2xw_mobility::ActorSnapshot::actors_within`] tests), in actor-id
    /// order, with its node.
    pub fn within(&self, p: Vec3, radius_m: f64) -> Vec<(ActorId, NodeId)> {
        if self.entries.is_empty() || !(radius_m >= 0.0) {
            return Vec::new();
        }
        let span = |v: f64, o: f64, n: i64| -> i64 {
            (((v - o) / Self::CELL_M).floor() as i64).clamp(0, n - 1)
        };
        let (x0, x1) = (
            span(p.x - radius_m, self.origin.0, self.cols),
            span(p.x + radius_m, self.origin.0, self.cols),
        );
        let (y0, y1) = (
            span(p.y - radius_m, self.origin.1, self.rows),
            span(p.y + radius_m, self.origin.1, self.rows),
        );
        let mut hits: Vec<u32> = Vec::new();
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let c = (cy * self.cols + cx) as usize;
                for &i in &self.items[self.start[c] as usize..self.start[c + 1] as usize] {
                    if self.entries[i as usize].2.distance_2d(p) <= radius_m {
                        hits.push(i);
                    }
                }
            }
        }
        hits.sort_unstable();
        hits.into_iter()
            .map(|i| {
                let (a, n, _) = self.entries[i as usize];
                (a, n)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute(entries: &[(ActorId, NodeId, Vec3)], p: Vec3, r: f64) -> Vec<(ActorId, NodeId)> {
        entries
            .iter()
            .filter(|(_, _, q)| q.distance_2d(p) <= r)
            .map(|&(a, n, _)| (a, n))
            .collect()
    }

    /// The grid answers what a scan of every entry answers, at radii from inside one cell
    /// to past the whole extent, at points inside and outside it — and on an empty index.
    #[test]
    fn the_grid_answers_what_a_full_scan_answers() {
        let mut entries = Vec::new();
        // A deterministic scatter over 2 km x 1.5 km, including points on cell edges.
        for i in 0..900u32 {
            let x = f64::from((i * 7919) % 2000) + if i % 5 == 0 { 0.0 } else { 0.37 };
            let y = f64::from((i * 104_729) % 1500) - 200.0;
            entries.push((
                ActorId::new(i * 3),
                NodeId::new(10_000 + i),
                Vec3::new(x, y, 0.0),
            ));
        }
        let index = RadioIndex::build(entries.clone());
        for (k, r) in [0.0, 5.0, 127.9, 128.0, 300.0, 500.0, 2_600.0, 1e9]
            .into_iter()
            .enumerate()
        {
            for p in [
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1_000.37, 500.0, 0.0),
                Vec3::new(-3_000.0, 9_000.0, 0.0),
                Vec3::new(1_999.37, -200.0, 0.0),
                entries[k * 97].2,
            ] {
                assert_eq!(
                    index.within(p, r),
                    brute(&entries, p, r),
                    "r = {r}, p = {p:?}"
                );
            }
        }
        assert!(RadioIndex::default().within(Vec3::ZERO, 1e9).is_empty());
        assert!(index.within(Vec3::ZERO, f64::NAN).is_empty());
    }
}
