//! `vru/pedestrian/social-force` — Helbing and Molnár 1995 (04-models.md §2.5).
//!
//! # The model
//!
//! ```text
//! dv_α/dt = (v0_α·e_α − v_α)/τ  +  Σ_β f_αβ  +  Σ_B f_αB  +  fluctuations
//! f_αβ = −∇_r V_αβ(b),   V_αβ(b) = V0·exp(−b/σ)
//! f_αB = −∇   U_αB(d),   U_αB(d) = U0·exp(−d/R)
//! b    = ½·√( (|r| + |r − y|)² − |y|² ),   y = v_β·Δt·e_β
//! ```
//!
//! `b` is the semi-minor axis of an ellipse whose foci are the other pedestrian's present
//! and next positions: the repulsion is *directed*, because a pedestrian steps out of the
//! way of where the other one is **going**. Its gradient has a closed form,
//! `∇_r b = ((|r| + |r − y|) / (4b))·(r̂ + (r − y)ˆ)`, which is what this module evaluates.
//!
//! **Status of the gradient:** R10 §B11 records the parameter table and the *form* of the
//! model (`dw/dt = F + fluctuations`, with a desired-direction term, pairwise repulsion,
//! border repulsion and optional attraction), not this reduction. The closed form above is
//! the standard one and is transcribed rather than quoted, so it is flagged on the card.
//! [`SocialForceParams::elliptical`] turns it off, which falls back to the isotropic
//! `b = |r|` that needs no reduction at all.
//!
//! # The parameters, all from §2.5
//!
//! | Parameter | Default |
//! |---|---|
//! | desired speed mean, std | 1.34, 0.26 m/s (Gaussian) |
//! | maximum speed | 1.3 × v0 |
//! | relaxation time τ | 0.5 s |
//! | pedestrian-pedestrian `V0`, `σ` | 2.1 m²/s², 0.3 m |
//! | pedestrian-border `U0`, `R` | 10 m²/s², 0.2 m |
//! | step width Δt (elliptical potential) | 2 s |
//! | field of view 2φ | 200° |
//! | behind-view weight c | 0.5 |
//! | vehicle blocking threshold | 10 m (`jmCrossingGap`, R10 §B13) |
//!
//! The fluctuation term has **no cited magnitude** — the paper says "fluctuations" — so its
//! default is zero and it carries a `TODO: calibrate` plan. A pedestrian model with an
//! invented noise term would look more realistic and be less true.
//!
//! # Staying on the pavement
//!
//! The border potential is soft: with a finite step a pedestrian *could* cross a kerb, and
//! at `U0 = 10 m²/s²` and `R = 0.2 m` the force only becomes large within a few
//! decimetres. So the model also **clamps** the lateral offset to the walkable half-width
//! at the end of every step. The clamp is a hard constraint on top of a soft potential,
//! recorded on the card: without it the published invariant "pedestrians walk on sidewalk
//! lanes and crossings" would hold only statistically, and the crate's own test asserts it
//! absolutely.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::geom::{Dims, LanePos, Vec3};
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::kinematics::Kinematics;
use v2xw_core::math;
use v2xw_core::rng::{EntityRef, RngDomain};
use v2xw_core::time::Duration;
use v2xw_world::{LaneKind, World};

use crate::classes::VehicleClass;
use crate::ctx::MobCtx;
use crate::error::{MobError, Result};
use crate::snapshot::ActorSnapshot;
use crate::traits::VruMobility;
use crate::vru::crosswalk::CrossingPermit;
use crate::vru::midblock::{MidblockIndex, MidblockParams, PathBand, path_bands};
use std::sync::Arc;
use v2xw_world::SignalState;

/// The model id.
pub const MODEL_ID: &str = "vru/pedestrian/social-force";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// The vehicle-side blocking threshold, metres (SUMO `jmCrossingGap`, R10 §B13).
pub const CROSSING_GAP_M: f64 = 10.0;

/// The stream a pedestrian's decision to cross against the signal is drawn from.
pub const JAYWALK_ID: &str = "vru/pedestrian/jaywalk";

/// The stream a pedestrian's own traits (age group, compliance, start-up and gap times)
/// are drawn from, once, at spawn.
pub const TRAITS_ID: &str = "vru/pedestrian/traits";

/// The stream a pedestrian's mid-block crossing decisions are drawn from: one draw per
/// step while it walks an eligible stretch, and the crossing's angle and next walk.
pub const MIDBLOCK_ID: &str = "vru/pedestrian/midblock";

/// How a pedestrian's desired walking speed is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpeedLaw {
    /// One Gaussian for everyone: Helbing & Molnár's 1.34 ± 0.26 m/s (§2.5).
    #[default]
    Helbing1995,
    /// Two age groups, each Gaussian, from the crosswalk field study of Knoblauch,
    /// Pietrucha & Nitzburg (TRR 1538, 1996): pedestrians 14–64 walk at a mean of
    /// 4.95 ft/s (1.51 m/s) with a 15th percentile of 4.09 ft/s (1.25 m/s); those 65 and
    /// over at 4.11 ft/s (1.25 m/s), 15th percentile 3.19 ft/s (0.97 m/s). The standard
    /// deviations are derived from the mean and the 15th percentile assuming normality
    /// (`σ = (mean − p15)/1.036`): 0.25 and 0.27 m/s. The MUTCD's 3.5 ft/s (1.07 m/s)
    /// clearance speed sits below both groups' 15th percentiles, as it is meant to.
    Knoblauch1996,
}

/// What a pedestrian is doing, for the viewer and the safety statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub enum PedActivity {
    /// Walking along a sidewalk.
    #[default]
    Walking = 0,
    /// Standing at the kerb of a crosswalk, waiting for walk or for a gap.
    WaitingAtKerb = 1,
    /// On a signalised crosswalk, having stepped off on walk.
    CrossingOnWalk = 2,
    /// On a signalised crosswalk, having stepped off on flashing or steady don't-walk.
    CrossingAgainstSignal = 3,
    /// On a crosswalk with no pedestrian signal.
    CrossingUnsignalised = 4,
    /// Standing at the kerb mid-block, waiting for a gap to jaywalk.
    WaitingMidblock = 5,
    /// In the carriageway mid-block, away from any crosswalk.
    CrossingMidblock = 6,
}

impl PedActivity {
    /// The one-byte code the live stream carries.
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// True while the pedestrian is in the carriageway.
    pub const fn in_carriageway(self) -> bool {
        matches!(
            self,
            PedActivity::CrossingOnWalk
                | PedActivity::CrossingAgainstSignal
                | PedActivity::CrossingUnsignalised
                | PedActivity::CrossingMidblock
        )
    }
}

/// What makes one pedestrian unlike another, drawn once at spawn ([`TRAITS_ID`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PedestrianTraits {
    /// 65 or over ([`SocialForceParams::older_share`]).
    pub older: bool,
    /// Crosses on flashing or steady don't-walk when the gap allows
    /// ([`SocialForceParams::red_crossing_share`]).
    pub violator: bool,
    /// Time from the onset of walk to stepping off the kerb, for one who was waiting,
    /// seconds ([`SocialForceParams::startup_median_s`]).
    pub startup_s: f64,
    /// The HCM's `t_s` for this pedestrian: the margin, beyond the time to walk across,
    /// that a gap must leave, seconds ([`SocialForceParams::gap_margin_s`]).
    pub gap_margin_s: f64,
}

/// Where a mid-block crossing is in its life.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidblockStage {
    /// Walking to the crossing point.
    Approaching,
    /// Standing at the kerb at the crossing point, since then.
    Waiting(v2xw_core::time::SimTime),
    /// In the carriageway, this far along the path, metres.
    Crossing(f64),
}

/// One pedestrian's mid-block crossing, from the decision to the far kerb.
#[derive(Debug, Clone, PartialEq)]
pub struct Midblock {
    /// Arc length on the near sidewalk lane where it steps off.
    pub at_s_m: f64,
    /// The far sidewalk lane.
    pub far_lane: LaneId,
    /// Where on it the path ends (diagonal included).
    pub far_s_m: f64,
    /// The driven lanes the path crosses.
    pub crossed: Vec<LaneId>,
    /// The path's start, set when the pedestrian reaches the crossing point.
    pub from: Vec3,
    /// The path's end, on the far sidewalk's centreline.
    pub to: Vec3,
    /// The bands the path cuts across the driven lanes (empty until it reaches the point).
    pub bands: Vec<PathBand>,
    /// Where it is.
    pub stage: MidblockStage,
}

impl Midblock {
    /// The path's length, metres.
    pub fn length_m(&self) -> f64 {
        self.from.distance_2d(self.to)
    }
}

/// Counts of what the pedestrians did, for the run's safety and delay statistics.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct PedestrianStats {
    /// Crossings started on walk.
    pub crossings_on_walk: u64,
    /// Crossings started on flashing don't-walk.
    pub crossings_on_flashing: u64,
    /// Crossings started on steady don't-walk.
    pub crossings_on_dont_walk: u64,
    /// Crossings of a crosswalk with no pedestrian signal.
    pub crossings_unsignalised: u64,
    /// Mid-block crossings started.
    pub midblock_crossings: u64,
    /// Mid-block crossings given up for want of a gap.
    pub midblock_abandoned: u64,
    /// Kerb waits completed (the pedestrian stepped off or gave up), at crosswalks and
    /// mid-block.
    pub waits: u64,
    /// Their total, seconds.
    pub wait_total_s: f64,
    /// The longest, seconds.
    pub wait_max_s: f64,
}

/// The model's parameters (§2.5).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SocialForceParams {
    /// Mean desired speed, m/s.
    pub desired_speed_mean_mps: f64,
    /// Standard deviation of the desired speed, m/s.
    pub desired_speed_std_mps: f64,
    /// Maximum speed, as a multiple of the drawn desired speed.
    pub max_speed_factor: f64,
    /// Relaxation time `τ`, seconds.
    pub tau_s: f64,
    /// Pedestrian-pedestrian potential `V0`, m²/s².
    pub v0_m2_s2: f64,
    /// Pedestrian-pedestrian decay length `σ`, metres.
    pub sigma_m: f64,
    /// Pedestrian-border potential `U0`, m²/s².
    pub u0_m2_s2: f64,
    /// Pedestrian-border decay length `R`, metres.
    pub r_m: f64,
    /// Step width `Δt` of the elliptical potential, seconds.
    pub step_width_s: f64,
    /// Field of view `2φ`, degrees.
    pub field_of_view_deg: f64,
    /// Weight of an influence from outside the field of view, `c`.
    pub behind_weight: f64,
    /// Whether to use the elliptical potential (see the module documentation).
    pub elliptical: bool,
    /// How far to look for other pedestrians and vehicles, metres. Not a model parameter:
    /// the potentials are exponential with a decay length of 0.3 m, so the force beyond a
    /// few metres is numerically zero, and this is where the query stops paying for it.
    pub interaction_radius_m: f64,
    /// SUMO's vehicle-side blocking threshold on a crossing, metres. **Superseded**: the
    /// kerb decision is now the kinematic test of [`crate::vru::crosswalk`] (can the
    /// approaching vehicle still stop?), which is what UVC §11-502(b) states. Kept so a
    /// parameter file that sets it still loads, and on the card as the SUMO reference.
    pub crossing_gap_m: f64,
    /// The probability that a pedestrian at the kerb facing a don't-walk crosses anyway,
    /// when no vehicle makes it a hazard. **Off (zero) by default.** Drawn once per
    /// pedestrian per crosswalk it waits at. This is crossing against the signal at a
    /// crosswalk; crossing mid-block, away from any crosswalk, is not modelled.
    pub jaywalk_probability: f64,
    /// How long a pedestrian waits at one kerb before giving up the walk, seconds. Not a
    /// cited value: a bound that keeps a pedestrian from waiting forever at a crosswalk
    /// that never shows walk (two 60 s cycles).
    pub max_wait_s: f64,
    /// How far back from the end of the pavement lane a waiting pedestrian stands,
    /// metres: the kerb edge.
    pub kerb_margin_m: f64,
    /// Standard deviation of the fluctuation acceleration, m/s². **`TODO: calibrate`** —
    /// zero by default.
    pub fluctuation_mps2: f64,
    /// How desired speeds are drawn ([`SpeedLaw`]).
    pub speed_law: SpeedLaw,
    /// Under [`SpeedLaw::Knoblauch1996`], the share of pedestrians 65 or over. **A choice**:
    /// about one New Yorker in six is 65 or over (US Census), and no count of the age mix
    /// on Midtown's sidewalks was read, so 0.15.
    pub older_share: f64,
    /// Mean walking speed of pedestrians 14–64, m/s (Knoblauch et al. 1996: 4.95 ft/s).
    pub younger_speed_mean_mps: f64,
    /// Its standard deviation, m/s (derived, see [`SpeedLaw::Knoblauch1996`]).
    pub younger_speed_std_mps: f64,
    /// Mean walking speed of pedestrians 65 and over, m/s (Knoblauch et al. 1996:
    /// 4.11 ft/s).
    pub older_speed_mean_mps: f64,
    /// Its standard deviation, m/s (derived).
    pub older_speed_std_mps: f64,
    /// The share of pedestrians who cross on flashing or steady don't-walk when the
    /// traffic leaves them a gap ([`PedestrianTraits::violator`]). Zero by default; the
    /// observed preset's value is calibrated to Basch et al. 2015 (*J. Community Health*
    /// 40:789), who watched 21,760 pedestrians at five Midtown Manhattan intersections:
    /// 5,414 crossing on walk were 27.8 % of the walk crossers and 974 crossing on don't
    /// walk were 42.0 % of those, so about 2,319 of 21,794 crossings (10.6 %) began on
    /// don't-walk. The observed preset's 0.15 was set on the signalised test grid of
    /// `tests/pedestrian_invariants.rs`, where 0.3 made 20.8 % of signalised crossings
    /// begin on (flashing or steady) don't-walk (35 of 168): halved, it lands near the
    /// count. A share of pedestrians, not of crossings: how many crossings it produces
    /// depends on how often traffic leaves a gap, so check it against a local count.
    pub red_crossing_share: f64,
    /// Median start-up time, from the onset of walk to stepping off the kerb, for a
    /// pedestrian who was waiting, seconds; zero for none. Knoblauch et al. 1996 measured
    /// it but their values could not be read; the observed preset's 2 s is **a choice**
    /// below the HCM's 3.2 s platoon start-up time, which includes the platoon's
    /// queue discharge.
    pub startup_median_s: f64,
    /// The lognormal shape of the start-up time.
    pub startup_sigma: f64,
    /// The HCM's pedestrian start-up and end clearance time `t_s` in its critical headway
    /// `t_c = L/S_p + t_s`, seconds: what a gap must leave beyond the time to walk across.
    /// **Not re-verified**: the HCM asks for a local measurement; 2 s is used here.
    pub gap_margin_s: f64,
    /// The lognormal spread of `t_s` between pedestrians.
    pub gap_margin_sigma: f64,
    /// Mid-block crossing ([`crate::vru::midblock`]); off by default.
    pub midblock: MidblockParams,
    /// The share of *groups* of one, two, three and four walkers the engine places. A
    /// group shares one walk, walks at its slowest member's pace and side by side, and
    /// takes its leader's decisions at the kerb. All singles by default. The observed
    /// preset's split, 0.65 / 0.27 / 0.06 / 0.02, puts 55 % of pedestrians in groups —
    /// Moussaïd et al. (*PLoS ONE* 5:e10047, 2010) counted more than half of the
    /// pedestrians in a commercial street walking in groups on a workday (70 % at the
    /// weekend) and slower the larger the group. The split itself is **a choice** that
    /// matches that share; the per-size counts could not be read.
    pub group_shares: [f64; 4],
}

impl Default for SocialForceParams {
    fn default() -> Self {
        Self {
            desired_speed_mean_mps: 1.34,
            desired_speed_std_mps: 0.26,
            max_speed_factor: 1.3,
            tau_s: 0.5,
            v0_m2_s2: 2.1,
            sigma_m: 0.3,
            u0_m2_s2: 10.0,
            r_m: 0.2,
            step_width_s: 2.0,
            field_of_view_deg: 200.0,
            behind_weight: 0.5,
            elliptical: true,
            interaction_radius_m: 10.0,
            crossing_gap_m: CROSSING_GAP_M,
            fluctuation_mps2: 0.0,
            jaywalk_probability: 0.0,
            max_wait_s: 120.0,
            kerb_margin_m: 0.3,
            speed_law: SpeedLaw::Helbing1995,
            older_share: 0.15,
            younger_speed_mean_mps: 1.51,
            younger_speed_std_mps: 0.25,
            older_speed_mean_mps: 1.25,
            older_speed_std_mps: 0.27,
            red_crossing_share: 0.0,
            startup_median_s: 0.0,
            startup_sigma: 0.5,
            gap_margin_s: 2.0,
            gap_margin_sigma: 0.3,
            midblock: MidblockParams::default(),
            group_shares: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

impl SocialForceParams {
    /// Pedestrians as they are observed rather than as the 1995 model's defaults: walking
    /// speeds by age group (Knoblauch et al. 1996), a start-up delay at the onset of walk,
    /// a share who cross against the signal when traffic allows (calibrated to Midtown
    /// Manhattan, Basch et al. 2015), and mid-block crossing ([`MidblockParams::urban`]).
    pub fn observed() -> Self {
        Self {
            speed_law: SpeedLaw::Knoblauch1996,
            red_crossing_share: 0.15,
            startup_median_s: 2.0,
            midblock: MidblockParams::urban(),
            group_shares: [0.65, 0.27, 0.06, 0.02],
            ..Self::default()
        }
    }
}

/// One pedestrian.
#[derive(Debug, Clone, PartialEq)]
pub struct Pedestrian {
    /// Which actor.
    pub actor: ActorId,
    /// The walkable lane it is on.
    pub lane: LaneId,
    /// Arc length along that lane, metres.
    pub s_m: f64,
    /// Lateral offset from the centreline, metres, positive to the left of travel.
    pub lateral_m: f64,
    /// Velocity, m/s, in world coordinates.
    pub vel: Vec3,
    /// This pedestrian's drawn desired speed, m/s.
    pub desired_speed_mps: f64,
    /// The walkable lanes it intends to use, in order.
    pub route: Vec<LaneId>,
    /// Where it is on that route.
    pub route_index: usize,
    /// True once it has walked off the end of its route.
    pub arrived: bool,
    /// Since when it has been standing at a kerb, waiting to cross.
    pub waiting_since: Option<v2xw_core::time::SimTime>,
    /// Its decision to cross against the signal at one crossing lane, once drawn.
    pub against_signal: Option<(LaneId, bool)>,
    /// Its own traits.
    pub traits: PedestrianTraits,
    /// The group it walks with (the leader's actor index), if any.
    pub group: Option<u32>,
    /// When walk came on while it stood at this kerb: its start-up runs from here.
    pub walk_onset: Option<v2xw_core::time::SimTime>,
    /// What it is doing.
    pub activity: PedActivity,
    /// Its mid-block crossing, from the decision to the far kerb.
    pub midblock: Option<Midblock>,
    /// The sidewalk lane on which it last gave a mid-block crossing up (it does not try
    /// again on the same lane).
    pub midblock_declined: Option<LaneId>,
    /// How many walks it has been given (its route is replaced after a mid-block crossing).
    pub walks: u32,
}

impl Pedestrian {
    /// Its world position, given the world.
    pub fn position(&self, world: &World) -> Vec3 {
        if let Some(m) = &self.midblock
            && let MidblockStage::Crossing(d) = m.stage
        {
            let len = m.length_m().max(1e-9);
            return m.from.lerp(m.to, (d / len).clamp(0.0, 1.0));
        }
        world
            .try_lane(self.lane)
            .map(|l| l.offset_point(self.s_m, self.lateral_m))
            .unwrap_or(Vec3::ZERO)
    }

    /// True while it is in the carriageway on a mid-block crossing.
    pub fn crossing_midblock(&self) -> bool {
        self.midblock
            .as_ref()
            .is_some_and(|m| matches!(m.stage, MidblockStage::Crossing(_)))
    }
}

/// The social-force pedestrian model.
#[derive(Debug, Clone)]
pub struct SocialForce {
    params: SocialForceParams,
    people: BTreeMap<ActorId, Pedestrian>,
    card: ModelCard,
    /// Whether each crossing lane may be stepped onto now: its pedestrian signal and
    /// whether a vehicle makes it a hazard. Set by the engine before each step
    /// ([`SocialForce::set_crossing_permits`]); a crossing lane with no entry has no signal
    /// and no hazard.
    permits: BTreeMap<LaneId, CrossingPermit>,
    /// The world's mid-block crossing points, when mid-block crossing is on.
    midblock_index: Option<Arc<MidblockIndex>>,
    /// Which pedestrians waiting mid-block may step off now (set by the engine, from the
    /// vehicles' positions after they moved).
    midblock_go: BTreeMap<ActorId, bool>,
    /// Whether the traffic beside each pedestrian stands still (set by the engine).
    beside_queue: std::collections::BTreeSet<ActorId>,
    /// Where each pedestrian crossing mid-block must stop short along its path, because
    /// the next lane is not clear (set by the engine).
    midblock_holds: BTreeMap<ActorId, f64>,
    stats: PedestrianStats,
}

impl Default for SocialForce {
    fn default() -> Self {
        SocialForce::new(SocialForceParams::default())
    }
}

impl SocialForce {
    /// The model with the given parameters.
    pub fn new(params: SocialForceParams) -> Self {
        Self {
            card: card(&params),
            params,
            people: BTreeMap::new(),
            permits: BTreeMap::new(),
            midblock_index: None,
            midblock_go: BTreeMap::new(),
            beside_queue: std::collections::BTreeSet::new(),
            midblock_holds: BTreeMap::new(),
            stats: PedestrianStats::default(),
        }
    }

    /// Hands the model the world's mid-block crossing points.
    pub fn set_midblock_index(&mut self, index: Arc<MidblockIndex>) {
        self.midblock_index = Some(index);
    }

    /// Hands the model, for each pedestrian waiting mid-block, whether the traffic leaves
    /// it a gap now.
    pub fn set_midblock_permits(&mut self, go: BTreeMap<ActorId, bool>) {
        self.midblock_go = go;
    }

    /// Hands the model, for each pedestrian crossing mid-block, how far along its path it
    /// may walk this step (the lane after that is not clear).
    pub fn set_midblock_holds(&mut self, holds: BTreeMap<ActorId, f64>) {
        self.midblock_holds = holds;
    }

    /// Hands the model the pedestrians beside traffic that stands still.
    pub fn set_beside_queue(&mut self, ids: std::collections::BTreeSet<ActorId>) {
        self.beside_queue = ids;
    }

    /// What the pedestrians have done so far.
    pub fn stats(&self) -> PedestrianStats {
        self.stats
    }

    /// The mid-block paths in use: each pedestrian waiting at, or walking, one, with its
    /// bands and — while it is in the carriageway — how far along it is.
    pub fn midblock_paths(&self) -> impl Iterator<Item = (ActorId, &Midblock)> + '_ {
        self.people.values().filter_map(|p| {
            p.midblock
                .as_ref()
                .filter(|m| !m.bands.is_empty())
                .map(|m| (p.actor, m))
        })
    }

    /// Draws a pedestrian's traits from its own stream ([`TRAITS_ID`]) and its desired
    /// speed under the speed law.
    pub fn draw_traits(&self, ctx: &dyn MobCtx, actor: ActorId) -> (PedestrianTraits, f64) {
        let p = &self.params;
        let mut rng = ctx.rng(RngDomain::plugin(TRAITS_ID), EntityRef::Actor(actor));
        let older = rng.uniform(0.0, 1.0) < p.older_share;
        let violator = rng.uniform(0.0, 1.0) < p.red_crossing_share;
        let z1 = rng.normal(0.0, 1.0);
        let z2 = rng.normal(0.0, 1.0);
        let startup_s = if p.startup_median_s > 0.0 {
            math::exp(math::ln(p.startup_median_s) + p.startup_sigma * z1).clamp(0.2, 8.0)
        } else {
            0.0
        };
        let gap_margin_s = if p.gap_margin_s > 0.0 {
            math::exp(math::ln(p.gap_margin_s) + p.gap_margin_sigma * z2).clamp(0.5, 6.0)
        } else {
            0.0
        };
        let speed = match p.speed_law {
            SpeedLaw::Helbing1995 => None,
            SpeedLaw::Knoblauch1996 => {
                let (m, sd) = if older {
                    (p.older_speed_mean_mps, p.older_speed_std_mps)
                } else {
                    (p.younger_speed_mean_mps, p.younger_speed_std_mps)
                };
                Some(rng.normal(m, sd).clamp(0.5, 2.5))
            }
        };
        let speed = speed.unwrap_or_else(|| {
            ctx.rng(RngDomain::DesiredSpeed, EntityRef::Actor(actor))
                .normal(p.desired_speed_mean_mps, p.desired_speed_std_mps)
                .max(0.1)
        });
        (
            PedestrianTraits {
                older,
                violator,
                startup_s,
                gap_margin_s,
            },
            speed,
        )
    }

    /// The parameters in force.
    pub fn params(&self) -> &SocialForceParams {
        &self.params
    }

    /// Hands the model this step's crossing permits (from
    /// [`crate::vru::crosswalk::CrosswalkIndex::permits`]).
    pub fn set_crossing_permits(&mut self, permits: BTreeMap<LaneId, CrossingPermit>) {
        self.permits = permits;
    }

    /// True if `lane` is somewhere a pedestrian may walk.
    pub fn is_walkable(world: &World, lane: LaneId) -> bool {
        world.try_lane(lane).is_some_and(|l| {
            matches!(l.kind, LaneKind::Sidewalk | LaneKind::Crossing)
                && l.admits(v2xw_world::ClassMask::PEDESTRIAN)
        })
    }

    /// Puts a pedestrian on `route` at `s_m` along its first lane.
    ///
    /// The desired speed is drawn from this actor's own `desired-speed` stream, so it does
    /// not depend on how many other pedestrians were spawned first.
    ///
    /// # Errors
    ///
    /// [`MobError::LaneNotAdmitted`] if any lane of the route is not walkable, and
    /// [`MobError::NoSuchLane`] if the route is empty or names a lane the world lacks.
    pub fn spawn(
        &mut self,
        ctx: &mut dyn MobCtx,
        actor: ActorId,
        route: Vec<LaneId>,
        s_m: f64,
    ) -> Result<()> {
        // A route that is not walkable is refused by `spawn_with`, before anything is
        // drawn.
        Self::check_route(ctx.world(), &route)?;
        let (traits, desired) = if self.params.speed_law == SpeedLaw::Helbing1995
            && self.params.red_crossing_share <= 0.0
            && self.params.startup_median_s <= 0.0
            && self.params.midblock.rate_per_100m <= 0.0
        {
            // The document model draws only the speed, from the one stream it always
            // drew from, so its runs are unchanged.
            let mut rng = ctx.rng(RngDomain::DesiredSpeed, EntityRef::Actor(actor));
            let v = rng
                .normal(
                    self.params.desired_speed_mean_mps,
                    self.params.desired_speed_std_mps,
                )
                .max(0.1);
            (PedestrianTraits::default(), v)
        } else {
            self.draw_traits(&*ctx, actor)
        };
        self.spawn_with(ctx, actor, route, s_m, 0.0, traits, desired, None)
    }

    /// [`SocialForce::spawn`] with the traits, desired speed, lateral offset and group
    /// given: how the engine puts a group on a sidewalk together, sharing the slowest
    /// member's speed and the leader's compliance.
    ///
    /// # Errors
    ///
    /// As [`SocialForce::spawn`].
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_with(
        &mut self,
        ctx: &mut dyn MobCtx,
        actor: ActorId,
        route: Vec<LaneId>,
        s_m: f64,
        lateral_m: f64,
        traits: PedestrianTraits,
        desired: f64,
        group: Option<u32>,
    ) -> Result<()> {
        let world = ctx.world();
        let first = Self::check_route(world, &route)?;
        let half = (0.5 * world.lane(first).width_m
            - 0.5 * VehicleClass::Pedestrian.spec().width_m)
            .max(0.0);
        self.people.insert(
            actor,
            Pedestrian {
                actor,
                lane: first,
                s_m,
                lateral_m: 0.0,
                vel: Vec3::ZERO,
                desired_speed_mps: desired.max(0.1),
                route,
                route_index: 0,
                arrived: false,
                waiting_since: None,
                against_signal: None,
                traits,
                group,
                walk_onset: None,
                activity: PedActivity::Walking,
                midblock: None,
                midblock_declined: None,
                walks: 0,
            },
        );
        if let Some(p) = self.people.get_mut(&actor) {
            p.lateral_m = lateral_m.clamp(-half, half);
        }
        Ok(())
    }

    /// The route's first lane, if every lane of it is walkable.
    fn check_route(world: &World, route: &[LaneId]) -> Result<LaneId> {
        let first = *route.first().ok_or(MobError::EmptyWorld {
            what: "lane in the pedestrian's route",
        })?;
        for lane in route {
            let l = world
                .try_lane(*lane)
                .ok_or(MobError::NoSuchLane { lane: *lane })?;
            if !Self::is_walkable(world, *lane) {
                return Err(MobError::LaneNotAdmitted {
                    lane: *lane,
                    kind: l.kind.wire_name(),
                    classes: "pedestrian".to_string(),
                });
            }
        }
        Ok(first)
    }

    /// Removes a pedestrian.
    pub fn despawn(&mut self, actor: ActorId) -> Option<Pedestrian> {
        self.people.remove(&actor)
    }

    /// Every pedestrian, in actor-id order.
    pub fn people(&self) -> impl Iterator<Item = &Pedestrian> + '_ {
        self.people.values()
    }

    /// One pedestrian.
    pub fn get(&self, actor: ActorId) -> Option<&Pedestrian> {
        self.people.get(&actor)
    }

    /// How many pedestrians the model holds.
    pub fn len(&self) -> usize {
        self.people.len()
    }

    /// True if it holds none.
    pub fn is_empty(&self) -> bool {
        self.people.is_empty()
    }

    /// The field-of-view weight for an influence `f` on a pedestrian facing `e`
    /// (§2.5: `w = 1` inside the field of view, `c` outside it).
    pub fn view_weight(&self, facing: Vec3, force: Vec3) -> f64 {
        let magnitude = force.norm_2d();
        if magnitude <= 0.0 {
            return 1.0;
        }
        let half_angle = 0.5 * self.params.field_of_view_deg * core::f64::consts::PI / 180.0;
        let cos_phi = math::cos(half_angle);
        if facing.dot(force) >= magnitude * cos_phi {
            1.0
        } else {
            self.params.behind_weight
        }
    }

    /// The repulsion one pedestrian feels from another (§2.5).
    ///
    /// `r` is the vector from the other pedestrian to this one; `other_vel` is the other's
    /// velocity, which the elliptical potential uses to look one step ahead.
    pub fn pedestrian_repulsion(&self, r: Vec3, other_vel: Vec3) -> Vec3 {
        let r_norm = r.norm_2d();
        if r_norm <= 1e-9 {
            return Vec3::ZERO;
        }
        if !self.params.elliptical {
            let magnitude = (self.params.v0_m2_s2 / self.params.sigma_m)
                * math::exp(-r_norm / self.params.sigma_m);
            return Vec3::new_2d(r.x / r_norm * magnitude, r.y / r_norm * magnitude);
        }
        // The elliptical potential: the other pedestrian's next position is a second focus.
        let y = Vec3::new_2d(
            other_vel.x * self.params.step_width_s,
            other_vel.y * self.params.step_width_s,
        );
        let r_minus_y = Vec3::new_2d(r.x - y.x, r.y - y.y);
        let r_minus_y_norm = r_minus_y.norm_2d();
        let y_norm = y.norm_2d();
        let sum = r_norm + r_minus_y_norm;
        let b_squared = 0.25 * (sum * sum - y_norm * y_norm);
        // Two numerical guards, both for the same degenerate case: the other pedestrian's
        // *next* position coincides with ours, so the ellipse collapses (`b → 0`) and the
        // second unit vector is undefined. The force there is maximal, not zero, so `b` is
        // floored at a millimetre — which caps the repulsion at
        // `(V0/σ)·(|r| + |r−y|)/(4·1 mm)` — and the collapsed direction falls back on the
        // separation itself.
        const FLOOR_M: f64 = 1e-3;
        let b = math::sqrt(b_squared.max(0.0)).max(FLOOR_M);
        let (dir_x, dir_y) = if r_minus_y_norm > FLOOR_M {
            (
                r.x / r_norm + r_minus_y.x / r_minus_y_norm,
                r.y / r_norm + r_minus_y.y / r_minus_y_norm,
            )
        } else {
            (2.0 * r.x / r_norm, 2.0 * r.y / r_norm)
        };
        // −dV/db = (V0/σ)·exp(−b/σ);  ∇_r b = (|r| + |r−y|)/(4b)·(r̂ + (r−y)ˆ).
        let dv_db =
            (self.params.v0_m2_s2 / self.params.sigma_m) * math::exp(-b / self.params.sigma_m);
        let scale = dv_db * sum / (4.0 * b);
        Vec3::new_2d(scale * dir_x, scale * dir_y)
    }

    /// The repulsion a border at distance `d` exerts, magnitude only (§2.5).
    pub fn border_repulsion_magnitude(&self, d_m: f64) -> f64 {
        (self.params.u0_m2_s2 / self.params.r_m) * math::exp(-d_m.max(0.0) / self.params.r_m)
    }
}

impl v2xw_core::model::Model for SocialForce {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl VruMobility for SocialForce {
    fn step(
        &mut self,
        ctx: &mut dyn MobCtx,
        dt: Duration,
        vehicles: &ActorSnapshot,
    ) -> Vec<(ActorId, Kinematics)> {
        let dt_s = dt.as_secs_f64();
        if dt_s <= 0.0 {
            return Vec::new();
        }
        let now = ctx.now();
        // The state this step computes is the state at the step's *end*, as a vehicle's
        // is: stamping it at the start filed every pedestrian's `gt.kinematics` one step
        // behind the vehicles', and a consumer that groups records by step (the live
        // server does) saw each step's actor set split in two and lost the pedestrians.
        let t_end = dt.after(now);
        // The start-of-step positions of every pedestrian: the same Jacobi discipline the
        // vehicles use, so one pedestrian's move cannot depend on another's having moved.
        let frozen: Vec<(ActorId, Vec3, Vec3)> = {
            let world = ctx.world();
            self.people
                .values()
                .map(|p| (p.actor, p.position(world), p.vel))
                .collect()
        };
        // The frozen positions by grid cell, one interaction radius across, so the pairwise
        // term looks at the neighbours only (it was every pair: a quadratic step that made
        // a few thousand pedestrians the slowest part of a run). Candidates are visited in
        // ascending index — actor order — exactly as the full scan visited them, so the
        // sum is the same to the last bit.
        let cell = self.params.interaction_radius_m.max(1.0);
        let key = |p: Vec3| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
        let mut grid: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
        for (i, (_, pos, _)) in frozen.iter().enumerate() {
            grid.entry(key(*pos)).or_default().push(i);
        }
        // Each group leader's mid-block plan at the step's start: a follower adopts it.
        let leader_plans: BTreeMap<u32, (LaneId, Midblock)> = self
            .people
            .values()
            .filter(|p| p.group.is_none_or(|g| g == p.actor.index()))
            .filter_map(|p| {
                p.midblock
                    .as_ref()
                    .filter(|m| !matches!(m.stage, MidblockStage::Crossing(_)))
                    .map(|m| (p.actor.index(), (p.lane, m.clone())))
            })
            .collect();
        let fluctuation = self.params.fluctuation_mps2;
        let midblock_on =
            self.params.midblock.rate_per_100m > 0.0 && self.midblock_index.is_some();
        let mut out: Vec<(ActorId, Kinematics)> = Vec::with_capacity(self.people.len());
        let actors: Vec<ActorId> = self.people.keys().copied().collect();
        for actor in actors {
            let noise = if fluctuation > 0.0 {
                let mut rng = ctx.rng(RngDomain::plugin(MODEL_ID), EntityRef::Actor(actor));
                Vec3::new_2d(rng.normal(0.0, fluctuation), rng.normal(0.0, fluctuation))
            } else {
                Vec3::ZERO
            };
            // One draw per step whether or not it is used, so a decision does not depend
            // on how many steps other pedestrians spent at a kerb. Only drawn at all when
            // crossing against the signal is switched on, so the default run's streams are
            // untouched.
            let jaywalk_draw = if self.params.jaywalk_probability > 0.0 {
                ctx.rng(RngDomain::plugin(JAYWALK_ID), EntityRef::Actor(actor))
                    .uniform(0.0, 1.0)
            } else {
                1.0
            };
            // The mid-block decision draw, one per step for the same reason.
            let mid_draw = if midblock_on {
                ctx.rng(RngDomain::plugin(MIDBLOCK_ID), EntityRef::Actor(actor))
                    .uniform(0.0, 1.0)
            } else {
                1.0
            };
            let world = ctx.world();
            let Some(person) = self.people.get(&actor) else {
                continue;
            };
            if person.arrived {
                out.push((actor, self.kinematics_of(person, world, t_end)));
                continue;
            }
            if person.crossing_midblock() {
                self.step_midblock_crossing(ctx, actor, dt_s, now);
                let world = ctx.world();
                let person = &self.people[&actor];
                out.push((actor, self.kinematics_of(person, world, t_end)));
                continue;
            }
            let Some(lane) = world.try_lane(person.lane) else {
                continue;
            };
            let position = lane.offset_point(person.s_m, person.lateral_m);
            let heading = lane.heading_at(person.s_m);
            let (sin_h, cos_h) = math::sin_cos(heading);
            let forward = Vec3::new_2d(cos_h, sin_h);

            // --- a mid-block crossing: plan, approach, wait ---------------------
            let mut midblock = person.midblock.clone().filter(|_| lane.kind == LaneKind::Sidewalk);
            if midblock.is_none()
                && lane.kind == LaneKind::Sidewalk
                && person.midblock_declined != Some(person.lane)
            {
                let leader = person.group.filter(|g| *g != actor.index());
                if let Some(g) = leader {
                    // A follower crosses where its group's leader does.
                    if let Some((l, plan)) = leader_plans.get(&g)
                        && *l == person.lane
                        && plan.at_s_m > person.s_m
                    {
                        midblock = Some(Midblock {
                            from: Vec3::ZERO,
                            bands: Vec::new(),
                            stage: MidblockStage::Approaching,
                            ..plan.clone()
                        });
                    }
                } else if midblock_on {
                    let p = &self.params.midblock;
                    let factor = if self.beside_queue.contains(&actor) {
                        p.stopped_traffic_factor
                    } else {
                        1.0
                    };
                    let chance = p.rate_per_100m / 100.0 * person.desired_speed_mps * dt_s * factor;
                    if mid_draw < chance
                        && let Some(index) = self.midblock_index.as_ref()
                    {
                        let ahead: Vec<&crate::vru::midblock::Site> = index
                            .sites_on(person.lane)
                            .iter()
                            .filter(|s| s.s_m > person.s_m + 3.0)
                            .collect();
                        if let Some(first) = ahead.first() {
                            let same: Vec<&&crate::vru::midblock::Site> =
                                ahead.iter().filter(|s| s.s_m == first.s_m).collect();
                            let mut rng =
                                ctx.rng(RngDomain::plugin(MIDBLOCK_ID), EntityRef::Actor(actor));
                            let site = *same[rng.below(same.len() as u64) as usize];
                            let max = p.diagonal_max_deg.clamp(0.0, 60.0) * core::f64::consts::PI
                                / 180.0;
                            let theta = rng.uniform(0.0, max);
                            let far = world.lane(site.far_lane);
                            let along = math::cos(far.heading_at(site.far_s_m) - heading);
                            let shift = site.width_m * math::tan(theta)
                                * if along >= 0.0 { 1.0 } else { -1.0 };
                            // The far end stays clear of the corners, as the near one does.
                            let margin = index.margin_m().min(0.5 * far.length_m);
                            let lo = margin;
                            let hi = (far.length_m - margin).max(lo);
                            let far_s = (site.far_s_m + shift).clamp(lo, hi);
                            midblock = Some(Midblock {
                                at_s_m: site.s_m,
                                far_lane: site.far_lane,
                                far_s_m: far_s,
                                crossed: site.crossed.clone(),
                                from: Vec3::ZERO,
                                to: far.point_at(far_s),
                                bands: Vec::new(),
                                stage: MidblockStage::Approaching,
                            });
                        }
                    }
                }
            }
            let mut mid_go = false;
            let mut mid_abandon = false;
            let mid_hold = match &midblock {
                Some(m) => match m.stage {
                    MidblockStage::Approaching => person.s_m >= m.at_s_m - 2.0,
                    MidblockStage::Waiting(since) => {
                        if self.midblock_go.get(&actor).copied().unwrap_or(false) {
                            // It steps off from where it stands: held on the sidewalk this
                            // step, in the carriageway from the next.
                            mid_go = true;
                            true
                        } else if Duration::between(since, now).as_secs_f64()
                            >= self.params.midblock.max_wait_s
                        {
                            mid_abandon = true;
                            false
                        } else {
                            true
                        }
                    }
                    MidblockStage::Crossing(_) => false,
                },
                None => false,
            };

            // --- the kerb ----------------------------------------------------
            // A pedestrian whose next lane is a crossing steps onto it only on walk (or
            // at an unsignalised crosswalk) and only when no vehicle is on it or too close
            // to yield (UVC §11-502(b), §11-203): the permit says both. One already on a
            // crossing always walks on — stopping in the carriageway is what a pedestrian
            // caught by a change to don't-walk does not do. Under the observed behaviour a
            // pedestrian who was waiting steps off only after their start-up time, and one
            // who crosses against the signal (a violator) does so only into a gap that the
            // HCM's critical headway accepts.
            let next = self.next_lane(person);
            let mut against = person.against_signal;
            let mut walk_onset = person.walk_onset;
            let mut how = PedActivity::Walking;
            let kerb_hold = match next {
                Some(n)
                    if midblock.is_none()
                        && lane.kind != LaneKind::Crossing
                        && world
                            .try_lane(n)
                            .is_some_and(|l| l.kind == LaneKind::Crossing) =>
                {
                    let permit = self.permits.get(&n).copied().unwrap_or_default();
                    let t_c = permit.length_m / person.desired_speed_mps.max(0.1)
                        + person.traits.gap_margin_s;
                    let gap_ok = permit.min_tta_s > t_c;
                    match permit.signal {
                        None | Some(SignalState::Off) => {
                            how = PedActivity::CrossingUnsignalised;
                            permit.hazard
                        }
                        Some(SignalState::Green) | Some(SignalState::GreenYield) => {
                            how = PedActivity::CrossingOnWalk;
                            // Start-up: one who stood here when walk came on steps off
                            // after their own start-up time.
                            let ready = match (person.waiting_since, walk_onset) {
                                (Some(_), None) => {
                                    walk_onset = Some(now);
                                    person.traits.startup_s <= 0.0
                                }
                                (Some(_), Some(t)) => {
                                    Duration::between(t, now).as_secs_f64()
                                        >= person.traits.startup_s
                                }
                                (None, _) => true,
                            };
                            permit.hazard || !ready
                        }
                        Some(state) => {
                            how = PedActivity::CrossingAgainstSignal;
                            walk_onset = None;
                            let legacy = match against {
                                Some((l, d)) if l == n => d,
                                _ if self.params.jaywalk_probability > 0.0 => {
                                    let d = jaywalk_draw < self.params.jaywalk_probability;
                                    against = Some((n, d));
                                    d
                                }
                                _ => false,
                            };
                            // A violator also starts on flashing don't-walk only if the
                            // gap is there; nobody starts on it otherwise (MUTCD §4E.02).
                            let violates = person.traits.violator && gap_ok;
                            let _ = state;
                            permit.hazard || !(legacy || violates)
                        }
                    }
                }
                _ => false,
            };
            let hold = kerb_hold || mid_hold;
            let stop_s = match &midblock {
                Some(m) if mid_hold => m.at_s_m.min(lane.length_m - self.params.kerb_margin_m),
                _ => lane.length_m - self.params.kerb_margin_m,
            };
            let to_kerb = stop_s - person.s_m;
            // Slowing into the kerb over the last two metres rather than stopping on it.
            let desired_speed = if hold {
                person.desired_speed_mps * (to_kerb / 2.0).clamp(0.0, 1.0)
            } else {
                person.desired_speed_mps
            };
            let mut accel = Vec3::new_2d(
                (desired_speed * forward.x - person.vel.x) / self.params.tau_s,
                (desired_speed * forward.y - person.vel.y) / self.params.tau_s,
            );

            // --- pairwise repulsion ---------------------------------------
            let (cx, cy) = key(position);
            let mut near: Vec<usize> = Vec::new();
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(v) = grid.get(&(cx + dx, cy + dy)) {
                        near.extend_from_slice(v);
                    }
                }
            }
            near.sort_unstable();
            for (other, other_pos, other_vel) in near.iter().map(|i| &frozen[*i]) {
                if *other == actor {
                    continue;
                }
                let r = Vec3::new_2d(position.x - other_pos.x, position.y - other_pos.y);
                if r.norm_2d() > self.params.interaction_radius_m {
                    continue;
                }
                let f = self.pedestrian_repulsion(r, *other_vel);
                let w = self.view_weight(forward, f);
                accel = Vec3::new_2d(accel.x + w * f.x, accel.y + w * f.y);
            }

            // --- borders: the kerbs of this lane --------------------------
            let half_width = 0.5 * lane.width_m;
            let body = 0.5 * VehicleClass::Pedestrian.spec().width_m;
            let walkable_half = (half_width - body).max(0.0);
            let left_distance = (walkable_half - person.lateral_m).max(0.0);
            let right_distance = (walkable_half + person.lateral_m).max(0.0);
            let left_normal = Vec3::new_2d(-forward.y, forward.x);
            let left_force = self.border_repulsion_magnitude(left_distance);
            let right_force = self.border_repulsion_magnitude(right_distance);
            accel = Vec3::new_2d(
                accel.x - left_normal.x * left_force + left_normal.x * right_force,
                accel.y - left_normal.y * left_force + left_normal.y * right_force,
            );

            // --- vehicles as borders --------------------------------------
            for vehicle in vehicles.actors_within(position, self.params.interaction_radius_m) {
                let Some(state) = vehicles.kinematics(vehicle) else {
                    continue;
                };
                let r = Vec3::new_2d(position.x - state.pos.x, position.y - state.pos.y);
                let d = r.norm_2d();
                if d <= 1e-9 {
                    continue;
                }
                let magnitude = self.border_repulsion_magnitude(d);
                accel = Vec3::new_2d(accel.x + r.x / d * magnitude, accel.y + r.y / d * magnitude);
            }
            accel = Vec3::new_2d(accel.x + noise.x, accel.y + noise.y);

            // --- integrate -------------------------------------------------
            let mut velocity =
                Vec3::new_2d(person.vel.x + accel.x * dt_s, person.vel.y + accel.y * dt_s);
            let max_speed = self.params.max_speed_factor * person.desired_speed_mps;
            let speed = velocity.norm_2d();
            if speed > max_speed && speed > 0.0 {
                velocity = Vec3::new_2d(
                    velocity.x * max_speed / speed,
                    velocity.y * max_speed / speed,
                );
            }
            let moved = Vec3::new_2d(
                position.x + velocity.x * dt_s,
                position.y + velocity.y * dt_s,
            );

            // --- put it back on its lane -----------------------------------
            let projection = lane.project_point(moved);
            let mut s_m = projection.s_m;
            let kerb = stop_s.max(0.0);
            if hold && s_m > kerb {
                // Standing at the kerb: no further along the pavement, and no velocity
                // carrying it into the road.
                s_m = kerb.max(person.s_m.min(kerb));
                let along = velocity.x * forward.x + velocity.y * forward.y;
                if along > 0.0 {
                    velocity = Vec3::new_2d(
                        velocity.x - along * forward.x,
                        velocity.y - along * forward.y,
                    );
                }
            }
            let mut lateral_m = projection.d_m.clamp(-walkable_half, walkable_half);
            let mut current = person.lane;
            let mut route_index = person.route_index;
            let mut arrived = false;
            if s_m >= lane.length_m {
                match self.next_lane(person) {
                    Some(next) if Self::is_walkable(world, next) => {
                        let overshoot = s_m - lane.length_m;
                        current = next;
                        route_index += 1;
                        s_m = overshoot.min(world.lane(next).length_m);
                        lateral_m = 0.0;
                    }
                    _ => {
                        s_m = lane.length_m;
                        arrived = true;
                    }
                }
            } else if s_m < 0.0 {
                s_m = 0.0;
            }
            // Waiting: counted from the first step it stood at the kerb; a pedestrian who
            // has waited `max_wait_s` gives the walk up (and is replaced).
            let at_kerb = hold && s_m >= kerb - 0.5;
            let waiting_since = if at_kerb {
                Some(person.waiting_since.unwrap_or(now))
            } else {
                None
            };
            let gave_up = waiting_since.is_some_and(|since| {
                Duration::between(since, now).as_secs_f64() >= self.params.max_wait_s
            });
            // The mid-block plan's next stage.
            let mut mid_start: Option<Vec3> = None;
            if let Some(m) = midblock.as_mut() {
                match m.stage {
                    MidblockStage::Approaching if at_kerb && mid_hold => {
                        let from = lane.offset_point(s_m, lateral_m);
                        m.from = from;
                        // The lanes the actual path cuts, diagonal included; a path that
                        // meets a junction connector or a crosswalk is given up.
                        let crossed = self
                            .midblock_index
                            .as_ref()
                            .map_or(Some(m.crossed.clone()), |ix| ix.path_lanes(world, from, m.to));
                        m.bands = match crossed {
                            Some(lanes) => {
                                m.crossed = lanes;
                                path_bands(
                                    world,
                                    &m.crossed,
                                    from,
                                    m.to,
                                    self.params.midblock.corridor_width_m,
                                    actor.index() as usize,
                                )
                            }
                            None => Vec::new(),
                        };
                        m.stage = MidblockStage::Waiting(now);
                        if m.bands.is_empty() {
                            // Nothing to cross after all (a lane the path misses).
                            mid_abandon = true;
                        }
                    }
                    MidblockStage::Waiting(_) if mid_go => {
                        mid_start = Some(m.from);
                        m.stage = MidblockStage::Crossing(0.0);
                    }
                    _ => {}
                }
            }
            let left_lane = current != person.lane;
            if left_lane || mid_abandon {
                midblock = None;
            }
            // Stepping off mid-block ends the wait.
            let waiting_since = if mid_start.is_some() {
                None
            } else {
                waiting_since
            };
            let previous_wait = person.waiting_since;
            let walk_onset = if at_kerb && !left_lane { walk_onset } else { None };
            let activity = if mid_start.is_some() {
                PedActivity::CrossingMidblock
            } else if left_lane && world.lane(current).kind == LaneKind::Crossing {
                how
            } else if left_lane {
                PedActivity::Walking
            } else if at_kerb && mid_hold {
                PedActivity::WaitingMidblock
            } else if at_kerb {
                PedActivity::WaitingAtKerb
            } else if lane.kind == LaneKind::Crossing {
                person.activity
            } else {
                PedActivity::Walking
            };
            // --- statistics ------------------------------------------------
            let ended_wait = previous_wait.filter(|_| waiting_since.is_none() || gave_up);
            if let Some(since) = ended_wait {
                let w = Duration::between(since, now).as_secs_f64();
                self.stats.waits += 1;
                self.stats.wait_total_s += w;
                self.stats.wait_max_s = self.stats.wait_max_s.max(w);
            }
            if left_lane && world.lane(current).kind == LaneKind::Crossing {
                let signal = self.permits.get(&current).and_then(|p| p.signal);
                match (how, signal) {
                    (PedActivity::CrossingOnWalk, _) => self.stats.crossings_on_walk += 1,
                    (PedActivity::CrossingAgainstSignal, Some(SignalState::Amber)) => {
                        self.stats.crossings_on_flashing += 1;
                    }
                    (PedActivity::CrossingAgainstSignal, _) => {
                        self.stats.crossings_on_dont_walk += 1;
                    }
                    _ => self.stats.crossings_unsignalised += 1,
                }
            }
            if mid_start.is_some() {
                self.stats.midblock_crossings += 1;
            }
            if mid_abandon {
                self.stats.midblock_abandoned += 1;
            }
            let declined = if mid_abandon {
                Some(person.lane)
            } else {
                person.midblock_declined
            };
            let person = self.people.get_mut(&actor).expect("present above");
            person.lane = current;
            person.s_m = s_m;
            person.lateral_m = lateral_m;
            person.vel = velocity;
            person.route_index = route_index;
            person.arrived = arrived || gave_up;
            person.waiting_since = waiting_since;
            person.against_signal = against;
            person.walk_onset = walk_onset;
            person.activity = activity;
            person.midblock = midblock;
            person.midblock_declined = declined;
            let person = &self.people[&actor];
            let world = ctx.world();
            out.push((actor, self.kinematics_of(person, world, t_end)));
        }
        out.sort_by_key(|(a, _)| *a);
        out
    }
}

impl SocialForce {
    /// One step of a pedestrian in the carriageway on a mid-block crossing: it walks the
    /// straight path at its desired speed (relaxing towards it with `τ`) and, at the far
    /// kerb, steps onto the far sidewalk and is given a new walk from there.
    fn step_midblock_crossing(&mut self, ctx: &mut dyn MobCtx, actor: ActorId, dt_s: f64, now: v2xw_core::time::SimTime) {
        let tau = self.params.tau_s.max(1e-3);
        let Some(person) = self.people.get(&actor) else {
            return;
        };
        let Some(m) = person.midblock.as_ref() else {
            return;
        };
        let MidblockStage::Crossing(d) = m.stage else {
            return;
        };
        let len = m.length_m().max(1e-6);
        let speed = person.vel.norm_2d();
        let mut v = (speed + (person.desired_speed_mps - speed) * (dt_s / tau).min(1.0)).max(0.0);
        let mut d_next = d + v * dt_s;
        // Waiting at a lane line for the lane ahead to clear.
        if let Some(h) = self.midblock_holds.get(&actor).copied()
            && d_next > h
        {
            d_next = d.max(h);
            v = 0.0;
        }
        let d = d_next;
        let dir = Vec3::new_2d((m.to.x - m.from.x) / len, (m.to.y - m.from.y) / len);
        let landed = d >= len;
        let (far_lane, far_s) = (m.far_lane, m.far_s_m);
        let route = if landed {
            let world = ctx.world();
            let mut rng = ctx.rng(RngDomain::plugin(MIDBLOCK_ID), EntityRef::Actor(actor));
            Some(random_walk(world, far_lane, &mut |n| rng.below(n)))
        } else {
            None
        };
        let _ = now;
        let person = self.people.get_mut(&actor).expect("present above");
        person.vel = Vec3::new_2d(dir.x * v, dir.y * v);
        if let Some(route) = route {
            person.lane = far_lane;
            person.s_m = far_s;
            person.lateral_m = 0.0;
            person.route = route;
            person.route_index = 0;
            person.midblock = None;
            person.activity = PedActivity::Walking;
            person.walks += 1;
            person.walk_onset = None;
            person.waiting_since = None;
        } else if let Some(m) = person.midblock.as_mut() {
            m.stage = MidblockStage::Crossing(d);
            person.activity = PedActivity::CrossingMidblock;
        }
    }
}

/// A walk of up to [`MAX_WALK_LANES`] walkable lanes from `first`, each next lane chosen by
/// `pick(n)` among the walkable successors not yet on it.
pub fn random_walk(world: &World, first: LaneId, pick: &mut dyn FnMut(u64) -> u64) -> Vec<LaneId> {
    let mut route = vec![first];
    while route.len() < MAX_WALK_LANES {
        let last = *route.last().expect("non-empty");
        let next: Vec<LaneId> = world
            .successor_lanes(last)
            .into_iter()
            .filter(|l| SocialForce::is_walkable(world, *l) && !route.contains(l))
            .collect();
        if next.is_empty() {
            break;
        }
        route.push(next[pick(next.len() as u64) as usize]);
    }
    route
}

/// How many walkable lanes a walk strings together at most.
pub const MAX_WALK_LANES: usize = 12;

impl SocialForce {
    /// The lane after the pedestrian's current one, if its route continues.
    fn next_lane(&self, person: &Pedestrian) -> Option<LaneId> {
        person.route.get(person.route_index + 1).copied()
    }

    /// The published kinematics of one pedestrian.
    fn kinematics_of(
        &self,
        person: &Pedestrian,
        world: &World,
        t: v2xw_core::time::SimTime,
    ) -> Kinematics {
        let position = person.position(world);
        let heading = if person.vel.norm_2d() > 1e-6 {
            person.vel.heading_2d()
        } else {
            world
                .try_lane(person.lane)
                .map(|l| l.heading_at(person.s_m))
                .unwrap_or(0.0)
        };
        Kinematics {
            t,
            pos: position,
            vel: person.vel,
            acc: Vec3::ZERO,
            heading_rad: heading,
            yaw_rate_rad_s: 0.0,
            lane: if person.crossing_midblock() {
                None
            } else {
                Some(LanePos::new(person.lane, person.s_m, person.lateral_m))
            },
            dims: Dims::new(
                VehicleClass::Pedestrian.spec().length_m,
                VehicleClass::Pedestrian.spec().width_m,
                VehicleClass::Pedestrian.spec().height_m,
            ),
        }
    }
}

/// The model card.
pub fn card(params: &SocialForceParams) -> ModelCard {
    let helbing = Source {
        kind: SourceKind::Paper,
        reference: "Helbing & Molnár, \"Social force model for pedestrian dynamics\", \
                    Phys. Rev. E 51, 4282 (1995); arXiv:cond-mat/9805244 [R10 §B11]"
            .to_string(),
        accessed: Some("2026-09-17".to_string()),
        note: None,
    };
    let sumo = Source {
        kind: SourceKind::Code,
        reference: "SUMO `jmCrossingGap` default 10 m [R10 §B6, §B13]".to_string(),
        accessed: Some("2026-09-17".to_string()),
        note: None,
    };
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Vru,
        MODEL_VERSION,
        "Pedestrians by the Helbing and Molnár force model: a relaxation towards the \
         desired direction, an elliptical pairwise repulsion that looks one step ahead, a \
         border repulsion from the kerbs of the walkable lane, and vehicles as borders \
         with the SUMO crossing-gap threshold. Pedestrians walk the world's sidewalk and \
         crossing lanes.",
    );
    card.tier = vec![Tier::Medium];
    card.equations = vec![
        Equation {
            name: "acceleration".to_string(),
            latex_or_text: "dv/dt = (v0·e − v)/τ + Σ f_αβ + Σ f_αB + fluctuations".to_string(),
            notes: None,
        },
        Equation {
            name: "pedestrian repulsion".to_string(),
            latex_or_text: "f_αβ = −∇_r V(b),  V(b) = V0·exp(−b/σ),  \
                            b = ½√((|r| + |r − y|)² − |y|²),  y = v_β·Δt·e_β"
                .to_string(),
            notes: Some(
                "the closed-form gradient ∇_r b = ((|r| + |r−y|)/(4b))·(r̂ + (r−y)ˆ) is the \
                 standard reduction and is **transcribed, not quoted**: R10 §B11 records \
                 the parameter table and the model's form, not this expression. \
                 `elliptical = false` falls back to the isotropic b = |r|."
                    .to_string(),
            ),
        },
        Equation {
            name: "border repulsion".to_string(),
            latex_or_text: "f_αB = (U0/R)·exp(−d/R) away from the border".to_string(),
            notes: None,
        },
        Equation {
            name: "field of view".to_string(),
            latex_or_text: "w = 1 if e·f ≥ |f|·cos(φ), else c".to_string(),
            notes: Some("2φ = 200°, c = 0.5".to_string()),
        },
    ];
    card.parameters = vec![
        Parameter::new(
            "desired_speed",
            "m/s",
            serde_json::json!({
                "mean": params.desired_speed_mean_mps,
                "std": params.desired_speed_std_mps,
            }),
            Source {
                kind: SourceKind::Paper,
                reference: "Helbing & Molnár 1995, citing Henderson 1971/1974 [R10 §B11]"
                    .to_string(),
                accessed: Some("2026-09-17".to_string()),
                note: None,
            },
        ),
        Parameter::new(
            "max_speed_factor",
            "1",
            serde_json::json!(params.max_speed_factor),
            helbing.clone(),
        ),
        Parameter::new("tau", "s", serde_json::json!(params.tau_s), helbing.clone()),
        Parameter::new(
            "V0",
            "m²/s²",
            serde_json::json!(params.v0_m2_s2),
            helbing.clone(),
        ),
        Parameter::new(
            "sigma",
            "m",
            serde_json::json!(params.sigma_m),
            helbing.clone(),
        ),
        Parameter::new(
            "U0",
            "m²/s²",
            serde_json::json!(params.u0_m2_s2),
            helbing.clone(),
        ),
        Parameter::new("R", "m", serde_json::json!(params.r_m), helbing.clone()),
        Parameter::new(
            "step_width",
            "s",
            serde_json::json!(params.step_width_s),
            helbing.clone(),
        ),
        Parameter::new(
            "field_of_view",
            "deg",
            serde_json::json!(params.field_of_view_deg),
            helbing.clone(),
        ),
        Parameter::new(
            "behind_weight",
            "1",
            serde_json::json!(params.behind_weight),
            helbing.clone(),
        ),
        Parameter::new(
            "jmCrossingGap",
            "m",
            serde_json::json!(params.crossing_gap_m),
            sumo,
        ),
        Parameter::new(
            "elliptical",
            "-",
            serde_json::json!(params.elliptical),
            helbing.clone(),
        ),
        Parameter::new(
            "jaywalk_probability",
            "1",
            serde_json::json!(params.jaywalk_probability),
            Source::new(
                SourceKind::Code,
                "a switch, off by default: no calibrated rate of crossing against the \
                 signal is cited, so none is assumed",
            ),
        ),
        Parameter::new(
            "max_wait_s",
            "s",
            serde_json::json!(params.max_wait_s),
            Source::new(
                SourceKind::Code,
                "an engineering bound, two 60 s cycles, so a pedestrian at a crosswalk \
                 that never shows walk does not wait forever; not a behavioural value",
            ),
        ),
        Parameter::new(
            "kerb_margin",
            "m",
            serde_json::json!(params.kerb_margin_m),
            Source::new(
                SourceKind::Code,
                "where a waiting pedestrian stands, back from the end of the pavement lane",
            ),
        ),
        Parameter::new(
            "interaction_radius",
            "m",
            serde_json::json!(params.interaction_radius_m),
            Source::new(
                SourceKind::Code,
                "an engineering bound: with σ = 0.3 m the repulsion beyond a few metres is \
                 numerically zero",
            ),
        ),
        Parameter::new(
            "speed_law",
            "-",
            serde_json::json!(params.speed_law),
            Source::new(
                SourceKind::Paper,
                "Knoblauch, Pietrucha & Nitzburg, Field studies of pedestrian walking speed \
                 and start-up time, TRR 1538 (1996): means 4.95 / 4.11 ft/s, 15th \
                 percentiles 4.09 / 3.19 ft/s for pedestrians under 65 / 65 and over; σ \
                 derived assuming normality",
            ),
        ),
        Parameter::new(
            "older_share",
            "1",
            serde_json::json!(params.older_share),
            Source::new(
                SourceKind::Code,
                "a choice near New York City's share of residents 65 and over (about one in \
                 six); no age count of Midtown's sidewalks was read",
            ),
        ),
        Parameter::new(
            "red_crossing_share",
            "1",
            serde_json::json!(params.red_crossing_share),
            Source::new(
                SourceKind::Paper,
                "calibrated against Basch, Ethan, Zybert & Basch, Pedestrian behavior at five \
                 dangerous and busy Manhattan intersections, J. Community Health 40:789 \
                 (2015): of 21,760 pedestrians, about 2,319 crossings (10.6 %) began on \
                 don't-walk",
            ),
        ),
        Parameter::new(
            "startup_median_s",
            "s",
            serde_json::json!(params.startup_median_s),
            Source::new(
                SourceKind::Code,
                "a choice below the HCM's 3.2 s pedestrian platoon start-up time; Knoblauch \
                 et al. 1996 measured start-up time but their values could not be read",
            ),
        ),
        Parameter::new(
            "gap_margin_s",
            "s",
            serde_json::json!(params.gap_margin_s),
            Source::new(
                SourceKind::Standard,
                "HCM pedestrian critical headway t_c = L/S_p + t_s (two-way stop-controlled \
                 pedestrian mode); t_s, the start-up and end clearance time, is to be \
                 measured locally and is not re-verified here",
            ),
        ),
        Parameter::new(
            "group_shares",
            "1",
            serde_json::json!(params.group_shares),
            Source::new(
                SourceKind::Paper,
                "Moussaïd, Perozo, Garnier, Helbing & Theraulaz, The walking behaviour of \
                 pedestrian social groups and its impact on crowd dynamics, PLoS ONE \
                 5:e10047 (2010): more than half of pedestrians walk in groups on a \
                 workday; the per-size split is a choice matching that share",
            ),
        ),
        Parameter::new(
            "midblock",
            "-",
            serde_json::json!(params.midblock),
            Source::new(
                SourceKind::Code,
                "crate::vru::midblock: the rate, the stopped-traffic factor and the driver \
                 yield probability are choices; the gap rule is the HCM critical headway, \
                 lane by lane",
            ),
        ),
        Parameter {
            name: "fluctuation".to_string(),
            unit: "m/s²".to_string(),
            default: serde_json::json!(params.fluctuation_mps2),
            range: None,
            source: Source::todo_calibrate(
                "Helbing & Molnár write \"+ fluctuations\" and R10 §B11 records no magnitude",
            ),
            calibration: Some(
                "Plan: fit the fluctuation magnitude to a measured speed-variance or \
                 lane-formation statistic (the paper's own N(W) ≈ 0.36·W + 0.59 lane count \
                 on a 10 m walkway is the check R10 §B11 records). Zero until then, so no \
                 invented noise term shapes a trajectory."
                    .to_string(),
            ),
        },
    ];
    card.assumptions = vec![
        "Pedestrians walk sidewalk and crossing lanes of the world; the lane's own width \
         gives the two borders."
            .to_string(),
        "The lateral offset is **clamped** to the walkable half-width at the end of every \
         step. The border potential is soft, so a finite step could otherwise cross a \
         kerb; the clamp makes \"pedestrians stay on walkable lanes\" an invariant rather \
         than a tendency."
            .to_string(),
        "A pedestrian about to step onto a crossing waits at the kerb unless its \
         pedestrian signal shows walk (or it has none) and no vehicle is on the crosswalk \
         or too close to yield (UVC §11-502(b), §11-203; the kinematic test of \
         vru::crosswalk). One already on a crossing walks on. The wait is a desired speed \
         that falls to zero over the last 2 m, and the kerb is a hard stop."
            .to_string(),
        "Crossing against the signal: the legacy switch `jaywalk_probability` (off by \
         default) draws once per pedestrian per crosswalk; the observed behaviour instead \
         makes `red_crossing_share` of pedestrians violators, who start on flashing or \
         steady don't-walk only into a gap the HCM critical headway t_c = L/S_p + t_s \
         accepts (L the crosswalk, S_p their own speed). Either still requires no vehicle \
         hazard."
            .to_string(),
        "Mid-block crossing (crate::vru::midblock): a constant hazard per metre of eligible \
         sidewalk walked, raised beside stopped traffic; straight or diagonal; the \
         pedestrian waits at the kerb for a lane-by-lane (rolling) gap and walks a straight \
         path to the far sidewalk, off every lane."
            .to_string(),
        "Groups (observed behaviour): a group shares one walk, starts side by side, walks \
         at its slowest member's pace and takes its leader's compliance; a follower crosses \
         mid-block where its leader does."
            .to_string(),
        "Every pedestrian reads the start-of-step positions of the others and of the \
         vehicles, so the update is the same Jacobi update the vehicles use."
            .to_string(),
    ];
    card.limitations = vec![
        "No jam state (§2.5 ignores it). A group is held together only by its shared walk \
         and pace, not by an attraction term."
            .to_string(),
        "A pedestrian crossing mid-block walks a straight line at its desired speed and does \
         not stop or run in the carriageway; the vehicles stop for it."
            .to_string(),
        "The attraction term of the 1995 model (shop windows, companions) is not \
         implemented: it is optional in the paper and has no cited parameters."
            .to_string(),
    ];
    card.ignores = vec![
        "Group behaviour, jam states and SUMO's stripe discretisation (medium relative to \
         high, 04-models.md §2.5)."
            .to_string(),
    ];
    card.sources = vec![helbing];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec![
            RngDomain::DesiredSpeed.as_str().to_string(),
            RngDomain::plugin(MODEL_ID).as_str().to_string(),
            RngDomain::plugin(JAYWALK_ID).as_str().to_string(),
            RngDomain::plugin(TRAITS_ID).as_str().to_string(),
            RngDomain::plugin(MIDBLOCK_ID).as_str().to_string(),
        ],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "vru::social_force::tests::pedestrians_stay_on_walkable_lanes".to_string(),
            "vru::social_force::tests::two_pedestrians_repel_each_other".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::MobilityCtx;
    use crate::worlds::{RingParams, ring};
    use v2xw_core::model::Model;
    use v2xw_core::rng::RngRegistry;
    use v2xw_core::time::NS_PER_MS;
    use v2xw_world::ClassMask;

    /// A ring whose single lane is a sidewalk, which is the smallest world a pedestrian can
    /// walk in circles on.
    fn pavement_ring() -> (World, Vec<LaneId>) {
        let base = ring(&RingParams {
            circumference_m: 400.0,
            segments: 4,
            lane_width_m: 4.0,
            ..RingParams::default()
        })
        .expect("a ring");
        let w = crate::worlds::rebuild(&base, |lanes, _| {
            for lane in lanes.iter_mut() {
                lane.kind = LaneKind::Sidewalk;
                lane.allowed = ClassMask::PEDESTRIAN;
            }
        })
        .expect("a pavement ring");
        let cycle = crate::worlds::ring_cycle(&w, 0);
        (w, cycle)
    }

    #[test]
    fn the_parameters_are_the_document_values() {
        let p = SocialForceParams::default();
        assert_eq!(p.desired_speed_mean_mps, 1.34);
        assert_eq!(p.desired_speed_std_mps, 0.26);
        assert_eq!(p.max_speed_factor, 1.3);
        assert_eq!(p.tau_s, 0.5);
        assert_eq!(p.v0_m2_s2, 2.1);
        assert_eq!(p.sigma_m, 0.3);
        assert_eq!(p.u0_m2_s2, 10.0);
        assert_eq!(p.r_m, 0.2);
        assert_eq!(p.step_width_s, 2.0);
        assert_eq!(p.field_of_view_deg, 200.0);
        assert_eq!(p.behind_weight, 0.5);
        assert_eq!(p.crossing_gap_m, 10.0);
        assert_eq!(p.fluctuation_mps2, 0.0, "no invented noise");
    }

    #[test]
    fn a_pedestrian_walks_at_its_desired_speed() {
        let (w, cycle) = pavement_ring();
        let rng = RngRegistry::new(3);
        let mut m = SocialForce::default();
        {
            let mut ctx = MobilityCtx::new(0, &w, &rng);
            m.spawn(&mut ctx, ActorId::new(1), cycle.clone(), 5.0)
                .expect("spawned");
        }
        let desired = m.get(ActorId::new(1)).unwrap().desired_speed_mps;
        let snapshot = ActorSnapshot::new(0, 50.0);
        let dt = Duration::from_millis(100);
        for k in 0..100u64 {
            let mut ctx = MobilityCtx::new(k * 100 * NS_PER_MS, &w, &rng);
            m.step(&mut ctx, dt, &snapshot);
        }
        // After ten seconds it is walking at its desired speed, to a per cent or so: the
        // relaxation time is 0.5 s, so twenty of them have passed.
        let speed = m.get(ActorId::new(1)).unwrap().vel.norm_2d();
        assert!(
            (speed - desired).abs() / desired < 0.05,
            "{speed} against {desired}"
        );
        // And it has covered about desired × 10 s of arc.
        let person = m.get(ActorId::new(1)).unwrap();
        let travelled = person.route_index as f64 * 100.0 + person.s_m - 5.0;
        assert!(
            (travelled - desired * 10.0).abs() < 2.0,
            "travelled {travelled} m against {} m",
            desired * 10.0
        );
    }

    #[test]
    fn pedestrians_stay_on_walkable_lanes() {
        // The required invariant: whatever the forces do, a pedestrian is always on a
        // walkable lane and inside its width.
        let (w, cycle) = pavement_ring();
        let rng = RngRegistry::new(11);
        let mut m = SocialForce::new(SocialForceParams {
            // A large fluctuation, to push hard against the kerbs.
            fluctuation_mps2: 5.0,
            ..SocialForceParams::default()
        });
        {
            let mut ctx = MobilityCtx::new(0, &w, &rng);
            for id in 1..=12u32 {
                m.spawn(
                    &mut ctx,
                    ActorId::new(id),
                    cycle.clone(),
                    f64::from(id) * 2.0,
                )
                .expect("spawned");
            }
        }
        let snapshot = ActorSnapshot::new(0, 50.0);
        let dt = Duration::from_millis(100);
        for k in 0..600u64 {
            let mut ctx = MobilityCtx::new(k * 100 * NS_PER_MS, &w, &rng);
            let states = m.step(&mut ctx, dt, &snapshot);
            for (actor, k) in states {
                let person = m.get(actor).expect("still here");
                assert!(
                    SocialForce::is_walkable(&w, person.lane),
                    "{actor} left the pavement onto {:?}",
                    w.lane(person.lane).kind
                );
                let half = 0.5 * w.lane(person.lane).width_m;
                assert!(
                    person.lateral_m.abs() <= half + 1e-9,
                    "{actor} is {} m off a {half} m half-width lane",
                    person.lateral_m
                );
                // And the published kinematics agree with the lane position.
                let lane_pos = k.lane.expect("a pedestrian is on a lane");
                assert_eq!(lane_pos.lane, person.lane);
                assert!((lane_pos.s_m - person.s_m).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn two_pedestrians_repel_each_other() {
        let m = SocialForce::default();
        // Head on, one metre apart: the force pushes them apart along the separation.
        let r = Vec3::new_2d(1.0, 0.0);
        let f = m.pedestrian_repulsion(r, Vec3::ZERO);
        assert!(f.x > 0.0, "the force is away from the other: {f:?}");
        assert!(f.y.abs() < 1e-12);
        // It decays exponentially: ten centimetres closer is markedly stronger.
        let near = m.pedestrian_repulsion(Vec3::new_2d(0.5, 0.0), Vec3::ZERO);
        assert!(near.x > f.x * 2.0, "{} against {}", near.x, f.x);
        // And the isotropic form matches the closed-form exponential exactly.
        let iso = SocialForce::new(SocialForceParams {
            elliptical: false,
            ..SocialForceParams::default()
        });
        let want = (2.1 / 0.3) * math::exp(-1.0 / 0.3);
        let got = iso.pedestrian_repulsion(r, Vec3::ZERO);
        assert!((got.x - want).abs() < 1e-9, "{} against {want}", got.x);
        // With a stationary neighbour the elliptical form reduces to the isotropic one.
        assert!((f.x - want).abs() < 1e-9, "{} against {want}", f.x);
    }

    #[test]
    fn the_elliptical_potential_looks_ahead() {
        let m = SocialForce::default();
        // A neighbour walking *towards* us repels more than a stationary one at the same
        // distance, because its next position is closer.
        let r = Vec3::new_2d(2.0, 0.0);
        let still = m.pedestrian_repulsion(r, Vec3::ZERO).norm_2d();
        let closing = m.pedestrian_repulsion(r, Vec3::new_2d(0.5, 0.0)).norm_2d();
        assert!(closing > still, "closing {closing} against still {still}");
        // And a neighbour walking away repels less.
        let leaving = m.pedestrian_repulsion(r, Vec3::new_2d(-0.5, 0.0)).norm_2d();
        assert!(leaving < still, "leaving {leaving} against still {still}");
        // The degenerate case — the neighbour's next position is exactly ours — is finite
        // and at least as strong as the stationary case, not zero.
        let collapsed = m.pedestrian_repulsion(r, Vec3::new_2d(1.0, 0.0)).norm_2d();
        assert!(collapsed.is_finite() && collapsed > still, "{collapsed}");
    }

    #[test]
    fn the_border_force_keeps_a_pedestrian_off_the_kerb() {
        let m = SocialForce::default();
        // At the kerb the force is U0/R = 50 m/s²; two decay lengths away it is e⁻² of that.
        assert!((m.border_repulsion_magnitude(0.0) - 50.0).abs() < 1e-9);
        let far = m.border_repulsion_magnitude(0.4);
        assert!((far - 50.0 * math::exp(-2.0)).abs() < 1e-9);
        assert!(m.border_repulsion_magnitude(2.0) < 0.01);
    }

    #[test]
    fn the_field_of_view_halves_an_influence_from_behind() {
        let m = SocialForce::default();
        let facing = Vec3::new_2d(1.0, 0.0);
        // 200° of view: anything within 100° of straight ahead is seen.
        assert_eq!(m.view_weight(facing, Vec3::new_2d(1.0, 0.0)), 1.0);
        assert_eq!(m.view_weight(facing, Vec3::new_2d(0.0, 1.0)), 1.0);
        assert_eq!(m.view_weight(facing, Vec3::new_2d(-1.0, 0.0)), 0.5);
    }

    /// A pavement ring whose second quarter is a crossing: a pedestrian walking the first
    /// quarter meets a kerb at its end.
    fn kerb_ring() -> (World, Vec<LaneId>) {
        let (base, cycle) = pavement_ring();
        let crossing = cycle[1];
        let w = crate::worlds::rebuild(&base, |lanes, _| {
            lanes[crossing.as_usize()].kind = LaneKind::Crossing;
        })
        .expect("a ring with a crossing");
        (w, cycle)
    }

    /// Walks one pedestrian from 90 m along the first quarter for `steps` steps under a
    /// fixed permit for the crossing; returns the model.
    fn walk_to_kerb(
        params: SocialForceParams,
        permit: CrossingPermit,
        start_s: f64,
        on: usize,
        steps: u64,
    ) -> (SocialForce, World, Vec<LaneId>) {
        let (w, cycle) = kerb_ring();
        let rng = RngRegistry::new(5);
        let mut m = SocialForce::new(params);
        {
            let mut ctx = MobilityCtx::new(0, &w, &rng);
            m.spawn(&mut ctx, ActorId::new(1), cycle[on..].to_vec(), start_s)
                .expect("spawned");
        }
        m.set_crossing_permits([(cycle[1], permit)].into_iter().collect());
        let empty = ActorSnapshot::new(0, 50.0);
        let dt = Duration::from_millis(100);
        for k in 0..steps {
            let mut ctx = MobilityCtx::new(k * 100 * NS_PER_MS, &w, &rng);
            m.step(&mut ctx, dt, &empty);
        }
        (m, w, cycle)
    }

    /// UVC §11-502(b) and §11-203: a pedestrian does not step off the kerb onto a crossing
    /// while a vehicle makes it a hazard, nor on flashing or steady don't-walk; it stands
    /// at the kerb, on the pavement, until the crossing is permitted.
    #[test]
    fn a_pedestrian_holds_at_the_kerb_until_the_crossing_is_permitted() {
        for permit in [
            CrossingPermit {
                signal: None,
                hazard: true,
                ..CrossingPermit::default()
            },
            CrossingPermit {
                signal: Some(SignalState::Green),
                hazard: true,
                ..CrossingPermit::default()
            },
            CrossingPermit {
                signal: Some(SignalState::Red),
                hazard: false,
                ..CrossingPermit::default()
            },
            CrossingPermit {
                signal: Some(SignalState::Amber),
                hazard: false,
                ..CrossingPermit::default()
            },
        ] {
            let (m, w, cycle) = walk_to_kerb(SocialForceParams::default(), permit, 90.0, 0, 200);
            let p = m.get(ActorId::new(1)).unwrap();
            assert_eq!(p.lane, cycle[0], "{permit:?}: it stepped onto the crossing");
            let kerb = w.lane(cycle[0]).length_m - SocialForceParams::default().kerb_margin_m;
            assert!(
                p.s_m <= kerb + 1e-9,
                "{permit:?}: past the kerb at {}",
                p.s_m
            );
            assert!(p.s_m > kerb - 1.0, "{permit:?}: it did not reach the kerb");
            assert!(p.vel.norm_2d() < 0.05, "{permit:?}: still moving");
            assert!(p.waiting_since.is_some());
        }
        // Walk, no vehicle: it crosses.
        let (m, _, cycle) = walk_to_kerb(
            SocialForceParams::default(),
            CrossingPermit {
                signal: Some(SignalState::Green),
                hazard: false,
                ..CrossingPermit::default()
            },
            90.0,
            0,
            200,
        );
        assert_ne!(m.get(ActorId::new(1)).unwrap().lane, cycle[0]);
    }

    /// A pedestrian already on a crossing walks on whatever the permit says: stopping in
    /// the carriageway is not what the kerb rule asks. (This replaces a test that had a
    /// pedestrian *on* a crossing slow down when a vehicle came within 10 m, which left
    /// pedestrians standing in the road.)
    #[test]
    fn a_pedestrian_on_a_crossing_walks_on() {
        let (m, _, cycle) = walk_to_kerb(
            SocialForceParams::default(),
            CrossingPermit {
                signal: Some(SignalState::Red),
                hazard: true,
                ..CrossingPermit::default()
            },
            2.0,
            1,
            30,
        );
        let p = m.get(ActorId::new(1)).unwrap();
        assert_eq!(p.lane, cycle[1]);
        assert!(p.s_m > 4.0, "it stopped on the crossing at {}", p.s_m);
        assert!(p.vel.norm_2d() > 0.8 * p.desired_speed_mps);
    }

    /// Crossing against the signal is a parameter, off by default; switched fully on, a
    /// pedestrian crosses on don't-walk — but never into a vehicle hazard.
    #[test]
    fn crossing_against_the_signal_is_a_switch_that_never_overrides_a_hazard() {
        let reckless = SocialForceParams {
            jaywalk_probability: 1.0,
            ..SocialForceParams::default()
        };
        let red = CrossingPermit {
            signal: Some(SignalState::Red),
            hazard: false,
            ..CrossingPermit::default()
        };
        let (m, _, cycle) = walk_to_kerb(reckless, red, 90.0, 0, 200);
        assert_ne!(m.get(ActorId::new(1)).unwrap().lane, cycle[0]);
        let (m, _, cycle) = walk_to_kerb(
            reckless,
            CrossingPermit {
                signal: Some(SignalState::Red),
                hazard: true,
                ..CrossingPermit::default()
            },
            90.0,
            0,
            200,
        );
        assert_eq!(m.get(ActorId::new(1)).unwrap().lane, cycle[0]);
        // The default does not cross on red.
        assert_eq!(SocialForceParams::default().jaywalk_probability, 0.0);
    }

    /// The state a step publishes is the state at the step's end, as a vehicle's is: the
    /// engine files `gt.kinematics` at the state's own instant, and a pedestrian stamped at
    /// the step's start landed one step behind every vehicle.
    #[test]
    fn a_step_publishes_the_state_at_its_end() {
        let (w, cycle) = pavement_ring();
        let rng = RngRegistry::new(3);
        let mut m = SocialForce::default();
        {
            let mut ctx = MobilityCtx::new(0, &w, &rng);
            m.spawn(&mut ctx, ActorId::new(1), cycle, 5.0)
                .expect("spawned");
        }
        let snapshot = ActorSnapshot::new(0, 50.0);
        let dt = Duration::from_millis(100);
        for k in 0..5u64 {
            let t0 = k * 100 * NS_PER_MS;
            let mut ctx = MobilityCtx::new(t0, &w, &rng);
            let out = m.step(&mut ctx, dt, &snapshot);
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].1.t, t0 + 100 * NS_PER_MS, "step from {t0}");
        }
    }

    /// Spawns one pedestrian with the given traits and speed 90 m along the first quarter
    /// of the kerb ring, and returns the model and the world.
    fn kerb_with(traits: PedestrianTraits, speed: f64) -> (SocialForce, World, Vec<LaneId>, RngRegistry) {
        let (w, cycle) = kerb_ring();
        let rng = RngRegistry::new(5);
        let mut m = SocialForce::new(SocialForceParams::observed());
        {
            let mut ctx = MobilityCtx::new(0, &w, &rng);
            m.spawn_with(&mut ctx, ActorId::new(1), cycle.clone(), 90.0, 0.0, traits, speed, None)
                .expect("spawned");
        }
        (m, w, cycle, rng)
    }

    fn steps(m: &mut SocialForce, w: &World, rng: &RngRegistry, from: u64, n: u64) {
        let empty = ActorSnapshot::new(0, 50.0);
        for k in from..from + n {
            let mut ctx = MobilityCtx::new(k * 100 * NS_PER_MS, w, rng);
            m.step(&mut ctx, Duration::from_millis(100), &empty);
        }
    }

    /// Knoblauch et al. 1996: pedestrians under 65 walk at 1.51 m/s on average, those 65
    /// and over at 1.25 m/s; the observed preset draws 15 % older walkers.
    #[test]
    fn observed_walking_speeds_follow_the_age_groups() {
        let (w, _) = pavement_ring();
        let rng = RngRegistry::new(11);
        let m = SocialForce::new(SocialForceParams::observed());
        let ctx = MobilityCtx::new(0, &w, &rng);
        let (mut young, mut old) = (Vec::new(), Vec::new());
        for a in 0..6000u32 {
            let (t, v) = m.draw_traits(&ctx, ActorId::new(a));
            if t.older { old.push(v) } else { young.push(v) }
        }
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let share = old.len() as f64 / 6000.0;
        assert!((share - 0.15).abs() < 0.015, "older share {share}");
        assert!((mean(&young) - 1.51).abs() < 0.02, "younger mean {}", mean(&young));
        assert!((mean(&old) - 1.25).abs() < 0.03, "older mean {}", mean(&old));
        // The MUTCD clearance speed (1.07 m/s) is below most of either group.
        let below = young.iter().filter(|v| **v < 1.0668).count() as f64 / young.len() as f64;
        assert!(below < 0.1, "{below}");
    }

    /// A violator steps off on don't-walk into a gap the HCM critical headway accepts —
    /// and not into one it does not, though no vehicle is close enough to be a hazard.
    #[test]
    fn a_violator_crosses_on_dont_walk_only_into_a_gap() {
        let traits = PedestrianTraits {
            older: false,
            violator: true,
            startup_s: 0.0,
            gap_margin_s: 2.0,
        };
        let red = |tta: f64| CrossingPermit {
            signal: Some(SignalState::Red),
            hazard: false,
            min_tta_s: tta,
            length_m: 10.0,
        };
        // t_c = 10 m / 1.4 m/s + 2 s = 9.1 s.
        for (tta, crosses) in [(f64::INFINITY, true), (12.0, true), (5.0, false)] {
            let (mut m, w, cycle, rng) = kerb_with(traits, 1.4);
            m.set_crossing_permits([(cycle[1], red(tta))].into_iter().collect());
            steps(&mut m, &w, &rng, 0, 200);
            let p = m.get(ActorId::new(1)).unwrap();
            assert_eq!(p.lane != cycle[0], crosses, "tta {tta}: lane {:?}", p.lane);
            if crosses {
                assert_eq!(p.activity, PedActivity::CrossingAgainstSignal);
            } else {
                assert_eq!(p.activity, PedActivity::WaitingAtKerb);
            }
        }
        // A compliant pedestrian waits whatever the gap.
        let (mut m, w, cycle, rng) = kerb_with(PedestrianTraits { violator: false, ..traits }, 1.4);
        m.set_crossing_permits([(cycle[1], red(f64::INFINITY))].into_iter().collect());
        steps(&mut m, &w, &rng, 0, 200);
        assert_eq!(m.get(ActorId::new(1)).unwrap().lane, cycle[0]);
        assert_eq!(m.stats().crossings_on_dont_walk, 0);
    }

    /// One who waited at the kerb steps off its start-up time after walk comes on: the
    /// same pedestrian with a 2.5 s start-up reaches the crossing 25 steps later than with
    /// none (both then walk the 0.3 m kerb margin from a standstill).
    #[test]
    fn a_waiting_pedestrian_steps_off_after_its_start_up_time() {
        let stepped_after = |startup_s: f64| -> (u64, PedestrianStats) {
            let traits = PedestrianTraits {
                older: false,
                violator: false,
                startup_s,
                gap_margin_s: 2.0,
            };
            let (mut m, w, cycle, rng) = kerb_with(traits, 1.4);
            let permit = |s| CrossingPermit {
                signal: Some(s),
                ..CrossingPermit::default()
            };
            m.set_crossing_permits([(cycle[1], permit(SignalState::Red))].into_iter().collect());
            steps(&mut m, &w, &rng, 0, 150);
            assert!(m.get(ActorId::new(1)).unwrap().waiting_since.is_some());
            m.set_crossing_permits(
                [(cycle[1], permit(SignalState::Green))].into_iter().collect(),
            );
            let mut stepped = None;
            for k in 150..250u64 {
                steps(&mut m, &w, &rng, k, 1);
                if stepped.is_none() && m.get(ActorId::new(1)).unwrap().lane != cycle[0] {
                    stepped = Some(k - 150);
                }
            }
            (stepped.expect("it crossed"), m.stats())
        };
        let (slow, stats) = stepped_after(2.5);
        let (prompt, _) = stepped_after(0.0);
        assert!(
            (24..=26).contains(&(slow - prompt)),
            "start-up of 2.5 s delayed the step off by {} steps ({slow} against {prompt})",
            slow - prompt
        );
        assert_eq!(stats.crossings_on_walk, 1);
        assert_eq!(stats.waits, 1);
    }

    /// A pedestrian who has waited `max_wait_s` at one kerb gives the walk up.
    #[test]
    fn a_pedestrian_gives_up_after_the_longest_wait() {
        let impatient = SocialForceParams {
            max_wait_s: 5.0,
            ..SocialForceParams::default()
        };
        let red = CrossingPermit {
            signal: Some(SignalState::Red),
            hazard: false,
            ..CrossingPermit::default()
        };
        let (m, _, _) = walk_to_kerb(impatient, red, 95.0, 0, 150);
        assert!(m.get(ActorId::new(1)).unwrap().arrived);
    }

    #[test]
    fn a_pedestrian_may_not_be_put_on_a_road() {
        let w = ring(&RingParams::default()).expect("a ring");
        let rng = RngRegistry::new(1);
        let mut m = SocialForce::default();
        let mut ctx = MobilityCtx::new(0, &w, &rng);
        let err = m
            .spawn(&mut ctx, ActorId::new(1), vec![w.roads.lanes()[0].id], 0.0)
            .expect_err("a driving lane is not walkable");
        assert!(matches!(err, MobError::LaneNotAdmitted { .. }));
        assert!(m.is_empty());
    }

    #[test]
    fn the_step_is_order_independent() {
        // The Jacobi property, for pedestrians: the same crowd stepped from the same state
        // gives the same result whatever order the model happens to hold them in, which is
        // what the frozen positions guarantee. Spawning in reverse order is the test.
        let (w, cycle) = pavement_ring();
        let run = |ids: Vec<u32>| {
            let rng = RngRegistry::new(21);
            let mut m = SocialForce::default();
            {
                let mut ctx = MobilityCtx::new(0, &w, &rng);
                for id in ids {
                    m.spawn(
                        &mut ctx,
                        ActorId::new(id),
                        cycle.clone(),
                        f64::from(id) * 1.5,
                    )
                    .expect("spawned");
                }
            }
            let snapshot = ActorSnapshot::new(0, 50.0);
            let dt = Duration::from_millis(100);
            let mut last = Vec::new();
            for k in 0..50u64 {
                let mut ctx = MobilityCtx::new(k * 100 * NS_PER_MS, &w, &rng);
                last = m.step(&mut ctx, dt, &snapshot);
            }
            last
        };
        let forward = run((1..=8).collect());
        let reverse = run((1..=8).rev().collect());
        assert_eq!(forward, reverse);
    }

    #[test]
    fn the_card_validates() {
        let m = SocialForce::default();
        m.card().validate().expect("validates");
        assert_eq!(m.card().family, Family::Vru);
        for p in &m.card().parameters {
            if p.source.kind == SourceKind::TodoCalibrate {
                assert!(p.calibration.as_ref().is_some_and(|c| !c.trim().is_empty()));
            }
        }
    }
}
