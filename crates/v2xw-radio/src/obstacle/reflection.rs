//! `obstacle/reflection/gemv2-single-bounce`: the best single specular reflection off a
//! building wall (WP-29, P29.1).
//!
//! GEMV² (Boban, Barros, Tonguz, IEEE TVT 63(9) 2014, §IV-B.3 and §IV-B.5, pp. 5-6)
//! prices an NLOSb link by its single-interaction rays: "We limit the calculation of
//! diffracted and reflected rays to single-interaction (single-bounce) rays", off
//! building walls, with "any building" a possible reflector because buildings are taller
//! than vehicles. This module finds, for one link, the wall that gives the strongest such
//! ray:
//!
//! * **The walls.** Every edge of every building footprint, in an R-tree of their boxes
//!   (as WP-27 put the footprints behind one). A wall reflects a link when both antennas
//!   stand on its outer side, the specular point (from the mirror image of the
//!   transmitter) falls inside the edge and below the building's roof, and the unfolded
//!   path is no longer than GEMV²'s NLOSb ellipse axis (Table VI, 300 m).
//! * **The legs.** Each leg, transmitter to wall and wall to receiver, must be clear of
//!   every footprint ([`crate::obstacle::segment_blocked`], the 2.5-D rule of the
//!   building model): a bounce through another building is not a ray.
//! * **The loss.** GEMV² eq. (6) adds each reflected ray with the free-space spreading
//!   `E0·d0/d_j` of its own length and the reflection coefficient `R_j`, so the ray's
//!   excess over the direct path's line-of-sight loss is
//!   `20·log10(d_unfolded / d_direct) − 20·log10|R|`.
//! * **The coefficient.** Eq. (8), `R⊥ = (sin θ − √(ε_r − cos²θ)) / (sin θ +
//!   √(ε_r − cos²θ))`, θ the grazing angle: a vertically polarised vehicle antenna
//!   reflecting off a vertical wall in the horizontal plane has its E-field parallel to
//!   the wall, perpendicular to the plane of incidence (ITU-R P.2040-3 §2.3, "TE").
//!   GEMV² states no ε_r for walls: it matches its coefficients to Landron et al.'s brick
//!   measurements. The ε_r here is ITU-R P.2040-3 (08/2023) Table 3, p. 24, complex by
//!   eqs. (57)-(59): `η' = a·f^b`, `σ = c·f^d` S/m, `η'' = 17.98·σ/f` (f in GHz), per the
//!   footprint's [`MaterialClass`]; an unknown wall is concrete (an ASSUMPTION, recorded
//!   on the card).
//!
//! Pure: the same world and link give the same ray, and the walls are tried in their
//! index order, so a tie keeps the lower wall.

use rstar::{AABB, RTree, RTreeObject};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::BuildingId;
use v2xw_core::math;
use v2xw_core::model::Model;
use v2xw_world::model::{MaterialClass, World};

use super::{BuildingIndex, segment_blocked};

/// One footprint edge's box in the R-tree.
#[derive(Debug, Clone, Copy)]
struct WallBox {
    id: u32,
    min: [f64; 2],
    max: [f64; 2],
}

impl RTreeObject for WallBox {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(self.min, self.max)
    }
}

/// One footprint edge.
#[derive(Debug, Clone, Copy)]
struct Wall {
    a: Vec3,
    b: Vec3,
    building: u32,
    /// +1 when the outside of the footprint is to the left of `a → b`, −1 to the right.
    outside: f64,
    base_z_m: f64,
    top_z_m: f64,
    material: MaterialClass,
}

/// A wall's electrical properties: ITU-R P.2040-3 Table 3's `a, b, c, d`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallMaterial {
    /// `η' = a·f^b`.
    pub a: f64,
    /// `η' = a·f^b`.
    pub b: f64,
    /// `σ = c·f^d`, S/m.
    pub c: f64,
    /// `σ = c·f^d`, S/m.
    pub d: f64,
}

impl WallMaterial {
    /// Concrete, 1-100 GHz (P.2040-3 Table 3).
    pub const CONCRETE: Self = Self {
        a: 5.24,
        b: 0.0,
        c: 0.0462,
        d: 0.7822,
    };
    /// Brick, 1-40 GHz.
    pub const BRICK: Self = Self {
        a: 3.91,
        b: 0.0,
        c: 0.0238,
        d: 0.16,
    };
    /// Glass, 0.1-100 GHz.
    pub const GLASS: Self = Self {
        a: 6.31,
        b: 0.0,
        c: 0.0036,
        d: 1.3394,
    };
    /// Wood, 0.001-100 GHz.
    pub const WOOD: Self = Self {
        a: 1.99,
        b: 0.0,
        c: 0.0047,
        d: 1.0718,
    };
    /// Metal, 1-100 GHz: σ = 10⁷ S/m.
    pub const METAL: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 1.0e7,
        d: 0.0,
    };

    /// The row a footprint's material class reads; an unknown wall is concrete.
    #[must_use]
    pub const fn of(class: MaterialClass) -> Self {
        match class {
            MaterialClass::Brick => Self::BRICK,
            MaterialClass::Glass => Self::GLASS,
            MaterialClass::Wood => Self::WOOD,
            MaterialClass::Metal => Self::METAL,
            _ => Self::CONCRETE,
        }
    }

    /// The complex relative permittivity at `f_hz`, `(η', η'')` with `η = η' − j·η''`
    /// (P.2040-3 eqs. 57-59).
    #[must_use]
    pub fn permittivity(&self, f_hz: f64) -> (f64, f64) {
        let f = f_hz * 1e-9;
        let real = self.a * math::pow(f, self.b);
        let sigma = self.c * math::pow(f, self.d);
        (real, 17.98 * sigma / f)
    }
}

/// `|R|` for a TE (perpendicular) wave meeting a half-space of complex permittivity
/// `η' − j·η''` at the grazing angle whose sine and cosine are given: GEMV² eq. (8),
/// P.2040-3 eq. (31a) in grazing form.
#[must_use]
pub fn te_reflection_magnitude(sin_grazing: f64, cos_grazing: f64, eta: (f64, f64)) -> f64 {
    // w = √(η − cos²θ), the principal root of a complex number.
    let (re, im) = (eta.0 - cos_grazing * cos_grazing, -eta.1);
    let r = math::hypot(re, im);
    let w_re = math::sqrt(0.5 * (r + re).max(0.0));
    let w_im = math::sqrt(0.5 * (r - re).max(0.0)).copysign(im);
    let num = math::hypot(sin_grazing - w_re, -w_im);
    let den = math::hypot(sin_grazing + w_re, w_im);
    if den > 0.0 { num / den } else { 1.0 }
}

/// The power sum of two paths given as excess losses over the same reference, dB:
/// `−10·log10(10^(−a/10) + 10^(−b/10))`, below both.
#[must_use]
pub fn power_sum_excess_db(a_db: f64, b_db: f64) -> f64 {
    let lo = a_db.min(b_db);
    let hi = a_db.max(b_db);
    lo - 10.0 * math::log10(1.0 + math::pow(10.0, -(hi - lo) / 10.0))
}

/// The best single-bounce ray of one link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reflection {
    /// The specular point on the wall.
    pub point: Vec3,
    /// The reflecting building.
    pub building: BuildingId,
    /// The ray's unfolded length, transmitter to wall to receiver, metres (3-D).
    pub unfolded_m: f64,
    /// The grazing angle at the wall, degrees (0 = along the wall, 90 = head on).
    pub grazing_deg: f64,
    /// `−20·log10|R|`, dB.
    pub reflection_db: f64,
    /// The ray's loss over the direct path's line-of-sight loss, dB:
    /// `20·log10(d_unfolded / d_direct) + reflection_db`.
    pub excess_db: f64,
}

/// The single-bounce wall-reflection tracer (module docs).
#[derive(Debug)]
pub struct ReflectionTracer {
    card: ModelCard,
    walls: Vec<Wall>,
    tree: RTree<WallBox>,
    buildings: BuildingIndex,
    max_path_m: f64,
    /// Whether the legs are tested against the other footprints. Always true outside the
    /// fault-injection test.
    check_legs: bool,
}

impl ReflectionTracer {
    /// The model's id.
    pub const ID: &'static str = "obstacle/reflection/gemv2-single-bounce";
    /// GEMV²'s NLOSb ellipse major axis, metres (Table VI): the longest ray looked for.
    pub const NLOSB_AXIS_M: f64 = 300.0;
    /// How far off the wall a leg's end is placed, metres, so the leg does not touch the
    /// reflecting wall itself.
    const STANDOFF_M: f64 = 0.05;
    /// The least distance of an antenna from a wall's line for that wall to reflect it.
    const MIN_OFFSET_M: f64 = 0.01;

    /// The tracer for `world`'s building footprints. Pure.
    #[must_use]
    pub fn build(world: &World) -> Self {
        let mut walls = Vec::new();
        for (i, b) in world.buildings.iter().enumerate() {
            let ring = &b.footprint;
            let area2 = v2xw_world::ring_signed_area_2x(ring);
            if area2 == 0.0 {
                continue;
            }
            // Counter-clockwise (positive area): the inside is on the left, so the
            // outside is on the right.
            let outside = if area2 > 0.0 { -1.0 } else { 1.0 };
            for w in ring.windows(2) {
                if w[0].distance_2d(w[1]) < 1e-6 {
                    continue;
                }
                walls.push(Wall {
                    a: w[0],
                    b: w[1],
                    building: i as u32,
                    outside,
                    base_z_m: b.base_z_m,
                    top_z_m: b.base_z_m + b.height_m,
                    material: b.material,
                });
            }
        }
        let boxes = walls
            .iter()
            .enumerate()
            .map(|(i, w)| WallBox {
                id: i as u32,
                min: [w.a.x.min(w.b.x), w.a.y.min(w.b.y)],
                max: [w.a.x.max(w.b.x), w.a.y.max(w.b.y)],
            })
            .collect();
        Self {
            card: reflection_card(),
            walls,
            tree: RTree::bulk_load(boxes),
            buildings: BuildingIndex::build(world),
            max_path_m: Self::NLOSB_AXIS_M,
            check_legs: true,
        }
    }

    /// How many walls the tracer holds.
    #[must_use]
    pub fn walls(&self) -> usize {
        self.walls.len()
    }

    /// The tracer with the leg test switched off: only the fault-injection test uses it.
    #[doc(hidden)]
    #[must_use]
    pub fn without_leg_check(mut self) -> Self {
        self.check_legs = false;
        self
    }

    /// The strongest single-bounce ray from `tx` to `rx` at `f_hz` (the least
    /// [`Reflection::excess_db`]), or `None` when no wall gives one.
    #[must_use]
    pub fn best(&self, world: &World, tx: Vec3, rx: Vec3, f_hz: f64) -> Option<Reflection> {
        let direct_m = tx.distance(rx);
        let d2 = tx.distance_2d(rx);
        if direct_m <= 1e-6 || d2 >= self.max_path_m || f_hz <= 0.0 {
            return None;
        }
        // Every point of a ray no longer than r lies within r/2 of the link's midpoint.
        let half = 0.5 * self.max_path_m + 1e-3;
        let c = tx.lerp(rx, 0.5);
        let query = AABB::from_corners([c.x - half, c.y - half], [c.x + half, c.y + half]);
        let mut ids: Vec<u32> = self
            .tree
            .locate_in_envelope_intersecting(&query)
            .map(|w| w.id)
            .collect();
        ids.sort_unstable();
        let mut best: Option<Reflection> = None;
        for id in ids {
            let wall = &self.walls[id as usize];
            let Some(ray) = self.ray_off(wall, tx, rx, direct_m, f_hz) else {
                continue;
            };
            if best.is_some_and(|b| ray.excess_db >= b.excess_db) {
                continue;
            }
            if self.check_legs && self.leg_blocked(world, wall, tx, rx, ray.point) {
                continue;
            }
            best = Some(ray);
        }
        best
    }

    /// The specular ray off one wall, before the leg test.
    fn ray_off(
        &self,
        wall: &Wall,
        tx: Vec3,
        rx: Vec3,
        direct_m: f64,
        f_hz: f64,
    ) -> Option<Reflection> {
        let (ux, uy) = (wall.b.x - wall.a.x, wall.b.y - wall.a.y);
        let len = math::hypot(ux, uy);
        let (ex, ey) = (ux / len, uy / len);
        // Signed distances from the wall's line, positive on its outer side.
        let offset = |p: Vec3| wall.outside * (ex * (p.y - wall.a.y) - ey * (p.x - wall.a.x));
        let along = |p: Vec3| ex * (p.x - wall.a.x) + ey * (p.y - wall.a.y);
        let (ht, hr) = (offset(tx), offset(rx));
        if ht < Self::MIN_OFFSET_M || hr < Self::MIN_OFFSET_M {
            return None;
        }
        // The mirror image of tx sees rx through the wall at the fraction ht/(ht+hr) of
        // the way along: similar triangles.
        let k = ht / (ht + hr);
        let (st, sr) = (along(tx), along(rx));
        let s = st + (sr - st) * k;
        if s <= 0.0 || s >= len {
            return None;
        }
        let z = tx.z + (rx.z - tx.z) * k;
        if z < wall.base_z_m || z > wall.top_z_m {
            return None;
        }
        let point = Vec3::new(wall.a.x + ex * s, wall.a.y + ey * s, z);
        let flat = math::hypot(sr - st, ht + hr);
        let unfolded_m = math::hypot(flat, rx.z - tx.z);
        if unfolded_m > self.max_path_m {
            return None;
        }
        let (sin_g, cos_g) = ((ht + hr) / flat, (sr - st).abs() / flat);
        let gamma = te_reflection_magnitude(
            sin_g,
            cos_g,
            WallMaterial::of(wall.material).permittivity(f_hz),
        );
        let reflection_db = -20.0 * math::log10(gamma.max(1e-12));
        let excess_db = 20.0 * math::log10(unfolded_m / direct_m) + reflection_db;
        Some(Reflection {
            point,
            building: BuildingId::new(wall.building),
            unfolded_m,
            grazing_deg: math::atan2(sin_g, cos_g).to_degrees(),
            reflection_db,
            excess_db,
        })
    }

    /// Whether a leg of the ray off `wall` at `point` crosses a footprint. The legs end
    /// [`Self::STANDOFF_M`] off the wall on its outer side, so the reflecting wall itself
    /// is not counted.
    fn leg_blocked(&self, world: &World, wall: &Wall, tx: Vec3, rx: Vec3, point: Vec3) -> bool {
        let (ux, uy) = (wall.b.x - wall.a.x, wall.b.y - wall.a.y);
        let len = math::hypot(ux, uy);
        // The outer normal: the left normal (−uy, ux) times `outside`.
        let (nx, ny) = (-uy / len * wall.outside, ux / len * wall.outside);
        let p = Vec3::new(
            point.x + nx * Self::STANDOFF_M,
            point.y + ny * Self::STANDOFF_M,
            point.z,
        );
        segment_blocked(world, &self.buildings, tx, p)
            || segment_blocked(world, &self.buildings, p, rx)
    }
}

impl Model for ReflectionTracer {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

fn reflection_card() -> ModelCard {
    let gemv2 = Source {
        kind: SourceKind::Paper,
        reference: "Boban, Barros, Tonguz, 'Geometry-Based Vehicle-to-Vehicle Channel \
                    Modeling for Large-Scale Simulation', IEEE TVT 63(9), 2014, doi \
                    10.1109/TVT.2014.2317803, §IV-B.3 and §IV-B.5, eqs. (6)-(8), pp. 5-6 \
                    (arXiv:1305.0124 v3)"
            .to_string(),
        accessed: Some("2026-10-28".to_string()),
        note: Some(
            "single-interaction reflections off building walls for NLOSb links; each ray \
             adds E0·d0/d_j·R_j (eq. 6); R⊥ by eq. (8). The paper gives no wall ε_r: it \
             matches Landron et al.'s brick-wall coefficients."
                .to_string(),
        ),
    };
    let p2040 = Source {
        kind: SourceKind::Standard,
        reference: "ITU-R P.2040-3 (08/2023), Table 3 (material properties), p. 24, and \
                    eqs. (57)-(59): η' = a·f^b, σ = c·f^d, η'' = 17.98·σ/f"
            .to_string(),
        accessed: Some("2026-10-28".to_string()),
        note: Some(
            "concrete a 5.24, c 0.0462, d 0.7822 (1-100 GHz); brick 3.91, 0.0238, 0.16 \
             (1-40 GHz); glass 6.31, 0.0036, 1.3394; wood 1.99, 0.0047, 1.0718; metal 1, \
             1e7, 0."
                .to_string(),
        ),
    };
    let mut card = ModelCard::new(
        ReflectionTracer::ID,
        Family::Obstacle,
        "1.0.0",
        "The strongest single specular reflection off a building wall for a link: the \
         wall whose mirror image of the transmitter sees the receiver, with both legs \
         clear of every footprint, priced by free-space spreading over the unfolded path \
         and the TE Fresnel coefficient of the wall's material.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.determinism = Determinism::default();
    card.equations = vec![
        Equation::new(
            "reflection coefficient (TE, grazing θ)",
            "R⊥ = (sin θ − √(η − cos²θ)) / (sin θ + √(η − cos²θ)), η = η' − j·17.98·σ/f_GHz",
        ),
        Equation::new(
            "excess over the direct LOS loss",
            "L = 20·log10(d_unfolded / d_direct) − 20·log10|R⊥| dB",
        ),
    ];
    card.parameters = vec![Parameter {
        name: "max_path_m".to_string(),
        unit: "m".to_string(),
        default: serde_json::json!(ReflectionTracer::NLOSB_AXIS_M),
        range: None,
        source: Source {
            kind: SourceKind::Paper,
            reference: "GEMV² Table VI: the NLOSb ellipse major axis".to_string(),
            accessed: Some("2026-10-28".to_string()),
            note: None,
        },
        calibration: None,
    }];
    card.assumptions = vec![
        "A footprint of unknown material reflects as concrete (P.2040-3 Table 3); GEMV²'s \
         own sites were brick and concrete."
            .to_string(),
        "Vehicle antennas are vertically polarised, so a vertical wall reflects the TE \
         component."
            .to_string(),
        "Walls are smooth and vertical, from the footprint's base to its roof.".to_string(),
    ];
    card.ignores = vec![
        "Reflections off vehicles (GEMV² models tall ones) and off roofs.".to_string(),
        "Higher-order bounces and diffuse scattering (GEMV² §IV-B.5, footnote 4).".to_string(),
        "The phase of the ray: the engine sums powers, not fields.".to_string(),
    ];
    card.sources = vec![gemv2.clone(), p2040.clone()];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: vec![gemv2, p2040],
        tests: vec![
            "a_mirror_wall_lifts_an_nlosb_link".to_string(),
            "a_blocked_bounce_is_not_used".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testctx::tiny_world;
    use v2xw_world::model::{Building, HeightSource};

    const F_HZ: f64 = 5.9e9;

    fn with_blocks(blocks: &[(f64, f64, f64, f64)]) -> World {
        let mut world = tiny_world();
        world.buildings.clear();
        for &(x0, y0, x1, y1) in blocks {
            let id = BuildingId::new(world.buildings.len() as u32);
            let b = Building::new(
                id,
                vec![
                    Vec3::new(x0, y0, 0.0),
                    Vec3::new(x1, y0, 0.0),
                    Vec3::new(x1, y1, 0.0),
                    Vec3::new(x0, y1, 0.0),
                ],
                Vec::<Vec<Vec3>>::new(),
                30.0,
                0.0,
                MaterialClass::Concrete,
                HeightSource::Tagged,
            )
            .expect("a rectangle");
            world.buildings.push(b);
        }
        world.reindex();
        world
    }

    #[test]
    fn the_coefficient_is_the_fresnel_value() {
        // Head on, |R| = |(1 − √η)/(1 + √η)|; along the wall it tends to 1. Concrete at
        // 5.9 GHz: η' = 5.24, σ = 0.0462·5.9^0.7822 = 0.1854 S/m, η'' = 0.565.
        let eta = WallMaterial::CONCRETE.permittivity(F_HZ);
        assert!((eta.0 - 5.24).abs() < 1e-12);
        assert!((eta.1 - 17.98 * 0.0462 * 5.9f64.powf(0.7822) / 5.9).abs() < 1e-9);
        let head_on = te_reflection_magnitude(1.0, 0.0, eta);
        // √(5.24 − j0.565) = 2.2918 − j0.1233.
        let want = ((1.0 - 2.2918f64).powi(2) + 0.1233f64.powi(2)).sqrt()
            / ((1.0 + 2.2918f64).powi(2) + 0.1233f64.powi(2)).sqrt();
        assert!((head_on - want).abs() < 1e-3, "{head_on} vs {want}");
        let grazing = te_reflection_magnitude(0.01, (1.0f64 - 1e-4).sqrt(), eta);
        assert!(grazing > 0.99, "{grazing}");
        assert!((power_sum_excess_db(10.0, 10.0) - (10.0 - 3.0103)).abs() < 1e-3);
    }

    #[test]
    fn a_mirror_wall_lifts_an_nlosb_link() {
        // A street canyon: walls along y = 0 (south block) and y = 20 (north block, from
        // x = 100). The transmitter stands west of the north block and above the canyon,
        // the receiver in the canyon behind the block's corner; the south block's north
        // wall reflects the link round it.
        let world = with_blocks(&[(0.0, -40.0, 200.0, 0.0), (100.0, 20.0, 200.0, 60.0)]);
        let tracer = ReflectionTracer::build(&world);
        assert_eq!(tracer.walls(), 8);
        let tx = Vec3::new(50.0, 40.0, 1.5);
        let rx = Vec3::new(150.0, 10.0, 1.5);
        assert!(
            segment_blocked(&world, &BuildingIndex::build(&world), tx, rx),
            "the direct path is blocked"
        );
        let ray = tracer
            .best(&world, tx, rx, F_HZ)
            .expect("the canyon wall reflects");
        assert_eq!(ray.building, BuildingId::new(0));
        // The image of tx in y = 0 is (50, −40): the specular point is where the line to
        // (150, 10) crosses y = 0, at x = 50 + 100·40/50 = 130.
        assert!(
            (ray.point.x - 130.0).abs() < 1e-6 && ray.point.y.abs() < 1e-6,
            "{ray:?}"
        );
        let unfolded = (100.0f64.powi(2) + 50.0f64.powi(2)).sqrt();
        assert!((ray.unfolded_m - unfolded).abs() < 1e-6);
        let direct = tx.distance(rx);
        let gamma = te_reflection_magnitude(
            50.0 / unfolded,
            100.0 / unfolded,
            WallMaterial::CONCRETE.permittivity(F_HZ),
        );
        let want = 20.0 * (unfolded / direct).log10() - 20.0 * gamma.log10();
        assert!(
            (ray.excess_db - want).abs() < 1e-9,
            "{} vs {want}",
            ray.excess_db
        );
        // The reflection costs a few dB over the direct LOS loss, far below what the
        // shadowed path through the north block costs (Sommer: 9.2 dB a wall), so the
        // power sum lifts the link.
        assert!(ray.reflection_db > 0.0 && ray.excess_db < 10.0, "{ray:?}");
        assert!(power_sum_excess_db(30.0, ray.excess_db) < ray.excess_db + 1e-9);
        // West of both blocks every specular point falls beyond its wall's ends: no ray.
        let (a, b) = (Vec3::new(-50.0, 10.0, 1.5), Vec3::new(-30.0, 10.0, 1.5));
        assert!(tracer.best(&world, a, b, F_HZ).is_none());
    }

    #[test]
    fn a_blocked_bounce_is_not_used() {
        // The canyon of the test above with a kiosk on the south pavement, standing on
        // the reflected leg towards the receiver; the direct path stays blocked, and no
        // other wall reflects the link, so there is no ray.
        let world = with_blocks(&[
            (0.0, -40.0, 200.0, 0.0),
            (100.0, 20.0, 200.0, 60.0),
            (137.0, 1.0, 141.0, 4.0),
        ]);
        let tracer = ReflectionTracer::build(&world);
        let tx = Vec3::new(50.0, 40.0, 1.5);
        let rx = Vec3::new(150.0, 10.0, 1.5);
        assert_eq!(tracer.best(&world, tx, rx, F_HZ), None);
        // Without the leg test the same bounce is (wrongly) taken.
        let blind = ReflectionTracer::build(&world).without_leg_check();
        assert!(blind.best(&world, tx, rx, F_HZ).is_some());
    }
}
