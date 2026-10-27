//! The geometry, kinematics and filtering the WP-10 checks of [`crate::ts103759`] are built
//! from (docs/design/frameworks/00-adoption-plan.md WP-10, f2md.md §5 P5–P8).
//!
//! Re-implemented from the cited papers and textbooks. F2MD was read for behaviour only
//! and no code was copied.
//!
//! * [`CvKalman`]: a constant-velocity Kalman filter per axis with the continuous
//!   white-noise-acceleration process model (Bar-Shalom, Li and Kirubarajan, *Estimation
//!   with Applications to Tracking and Navigation*, Wiley 2001, §6.2.2), and its
//!   normalised innovation squared (NIS, *ibid.* §5.4.2).
//! * [`chi2_quantile`]: the Wilson–Hilferty cube-root approximation to a χ² quantile
//!   (Wilson and Hilferty, PNAS 17, 1931).
//! * [`reachable_distance`]: the closed-form minimum and maximum distance a vehicle can
//!   cover in Δt from speed v₀ to speed v₁ under bounded acceleration and braking, the
//!   envelope F2MD's PositionSpeedConsistency describes (Kamel et al., IEEE TVT 69(6),
//!   2020), re-derived here from constant-acceleration kinematics.
//! * [`Footprint`] and [`overlap_ratio`]: oriented rectangles from the claimed vehicle
//!   size, a separating-axis test (Gottschalk, Lin and Manocha, SIGGRAPH 1996) as the
//!   early-out, and the convex intersection area by Sutherland–Hodgman clipping.
//! * [`circle_inside_fraction`] and [`interval_inside_fraction`]: the confidence-overlap
//!   factors of CaTCh (Kamel et al., "CaTCh: a confidence range tolerant misbehavior
//!   detection approach", IEEE WCNC 2019).
//!
//! Everything here is a pure function of its arguments: no clock, no RNG, no world.

use v2xw_core::math;

// ---------------------------------------------------------------------------------------
// Oriented boxes
// ---------------------------------------------------------------------------------------

/// Where a claimed position sits on the vehicle's footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReferencePoint {
    /// The centre of the footprint (SAE J2945/1 BSM position, as read).
    #[default]
    Centre,
    /// The centre of the front edge of the footprint (ETSI TS 102 894-2
    /// `ReferencePosition` for a CAM, as read).
    FrontCentre,
}

/// One vehicle's footprint on the ground: an oriented rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Footprint {
    /// The rectangle's centre, east, metres.
    pub cx: f64,
    /// The rectangle's centre, north, metres.
    pub cy: f64,
    /// The heading of the long axis, ENU radians.
    pub heading_rad: f64,
    /// Length along the heading, metres.
    pub length_m: f64,
    /// Width across the heading, metres.
    pub width_m: f64,
}

impl Footprint {
    /// The footprint of a vehicle whose claimed position is `(x, y)` at `reference`.
    #[must_use]
    pub fn from_claim(
        x: f64,
        y: f64,
        heading_rad: f64,
        length_m: f64,
        width_m: f64,
        reference: ReferencePoint,
    ) -> Self {
        let (s, c) = math::sin_cos(heading_rad);
        let back = match reference {
            ReferencePoint::Centre => 0.0,
            ReferencePoint::FrontCentre => 0.5 * length_m,
        };
        Self {
            cx: x - back * c,
            cy: y - back * s,
            heading_rad,
            length_m: length_m.max(0.0),
            width_m: width_m.max(0.0),
        }
    }

    /// The area, square metres.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.length_m * self.width_m
    }

    /// The four corners, counter-clockwise.
    #[must_use]
    pub fn corners(&self) -> [(f64, f64); 4] {
        let (s, c) = math::sin_cos(self.heading_rad);
        let (hl, hw) = (0.5 * self.length_m, 0.5 * self.width_m);
        let p = |a: f64, b: f64| (self.cx + a * c - b * s, self.cy + a * s + b * c);
        [p(hl, -hw), p(hl, hw), p(-hl, hw), p(-hl, -hw)]
    }

    /// This footprint moved by `(dx, dy)`.
    #[must_use]
    pub fn shifted(mut self, dx: f64, dy: f64) -> Self {
        self.cx += dx;
        self.cy += dy;
        self
    }
}

/// Whether some axis separates the two rectangles (the separating-axis theorem: two
/// convex polygons are disjoint iff their projections on one of their edge normals are).
#[must_use]
pub fn separated(a: &Footprint, b: &Footprint) -> bool {
    let (ca, cb) = (a.corners(), b.corners());
    for h in [a.heading_rad, b.heading_rad] {
        let (s, c) = math::sin_cos(h);
        for (ux, uy) in [(c, s), (-s, c)] {
            let proj = |pts: &[(f64, f64); 4]| {
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                for (x, y) in pts {
                    let d = x * ux + y * uy;
                    lo = lo.min(d);
                    hi = hi.max(d);
                }
                (lo, hi)
            };
            let (alo, ahi) = proj(&ca);
            let (blo, bhi) = proj(&cb);
            if ahi <= blo || bhi <= alo {
                return true;
            }
        }
    }
    false
}

/// The area two rectangles share, square metres (Sutherland–Hodgman clipping of one by
/// the other's four edges, then the shoelace formula).
#[must_use]
pub fn intersection_area(a: &Footprint, b: &Footprint) -> f64 {
    if a.area() <= 0.0 || b.area() <= 0.0 || separated(a, b) {
        return 0.0;
    }
    let mut poly: Vec<(f64, f64)> = a.corners().to_vec();
    let clip = b.corners();
    for i in 0..4 {
        let (p0, p1) = (clip[i], clip[(i + 1) % 4]);
        // Inside = left of the directed edge p0 → p1 (the corners are counter-clockwise).
        let side = |q: (f64, f64)| (p1.0 - p0.0) * (q.1 - p0.1) - (p1.1 - p0.1) * (q.0 - p0.0);
        let mut next: Vec<(f64, f64)> = Vec::with_capacity(poly.len() + 2);
        for j in 0..poly.len() {
            let cur = poly[j];
            let prev = poly[(j + poly.len() - 1) % poly.len()];
            let (sc, sp) = (side(cur), side(prev));
            if sc >= 0.0 {
                if sp < 0.0 {
                    next.push(cut(prev, cur, sp, sc));
                }
                next.push(cur);
            } else if sp >= 0.0 {
                next.push(cut(prev, cur, sp, sc));
            }
        }
        poly = next;
        if poly.is_empty() {
            return 0.0;
        }
    }
    let mut twice = 0.0;
    for j in 0..poly.len() {
        let (x0, y0) = poly[j];
        let (x1, y1) = poly[(j + 1) % poly.len()];
        twice += x0 * y1 - x1 * y0;
    }
    (0.5 * twice).abs()
}

fn cut(p: (f64, f64), q: (f64, f64), sp: f64, sq: f64) -> (f64, f64) {
    let t = sp / (sp - sq);
    (p.0 + t * (q.0 - p.0), p.1 + t * (q.1 - p.1))
}

/// The share of the smaller footprint the two have in common, in `[0, 1]`: 1 when one
/// footprint lies wholly inside the other, 0 when they do not touch.
#[must_use]
pub fn overlap_ratio(a: &Footprint, b: &Footprint) -> f64 {
    let small = a.area().min(b.area());
    if small <= 0.0 {
        return 0.0;
    }
    (intersection_area(a, b) / small).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------------------
// The reachable-distance envelope
// ---------------------------------------------------------------------------------------

/// The least and greatest distance a vehicle can cover in `dt_s` seconds starting at
/// `v0` and ending at `v1` m/s, accelerating at most `a` and braking at most `b` m/s²,
/// never faster than `v_max` and never backwards.
///
/// The greatest distance accelerates to a peak and brakes down to `v1`; the least brakes
/// to a valley (or to a stop) and accelerates up to `v1`. When `v1` cannot be reached from
/// `v0` in `dt_s` at all, the bound returned is the intersection of the forward envelope
/// from `v0` and the backward envelope into `v1` (each ignores the other end), which is
/// then empty (`min > max`): the speed pair itself is implausible, and the distance check
/// reports by how much.
#[must_use]
pub fn reachable_distance(v0: f64, v1: f64, dt_s: f64, a: f64, b: f64, v_max: f64) -> (f64, f64) {
    let (v0, v1) = (v0.max(0.0), v1.max(0.0));
    let dt = dt_s.max(0.0);
    let (a, b) = (a.max(1e-9), b.max(1e-9));
    let vm = v_max.max(v0).max(v1);
    let feasible = v1 - v0 <= a * dt + 1e-12 && v0 - v1 <= b * dt + 1e-12;
    if !feasible {
        // Forward from v0: brake to a stop at most, accelerate to vm at most.
        let fwd_min = if v0 >= b * dt {
            v0 * dt - 0.5 * b * dt * dt
        } else {
            v0 * v0 / (2.0 * b)
        };
        let fwd_max = accel_capped(v0, a, dt, vm);
        // Backward into v1: having been slower (accelerated into v1) or faster (braked).
        let bwd_min = if v1 >= a * dt {
            v1 * dt - 0.5 * a * dt * dt
        } else {
            v1 * v1 / (2.0 * a)
        };
        let bwd_max = accel_capped(v1, b, dt, vm);
        return (fwd_min.max(bwd_min), fwd_max.min(bwd_max));
    }
    // Greatest: accelerate to the peak vp, then brake to v1.
    let inv = 1.0 / a + 1.0 / b;
    let vp = (dt + v0 / a + v1 / b) / inv;
    let d_max = if vp <= vm {
        (vp * vp - v0 * v0) / (2.0 * a) + (vp * vp - v1 * v1) / (2.0 * b)
    } else {
        let t1 = (vm - v0) / a;
        let t2 = (vm - v1) / b;
        (vm * vm - v0 * v0) / (2.0 * a) + (vm * vm - v1 * v1) / (2.0 * b) + vm * (dt - t1 - t2)
    };
    // Least: brake to the valley vv, then accelerate to v1; a stop in between when the
    // valley would be negative.
    let vv = (v0 / b + v1 / a - dt) / inv;
    let d_min = if vv >= 0.0 {
        (v0 * v0 - vv * vv) / (2.0 * b) + (v1 * v1 - vv * vv) / (2.0 * a)
    } else {
        v0 * v0 / (2.0 * b) + v1 * v1 / (2.0 * a)
    };
    (d_min.max(0.0), d_max.max(0.0))
}

/// Distance covered in `dt` starting at `v`, changing speed at `rate` up to `cap`.
fn accel_capped(v: f64, rate: f64, dt: f64, cap: f64) -> f64 {
    let t_cap = ((cap - v) / rate).max(0.0);
    if t_cap >= dt {
        v * dt + 0.5 * rate * dt * dt
    } else {
        v * t_cap + 0.5 * rate * t_cap * t_cap + cap * (dt - t_cap)
    }
}

/// How much shorter than its path a chord is on a constant-curvature arc that turns by
/// `turn_rad`: chord = path · sin(θ/2)/(θ/2). Used to compare a straight-line
/// displacement with a path-length envelope on a curve.
#[must_use]
pub fn chord_factor(turn_rad: f64) -> f64 {
    let h = 0.5 * turn_rad.abs().min(core::f64::consts::PI);
    if h < 1e-6 { 1.0 } else { math::sin(h) / h }
}

// ---------------------------------------------------------------------------------------
// The constant-velocity Kalman filter
// ---------------------------------------------------------------------------------------

/// One axis of the filter: position and velocity, and their 2 × 2 covariance.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Axis {
    p: f64,
    v: f64,
    pp: f64,
    pv: f64,
    vv: f64,
}

impl Axis {
    fn new(p: f64, v: f64, rp: f64, rv: f64) -> Self {
        Self {
            p,
            v,
            pp: rp,
            pv: 0.0,
            vv: rv,
        }
    }

    /// Predict over `dt` with the white-noise-acceleration process noise of spectral
    /// density `q`, then update with the measurement `(zp, zv)` of variances `(rp, rv)`.
    /// Returns this axis's NIS (two degrees of freedom).
    fn step(&mut self, dt: f64, q: f64, zp: f64, zv: f64, rp: f64, rv: f64) -> f64 {
        // Predict: x = F x, P = F P Fᵀ + Q, F = [[1, dt], [0, 1]],
        // Q = q · [[dt³/3, dt²/2], [dt²/2, dt]].
        let p = self.p + dt * self.v;
        let v = self.v;
        let pp = self.pp + 2.0 * dt * self.pv + dt * dt * self.vv + q * dt * dt * dt / 3.0;
        let pv = self.pv + dt * self.vv + q * dt * dt / 2.0;
        let vv = self.vv + q * dt;
        // Innovation and its covariance S = P + R (H = I).
        let (yp, yv) = (zp - p, zv - v);
        let (s11, s12, s22) = (pp + rp, pv, vv + rv);
        let det = (s11 * s22 - s12 * s12).max(1e-12);
        let (i11, i12, i22) = (s22 / det, -s12 / det, s11 / det);
        let nis = yp * (i11 * yp + i12 * yv) + yv * (i12 * yp + i22 * yv);
        // Gain K = P S⁻¹, update x += K y, P = (I − K) P.
        let k11 = pp * i11 + pv * i12;
        let k12 = pp * i12 + pv * i22;
        let k21 = pv * i11 + vv * i12;
        let k22 = pv * i12 + vv * i22;
        self.p = p + k11 * yp + k12 * yv;
        self.v = v + k21 * yp + k22 * yv;
        let npp = (1.0 - k11) * pp - k12 * pv;
        let npv = (1.0 - k11) * pv - k12 * vv;
        let nvv = -k21 * pv + (1.0 - k22) * vv;
        self.pp = npp.max(0.0);
        self.pv = npv;
        self.vv = nvv.max(0.0);
        nis
    }
}

/// A constant-velocity Kalman filter on the claimed east and north position and velocity.
///
/// The measurement is the claimed position and the claimed velocity vector
/// `v·(cos h, sin h)`. The axes are filtered independently: with an isotropic measurement
/// covariance and the per-axis process model they do not couple.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CvKalman {
    x: Axis,
    y: Axis,
}

impl CvKalman {
    /// A filter initialised on its first measurement.
    #[must_use]
    pub fn init(z: [f64; 4], r_pos: f64, r_vel: f64) -> Self {
        Self {
            x: Axis::new(z[0], z[1], r_pos, r_vel),
            y: Axis::new(z[2], z[3], r_pos, r_vel),
        }
    }

    /// One predict-and-update over `dt_s` with the measurement `z = (x, vx, y, vy)` of
    /// position variance `r_pos` and velocity variance `r_vel`; returns the NIS (four
    /// degrees of freedom).
    pub fn step(&mut self, dt_s: f64, q: f64, z: [f64; 4], r_pos: f64, r_vel: f64) -> f64 {
        self.x.step(dt_s, q, z[0], z[1], r_pos, r_vel) + self.y.step(dt_s, q, z[2], z[3], r_pos, r_vel)
    }

    /// The filtered position.
    #[must_use]
    pub fn position(&self) -> (f64, f64) {
        (self.x.p, self.y.p)
    }
}

/// The `p` quantile of a χ² distribution with `k` degrees of freedom, `z` being the
/// standard-normal quantile of `p` (Wilson–Hilferty: χ² ≈ k·(1 − 2/(9k) + z·√(2/(9k)))³).
/// At k = 4 and z = 3.0902 (p = 0.999) it gives 18.72 against the exact 18.47, so it
/// errs on the side of not firing.
#[must_use]
pub fn chi2_quantile(k: f64, z: f64) -> f64 {
    let k = k.max(1.0);
    let c = 2.0 / (9.0 * k);
    let base = 1.0 - c + z * math::sqrt(c);
    k * base * base * base
}

// ---------------------------------------------------------------------------------------
// CaTCh confidence overlaps
// ---------------------------------------------------------------------------------------

/// The area two discs of radii `r1`, `r2` whose centres are `d` apart have in common.
#[must_use]
pub fn circle_overlap_area(r1: f64, r2: f64, d: f64) -> f64 {
    let (r1, r2, d) = (r1.max(0.0), r2.max(0.0), d.abs());
    if d >= r1 + r2 {
        return 0.0;
    }
    if d <= (r1 - r2).abs() {
        let r = r1.min(r2);
        return core::f64::consts::PI * r * r;
    }
    let a1 = ((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1)).clamp(-1.0, 1.0);
    let a2 = ((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2)).clamp(-1.0, 1.0);
    let k = ((-d + r1 + r2) * (d + r1 - r2) * (d - r1 + r2) * (d + r1 + r2)).max(0.0);
    r1 * r1 * math::acos(a1) + r2 * r2 * math::acos(a2) - 0.5 * math::sqrt(k)
}

/// The share of a confidence disc of radius `r` whose centre is `d` from the centre of a
/// plausible disc of radius `big`, that lies inside the plausible disc. A zero radius is
/// the point test.
#[must_use]
pub fn circle_inside_fraction(r: f64, d: f64, big: f64) -> f64 {
    if r <= 1e-9 {
        return if d <= big { 1.0 } else { 0.0 };
    }
    (circle_overlap_area(r, big, d) / (core::f64::consts::PI * r * r)).clamp(0.0, 1.0)
}

/// The share of the interval `[lo, hi]` that lies inside `[a, b]`. A point interval is the
/// point test.
#[must_use]
pub fn interval_inside_fraction(lo: f64, hi: f64, a: f64, b: f64) -> f64 {
    if hi - lo <= 1e-12 {
        return if lo >= a && lo <= b { 1.0 } else { 0.0 };
    }
    ((hi.min(b) - lo.max(a)).max(0.0) / (hi - lo)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(x: f64, y: f64, h: f64) -> Footprint {
        Footprint::from_claim(x, y, h, 5.0, 1.8, ReferencePoint::Centre)
    }

    #[test]
    fn coincident_boxes_overlap_fully_and_disjoint_ones_not_at_all() {
        assert!((overlap_ratio(&car(0.0, 0.0, 0.3), &car(0.0, 0.0, 0.3)) - 1.0).abs() < 1e-9);
        assert_eq!(overlap_ratio(&car(0.0, 0.0, 0.0), &car(0.0, 3.5, 0.0)), 0.0);
        // Half a car length along the axis: half the area.
        let r = overlap_ratio(&car(0.0, 0.0, 0.0), &car(2.5, 0.0, 0.0));
        assert!((r - 0.5).abs() < 1e-9, "{r}");
        // A box crossing at right angles: the 1.8 × 1.8 square in the middle.
        let r = intersection_area(&car(0.0, 0.0, 0.0), &car(0.0, 0.0, core::f64::consts::FRAC_PI_2));
        assert!((r - 1.8 * 1.8).abs() < 1e-9, "{r}");
    }

    #[test]
    fn the_front_reference_point_moves_the_box_back_by_half_a_length() {
        let f = Footprint::from_claim(10.0, 0.0, 0.0, 5.0, 1.8, ReferencePoint::FrontCentre);
        assert!((f.cx - 7.5).abs() < 1e-12 && f.cy.abs() < 1e-12);
    }

    #[test]
    fn the_envelope_is_exact_for_a_constant_speed_and_brackets_a_stop() {
        let (lo, hi) = reachable_distance(15.0, 15.0, 1.0, 3.0, 9.0, 60.0);
        assert!(lo < 15.0 && hi > 15.0, "{lo} {hi}");
        // A 9 m/s² stop from 18 m/s takes 2 s and 18 m: inside the envelope at b = 9 …
        let (lo, hi) = reachable_distance(18.0, 0.0, 2.0, 3.0, 9.0, 60.0);
        assert!(lo <= 18.0 + 1e-9 && hi >= 18.0 - 1e-9, "{lo} {hi}");
        // … and an infeasible pair at b = 4.5 (from 18 to 0 in 2 s needs 9).
        let (lo, hi) = reachable_distance(18.0, 0.0, 2.0, 3.0, 4.5, 60.0);
        assert!(lo > hi || 18.0 < lo || 18.0 > hi, "{lo} {hi}");
    }

    #[test]
    fn the_filter_is_quiet_on_a_straight_line_and_loud_on_a_jump() {
        let mut k = CvKalman::init([0.0, 15.0, 0.0, 0.0], 16.0, 1.0);
        let mut worst: f64 = 0.0;
        for i in 1..50 {
            let t = f64::from(i) * 0.1;
            worst = worst.max(k.step(0.1, 10.0, [15.0 * t, 15.0, 0.0, 0.0], 16.0, 1.0));
        }
        assert!(worst < 1e-6, "{worst}");
        let jump = k.step(0.1, 10.0, [15.0 * 5.0 + 40.0, 15.0, 0.0, 0.0], 16.0, 1.0);
        assert!(jump > chi2_quantile(4.0, 3.0902), "{jump}");
    }

    #[test]
    fn the_quantile_is_close_to_the_table() {
        // χ²(4, 0.999) = 18.467, χ²(40, 0.999) = 73.402 (standard tables).
        assert!((chi2_quantile(4.0, 3.0902) - 18.467).abs() < 0.3);
        assert!((chi2_quantile(40.0, 3.0902) - 73.402).abs() < 0.3);
    }

    #[test]
    fn the_catch_overlaps_are_areas_and_lengths() {
        assert!((circle_inside_fraction(2.0, 0.0, 10.0) - 1.0).abs() < 1e-12);
        assert_eq!(circle_inside_fraction(2.0, 13.0, 10.0), 0.0);
        // A disc centred on the boundary of a much larger one is about half inside.
        let f = circle_inside_fraction(1.0, 1000.0, 1000.0);
        assert!((f - 0.5).abs() < 1e-3, "{f}");
        assert!((interval_inside_fraction(38.0, 42.0, 0.0, 40.0) - 0.5).abs() < 1e-12);
    }
}
