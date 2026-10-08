//! Geographical areas and the geometric function `F` of EN 302 931 V1.1.1 §5.
//!
//! Coordinates are a local Cartesian frame in metres (x east, y north), the frame the
//! engine's world already uses. `F(x, y)` is positive inside the area, zero on its border
//! and negative outside; GeoNetworking decides "inside" by `F >= 0` (EN 302 636-4-1
//! Annex D).

use serde::{Deserialize, Serialize};

/// The area's shape [EN 302 931 §5: circle, rectangle, ellipse].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AreaShape {
    /// A circle of radius `a` (`b` is ignored).
    Circle,
    /// A rectangle with half-lengths `a` (along the azimuth) and `b`.
    Rectangle,
    /// An ellipse with semi-axes `a` and `b`.
    Ellipse,
}

/// A GeoBroadcast destination area: centre, half-distances `a` and `b`, and the azimuth
/// angle of the long side, degrees clockwise from north [EN 302 931 §5].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Area {
    /// The shape.
    pub shape: AreaShape,
    /// Centre, x east, m.
    pub cx: f64,
    /// Centre, y north, m.
    pub cy: f64,
    /// Distance a, m.
    pub a: f64,
    /// Distance b, m.
    pub b: f64,
    /// Azimuth angle, degrees clockwise from north.
    pub angle_deg: f64,
}

impl Area {
    /// A circle of `radius` around `(cx, cy)`.
    pub fn circle(cx: f64, cy: f64, radius: f64) -> Self {
        Area {
            shape: AreaShape::Circle,
            cx,
            cy,
            a: radius,
            b: radius,
            angle_deg: 0.0,
        }
    }

    /// An axis-aligned rectangle spanning `[x0, x1] × [y0, y1]`.
    pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        // With angle 0 the long side `a` points north (y), `b` east (x).
        Area {
            shape: AreaShape::Rectangle,
            cx: (x0 + x1) / 2.0,
            cy: (y0 + y1) / 2.0,
            a: (y1 - y0).abs() / 2.0,
            b: (x1 - x0).abs() / 2.0,
            angle_deg: 0.0,
        }
    }

    /// The point in the area's own frame: `(along a, along b)`.
    fn local(&self, x: f64, y: f64) -> (f64, f64) {
        let dx = x - self.cx;
        let dy = y - self.cy;
        let th = self.angle_deg.to_radians();
        // Rotate so that the azimuth direction (north rotated clockwise by θ) is the
        // first axis. Deterministic sin/cos (ADR 0003).
        let (s, c) = v2xw_core::math::sin_cos(th);
        let along_a = dx * s + dy * c;
        let along_b = dx * c - dy * s;
        (along_a, along_b)
    }

    /// The geometric function `F(x, y)` [EN 302 931 §5]: `> 0` inside, `0` on the border,
    /// `< 0` outside.
    pub fn f(&self, x: f64, y: f64) -> f64 {
        let (u, v) = self.local(x, y);
        let a = self.a.max(f64::MIN_POSITIVE);
        let b = self.b.max(f64::MIN_POSITIVE);
        match self.shape {
            AreaShape::Circle => 1.0 - (u / a).powi(2) - (v / a).powi(2),
            AreaShape::Ellipse => 1.0 - (u / a).powi(2) - (v / b).powi(2),
            AreaShape::Rectangle => (1.0 - (u / a).powi(2)).min(1.0 - (v / b).powi(2)),
        }
    }

    /// True if `(x, y)` is inside or on the border (`F >= 0`).
    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.f(x, y) >= 0.0
    }

    /// Distance from `(x, y)` to the area's centre, m — the target of line forwarding
    /// towards an area.
    pub fn distance_to_centre(&self, x: f64, y: f64) -> f64 {
        v2xw_core::math::hypot(x - self.cx, y - self.cy)
    }

    /// The area's size, km² (checked against `itsGnMaxGeoAreaSize`).
    pub fn size_km2(&self) -> f64 {
        let m2 = match self.shape {
            AreaShape::Circle => core::f64::consts::PI * self.a * self.a,
            AreaShape::Ellipse => core::f64::consts::PI * self.a * self.b,
            AreaShape::Rectangle => 4.0 * self.a * self.b,
        };
        m2 / 1e6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_is_positive_inside_zero_on_border_negative_outside() {
        let c = Area::circle(0.0, 0.0, 100.0);
        assert!(c.f(0.0, 0.0) > 0.0);
        assert!(c.f(100.0, 0.0).abs() < 1e-12);
        assert!(c.f(101.0, 0.0) < 0.0);
        let r = Area::rect(0.0, 0.0, 4_000.0, 30.0);
        assert!(r.contains(3_999.0, 15.0));
        assert!(!r.contains(4_001.0, 15.0));
        assert!(!r.contains(10.0, 31.0));
    }
}
