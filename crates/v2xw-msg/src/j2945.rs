//! The SAE J2945/1 on-board algorithms behind a BSM's Part II: path history, path
//! prediction, and the rules for event flags and exterior lights.
//!
//! # What is cited, what is recalled, what is chosen
//!
//! | Item | Value | Source |
//! |---|---|---|
//! | Path-history point offsets are *anchor − point*, positive south, west and down | — | SAE J2735 2024-09 §6.90 (DF_PathHistory, "Use"), read |
//! | `PathHistoryPointList` holds 1 to 23 points; `TimeOffset` in 10 ms, `1..65535`, 65535 unavailable | — | J2735 2024-09 §6.91, §7.203, read |
//! | Radius of curvature: 10 cm LSB, positive to the **right** (SAE frame), 32767 straight, overflow reads as straight | — | J2735 2024-09 §7.160, read |
//! | Hard-braking event flag: > 0.4 g, or > 0.2 g for a Class 4 bus and Class 7–13 trucks | — | J2735 2024-09 §7.234, read |
//! | Lights-changed flag: the exterior lights changed within the last 2 s | 2 s | J2735 2024-09 §7.234, read |
//! | Path history covers at least 300 m, with at most 15 concise points | 300 m, 15 | SAE J2945/1 §6.3.8, **recalled** (the document is not in this repository) |
//! | Concise representation: a point is kept when the chord's estimated perpendicular error would exceed 1 m | 1 m | J2945/1 Annex (PH_AllowableError), **recalled** |
//! | The longest chord between two kept points | 300 m | this build's choice, stated: J2945/1 has a chord-length limit whose value could not be re-read |
//! | Heading changes below which a chord is treated as straight | 1° | this build's choice |
//! | Path prediction from yaw rate: R = v / ψ̇, straight past 2,500 m | 2,500 m | the radius cap is **recalled** from J2945/1; R = v/ψ̇ is the steady-state kinematic identity |
//! | Path-prediction confidence | 100 % in steady state, falling linearly to 0 as the yaw rate departs from its 1 s mean by 0.1 rad/s | this build's choice: J2735 §6.93 defines the steady-state distinction, J2945/1 its error limits (not re-read) |
//!
//! Every "recalled" or "chosen" row is a field of [`PathHistoryParams`] or
//! [`PathPredictionParams`], so a user can set the value their copy of J2945/1 gives.

use std::collections::VecDeque;

use v2xw_core::geo::GeoOrigin;
use v2xw_core::geom::Vec3;
use v2xw_core::math;
use v2xw_core::time::SimTime;

use crate::j2735::bsm::{
    self, OFFSET_LL_B18_MAX, OFFSET_LL_B18_MIN, PathHistory, PathHistoryPoint, PathPrediction,
    RADIUS_OF_CURVATURE_STRAIGHT, TIME_OFFSET_UNAVAILABLE, VERT_OFFSET_B12_MAX,
    VERT_OFFSET_B12_UNAVAILABLE,
};

/// Standard gravity, m/s².
pub const G_MPS2: f64 = 9.806_65;

/// The J2735 hard-braking threshold for a vehicle of this weight class: 0.2 g for a
/// Class 4 bus or a Class 7–13 truck, 0.4 g for everything else (J2735 2024-09 §7.234).
pub const fn hard_braking_threshold_mps2(heavy: bool) -> f64 {
    if heavy { 0.2 * G_MPS2 } else { 0.4 * G_MPS2 }
}

/// How long the lights-changed flag stays set after the lights change (J2735 §7.234).
pub const LIGHTS_CHANGED_HOLD_NS: u64 = 2_000_000_000;

/// The path-history algorithm's parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathHistoryParams {
    /// The largest perpendicular error a chord may have from the path it stands for,
    /// metres (J2945/1's `PH_AllowableError`, recalled as 1 m).
    pub allowable_error_m: f64,
    /// The longest chord between two kept points, metres (chosen, see the module table).
    pub max_chord_m: f64,
    /// A chord whose heading change is below this is straight, radians.
    pub small_heading_change_rad: f64,
    /// The distance the kept points must cover, metres (J2945/1, recalled as 300 m).
    pub min_distance_m: f64,
    /// The most points a message carries (J2945/1, recalled as 15; J2735 allows 23).
    pub max_points: usize,
    /// Breadcrumbs closer than this to the last one are not taken, metres: a standing
    /// vehicle's GNSS wander is not a path.
    pub min_step_m: f64,
}

impl PathHistoryParams {
    /// The J2945/1 values, as recalled and chosen above.
    pub const J2945_1: PathHistoryParams = PathHistoryParams {
        allowable_error_m: 1.0,
        max_chord_m: 300.0,
        small_heading_change_rad: 1.0 * core::f64::consts::PI / 180.0,
        min_distance_m: 300.0,
        max_points: 15,
        min_step_m: 0.5,
    };
}

impl Default for PathHistoryParams {
    fn default() -> Self {
        Self::J2945_1
    }
}

/// One position the vehicle was at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breadcrumb {
    /// When, on the vehicle's own clock.
    pub t: SimTime,
    /// Where, world-local ENU metres.
    pub pos: Vec3,
    /// Its heading then, ENU radians.
    pub heading_rad: f64,
    /// Its speed then, m/s.
    pub speed_mps: f64,
}

/// The J2945/1 concise path history: the breadcrumbs a vehicle keeps so that straight
/// chords between them stay within [`PathHistoryParams::allowable_error_m`] of the path it
/// actually drove.
///
/// Fed with every position fix ([`PathHistoryBuilder::push`]); read with
/// [`PathHistoryBuilder::points`], newest first, trimmed to the distance and count the
/// parameters ask for.
#[derive(Debug, Clone, PartialEq)]
pub struct PathHistoryBuilder {
    params: PathHistoryParams,
    /// Kept (concise) points, oldest first.
    kept: VecDeque<Breadcrumb>,
    /// The breadcrumb before the newest: the point that is kept when the chord to the
    /// newest one fails.
    prev: Option<Breadcrumb>,
    /// The newest breadcrumb.
    last: Option<Breadcrumb>,
}

impl PathHistoryBuilder {
    /// An empty history.
    pub fn new(params: PathHistoryParams) -> Self {
        Self {
            params,
            kept: VecDeque::new(),
            prev: None,
            last: None,
        }
    }

    /// The parameters.
    pub const fn params(&self) -> &PathHistoryParams {
        &self.params
    }

    /// Forgets everything: a pseudonym change must not carry the old identity's track
    /// into the new identity's messages (J2945/1's privacy rule for the path history,
    /// recalled; the linkage it would give is exactly what a pseudonym change defeats).
    pub fn clear(&mut self) {
        self.kept.clear();
        self.prev = None;
        self.last = None;
    }

    /// Takes one position fix.
    pub fn push(&mut self, crumb: Breadcrumb) {
        let Some(last) = self.last else {
            self.kept.push_back(crumb);
            self.last = Some(crumb);
            return;
        };
        if crumb.pos.distance_2d(last.pos) < self.params.min_step_m {
            return;
        }
        let start = *self.kept.back().expect("the first crumb is always kept");
        // The chord from the last kept point to the new crumb, and the arc it stands for.
        let chord = crumb.pos.distance_2d(start.pos);
        let dphi = wrap_pi(crumb.heading_rad - start.heading_rad).abs();
        let error = if dphi < self.params.small_heading_change_rad {
            0.0
        } else {
            let half = 0.5 * dphi;
            let s = math::sin(half);
            if s.abs() < 1e-9 {
                0.0
            } else {
                let radius = chord / (2.0 * s);
                radius * (1.0 - math::cos(half))
            }
        };
        if (error > self.params.allowable_error_m || chord > self.params.max_chord_m)
            && last.pos.distance_2d(start.pos) > 0.0
        {
            // The chord to the new crumb would stray too far: the previous crumb becomes a
            // kept point and the next chord starts there.
            self.kept.push_back(last);
        }
        self.prev = Some(last);
        self.last = Some(crumb);
        self.trim();
    }

    /// Drops kept points no message would carry any more.
    fn trim(&mut self) {
        // Keep enough to cover the distance twice over and the count once over; the
        // exact trimming against the anchor happens in `points`.
        let cap = self.params.max_points.saturating_mul(2).max(8);
        while self.kept.len() > cap {
            self.kept.pop_front();
        }
    }

    /// The kept points a message anchored at `anchor` carries: newest first, each older
    /// than the anchor's instant, until they cover [`PathHistoryParams::min_distance_m`]
    /// (the first point past it included) or number [`PathHistoryParams::max_points`].
    pub fn points(&self, anchor: Vec3, anchor_t: SimTime) -> Vec<Breadcrumb> {
        let mut out = Vec::new();
        let mut covered = 0.0;
        let mut from = anchor;
        // The newest crumb is the anchor's own fix (or close to it) and is not a history
        // point; the history is the kept points.
        for p in self.kept.iter().rev() {
            if p.t >= anchor_t {
                continue;
            }
            if out.len() >= self.params.max_points {
                break;
            }
            covered += p.pos.distance_2d(from);
            from = p.pos;
            out.push(*p);
            if covered >= self.params.min_distance_m {
                break;
            }
        }
        out
    }
}

/// `x` wrapped to `(-π, π]`.
pub fn wrap_pi(x: f64) -> f64 {
    let two_pi = 2.0 * core::f64::consts::PI;
    let mut y = x % two_pi;
    if y > core::f64::consts::PI {
        y -= two_pi;
    } else if y <= -core::f64::consts::PI {
        y += two_pi;
    }
    y
}

/// The J2735 `PathHistory` of `points` (newest first) for a BSM anchored at `anchor` at
/// `anchor_t`: offsets are anchor − point (J2735 2024-09 §6.90), time offsets backwards in
/// 10 ms. `None` when there is no point to send.
pub fn j2735_path_history(
    points: &[Breadcrumb],
    anchor: Vec3,
    anchor_t: SimTime,
    origin: GeoOrigin,
) -> Option<PathHistory> {
    if points.is_empty() {
        return None;
    }
    let (alat, alon, aalt) = origin.to_geodetic(anchor);
    let crumbs: Vec<PathHistoryPoint> = points
        .iter()
        .take(bsm::MAX_PATH_HISTORY_POINTS)
        .map(|p| {
            let (lat, lon, alt) = origin.to_geodetic(p.pos);
            let ll = |a: f64, b: f64| -> i32 {
                // 0.1 microdegree, saturating at the type's ±0.0131071 degrees.
                let v = f64::round((a - b) * 1e7) as i64;
                v.clamp(-OFFSET_LL_B18_MAX, OFFSET_LL_B18_MAX) as i32
            };
            let dz = f64::round((aalt - alt) * 10.0) as i64;
            let elevation_offset = if dz.abs() > VERT_OFFSET_B12_MAX {
                VERT_OFFSET_B12_UNAVAILABLE
            } else {
                dz as i16
            };
            let dt_10ms = anchor_t.saturating_sub(p.t) / 10_000_000;
            let time_offset = if dt_10ms >= u64::from(TIME_OFFSET_UNAVAILABLE) {
                TIME_OFFSET_UNAVAILABLE
            } else {
                dt_10ms.max(1) as u16
            };
            PathHistoryPoint::new(ll(alat, lat), ll(alon, lon), elevation_offset, time_offset)
        })
        .collect();
    debug_assert!(OFFSET_LL_B18_MIN < -OFFSET_LL_B18_MAX);
    Some(PathHistory::new(crumbs))
}

/// The path-prediction parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathPredictionParams {
    /// A radius longer than this is sent as straight, metres (recalled from J2945/1).
    pub straight_beyond_m: f64,
    /// The yaw-rate departure from its recent mean at which confidence reaches zero,
    /// rad/s (chosen).
    pub zero_confidence_departure_rad_s: f64,
    /// The time constant of the yaw-rate mean, seconds (chosen).
    pub mean_time_constant_s: f64,
    /// Below this speed the prediction is straight at full confidence: a standing
    /// vehicle has no curvature to predict, m/s.
    pub min_speed_mps: f64,
}

impl PathPredictionParams {
    /// The values in the module table.
    pub const J2945_1: PathPredictionParams = PathPredictionParams {
        straight_beyond_m: 2_500.0,
        zero_confidence_departure_rad_s: 0.1,
        mean_time_constant_s: 1.0,
        min_speed_mps: 1.0,
    };
}

impl Default for PathPredictionParams {
    fn default() -> Self {
        Self::J2945_1
    }
}

/// The J2945/1-style path prediction: a radius from the yaw rate and speed, and a
/// confidence from how steady the yaw rate is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathPredictor {
    params: PathPredictionParams,
    mean_yaw: Option<f64>,
    last_t: Option<SimTime>,
}

impl PathPredictor {
    /// A predictor with no history.
    pub const fn new(params: PathPredictionParams) -> Self {
        Self {
            params,
            mean_yaw: None,
            last_t: None,
        }
    }

    /// Takes one yaw-rate reading (rad/s, positive counter-clockwise) at `t`.
    pub fn push(&mut self, t: SimTime, yaw_rate_rad_s: f64) {
        match (self.mean_yaw, self.last_t) {
            (Some(m), Some(t0)) if t > t0 => {
                let dt = (t - t0) as f64 * 1e-9;
                let a = 1.0 - math::exp(-dt / self.params.mean_time_constant_s.max(1e-3));
                self.mean_yaw = Some(m + a * (yaw_rate_rad_s - m));
            }
            (None, _) => self.mean_yaw = Some(yaw_rate_rad_s),
            _ => {}
        }
        self.last_t = Some(t);
    }

    /// The prediction for a vehicle at `speed_mps` turning at `yaw_rate_rad_s` now.
    pub fn predict(&self, speed_mps: f64, yaw_rate_rad_s: f64) -> PathPrediction {
        let p = &self.params;
        if speed_mps < p.min_speed_mps || !yaw_rate_rad_s.is_finite() {
            return PathPrediction::straight(200);
        }
        let departure = self
            .mean_yaw
            .map_or(0.0, |m| (yaw_rate_rad_s - m).abs());
        let confidence =
            (200.0 * (1.0 - departure / p.zero_confidence_departure_rad_s)).clamp(0.0, 200.0);
        let confidence = f64::round(confidence) as u8;
        if yaw_rate_rad_s.abs() < 1e-9 {
            return PathPrediction::straight(confidence);
        }
        // SAE frame: positive to the right. ENU yaw is positive counter-clockwise (to the
        // left), so a positive yaw rate is a negative radius.
        let radius_m = -speed_mps / yaw_rate_rad_s;
        if radius_m.abs() > p.straight_beyond_m {
            return PathPrediction::straight(confidence);
        }
        let r10 = f64::round(radius_m * 10.0) as i64;
        let r10 = r10.clamp(-32_767, 32_766);
        PathPrediction {
            radius_of_curve: if r10 == i64::from(RADIUS_OF_CURVATURE_STRAIGHT) {
                32_766
            } else {
                r10 as i16
            },
            confidence,
        }
    }
}

/// ETSI `Path` points for `points` (newest first): each a delta from the one before it,
/// the first from `anchor` (ETSI TS 102 894-2, DF Path), in 0.1 microdegree and 10 ms.
/// Returns `(delta_lat, delta_lon, delta_alt_cm, delta_time_10ms)` per point; the DENM
/// and CAM builders turn them into their generated types.
pub fn etsi_path_deltas(
    points: &[Breadcrumb],
    anchor: Vec3,
    anchor_t: SimTime,
    origin: GeoOrigin,
    max_points: usize,
) -> Vec<(i32, i32, i16, u16)> {
    let mut out = Vec::new();
    let (mut plat, mut plon, mut palt) = origin.to_geodetic(anchor);
    let mut pt = anchor_t;
    for p in points.iter().take(max_points) {
        let (lat, lon, alt) = origin.to_geodetic(p.pos);
        // DeltaLatitude/DeltaLongitude ::= INTEGER (-131071..131072), 131072 unavailable.
        let d = |a: f64, b: f64| (f64::round((a - b) * 1e7) as i64).clamp(-131_071, 131_071) as i32;
        // DeltaAltitude ::= INTEGER (-12700..12800), centimetres.
        let dz = (f64::round((alt - palt) * 100.0) as i64).clamp(-12_700, 12_799) as i16;
        let dt = (pt.saturating_sub(p.t) / 10_000_000).clamp(1, 65_535) as u16;
        out.push((d(lat, plat), d(lon, plon), dz, dt));
        plat = lat;
        plon = lon;
        palt = alt;
        pt = p.t;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crumb(t_s: f64, x: f64, y: f64, heading: f64) -> Breadcrumb {
        Breadcrumb {
            t: (t_s * 1e9) as SimTime,
            pos: Vec3::new(x, y, 0.0),
            heading_rad: heading,
            speed_mps: 10.0,
        }
    }

    /// A straight road keeps only the chord-length points: 1 km at 10 m/s in 0.1 s steps
    /// gives the first crumb and one point every 300 m.
    #[test]
    fn a_straight_path_needs_few_points() {
        let mut ph = PathHistoryBuilder::new(PathHistoryParams::J2945_1);
        for i in 0..=1000 {
            let t = f64::from(i) * 0.1;
            ph.push(crumb(t, f64::from(i), 0.0, 0.0));
        }
        let pts = ph.points(Vec3::new(1000.0, 0.0, 0.0), 100_000_000_000);
        assert!(pts.len() <= 3, "{} points for a straight 1 km", pts.len());
        // They cover at least 300 m behind the anchor.
        let far = pts.last().expect("points").pos.x;
        assert!(1000.0 - far >= 300.0, "covers only {} m", 1000.0 - far);
    }

    /// On a circle of radius 50 m the kept chords stay within the allowable error of the
    /// arc, which is the property the concise representation exists for.
    #[test]
    fn a_curve_is_kept_within_the_allowable_error() {
        let mut ph = PathHistoryBuilder::new(PathHistoryParams::J2945_1);
        let r = 50.0;
        let mut t = 0.0;
        let mut all = Vec::new();
        for i in 0..400 {
            let a = f64::from(i) * 0.02; // 1 m per step
            let p = crumb(t, r * math::cos(a), r * math::sin(a), a + core::f64::consts::FRAC_PI_2);
            all.push(p.pos);
            ph.push(p);
            t += 0.1;
        }
        let kept: Vec<Vec3> = ph.kept.iter().map(|b| b.pos).collect();
        assert!(kept.len() > 10, "{} kept points on a 400 m circle", kept.len());
        // Every recorded position lies within 1.2 m of the polyline through kept points
        // (1 m, plus the 1 m step's own discretisation).
        for p in &all[..all.len() - 2] {
            let d = kept
                .windows(2)
                .map(|w| seg_dist(*p, w[0], w[1]))
                .fold(f64::INFINITY, f64::min);
            assert!(d <= 1.2, "a point is {d} m from the concise path");
        }
    }

    fn seg_dist(p: Vec3, a: Vec3, b: Vec3) -> f64 {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let l2 = dx * dx + dy * dy;
        let u = if l2 > 0.0 {
            (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        math::hypot(p.x - (a.x + u * dx), p.y - (a.y + u * dy))
    }

    /// Anchor − point: a point 100 m south of the anchor has a positive latitude offset.
    #[test]
    fn path_history_offsets_are_anchor_minus_point() {
        let origin = GeoOrigin::new(40.75, -73.99, 0.0);
        let pts = [crumb(0.0, 0.0, -100.0, 0.0)];
        let ph = j2735_path_history(&pts, Vec3::new(0.0, 0.0, 0.0), 10_000_000_000, origin)
            .expect("one point");
        assert!(ph.crumb_data[0].lat_offset > 0, "{:?}", ph.crumb_data[0]);
        assert_eq!(ph.crumb_data[0].lon_offset, 0);
        assert_eq!(ph.crumb_data[0].time_offset, 1000);
    }

    /// A left turn (positive ENU yaw rate) is a negative radius: J2735 counts right as
    /// positive.
    #[test]
    fn a_left_turn_has_a_negative_radius() {
        let mut pp = PathPredictor::new(PathPredictionParams::J2945_1);
        for i in 0..20 {
            pp.push(i * 100_000_000, 0.2);
        }
        let p = pp.predict(10.0, 0.2);
        assert_eq!(p.radius_of_curve, -500);
        assert_eq!(p.confidence, 200);
        let straight = pp.predict(10.0, 0.001);
        assert_eq!(straight.radius_of_curve, RADIUS_OF_CURVATURE_STRAIGHT);
        // A sudden change of yaw rate lowers the confidence.
        assert!(pp.predict(10.0, 0.25).confidence < 200);
    }

    #[test]
    fn hard_braking_depends_on_the_weight_class() {
        assert!((hard_braking_threshold_mps2(false) - 3.922_66).abs() < 1e-4);
        assert!((hard_braking_threshold_mps2(true) - 1.961_33).abs() < 1e-4);
    }
}
