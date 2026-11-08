//! Country and US state polygons for IEEE 1609.2 `identifiedRegion` checks.
//!
//! A certificate's region may be `countryOnly(n)`, `countryAndRegions(n, [r…])` or a
//! sequence of them (`Ieee1609Dot2BaseTypes.asn` l.462-501). The receiver places the
//! SPDU's `generationLocation` in that region; this module answers "is this point inside
//! country `n`" and "inside US state `r`" from polygons compiled into the binary.
//!
//! # Data
//!
//! `data/regions.bin` is written by `tools/regions/build.py` from two public-domain
//! sources (with sha256 and URLs):
//! - Natural Earth admin-0 countries, 1:50m, v5.1.1: Canada (124) and Mexico (484);
//! - US Census cartographic boundary states, 1:20m, 2020 (`cb_2020_us_state_20m`), keyed
//!   by `STATEFP`, the 2010 FIPS state code 1609.2 names for `countryAndRegions(840, …)`.
//!
//! The United States (840) is the union of the 50 states and DC: Natural Earth's 1:50m
//! coastline puts parts of Manhattan, Miami, San Francisco and Honolulu in the sea, which
//! would refuse a `countryOnly(840)` certificate on those streets (measured by
//! `build.py --check worlds/cities.yaml`). FIPS 72 (Puerto Rico) is ISO 3166-1 630, so it
//! is a state region but not part of 840. Any other country, and any region code of a
//! country other than 840, is undefined here and the caller counts it as unchecked.
//!
//! # Lookup
//!
//! Rings are integer microdegrees (scaled to the 1e-7 degree units of a `TwoDLocation`).
//! A 0.25° grid over each region's box marks every cell *inside*, *outside* or *border*;
//! a border cell is one that a ring edge meets. A point in an inside or outside cell is
//! answered from the grid. In a border cell the crossing-number (even-odd) test runs, in
//! integer arithmetic, over only the edges whose latitude range meets that cell's 0.25°
//! band: an edge outside the band cannot straddle the point's latitude, so the answer is
//! the same as over every edge (the self-test checks both). The sources' vertices are
//! kept as published; this band index, not a vertex budget, bounds the cost.
//!
//! Determinism: integer rings, integer cell indices and an integer crossing test, so a
//! point's verdict is the same on every machine.

use std::sync::OnceLock;

/// The compiled-in polygons (`tools/regions/build.py`).
static DATA: &[u8] = include_bytes!("../data/regions.bin");

/// One grid cell in 1e-7 degree: 0.25°.
const CELL: i64 = 2_500_000;
/// Latitude and longitude offsets that make cell indices non-negative, 1e-7 degree.
const LAT0: i64 = 900_000_000;
const LON0: i64 = 1_800_000_000;

const OUTSIDE: u8 = 0;
const INSIDE: u8 = 1;
const BORDER: u8 = 2;

/// Kind byte in `regions.bin`.
const KIND_COUNTRY: u8 = 0;
const KIND_US_STATE: u8 = 1;

/// ISO 3166-1 numeric code of the United States.
pub const US: u16 = 840;
/// FIPS codes in the Census file that are not part of ISO 3166-1 840 (Puerto Rico, 630).
const NOT_840: &[u16] = &[72];

/// A ring edge, `[lat_a, lon_a, lat_b, lon_b]` in 1e-7 degree.
type Edge = [i32; 4];

#[derive(Debug)]
struct Region {
    kind: u8,
    code: u16,
    row0: i64,
    col0: i64,
    rows: i64,
    cols: i64,
    /// Row-major cell classes.
    cells: Vec<u8>,
    /// Rings as `(lat, lon)` in 1e-7 degree, not closed (the self-test's reference).
    #[cfg(test)]
    rings: Vec<Vec<(i32, i32)>>,
    /// Per grid row, the edges whose latitude range meets the row's band.
    bands: Vec<Vec<Edge>>,
}

/// The decoded polygon set.
#[derive(Debug)]
pub struct Regions {
    regions: Vec<Region>,
}

/// The compiled-in polygon set, decoded once.
pub fn regions() -> &'static Regions {
    static R: OnceLock<Regions> = OnceLock::new();
    R.get_or_init(|| Regions::decode(DATA).expect("regions.bin is malformed"))
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let s = self.b.get(self.at..self.at + N)?;
        self.at += N;
        s.try_into().ok()
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take()?))
    }
}

fn row_of(lat: i64) -> i64 {
    (lat + LAT0).div_euclid(CELL)
}

fn col_of(lon: i64) -> i64 {
    (lon + LON0).div_euclid(CELL)
}

impl Regions {
    fn decode(b: &[u8]) -> Option<Regions> {
        let mut r = Reader { b, at: 0 };
        if r.take::<8>()? != *b"V2XRGN01" {
            return None;
        }
        let n = r.u32()?;
        let mut regions = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let kind = r.u8()?;
            let code = r.u16()?;
            let row0 = i64::from(r.i32()?);
            let col0 = i64::from(r.i32()?);
            let rows = i64::from(r.u16()?);
            let cols = i64::from(r.u16()?);
            let nruns = r.u32()?;
            let mut cells = Vec::with_capacity((rows * cols) as usize);
            for _ in 0..nruns {
                let len = r.u16()?;
                let class = r.u8()?;
                if class > BORDER {
                    return None;
                }
                cells.extend(std::iter::repeat_n(class, usize::from(len)));
            }
            if cells.len() as i64 != rows * cols {
                return None;
            }
            let nrings = r.u32()?;
            let mut rings = Vec::with_capacity(nrings as usize);
            for _ in 0..nrings {
                let npts = r.u32()?;
                let mut ring = Vec::with_capacity(npts as usize);
                for _ in 0..npts {
                    let lat = r.i32()?.checked_mul(10)?;
                    let lon = r.i32()?.checked_mul(10)?;
                    ring.push((lat, lon));
                }
                rings.push(ring);
            }
            let mut bands = vec![Vec::new(); rows as usize];
            for ring in &rings {
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    let lo = row_of(i64::from(a.0.min(b.0))) - row0;
                    let hi = row_of(i64::from(a.0.max(b.0))) - row0;
                    for row in lo.max(0)..=hi.min(rows - 1) {
                        bands[row as usize].push([a.0, a.1, b.0, b.1]);
                    }
                }
            }
            regions.push(Region {
                kind,
                code,
                row0,
                col0,
                rows,
                cols,
                cells,
                #[cfg(test)]
                rings,
                bands,
            });
        }
        (r.at == b.len()).then_some(Regions { regions })
    }

    fn find(&self, kind: u8, code: u16) -> Option<&Region> {
        self.regions
            .iter()
            .find(|r| r.kind == kind && r.code == code)
    }

    /// Whether `(lat, lon)` (1e-7 degree) lies in country `iso` (ISO 3166-1 numeric).
    /// `None` when the country is not in the data.
    pub fn in_country(&self, iso: u16, lat: i32, lon: i32) -> Option<bool> {
        if iso == US {
            return Some(self.regions.iter().any(|r| {
                r.kind == KIND_US_STATE && !NOT_840.contains(&r.code) && r.contains(lat, lon)
            }));
        }
        self.find(KIND_COUNTRY, iso).map(|r| r.contains(lat, lon))
    }

    /// Whether `(lat, lon)` lies in region `code` of country `iso`. Only the United
    /// States' regions are defined (2010 FIPS state codes, 1609.2
    /// `CountryAndRegions`); anything else is `None`.
    pub fn in_region(&self, iso: u16, code: u16, lat: i32, lon: i32) -> Option<bool> {
        if iso != US {
            return None;
        }
        self.find(KIND_US_STATE, code).map(|r| r.contains(lat, lon))
    }
}

/// Crossing number (even-odd) over `edges` for the point `(lat, lon)`, integer only.
fn crossings(edges: impl Iterator<Item = Edge>, lat: i32, lon: i32) -> bool {
    let (lat, lon) = (i128::from(lat), i128::from(lon));
    let mut c = false;
    for [ya, xa, yb, xb] in edges {
        let (ya, xa, yb, xb) = (
            i128::from(ya),
            i128::from(xa),
            i128::from(yb),
            i128::from(xb),
        );
        if (ya > lat) != (yb > lat) {
            // lon < xa + (lat - ya)(xb - xa)/(yb - ya), multiplied through by (yb - ya).
            let num = (lat - ya) * (xb - xa);
            let den = yb - ya;
            let lhs = (lon - xa) * den;
            if if den > 0 { lhs < num } else { lhs > num } {
                c = !c;
            }
        }
    }
    c
}

#[cfg(test)]
fn ring_edges(rings: &[Vec<(i32, i32)>]) -> impl Iterator<Item = Edge> + '_ {
    rings.iter().flat_map(|ring| {
        (0..ring.len()).map(move |i| {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            [a.0, a.1, b.0, b.1]
        })
    })
}

impl Region {
    fn cell(&self, lat: i32, lon: i32) -> Option<(usize, usize)> {
        let row = row_of(i64::from(lat)) - self.row0;
        let col = col_of(i64::from(lon)) - self.col0;
        ((0..self.rows).contains(&row) && (0..self.cols).contains(&col))
            .then_some((row as usize, col as usize))
    }

    fn contains(&self, lat: i32, lon: i32) -> bool {
        let Some((row, col)) = self.cell(lat, lon) else {
            return false;
        };
        match self.cells[row * self.cols as usize + col] {
            INSIDE => true,
            OUTSIDE => false,
            _ => crossings(self.bands[row].iter().copied(), lat, lon),
        }
    }

    /// Crossing number over every ring edge: the reference the grid must agree with.
    #[cfg(test)]
    fn contains_brute(&self, lat: i32, lon: i32) -> bool {
        crossings(ring_edges(&self.rings), lat, lon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(lat: f64, lon: f64) -> (i32, i32) {
        ((lat * 1e7).round() as i32, (lon * 1e7).round() as i32)
    }

    /// splitmix64: a fixed, seedable sequence for the self-test.
    fn mix(x: &mut u64) -> u64 {
        *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn uniform(x: &mut u64, lo: i64, hi: i64) -> i32 {
        (lo + (mix(x) % (hi - lo) as u64) as i64) as i32
    }

    /// Exact integer test: does segment a-b meet the closed rectangle?
    fn seg_meets_rect(e: Edge, y0: i64, x0: i64, y1: i64, x1: i64) -> bool {
        let [ay, ax, by, bx] = e.map(i64::from);
        if ay.max(by) < y0 || ay.min(by) > y1 || ax.max(bx) < x0 || ax.min(bx) > x1 {
            return false;
        }
        let (dy, dx) = (i128::from(by - ay), i128::from(bx - ax));
        let mut pos = false;
        let mut neg = false;
        for (cy, cx) in [(y0, x0), (y0, x1), (y1, x0), (y1, x1)] {
            let c = dx * i128::from(cy - ay) - dy * i128::from(cx - ax);
            pos |= c >= 0;
            neg |= c <= 0;
        }
        pos && neg
    }

    #[test]
    fn the_data_decodes_with_every_region() {
        let r = regions();
        assert!(r.find(KIND_COUNTRY, 124).is_some());
        assert!(r.find(KIND_COUNTRY, 484).is_some());
        let states = r.regions.iter().filter(|g| g.kind == KIND_US_STATE).count();
        assert_eq!(states, 52, "50 states, DC and Puerto Rico");
    }

    #[test]
    fn known_places_fall_where_they_are() {
        let r = regions();
        let manhattan = p(40.758, -73.9855);
        let newark = p(40.7357, -74.1724);
        let toronto = p(43.65, -79.38);
        let havana = p(23.1136, -82.3666);
        assert_eq!(r.in_country(840, manhattan.0, manhattan.1), Some(true));
        assert_eq!(r.in_country(840, toronto.0, toronto.1), Some(false));
        assert_eq!(r.in_country(124, toronto.0, toronto.1), Some(true));
        assert_eq!(r.in_region(840, 36, manhattan.0, manhattan.1), Some(true));
        assert_eq!(r.in_region(840, 36, newark.0, newark.1), Some(false));
        assert_eq!(r.in_region(840, 34, newark.0, newark.1), Some(true));
        for iso in [840, 124, 484] {
            assert_eq!(r.in_country(iso, havana.0, havana.1), Some(false));
        }
        assert_eq!(
            r.in_country(192, havana.0, havana.1),
            None,
            "Cuba is not in the data"
        );
        assert_eq!(r.in_region(124, 35, toronto.0, toronto.1), None);
        assert_eq!(r.in_region(840, 99, manhattan.0, manhattan.1), None);
    }

    /// On 10,000 deterministic points the grid answers as brute-force
    /// point-in-polygon does, and every cell a ring edge meets is a border cell while
    /// every other cell carries its centre's verdict.
    #[test]
    fn the_grid_equals_brute_force_point_in_polygon() {
        let r = regions();
        let mut seed = 0x5C_9011_u64;
        let mut checked = 0u32;
        let per = 10_000 / r.regions.len() as u32 + 1;
        for g in &r.regions {
            let (y0, x0) = (g.row0 * CELL - LAT0, g.col0 * CELL - LON0);
            let (y1, x1) = (y0 + g.rows * CELL, x0 + g.cols * CELL);
            let verts: Vec<(i32, i32)> = g.rings.iter().flatten().copied().collect();
            for k in 0..per {
                // Half uniform over the grid's box, half within 0.3° of a vertex, where
                // the border cells are.
                let (lat, lon) = if k % 2 == 0 {
                    (uniform(&mut seed, y0, y1), uniform(&mut seed, x0, x1))
                } else {
                    let v = verts[(mix(&mut seed) % verts.len() as u64) as usize];
                    let j = 3_000_000;
                    (
                        uniform(&mut seed, i64::from(v.0) - j, i64::from(v.0) + j),
                        uniform(&mut seed, i64::from(v.1) - j, i64::from(v.1) + j),
                    )
                };
                assert_eq!(
                    g.contains(lat, lon),
                    g.contains_brute(lat, lon),
                    "region {}/{} at ({lat}, {lon})",
                    g.kind,
                    g.code
                );
                checked += 1;
            }
            // Every cell an edge meets is a border cell.
            let cols = g.cols as usize;
            let mut touched = vec![false; g.cells.len()];
            for e in ring_edges(&g.rings) {
                let ra = row_of(i64::from(e[0].min(e[2]))) - g.row0;
                let rb = row_of(i64::from(e[0].max(e[2]))) - g.row0;
                let ca = col_of(i64::from(e[1].min(e[3]))) - g.col0;
                let cb = col_of(i64::from(e[1].max(e[3]))) - g.col0;
                for row in (ra - 1).max(0)..=(rb + 1).min(g.rows - 1) {
                    for col in (ca - 1).max(0)..=(cb + 1).min(g.cols - 1) {
                        let cy = (g.row0 + row) * CELL - LAT0;
                        let cx = (g.col0 + col) * CELL - LON0;
                        if seg_meets_rect(e, cy, cx, cy + CELL, cx + CELL) {
                            touched[row as usize * cols + col as usize] = true;
                        }
                    }
                }
            }
            for (k, &class) in g.cells.iter().enumerate() {
                let (row, col) = ((k / cols) as i64, (k % cols) as i64);
                if touched[k] {
                    assert_eq!(
                        class, BORDER,
                        "region {}/{} cell {row},{col}",
                        g.kind, g.code
                    );
                    continue;
                }
                if class == BORDER {
                    continue;
                }
                // A cell no edge meets has one verdict throughout; its centre gives it
                // (crossings over the row's band, which the points above prove equal to
                // every edge).
                let cy = (g.row0 + row) * CELL - LAT0 + CELL / 2;
                let cx = (g.col0 + col) * CELL - LON0 + CELL / 2;
                let band = g.bands[row as usize].iter().copied();
                let centre = crossings(band, cy as i32, cx as i32);
                assert_eq!(
                    class == INSIDE,
                    centre,
                    "region {}/{} cell {row},{col}",
                    g.kind,
                    g.code
                );
            }
        }
        assert!(checked >= 10_000);
    }
}
