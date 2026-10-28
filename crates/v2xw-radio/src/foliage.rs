//! `obstacle/foliage/p833`: excess loss of a path through trees (WP-18, P18.3).
//!
//! A link whose straight line crosses the world's vegetation layer
//! ([`v2xw_world::VegetationArea`]: woods, tree rows, single trees) loses
//!
//! `A = min(d·γ, J(ν_canopy))`
//!
//! * `d` is the length of the line inside vegetation, summed over the areas it crosses
//!   ([`crate::obstacle::ring_crossing`]).
//! * `γ = 0.79·f^0.61` dB/m (f in GHz) is the specific attenuation GEMV² uses (Boban,
//!   Barros, Tonguz, IEEE TVT 63(9), 2014, eq. (4), pp. 6-7, after Goldhirsh & Vogel):
//!   **2.33 dB/m at 5.9 GHz**, which agrees with Durgin et al. 1998 at 5.85 GHz.
//! * The `min` is ITU-R P.833-10 §3.1 (09/2021), eq. (7): for a terrestrial path with both
//!   terminals outside the vegetation, `A = d·γ`, capped at the loss of the lowest-loss
//!   path around it. Here that path is the diffraction over the canopy top, a single
//!   P.526 knife edge ([`crate::obstacle::knife_edge_loss_db`]) at the vegetation's
//!   along-path centre.
//!
//! **Where the model departs from P.833.** §3.1 is stated for frequencies up to 1 GHz.
//! Above that, §3.2.1 asks for the radiative-energy-transfer method, whose fitted
//! parameters (Tables 4-8) stop at 2.2 and 11 GHz with no 5.9 GHz column. So §3.1 is used
//! above its range, with GEMV²'s γ. This is recorded on the card.
//!
//! Other approximations, also recorded on the card:
//! - The canopy height is a parameter (15 m, an **ASSUMPTION**), because OSM rarely tags
//!   the height of a wood.
//! - Side diffraction is not taken, so the cap is an upper bound on the true
//!   lowest-loss path.
//! - Overlapping areas (a tree row mapped inside a wood) are counted once each.
//!
//! A physical law: it reads the ground-truth vegetation layer (plan rule 6).

use rstar::{AABB, RTree, RTreeObject};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::ctx::Ctx;
use v2xw_core::geom::Vec3;
use v2xw_core::math;
use v2xw_core::model::Model;
use v2xw_world::model::World;

use crate::obstacle::{knife_edge_loss_db, knife_edge_parameter, ring_crossing};
use crate::traits::ObstacleModel;
use crate::types::{ActorSet, LosResult, RadioEndpoint};

/// The coefficient of GEMV²'s foliage specific attenuation, dB/m at 1 GHz (Boban et al.
/// 2014, eq. (4)).
pub const MEL_COEFFICIENT_DB_PER_M: f64 = 0.79;

/// The frequency exponent of GEMV²'s foliage specific attenuation (Boban et al. 2014,
/// eq. (4)).
pub const MEL_EXPONENT: f64 = 0.61;

/// The canopy top above the vegetation's ground, metres. **ASSUMPTION**: OSM rarely tags
/// a wood's height; 15 m is a mature deciduous canopy, between a street tree (about
/// 10 m) and a forest (20-25 m).
pub const DEFAULT_CANOPY_HEIGHT_M: f64 = 15.0;

/// Speed of light, m/s.
const C_MPS: f64 = 299_792_458.0;

/// The foliage specific attenuation at `f_hz`, dB/m: `0.79·f^0.61`, with f in GHz.
#[must_use]
pub fn specific_attenuation_db_per_m(f_hz: f64) -> f64 {
    if f_hz <= 0.0 {
        return 0.0;
    }
    MEL_COEFFICIENT_DB_PER_M * math::pow(f_hz / 1e9, MEL_EXPONENT)
}

#[derive(Debug, Clone)]
struct AreaBox {
    id: u32,
    min: [f64; 2],
    max: [f64; 2],
}

impl RTreeObject for AreaBox {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(self.min, self.max)
    }
}

/// What a path crosses of the vegetation layer.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FoliageCrossing {
    /// The length of the line inside vegetation, metres, summed over the areas.
    pub depth_m: f64,
    /// Where along the path the vegetation's centre is, as a fraction of its length.
    pub centre_fraction: f64,
    /// The mean ground height of the crossed areas, metres.
    pub ground_z_m: f64,
}

/// The vegetation layer's areas behind an R-tree of their boxes, by analogy with
/// [`crate::obstacle::BuildingIndex`].
#[derive(Debug)]
pub struct VegetationIndex {
    tree: RTree<AreaBox>,
    rings: Vec<Vec<Vec3>>,
    world_hash: [u8; 32],
}

impl VegetationIndex {
    /// Builds the index for a world. Pure: same world, same tree.
    #[must_use]
    pub fn build(world: &World) -> Self {
        let rings: Vec<Vec<Vec3>> = world.vegetation.iter().map(|v| v.ring.clone()).collect();
        let boxes: Vec<AreaBox> = rings
            .iter()
            .enumerate()
            .map(|(i, ring)| {
                let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
                let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
                for p in ring {
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
                AreaBox {
                    id: i as u32,
                    min: [min_x, min_y],
                    max: [max_x, max_y],
                }
            })
            .collect();
        Self {
            tree: RTree::bulk_load(boxes),
            rings,
            world_hash: world.content_hash,
        }
    }

    /// The hash of the world this index belongs to.
    #[must_use]
    pub const fn world_hash(&self) -> [u8; 32] {
        self.world_hash
    }

    /// Whether the layer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rings.is_empty()
    }

    /// What the segment `a → b` crosses: the summed depth, and the depth-weighted
    /// along-path position and ground height of the areas it crosses. The candidate
    /// areas are taken in id order, so the sums do not depend on the tree's layout.
    #[must_use]
    pub fn crossing(&self, a: Vec3, b: Vec3) -> FoliageCrossing {
        if self.rings.is_empty() {
            return FoliageCrossing::default();
        }
        let query = AABB::from_corners([a.x.min(b.x), a.y.min(b.y)], [a.x.max(b.x), a.y.max(b.y)]);
        let mut ids: Vec<u32> = self
            .tree
            .locate_in_envelope_intersecting(&query)
            .map(|e| e.id)
            .collect();
        ids.sort_unstable();
        let total = a.distance_2d(b);
        let (mut depth, mut along, mut ground) = (0.0, 0.0, 0.0);
        for id in ids {
            let ring = &self.rings[id as usize];
            let c = ring_crossing(ring, a, b);
            if c.inside_len_m <= 0.0 {
                continue;
            }
            let n = ring.len().saturating_sub(1).max(1) as f64;
            let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
            for p in &ring[..ring.len().saturating_sub(1).max(1)] {
                cx += p.x;
                cy += p.y;
                cz += p.z;
            }
            let centre = Vec3::new(cx / n, cy / n, cz / n);
            let f = if total > 0.0 {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                (((centre.x - a.x) * dx + (centre.y - a.y) * dy) / (total * total)).clamp(0.0, 1.0)
            } else {
                0.5
            };
            depth += c.inside_len_m;
            along += c.inside_len_m * f;
            ground += c.inside_len_m * centre.z;
        }
        if depth <= 0.0 {
            return FoliageCrossing::default();
        }
        FoliageCrossing {
            depth_m: depth,
            centre_fraction: along / depth,
            ground_z_m: ground / depth,
        }
    }
}

/// `obstacle/foliage/p833` ([module docs](self)).
#[derive(Debug)]
pub struct FoliageLoss {
    card: ModelCard,
    tier: Tier,
    index: VegetationIndex,
    canopy_height_m: f64,
}

impl FoliageLoss {
    /// The model's id.
    pub const ID: &'static str = "obstacle/foliage/p833";

    /// The model over `world`'s vegetation layer, with the default canopy height.
    #[must_use]
    pub fn new(world: &World, tier: Tier) -> Self {
        Self::with_canopy(world, tier, DEFAULT_CANOPY_HEIGHT_M)
    }

    /// The model with a caller-chosen canopy height, metres.
    #[must_use]
    pub fn with_canopy(world: &World, tier: Tier, canopy_height_m: f64) -> Self {
        Self {
            card: foliage_card(canopy_height_m),
            tier,
            index: VegetationIndex::build(world),
            canopy_height_m,
        }
    }

    /// The index this model reads.
    #[must_use]
    pub const fn index(&self) -> &VegetationIndex {
        &self.index
    }

    /// The excess loss of the straight path `a → b` at `f_hz`, dB: `min(d·γ, J(ν))`.
    #[must_use]
    pub fn loss_db(&self, a: Vec3, b: Vec3, f_hz: f64) -> f64 {
        let c = self.index.crossing(a, b);
        if c.depth_m <= 0.0 || f_hz <= 0.0 {
            return 0.0;
        }
        let through = c.depth_m * specific_attenuation_db_per_m(f_hz);
        let total = a.distance_2d(b);
        let d1 = (total * c.centre_fraction).max(1e-3);
        let d2 = (total - d1).max(1e-3);
        let line_z = a.z + (b.z - a.z) * c.centre_fraction;
        let h = c.ground_z_m + self.canopy_height_m - line_z;
        let over = knife_edge_loss_db(knife_edge_parameter(h, d1, d2, C_MPS / f_hz));
        through.min(over).max(0.0)
    }
}

impl Model for FoliageLoss {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl<C: Ctx + ?Sized> ObstacleModel<C> for FoliageLoss {
    fn tier(&self) -> Tier {
        self.tier
    }

    /// Foliage does not change a link's class (a path through a tree row is still LOS
    /// to the building and vehicle models); it only adds [`FoliageLoss::loss_db`].
    fn los(&self, _world: &World, _a: Vec3, _b: Vec3, _actors: Option<&ActorSet>) -> LosResult {
        LosResult::clear()
    }

    fn obstacle_loss_db(
        &mut self,
        _ctx: &mut C,
        tx: &RadioEndpoint,
        rx: &RadioEndpoint,
        _los: &LosResult,
        f_hz: f64,
    ) -> f64 {
        self.loss_db(tx.pos, rx.pos, f_hz)
    }
}

fn foliage_card(canopy_height_m: f64) -> ModelCard {
    let p833 = Source {
        kind: SourceKind::Standard,
        reference: "ITU-R P.833-10 (09/2021) §3.1, eq. (7), p. 9: A = d·γ for a terrestrial \
                    path with both terminals outside vegetation, capped at the lowest-loss \
                    path around it"
            .to_string(),
        accessed: Some("2026-10-27".to_string()),
        note: Some(
            "§3.1 is stated up to 1 GHz; above it §3.2.1 (RET) applies, whose parameters \
             stop at 2.2 and 11 GHz. Used above its range, with GEMV²'s γ."
                .to_string(),
        ),
    };
    let gemv2 = Source {
        kind: SourceKind::Paper,
        reference: "Boban, Barros, Tonguz, 'Geometry-Based Vehicle-to-Vehicle Channel \
                    Modeling for Large-Scale Simulation', IEEE TVT 63(9), 2014, doi \
                    10.1109/TVT.2014.2317803, eq. (4), pp. 6-7 (arXiv:1305.0124 v3)"
            .to_string(),
        accessed: Some("2026-10-27".to_string()),
        note: Some(
            "γ = 0.79·f^0.61 dB/m after Goldhirsh & Vogel; 2.33 dB/m at 5.9 GHz, in line \
             with Durgin et al. 1998 at 5.85 GHz."
                .to_string(),
        ),
    };
    let mut card = ModelCard::new(
        FoliageLoss::ID,
        Family::Obstacle,
        "1.0.0",
        "Excess loss of a path through trees: the depth inside the world's vegetation \
         layer times GEMV²'s specific attenuation, capped at the P.526 knife-edge loss \
         over the canopy, as P.833 §3.1 caps it at the lowest-loss path around the \
         vegetation.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.determinism = Determinism::default();
    card.equations = vec![
        Equation::new("specific attenuation", "γ = 0.79·f_GHz^0.61 dB/m"),
        Equation::new("excess loss", "A = min(d·γ, J(ν_canopy)) dB"),
    ];
    card.parameters = vec![Parameter {
        name: "canopy_height_m".to_string(),
        unit: "m".to_string(),
        default: serde_json::json!(canopy_height_m),
        range: Some(vec![serde_json::json!(2.0), serde_json::json!(60.0)]),
        source: Source {
            kind: SourceKind::TodoCalibrate,
            reference: "the canopy top above the vegetation's ground; OSM rarely tags a \
                        wood's height"
                .to_string(),
            accessed: None,
            note: Some("ASSUMPTION: 15 m, a mature deciduous canopy.".to_string()),
        },
        calibration: Some("Read OSM height on the wood where tagged.".to_string()),
    }];
    card.assumptions = vec![
        "P.833 §3.1 is used above its 1 GHz range: the §3.2.1 RET method has no 5.9 GHz \
         parameter set."
            .to_string(),
        "Only the diffraction over the canopy top is taken as the path around the \
         vegetation; side diffraction is not, so the cap is an upper bound."
            .to_string(),
        "Overlapping vegetation areas are each counted.".to_string(),
    ];
    card.ignores = vec![
        "Seasonal leaf state (P.833 notes in-leaf/out-of-leaf differences).".to_string(),
        "Ground reflection under the canopy.".to_string(),
    ];
    card.sources = vec![p833.clone(), gemv2.clone()];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: vec![p833, gemv2],
        tests: vec![
            "thirty_metres_of_wood_lose_the_p833_value".to_string(),
            "a_path_beside_the_wood_loses_nothing".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testctx::tiny_world;
    use v2xw_world::{VegetationArea, VegetationKind};

    const F_HZ: f64 = 5.9e9;

    /// The 2 × 2 test grid with one wood, `[x0, x1] × [y0, y1]`.
    fn wooded(x0: f64, y0: f64, x1: f64, y1: f64) -> World {
        let mut world = tiny_world();
        world.vegetation.push(
            VegetationArea::new(
                [
                    Vec3::new(x0, y0, 0.0),
                    Vec3::new(x1, y0, 0.0),
                    Vec3::new(x1, y1, 0.0),
                    Vec3::new(x0, y1, 0.0),
                ],
                VegetationKind::Wood,
            )
            .expect("a rectangle"),
        );
        world
    }

    #[test]
    fn gemv2_specific_attenuation_at_5_9_ghz() {
        let g = specific_attenuation_db_per_m(F_HZ);
        assert!((g - 2.33).abs() < 0.01, "{g} dB/m");
    }

    #[test]
    fn thirty_metres_of_wood_lose_the_p833_value() {
        // A 200 m link at 1.5 m through a wood 30 m deep at mid-path.
        let world = wooded(85.0, -50.0, 115.0, 50.0);
        let model = FoliageLoss::new(&world, Tier::Medium);
        let (a, b) = (Vec3::new(0.0, 0.0, 1.5), Vec3::new(200.0, 0.0, 1.5));
        let c = model.index().crossing(a, b);
        assert!((c.depth_m - 30.0).abs() < 1e-6, "depth {}", c.depth_m);
        assert!((c.centre_fraction - 0.5).abs() < 1e-9);
        // By hand: through, 30 · 0.79 · 5.9^0.61; over, J(ν) for a 13.5 m canopy edge
        // at 100 m / 100 m, λ = c / 5.9 GHz. P.833 §3.1 takes the lower.
        let through = 30.0 * 0.79 * math::pow(5.9, 0.61);
        let lambda = C_MPS / F_HZ;
        let nu = 13.5 * math::sqrt(2.0 * 200.0 / (lambda * 100.0 * 100.0));
        let over = 6.9 + 20.0 * math::log10(math::sqrt((nu - 0.1) * (nu - 0.1) + 1.0) + nu - 0.1);
        let want = through.min(over);
        let got = model.loss_db(a, b, F_HZ);
        assert!((got - want).abs() < 0.1, "{got} dB, by hand {want} dB");
        assert!(
            over < through,
            "at 30 m the canopy path is the lower ({over} < {through})"
        );
        // A shallow belt is charged its depth: 5 m costs 5·γ, below the canopy cap.
        let belt = FoliageLoss::new(&wooded(98.0, -50.0, 103.0, 50.0), Tier::Medium);
        let five = belt.loss_db(a, b, F_HZ);
        assert!(
            (five - 5.0 * specific_attenuation_db_per_m(F_HZ)).abs() < 0.1,
            "{five}"
        );
    }

    #[test]
    fn a_path_beside_the_wood_loses_nothing() {
        // A diagonal link whose bounding box holds the wood but whose line passes 40 m
        // north of it: the box query finds the wood and the ring test must reject it.
        let world = wooded(150.0, 0.0, 190.0, 20.0);
        let model = FoliageLoss::new(&world, Tier::Medium);
        let (a, b) = (Vec3::new(0.0, 0.0, 1.5), Vec3::new(200.0, 100.0, 1.5));
        assert_eq!(model.index().crossing(a, b).depth_m, 0.0);
        assert_eq!(model.loss_db(a, b, F_HZ), 0.0);
        let (a, b) = (Vec3::new(0.0, 0.0, 1.5), Vec3::new(200.0, 0.0, 1.5));
        // And a world without vegetation costs nothing anywhere.
        let bare = FoliageLoss::new(&tiny_world(), Tier::Medium);
        assert!(bare.index().is_empty());
        assert_eq!(bare.loss_db(a, b, F_HZ), 0.0);
    }
}
