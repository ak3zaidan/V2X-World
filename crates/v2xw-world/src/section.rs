//! Road cross-sections: which lanes a road carries, of what kind, how wide and where.
//!
//! # Why
//!
//! An OpenStreetMap way describes a street as a line with tags. Before this module the
//! importer read only `lanes`, so a Midtown avenue became four identical general-traffic
//! lanes: its bus lane, its parking-protected bicycle lane and its two parking lanes were
//! not there, the carriageway was two parking lanes narrower than the street, and the
//! pavement the map draws at the kerb lay 2.4 m from the nearest lane instead of against
//! it. This module reads the tags that describe the rest of the cross-section and lays
//! every lane out across the street, kerb to kerb.
//!
//! # What it reads
//!
//! | Element | Tags (OSM wiki) | Built as |
//! |---|---|---|
//! | Bus lanes | `bus:lanes[:forward\|:backward]`, `psv:lanes…` (per lane, `designated`); `lanes:bus[…]`, `lanes:psv[…]` (a count); `busway[:left\|:right\|:both]=lane` | a [`LaneKind::Bus`] lane in place of a general lane — `lanes` counts bus lanes, so the carriageway does not widen |
//! | Cycle lanes | `cycleway[:left\|:right\|:both]=lane\|track\|opposite_lane\|opposite_track`, `…:oneway`, `…:width`, `…:buffer`, `…:separation`, `…:traffic_mode` | a [`LaneKind::Cycle`] lane beside the carriageway (`lane`) or beyond the kerb (`track`) |
//! | Parking lanes | `parking[:left\|:right\|:both]=lane\|yes\|half_on_kerb\|street_side` with `…:orientation`; the older `parking:lane[:side]=parallel\|diagonal\|perpendicular\|marked` | a [`LaneKind::Parking`] lane that admits no class — it is road surface no vehicle drives along |
//!
//! `shared_lane`, `share_busway`, `separate`, `no` and `on_kerb` build nothing: the first
//! two are markings on a general lane, `separate` means the facility is its own way (which
//! the importer reads as such), and `on_kerb` parking is on the pavement.
//!
//! # Where each lane goes
//!
//! The carriageway — every lane on the road surface, general, bus, painted cycle lane and
//! parking — is centred on the way, as mappers draw it. From each kerb outward come the
//! elements beyond it: a cycle track and its buffer, a `street_side` parking bay. Across a
//! side, in NYC practice (NYC DOT *Street Design Manual*, Bike Lane Table): a painted lane
//! runs between the parking lane and traffic (`cycleway:*:traffic_mode=parking` puts it at
//! the kerb instead); a track runs outside the parking lane, with a buffer between the
//! two, and is laid beyond the carriageway the way is centred on. A two-way track keeps right-hand traffic inside
//! itself: on the right of the way the forward half is the outer one.
//!
//! Each lane is assigned to the direction it serves. Parking has no direction: it belongs
//! to the edge whose kerb it is on (the forward edge for the right kerb), or to the one-way
//! road's only edge. A contraflow cycle lane on a one-way street gives the street a
//! backward edge that holds only that lane.
//!
//! # Widths
//!
//! | Element | Default | Source |
//! |---|---|---|
//! | Bus lane | 11 ft (3.353 m) | NYC DOT *Street Design Manual* (2020), "Lanes": buses and trucks need 11-12 ft; "Curb Bus Lane": 11 ft minimum |
//! | Parallel parking lane | 8 ft (2.438 m) | NYC DOT *Street Design Manual*, "Lanes": parking lanes are typically 8 ft |
//! | Angled or perpendicular parking | 5.5 m | **this importer's choice**: an 18 ft stall depth; Manhattan has none |
//! | Painted cycle lane | 5.5 ft (1.676 m) | NYC DOT *Street Design Manual*, Bike Lane Table: 5-6 ft standard; the midpoint |
//! | Protected track | 5 ft (1.524 m) plus a 3 ft (0.914 m) buffer | the same table: a one-way protected lane needs a 4 ft minimum lane and a 3 ft minimum buffer, 7-8 ft in all; 8 ft taken |
//!
//! A `cycleway:<side>:width` or `cycleway:width` tag overrides the cycle lane's width.
//! Where the way has a `width` tag (the carriageway, kerb to kerb), the general lanes share
//! what the parking and painted cycle lanes leave of it — see
//! [`CrossSection::general_width_for`].
//!
//! Everything here is arithmetic on the tags, so the section is the same on every run.

use serde::{Deserialize, Serialize};

use crate::model::{ClassMask, LaneKind};
use crate::osm::Tags;

/// Which cross-section elements the importer builds, and their default widths.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrossSectionOptions {
    /// Build bus lanes from the bus-lane tags. Off, every lane is a general lane, as before.
    pub bus_lanes: bool,
    /// Build cycle lanes and tracks tagged on a road.
    pub cycle_lanes: bool,
    /// Build parking lanes.
    pub parking_lanes: bool,
    /// Width of a bus lane, metres: 11 ft, NYC DOT *Street Design Manual*.
    pub bus_lane_width_m: f64,
    /// Width of a parallel parking lane, metres: 8 ft, NYC DOT *Street Design Manual*.
    pub parking_lane_width_m: f64,
    /// Depth of angled or perpendicular parking, metres (this importer's choice, 18 ft).
    pub angled_parking_depth_m: f64,
    /// Width of a painted cycle lane, metres: 5.5 ft, NYC DOT Bike Lane Table (5-6 ft).
    pub cycle_lane_width_m: f64,
    /// Width of a protected cycle track, metres: 5 ft, NYC DOT Bike Lane Table.
    pub cycle_track_width_m: f64,
    /// Buffer between a cycle track (or a buffered lane) and what is beside it, metres:
    /// 3 ft, the NYC DOT Bike Lane Table minimum.
    pub cycle_buffer_m: f64,
}

impl Default for CrossSectionOptions {
    fn default() -> Self {
        Self {
            bus_lanes: true,
            cycle_lanes: true,
            parking_lanes: true,
            bus_lane_width_m: 3.353,
            parking_lane_width_m: 2.438,
            angled_parking_depth_m: 5.5,
            cycle_lane_width_m: 1.676,
            cycle_track_width_m: 1.524,
            cycle_buffer_m: 0.914,
        }
    }
}

/// One lane of a road in one direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneSpec {
    /// What it is.
    pub kind: LaneKind,
    /// Its width, metres.
    pub width_m: f64,
    /// Who may use it.
    pub allowed: ClassMask,
    /// Its centre's offset from the way, metres, **left of its own direction of travel**
    /// positive (so a forward lane on the right of the way has a negative offset).
    pub offset_m: f64,
}

/// A road's lanes in both directions, each list rightmost lane first in its own direction.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrossSection {
    /// Lanes along the way's node order.
    pub fwd: Vec<LaneSpec>,
    /// Lanes against it.
    pub bwd: Vec<LaneSpec>,
    /// How far the section reaches right of the way, metres (positive).
    pub right_extent_m: f64,
    /// How far it reaches left of the way, metres (positive).
    pub left_extent_m: f64,
    /// Bus lanes built.
    pub bus_lanes: u32,
    /// Cycle lanes and tracks built.
    pub cycle_lanes: u32,
    /// Parking lanes built.
    pub parking_lanes: u32,
    /// Tag values this module could not use (a bus-lane list of the wrong length, say).
    pub unusable_tags: u32,
    /// The width the carriageway gives to parking lanes, painted cycle lanes and their
    /// buffers, metres: what [`CrossSection::general_width_for`] takes out of a `width` tag.
    pub reserved_m: f64,
}

impl CrossSection {
    /// The widest half of the section: the junction-trimming radius the road contributes.
    pub fn half_width_m(&self) -> f64 {
        self.right_extent_m.max(self.left_extent_m)
    }

    /// The width of one general lane when the way's `width` tag gives the carriageway
    /// `tagged_m` kerb to kerb: what the parking lanes and painted cycle lanes (which are on
    /// the carriageway) leave, shared equally by the `general` general and bus lanes.
    /// `None` when there is nothing left to share.
    pub fn general_width_for(tagged_m: f64, reserved_m: f64, general: u32) -> Option<f64> {
        if general == 0 {
            return None;
        }
        let w = (tagged_m - reserved_m) / f64::from(general);
        (w.is_finite() && w > 0.0).then_some(w)
    }
}

/// A direction of travel along the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Fwd,
    Bwd,
}

/// One element of the physical row across the street.
#[derive(Debug, Clone, Copy)]
struct Element {
    /// The lane, or `None` for a buffer.
    lane: Option<(LaneKind, ClassMask, Dir)>,
    width_m: f64,
    /// On the carriageway (between the kerbs) rather than beyond one.
    on_carriageway: bool,
}

/// What [`build`] needs to know about the road besides its tags.
#[derive(Debug, Clone, Copy)]
pub struct SectionInput {
    /// General-plus-bus lanes along the way (the `lanes` count for that direction).
    pub fwd: u8,
    /// … and against it.
    pub bwd: u8,
    /// Width of one general lane, metres.
    pub general_width_m: f64,
    /// The general lanes' kind.
    pub general_kind: LaneKind,
    /// Who may use a general lane.
    pub general_allowed: ClassMask,
    /// Whether bicycles may ride in general and bus lanes where no cycle lane is provided.
    pub bicycles_on_roads: bool,
}

/// The cycle facility tagged on one side of a road.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CycleTag {
    track: bool,
    /// Runs against the direction its side's traffic runs.
    contraflow: bool,
    /// Both directions.
    two_way: bool,
    width_m: Option<f64>,
    buffered: bool,
    /// The parking lane is between it and the traffic.
    parking_inside: bool,
}

/// The parking tagged on one side of a road.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ParkingTag {
    width_m: f64,
    on_carriageway: bool,
}

fn side_value<'a>(tags: &'a Tags, family: &str, side: &str) -> Option<&'a str> {
    tags.get(&format!("{family}:{side}"))
        .or_else(|| tags.get(&format!("{family}:both")))
        .or_else(|| tags.get(family))
}

fn metres(v: &str) -> Option<f64> {
    let v = v.trim();
    if let Some((ft, rest)) = v.split_once('\'') {
        let inches = rest.trim_end_matches('"').trim();
        let inches = if inches.is_empty() { 0.0 } else { inches.parse::<f64>().ok()? };
        return Some((ft.trim().parse::<f64>().ok()? * 12.0 + inches) * 0.0254);
    }
    let v = v.trim_end_matches('m').trim();
    v.parse::<f64>().ok().filter(|x| x.is_finite() && *x > 0.3 && *x < 10.0)
}

fn cycle_tag(tags: &Tags, side: &str, one_way_road: bool) -> Option<CycleTag> {
    let specific = tags.get(&format!("cycleway:{side}"));
    let both = tags.get("cycleway:both");
    let plain = tags.get("cycleway");
    // A bare `cycleway=lane` on a one-way street is on its right; on a two-way street, on
    // both sides (OSM wiki, Key:cycleway).
    let value = specific.or(both).or_else(|| {
        let v = plain?;
        if one_way_road && side == "left" && !v.starts_with("opposite") {
            return None;
        }
        if one_way_road && side == "right" && v.starts_with("opposite") {
            return None;
        }
        Some(v)
    })?;
    let (track, opposite) = match value.trim() {
        "lane" => (false, false),
        "track" => (true, false),
        "opposite_lane" => (false, true),
        "opposite_track" => (true, true),
        _ => return None,
    };
    let key = |k: &str| {
        tags.get(&format!("cycleway:{side}:{k}"))
            .or_else(|| tags.get(&format!("cycleway:both:{k}")))
            .or_else(|| tags.get(&format!("cycleway:{k}")))
    };
    let oneway = key("oneway");
    Some(CycleTag {
        track,
        contraflow: opposite || oneway == Some("-1"),
        two_way: oneway == Some("no"),
        width_m: key("width").and_then(metres),
        buffered: track
            || key("buffer").is_some_and(|v| v != "no")
            || key("separation").is_some_and(|v| v != "no"),
        parking_inside: track || key("traffic_mode").is_some_and(|v| v.starts_with("parking")),
    })
}

fn parking_tag(tags: &Tags, side: &str, options: &CrossSectionOptions) -> Option<ParkingTag> {
    let orientation = |v: Option<&str>| match v.map(str::trim) {
        Some("diagonal" | "perpendicular") => options.angled_parking_depth_m,
        _ => options.parking_lane_width_m,
    };
    if let Some(v) = side_value(tags, "parking", side) {
        let orient = tags
            .get(&format!("parking:{side}:orientation"))
            .or_else(|| tags.get("parking:both:orientation"))
            .or_else(|| tags.get("parking:orientation"));
        return match v.trim() {
            "lane" | "yes" => Some(ParkingTag {
                width_m: orientation(orient),
                on_carriageway: true,
            }),
            "half_on_kerb" => Some(ParkingTag {
                width_m: 0.5 * orientation(orient),
                on_carriageway: true,
            }),
            "street_side" => Some(ParkingTag {
                width_m: orientation(orient),
                on_carriageway: false,
            }),
            _ => None,
        };
    }
    match side_value(tags, "parking:lane", side).map(str::trim) {
        Some(v @ ("parallel" | "diagonal" | "perpendicular" | "marked")) => Some(ParkingTag {
            width_m: orientation(Some(v)),
            on_carriageway: true,
        }),
        _ => None,
    }
}

/// Which of `n` lanes (rightmost first) are bus lanes, or `None` if the tags say nothing
/// usable about this direction.
fn bus_positions(tags: &Tags, dir: &str, n: u8, one_way: bool, two_way_share: bool) -> Option<Vec<bool>> {
    if n == 0 {
        return None;
    }
    let n = usize::from(n);
    let list = |key: &str| -> Option<Vec<bool>> {
        let raw = tags.get(key)?;
        let mut v: Vec<bool> = raw.split('|').map(|t| t.trim() == "designated").collect();
        if v.len() != n {
            return None;
        }
        v.reverse(); // OSM lists lanes left to right; index 0 is the rightmost
        Some(v)
    };
    let from_list = list(&format!("bus:lanes:{dir}"))
        .or_else(|| list(&format!("psv:lanes:{dir}")))
        .or_else(|| if one_way { list("bus:lanes").or_else(|| list("psv:lanes")) } else { None });
    if let Some(v) = from_list {
        return Some(v);
    }
    let count = |key: &str| tags.get(key).and_then(|v| v.trim().parse::<usize>().ok());
    let mut k = count(&format!("lanes:bus:{dir}")).or_else(|| count(&format!("lanes:psv:{dir}")));
    if k.is_none() {
        let total = count("lanes:bus").or_else(|| count("lanes:psv"));
        k = if one_way {
            total
        } else if two_way_share {
            total.filter(|t| t % 2 == 0).map(|t| t / 2)
        } else {
            None
        };
    }
    let mut left = false;
    if k.is_none() {
        let right = matches!(tags.get("busway:right"), Some("lane"))
            || matches!(tags.get("busway:both"), Some("lane"))
            || (matches!(tags.get("busway"), Some("lane")));
        let on_left = matches!(tags.get("busway:left"), Some("lane"));
        // On a two-way road the left side's bus lane serves the backward direction, the
        // right side's the forward one.
        let (r, l) = if one_way {
            (right, on_left)
        } else if dir == "forward" {
            (right, false)
        } else {
            (on_left || matches!(tags.get("busway:both"), Some("lane")) || matches!(tags.get("busway"), Some("lane")), false)
        };
        if r {
            k = Some(1);
        } else if l {
            k = Some(1);
            left = true;
        }
    }
    let k = k?.min(n);
    if k == 0 {
        return None;
    }
    Some((0..n).map(|i| if left { i >= n - k } else { i < k }).collect())
}

/// Lays out the road's cross-section; see the module documentation.
pub fn build(tags: &Tags, input: &SectionInput, options: &CrossSectionOptions) -> CrossSection {
    let mut out = CrossSection::default();
    let one_way = input.fwd == 0 || input.bwd == 0;
    let one_way_road = one_way;

    // --- the traffic lanes, rightmost first in each direction -----------------------
    let bus = |dir: &str, n: u8, out: &mut CrossSection| -> Vec<bool> {
        if !options.bus_lanes || n == 0 {
            return vec![false; usize::from(n)];
        }
        match bus_positions(tags, dir, n, one_way, true) {
            Some(v) => v,
            None => {
                let any = ["bus:lanes", "lanes:bus", "psv:lanes", "lanes:psv"]
                    .iter()
                    .any(|k| tags.has(k) || tags.has(&format!("{k}:{dir}")));
                if any && one_way {
                    out.unusable_tags += 1;
                }
                vec![false; usize::from(n)]
            }
        }
    };
    let bus_f = bus("forward", input.fwd, &mut out);
    let bus_b = bus("backward", input.bwd, &mut out);

    // --- what each side carries ------------------------------------------------------
    let default_dir = |side: &str| match (side, input.fwd > 0, input.bwd > 0) {
        ("right", true, _) | ("left", true, false) => Dir::Fwd,
        _ => Dir::Bwd,
    };
    let flip = |d: Dir| if d == Dir::Fwd { Dir::Bwd } else { Dir::Fwd };
    let mut cycle_dirs: [Vec<Dir>; 2] = [Vec::new(), Vec::new()];
    // Elements from the carriageway outward, per side (0 = right, 1 = left).
    let mut sides: [Vec<Element>; 2] = [Vec::new(), Vec::new()];
    for (s, side) in ["right", "left"].iter().enumerate() {
        let parking = if options.parking_lanes {
            parking_tag(tags, side, options)
        } else {
            None
        };
        let cycle = if options.cycle_lanes {
            cycle_tag(tags, side, one_way_road)
        } else {
            None
        };
        let d = default_dir(side);
        let park_el = parking.map(|p| Element {
            lane: Some((LaneKind::Parking, ClassMask::NONE, d)),
            width_m: p.width_m,
            on_carriageway: p.on_carriageway,
        });
        let mut cycle_els: Vec<Element> = Vec::new();
        if let Some(c) = cycle {
            let main = if c.contraflow { flip(d) } else { d };
            let dirs: Vec<Dir> = if c.two_way {
                // Right-hand traffic inside the track: on the right of the way the forward
                // half is the outer one, on the left the backward half is.
                if s == 0 { vec![Dir::Bwd, Dir::Fwd] } else { vec![Dir::Fwd, Dir::Bwd] }
            } else {
                vec![main]
            };
            let width = c.width_m.map_or(
                if c.track { options.cycle_track_width_m } else { options.cycle_lane_width_m },
                |w| if c.two_way { 0.5 * w } else { w },
            );
            // A track is laid beyond the carriageway the way is centred on. Counting a
            // parking-protected track into that carriageway (it is on the roadway in NYC)
            // shifted every Midtown avenue's drive lanes 1.2 m towards the far kerb and
            // put 648 m of pavement on them. Where a mapped pavement lies on a track,
            // `validate`'s `sidewalk-on-cycle-lane` counts it.
            let on_road = !c.track;
            for dir in &dirs {
                cycle_els.push(Element {
                    lane: Some((LaneKind::Cycle, ClassMask::BICYCLE, *dir)),
                    width_m: width,
                    on_carriageway: on_road,
                });
                cycle_dirs[s].push(*dir);
            }
            let buffer = c.buffered.then_some(Element {
                lane: None,
                width_m: options.cycle_buffer_m,
                on_carriageway: on_road,
            });
            // Outward order.
            let row = &mut sides[s];
            if c.parking_inside {
                row.extend(park_el.filter(|p| p.on_carriageway));
                row.extend(buffer);
                row.extend(cycle_els.iter().copied());
                row.extend(park_el.filter(|p| !p.on_carriageway));
            } else {
                row.extend(buffer);
                row.extend(cycle_els.iter().copied());
                row.extend(park_el);
            }
        } else {
            sides[s].extend(park_el);
        }
        out.parking_lanes += u32::from(park_el.is_some());
        out.cycle_lanes += cycle_els.len() as u32;
    }

    // Bicycles may use a bus lane where no bicycle lane is provided (NYC Traffic Rules
    // §4-12(m), as NYC DOT's bus-lane rules summarise them).
    let has_cycle = |d: Dir| cycle_dirs.iter().flatten().any(|x| *x == d);
    let bus_allowed = |d: Dir| {
        let base = ClassMask::BUS.union(ClassMask::EMERGENCY);
        if input.bicycles_on_roads && !has_cycle(d) {
            base.union(ClassMask::BICYCLE)
        } else {
            base
        }
    };
    let traffic = |d: Dir, is_bus: bool| Element {
        lane: Some(if is_bus {
            (LaneKind::Bus, bus_allowed(d), d)
        } else {
            (input.general_kind, input.general_allowed, d)
        }),
        width_m: if is_bus {
            options.bus_lane_width_m.max(input.general_width_m)
        } else {
            input.general_width_m
        },
        on_carriageway: true,
    };

    // --- the physical row, right kerb side to left, in the way's forward frame -----------
    let mut row: Vec<Element> = Vec::new();
    row.extend(sides[0].iter().rev().copied());
    for b in &bus_f {
        row.push(traffic(Dir::Fwd, *b));
    }
    // The backward lanes, from the centre outward: the backward direction's leftmost lane
    // is nearest the centre line.
    for b in bus_b.iter().rev() {
        row.push(traffic(Dir::Bwd, *b));
    }
    row.extend(sides[1].iter().copied());
    out.bus_lanes = (bus_f.iter().chain(&bus_b).filter(|b| **b).count()) as u32;

    let carriageway: f64 = row.iter().filter(|e| e.on_carriageway).map(|e| e.width_m).sum();
    out.reserved_m = row
        .iter()
        .filter(|e| {
            e.on_carriageway
                && !matches!(e.lane, Some((k, _, _)) if matches!(k, LaneKind::Driving | LaneKind::Bus))
        })
        .map(|e| e.width_m)
        .sum();
    let right_off: f64 = row
        .iter()
        .take_while(|e| !e.on_carriageway)
        .map(|e| e.width_m)
        .sum();
    let total: f64 = row.iter().map(|e| e.width_m).sum();
    let mut x = -0.5 * carriageway - right_off;
    out.right_extent_m = -x;
    out.left_extent_m = x + total;
    let mut fwd: Vec<(f64, LaneSpec)> = Vec::new();
    let mut bwd: Vec<(f64, LaneSpec)> = Vec::new();
    for e in &row {
        let centre = x + 0.5 * e.width_m;
        x += e.width_m;
        let Some((kind, allowed, dir)) = e.lane else {
            continue;
        };
        match dir {
            Dir::Fwd => fwd.push((
                centre,
                LaneSpec { kind, width_m: e.width_m, allowed, offset_m: centre },
            )),
            Dir::Bwd => bwd.push((
                -centre,
                LaneSpec { kind, width_m: e.width_m, allowed, offset_m: -centre },
            )),
        }
    }
    fwd.sort_by(|a, b| a.0.total_cmp(&b.0));
    bwd.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.fwd = fwd.into_iter().map(|(_, s)| s).collect();
    out.bwd = bwd.into_iter().map(|(_, s)| s).collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(fwd: u8, bwd: u8) -> SectionInput {
        SectionInput {
            fwd,
            bwd,
            general_width_m: 3.0,
            general_kind: LaneKind::Driving,
            general_allowed: ClassMask::MOTOR_TRAFFIC.union(ClassMask::BICYCLE),
            bicycles_on_roads: true,
        }
    }

    fn tags(pairs: &[(&str, &str)]) -> Tags {
        Tags::from_pairs(pairs.iter().copied())
    }

    #[test]
    fn a_plain_road_is_its_general_lanes_centred_on_the_way() {
        let s = build(&tags(&[]), &input(2, 1), &CrossSectionOptions::default());
        let off: Vec<f64> = s.fwd.iter().map(|l| l.offset_m).collect();
        assert_eq!(off, vec![-3.0, 0.0]);
        assert_eq!(s.bwd.len(), 1);
        assert!((s.bwd[0].offset_m - -3.0).abs() < 1e-12);
        assert!((s.right_extent_m - 4.5).abs() < 1e-12 && (s.left_extent_m - 4.5).abs() < 1e-12);
    }

    #[test]
    fn a_manhattan_avenue_gets_its_bus_track_and_parking_lanes() {
        // First Avenue as mapped: one-way, 4 lanes, the rightmost a bus lane, a
        // parking-protected track on the left, parking both sides.
        let t = tags(&[
            ("oneway", "yes"),
            ("lanes", "4"),
            ("bus:lanes", "|||designated"),
            ("cycleway:left", "track"),
            ("parking:both", "lane"),
        ]);
        let o = CrossSectionOptions::default();
        let s = build(&t, &input(4, 0), &o);
        let kinds: Vec<LaneKind> = s.fwd.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            vec![
                LaneKind::Parking,
                LaneKind::Bus,
                LaneKind::Driving,
                LaneKind::Driving,
                LaneKind::Driving,
                LaneKind::Parking,
                LaneKind::Cycle
            ]
        );
        assert!(s.bwd.is_empty());
        assert_eq!((s.bus_lanes, s.cycle_lanes, s.parking_lanes), (1, 1, 2));
        // Lanes tile the row without overlapping; the track is a buffer beyond the parking.
        for w in s.fwd.windows(2) {
            let gap = (w[1].offset_m - 0.5 * w[1].width_m) - (w[0].offset_m + 0.5 * w[0].width_m);
            assert!(gap > -1e-9, "{w:?}");
        }
        let park = s.fwd[5];
        let track = s.fwd[6];
        let gap = (track.offset_m - 0.5 * track.width_m) - (park.offset_m + 0.5 * park.width_m);
        assert!((gap - o.cycle_buffer_m).abs() < 1e-9);
        // The bus lane admits buses, not cars; bicycles have their track, so not the bus lane.
        assert!(s.fwd[1].allowed.contains_all(ClassMask::BUS));
        assert!(!s.fwd[1].allowed.contains_all(ClassMask::CAR));
        assert!(!s.fwd[1].allowed.contains_all(ClassMask::BICYCLE));
        assert_eq!(s.fwd[0].allowed, ClassMask::NONE);
        // The carriageway (everything but the track and its buffer) is centred on the way.
        let c = s.right_extent_m + s.left_extent_m - track.width_m - o.cycle_buffer_m;
        assert!((s.right_extent_m - 0.5 * c).abs() < 1e-9);
        assert!((s.reserved_m - 2.0 * 2.438).abs() < 1e-9);

        // With no parking beside it, the same.
        let t = tags(&[("oneway", "yes"), ("lanes", "2"), ("cycleway:left", "track")]);
        let s = build(&t, &input(2, 0), &o);
        let track = *s.fwd.last().unwrap();
        assert_eq!(track.kind, LaneKind::Cycle);
        let c = s.right_extent_m + s.left_extent_m - track.width_m - o.cycle_buffer_m;
        assert!((s.right_extent_m - 0.5 * c).abs() < 1e-9);
        assert!(s.reserved_m.abs() < 1e-12);
    }

    #[test]
    fn a_painted_lane_runs_between_parking_and_traffic() {
        let t = tags(&[("cycleway:right", "lane"), ("parking:right", "lane")]);
        let s = build(&t, &input(1, 1), &CrossSectionOptions::default());
        let kinds: Vec<LaneKind> = s.fwd.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, vec![LaneKind::Parking, LaneKind::Cycle, LaneKind::Driving]);
    }

    #[test]
    fn a_contraflow_lane_gives_a_one_way_street_a_backward_cycle_lane() {
        let t = tags(&[("oneway", "yes"), ("cycleway:left", "lane"), ("cycleway:left:oneway", "-1")]);
        let s = build(&t, &input(1, 0), &CrossSectionOptions::default());
        assert_eq!(s.fwd.len(), 1);
        assert_eq!(s.bwd.len(), 1);
        assert_eq!(s.bwd[0].kind, LaneKind::Cycle);
        // It is on the way's left, which is the right of its own direction.
        assert!(s.bwd[0].offset_m < 0.0);
    }

    #[test]
    fn switched_off_nothing_but_general_lanes_is_built() {
        let t = tags(&[("lanes:bus", "1"), ("oneway", "yes"), ("parking:both", "lane"), ("cycleway:left", "track")]);
        let o = CrossSectionOptions {
            bus_lanes: false,
            cycle_lanes: false,
            parking_lanes: false,
            ..CrossSectionOptions::default()
        };
        let s = build(&t, &input(3, 0), &o);
        assert!(s.fwd.iter().all(|l| l.kind == LaneKind::Driving));
        assert!(s.bwd.is_empty());
    }

    #[test]
    fn a_bus_count_goes_to_the_kerb_lanes() {
        let t = tags(&[("oneway", "yes"), ("lanes:bus", "2")]);
        let s = build(&t, &input(4, 0), &CrossSectionOptions::default());
        let kinds: Vec<LaneKind> = s.fwd.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, vec![LaneKind::Bus, LaneKind::Bus, LaneKind::Driving, LaneKind::Driving]);
    }

    #[test]
    fn the_width_tag_is_shared_after_parking() {
        let t = tags(&[("parking:both", "lane")]);
        let reserved = build(&t, &input(1, 1), &CrossSectionOptions::default()).reserved_m;
        assert!((reserved - 2.0 * 2.438).abs() < 1e-9);
        let w = CrossSection::general_width_for(12.0, reserved, 2).unwrap();
        assert!((w - (12.0 - 4.876) / 2.0).abs() < 1e-9);
    }
}
