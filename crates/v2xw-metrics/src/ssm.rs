//! Full surrogate safety measures (adoption plan WP-11): time to collision for every
//! encounter type by trajectory extrapolation, DRAC, MDRAC with a reaction time, predicted
//! post-encroachment time, encounter-type classification, and a conflict log with begin and
//! end.
//!
//! Re-implemented from the papers and the SUMO documentation below. SUMO's SSM device
//! (`MSDevice_SSM`) was read for behaviour only and no code was copied; its documented
//! defaults are the thresholds here, and its documented encounter-type codes are reported
//! beside each type.
//!
//! # What is computed, per pair and step
//!
//! Two kinds of pair are measured:
//!
//! * **Following pairs the caller names** ([`Follow`]): a follower and its leader on one
//!   lane with the gap the caller measures. `TTC = gap / Δv`, `DRAC = Δv² / (2·gap)` while
//!   closing, exactly as the same-lane measures before this module, so the following
//!   results are those measures unchanged.
//! * **Every other pair within `range_m`** (50 m), not sharing a lane, by **trajectory
//!   extrapolation**: each body is an oriented rectangle moving at its current velocity and
//!   heading, and the TTC is the first instant the two rectangles overlap (Laureshyn,
//!   Svensson and Hydén 2010, "Evaluation of traffic safety, based on micro-level
//!   behavioural data: theoretical framework and first implementation", *Accident Analysis
//!   and Prevention* 42(6), 1637-1646: the "extended" TTC, defined whenever continuing at
//!   current speeds and directions ends in a collision). The overlap interval is exact for
//!   two rectangles of fixed orientation: by the separating-axis theorem they overlap iff
//!   their projections overlap on all four edge normals, and each projection gap is linear
//!   in time, so each axis gives an interval and the intersection is the collision interval.
//!
//! The encounter type is read from the conflict angle, as SSAM reads it (Gettman, Pu, Sayed
//! and Shelby 2008, *Surrogate Safety Assessment Model and Validation: Final Report*,
//! FHWA-HRT-08-051, §2: rear-end below 30°, crossing above 85°, lane-change between):
//!
//! | angle | type | SUMO code |
//! |---|---|---|
//! | < 30°, bodies overlapping laterally now | following | 1 |
//! | < 30°, side by side; 30°-85° | merging (SSAM "lane change") | 5 |
//! | 85°-150° | crossing | 9 |
//! | ≥ 150° | oncoming | 20 |
//! | bodies overlap now | collision | 111 |
//!
//! The 150° line between crossing and oncoming is this module's: SSAM has no head-on class
//! (it reads every angle above 85° as crossing) and SUMO classifies oncoming traffic by lane
//! topology, which this module does not read.
//!
//! For crossing and merging pairs (30°-150°) the **conflict area** is the parallelogram the
//! two body-width strips share, and each vehicle's occupation of it at current speed is
//! `[t_in, t_out]` (front reaches it, rear leaves it). Then (SUMO SSM device documentation,
//! "Measures"; Gettman et al. 2008 for PET):
//!
//! * **DRAC** for a crossing pair with a TTC: the deceleration that delays the later
//!   vehicle's arrival until the earlier one has left, `2·(v·T − d)/T²` with `d` the later
//!   vehicle's distance to the area and `T` the earlier one's exit time, or `v²/(2·d)` when
//!   that deceleration would stop it short of the area first;
//! * **PPET**, the predicted post-encroachment time, when no collision is predicted:
//!   `t_in(second) − t_out(first)`.
//!
//! **MDRAC** (Fazekas, Hennecke, Kalló and Horváth 2017, "A novel surrogate indicator based
//! on constant initial acceleration and reaction time assumption", *J. Advanced
//! Transportation* 2017, 8376572) is the DRAC after a perception-reaction time `R` at
//! constant speed: `Δv / (2·(TTC − R))` for following, merging and oncoming pairs; for a
//! crossing pair, the DRAC rule above with `d − v·R` and `T − R`. Undefined when the
//! collision falls inside the reaction time.
//!
//! # Encounters and the conflict log
//!
//! A pair's **encounter** begins at the first step a measure is defined for it and ends
//! once no measure has been defined for `extra_time_s` (5 s, SUMO's
//! `device.ssm.extratime`). Closed, it is logged as a [`Conflict`] when any extreme passed
//! its threshold: min TTC < 3.0 s, max DRAC > 3.0 m/s², max MDRAC > 3.4 m/s², min PPET <
//! 2.0 s (SUMO SSM device defaults), or the bodies overlapped.
//!
//! # Determinism
//!
//! Actors are sorted by id, the neighbour grid is a `BTreeMap` of integer cells, pairs are
//! visited in a fixed order, every transcendental goes through `v2xw_core::math`, and every
//! threshold comparison is made on the quantised value (build decision D10).

use std::collections::BTreeMap;

use v2xw_core::ids::ActorId;
use v2xw_core::math;

use crate::quant::Quantum;

/// SUMO SSM device default TTC threshold, s (`device.ssm.thresholds`, TTC 3.0).
pub const SSM_TTC_S: f64 = 3.0;
/// SUMO SSM device default DRAC threshold, m/s² (DRAC 3.0).
pub const SSM_DRAC_MPS2: f64 = 3.0;
/// SUMO SSM device default MDRAC threshold, m/s² (MDRAC 3.4; Fazekas et al. 2017).
pub const SSM_MDRAC_MPS2: f64 = 3.4;
/// SUMO SSM device default PPET threshold, s (PPET 2.0).
pub const SSM_PPET_S: f64 = 2.0;
/// SUMO SSM device default range, m (`device.ssm.range` 50).
pub const SSM_RANGE_M: f64 = 50.0;
/// SUMO SSM device default extra time, s (`device.ssm.extratime` 5).
pub const SSM_EXTRA_TIME_S: f64 = 5.0;
/// SUMO SSM device default MDRAC perception-reaction time, s (`device.ssm.mdrac.prt` 1).
pub const SSM_REACTION_S: f64 = 1.0;
/// SSAM's rear-end / lane-change boundary on the conflict angle, degrees (FHWA-HRT-08-051).
pub const SSAM_REAR_END_DEG: f64 = 30.0;
/// SSAM's lane-change / crossing boundary on the conflict angle, degrees.
pub const SSAM_CROSSING_DEG: f64 = 85.0;
/// This module's crossing / oncoming boundary, degrees (see the module documentation).
pub const ONCOMING_DEG: f64 = 150.0;
/// The yaw rate above which a vehicle is extrapolated on an arc, rad/s: 0.02 rad/s is a
/// 500 m radius at 10 m/s, straighter than any urban street's curve, so a car driving
/// straight stays on a straight line. The arc (constant turn rate and velocity) is the
/// motion model of Ward, Agamennoni, Worrall, Bender and Nebot 2015, "Extending time to
/// collision for probabilistic reasoning in general traffic scenarios", *Transportation
/// Research Part C* 51, 66-82; straight-line extrapolation sends a turning car across the
/// cross street's stop line, where SUMO's route-based extrapolation does not.
pub const SSM_MIN_YAW_RPS: f64 = 0.02;
/// How far ahead a turning pair is swept, s: beyond the 3.0 s TTC threshold with margin,
/// short enough that an arc does not wrap round a full circle at junction turn rates.
pub const SSM_TURNING_HORIZON_S: f64 = 5.0;
/// The acceleration above which a vehicle's speed is extrapolated as changing, m/s²: a car
/// braking for a red light is predicted to stop at it, as Fazekas et al. 2017 assume a
/// constant initial acceleration (SUMO's device is understood to estimate its conflict
/// entry times with the current acceleration too; to be confirmed against its cached
/// documentation). 0.1 m/s² is below any
/// deliberate speed change and above step-to-step noise.
pub const SSM_MIN_ACC_MPS2: f64 = 0.1;
/// Below this speed a vehicle is standing (SUMO's halting threshold, 0.1 m/s).
const STANDING_MPS: f64 = 0.1;

/// The encounter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EncounterType {
    /// One behind the other, same direction.
    Following,
    /// Converging at a small angle: a merge or a lane change.
    Merging,
    /// Paths crossing.
    Crossing,
    /// Driving towards each other.
    Oncoming,
    /// The bodies overlap.
    Collision,
}

impl EncounterType {
    /// Every type, in report order.
    pub const ALL: [EncounterType; 5] = [
        EncounterType::Following,
        EncounterType::Merging,
        EncounterType::Crossing,
        EncounterType::Oncoming,
        EncounterType::Collision,
    ];

    /// The label used in metric names and reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            EncounterType::Following => "following",
            EncounterType::Merging => "merging",
            EncounterType::Crossing => "crossing",
            EncounterType::Oncoming => "oncoming",
            EncounterType::Collision => "collision",
        }
    }

    /// SUMO's documented encounter-type code for the main type (SUMO documentation,
    /// "Simulation/Output/SSM Device", encounter types).
    #[must_use]
    pub const fn sumo_code(self) -> u32 {
        match self {
            EncounterType::Following => 1,
            EncounterType::Merging => 5,
            EncounterType::Crossing => 9,
            EncounterType::Oncoming => 20,
            EncounterType::Collision => 111,
        }
    }

    /// The type a SUMO encounter-type code belongs to, for reading SUMO's output: 1-3
    /// following, 4 (adjacent lanes) none, 5-8 merging, 9-11 crossing, 12-17 the conflict
    /// area phases of a crossing or merging encounter (crossing), 18 following passed, 19
    /// merging passed, 20 oncoming, 111 collision.
    #[must_use]
    pub const fn from_sumo_code(code: u32) -> Option<Self> {
        match code {
            1..=3 | 18 => Some(EncounterType::Following),
            5..=8 | 19 => Some(EncounterType::Merging),
            9..=17 => Some(EncounterType::Crossing),
            20 => Some(EncounterType::Oncoming),
            111 => Some(EncounterType::Collision),
            _ => None,
        }
    }
}

/// The thresholds and geometry of the measures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SsmParams {
    /// TTC conflict threshold, s.
    pub ttc_s: f64,
    /// DRAC conflict threshold, m/s².
    pub drac_mps2: f64,
    /// MDRAC conflict threshold, m/s².
    pub mdrac_mps2: f64,
    /// PPET conflict threshold, s.
    pub ppet_s: f64,
    /// Pairs farther apart than this are not measured, m.
    pub range_m: f64,
    /// An encounter ends this long after its last defined measure, s.
    pub extra_time_s: f64,
    /// The MDRAC perception-reaction time, s.
    pub reaction_s: f64,
    /// Trajectory extrapolation on (the shipped setting). Off, a pair that is not a named
    /// following pair is measured only for overlap now: the fault the validation injects
    /// to show the crossing case depends on extrapolation.
    pub extrapolate: bool,
    /// Above this yaw rate a vehicle is extrapolated on a constant-turn-rate arc rather than
    /// a straight line, rad/s; infinite turns it off (straight lines only).
    pub min_yaw_rps: f64,
    /// How far ahead a turning pair is swept, s.
    pub turning_horizon_s: f64,
    /// Above this acceleration or braking a vehicle's speed is extrapolated as changing,
    /// m/s²; infinite holds every speed constant.
    pub min_acc_mps2: f64,
}

impl Default for SsmParams {
    fn default() -> Self {
        Self {
            ttc_s: SSM_TTC_S,
            drac_mps2: SSM_DRAC_MPS2,
            mdrac_mps2: SSM_MDRAC_MPS2,
            ppet_s: SSM_PPET_S,
            range_m: SSM_RANGE_M,
            extra_time_s: SSM_EXTRA_TIME_S,
            reaction_s: SSM_REACTION_S,
            extrapolate: true,
            min_yaw_rps: SSM_MIN_YAW_RPS,
            turning_horizon_s: SSM_TURNING_HORIZON_S,
            min_acc_mps2: SSM_MIN_ACC_MPS2,
        }
    }
}

/// One road user's state for the measures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SsmActor {
    /// Which actor.
    pub actor: ActorId,
    /// East, m: the published reference point, which is the **rear bumper** centre
    /// (`v2xw-mobility` engine.rs, and the SUMO plug-in's conversion).
    pub x_m: f64,
    /// North, m.
    pub y_m: f64,
    /// Heading, radians, ENU, 0 = east, counter-clockwise.
    pub heading_rad: f64,
    /// Speed along the heading, m/s (never negative here).
    pub speed_mps: f64,
    /// Body length, m.
    pub length_m: f64,
    /// Body width, m.
    pub width_m: f64,
    /// The lane, when known: two actors on one lane are measured only as a named
    /// [`Follow`] pair, as the same-lane measures always were.
    pub lane: Option<u32>,
}

/// A following pair the caller found on one lane, with its gap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Follow {
    /// The follower.
    pub follower: ActorId,
    /// Its leader.
    pub leader: ActorId,
    /// The gap as the caller measures it, m.
    pub gap_m: f64,
}

/// One pair's measures at one step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairEval {
    /// The encounter type.
    pub ty: EncounterType,
    /// Time to collision, s.
    pub ttc_s: Option<f64>,
    /// Deceleration rate to avoid the crash, m/s².
    pub drac_mps2: Option<f64>,
    /// Modified DRAC (with reaction time), m/s².
    pub mdrac_mps2: Option<f64>,
    /// Predicted post-encroachment time, s.
    pub ppet_s: Option<f64>,
}

impl PairEval {
    fn any(&self) -> bool {
        self.ttc_s.is_some() || self.drac_mps2.is_some() || self.ppet_s.is_some()
    }
}

/// One closed encounter whose extremes passed a threshold: a line of the conflict log.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    /// The lower actor id of the pair.
    pub a: ActorId,
    /// The higher.
    pub b: ActorId,
    /// When the encounter began, s.
    pub begin_s: f64,
    /// When it ended (its last defined measure), s.
    pub end_s: f64,
    /// The type at the minimum TTC, else at the minimum PPET, else at the maximum DRAC.
    pub ty: EncounterType,
    /// (time, value) of the minimum TTC.
    pub min_ttc: Option<(f64, f64)>,
    /// (time, value) of the maximum DRAC.
    pub max_drac: Option<(f64, f64)>,
    /// (time, value) of the maximum MDRAC.
    pub max_mdrac: Option<(f64, f64)>,
    /// (time, value) of the minimum PPET.
    pub min_ppet: Option<(f64, f64)>,
}

impl Conflict {
    /// Whether the minimum TTC is below `threshold_s` (compared on the quantised value).
    #[must_use]
    pub fn ttc_below(&self, threshold_s: f64) -> bool {
        self.min_ttc.is_some_and(|(_, v)| below(v, threshold_s, Quantum::TIME_S))
    }
}

fn below(v: f64, thr: f64, q: Quantum) -> bool {
    q.grid(v) < q.grid(thr)
}

fn above(v: f64, thr: f64, q: Quantum) -> bool {
    q.grid(v) > q.grid(thr)
}

#[derive(Debug, Clone)]
struct Open {
    begin_s: f64,
    last_s: f64,
    ty: EncounterType,
    ty_rank: u8,
    min_ttc: Option<(f64, f64)>,
    max_drac: Option<(f64, f64)>,
    max_mdrac: Option<(f64, f64)>,
    min_ppet: Option<(f64, f64)>,
}

impl Open {
    fn update(&mut self, t: f64, e: &PairEval) {
        self.last_s = t;
        if let Some(v) = e.ttc_s
            && self.min_ttc.is_none_or(|(_, m)| v < m)
        {
            self.min_ttc = Some((t, v));
            self.ty = e.ty;
            self.ty_rank = 2;
        }
        if let Some(v) = e.ppet_s
            && self.min_ppet.is_none_or(|(_, m)| v < m)
        {
            self.min_ppet = Some((t, v));
            if self.ty_rank < 2 {
                self.ty = e.ty;
                self.ty_rank = 1;
            }
        }
        if let Some(v) = e.drac_mps2
            && self.max_drac.is_none_or(|(_, m)| v > m)
        {
            self.max_drac = Some((t, v));
        }
        if let Some(v) = e.mdrac_mps2
            && self.max_mdrac.is_none_or(|(_, m)| v > m)
        {
            self.max_mdrac = Some((t, v));
        }
        if e.ty == EncounterType::Collision {
            self.ty = EncounterType::Collision;
            self.ty_rank = 3;
        }
    }

    fn is_conflict(&self, p: &SsmParams) -> bool {
        self.ty == EncounterType::Collision
            || self.min_ttc.is_some_and(|(_, v)| below(v, p.ttc_s, Quantum::TIME_S))
            || self.max_drac.is_some_and(|(_, v)| above(v, p.drac_mps2, Quantum::SPEED))
            || self.max_mdrac.is_some_and(|(_, v)| above(v, p.mdrac_mps2, Quantum::SPEED))
            || self.min_ppet.is_some_and(|(_, v)| below(v, p.ppet_s, Quantum::TIME_S))
    }
}

/// The encounter tracker.
#[derive(Debug, Clone)]
pub struct SsmTracker {
    params: SsmParams,
    open: BTreeMap<(ActorId, ActorId), Open>,
    log: Vec<Conflict>,
    encounters: u64,
    /// Each actor's last (time, heading), for its yaw rate.
    last_heading: BTreeMap<ActorId, (f64, f64)>,
    /// Each actor's last (time, speed), for its acceleration.
    last_speed: BTreeMap<ActorId, (f64, f64)>,
}

impl SsmTracker {
    /// A tracker with `params`.
    #[must_use]
    pub fn new(params: SsmParams) -> Self {
        Self {
            params,
            open: BTreeMap::new(),
            log: Vec::new(),
            encounters: 0,
            last_heading: BTreeMap::new(),
            last_speed: BTreeMap::new(),
        }
    }

    /// The parameters.
    #[must_use]
    pub const fn params(&self) -> &SsmParams {
        &self.params
    }

    /// Encounters closed so far, conflicts or not.
    #[must_use]
    pub const fn encounters(&self) -> u64 {
        self.encounters
    }

    /// One step at `t_s`: `actors` (any order) and the caller's following pairs.
    pub fn step(&mut self, t_s: f64, actors: &[SsmActor], follows: &[Follow]) {
        let mut sorted: Vec<SsmActor> = actors.to_vec();
        sorted.sort_by_key(|a| a.actor);
        sorted.dedup_by_key(|a| a.actor);
        let index: BTreeMap<ActorId, usize> =
            sorted.iter().enumerate().map(|(i, a)| (a.actor, i)).collect();
        // Yaw rates from each actor's previous heading (within a second), wrapped to ±π.
        let yaw: Vec<f64> = sorted
            .iter()
            .map(|a| match self.last_heading.get(&a.actor) {
                Some((tp, hp)) if t_s > *tp && t_s - *tp <= 1.0 => {
                    let tau = core::f64::consts::TAU;
                    let d = (a.heading_rad - hp + core::f64::consts::PI).rem_euclid(tau)
                        - core::f64::consts::PI;
                    d / (t_s - tp)
                }
                _ => 0.0,
            })
            .collect();
        // Accelerations from each actor's previous speed, the same way.
        let acc: Vec<f64> = sorted
            .iter()
            .map(|a| match self.last_speed.get(&a.actor) {
                Some((tp, vp)) if t_s > *tp && t_s - *tp <= 1.0 => {
                    (a.speed_mps - vp) / (t_s - tp)
                }
                _ => 0.0,
            })
            .collect();
        self.last_heading = sorted
            .iter()
            .map(|a| (a.actor, (t_s, a.heading_rad)))
            .collect();
        self.last_speed = sorted
            .iter()
            .map(|a| (a.actor, (t_s, a.speed_mps)))
            .collect();
        let mut named: BTreeMap<(ActorId, ActorId), PairEval> = BTreeMap::new();
        for f in follows {
            let (Some(&fi), Some(&li)) = (index.get(&f.follower), index.get(&f.leader)) else {
                continue;
            };
            let key = ordered(f.follower, f.leader);
            if let Some(e) = evaluate_following(&sorted[fi], &sorted[li], f.gap_m, &self.params)
            {
                named.insert(key, e);
            }
        }
        // The neighbour grid: cells of `range_m`, integer keys.
        let cell = self.params.range_m.max(1.0);
        let mut grid: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
        for (i, a) in sorted.iter().enumerate() {
            grid.entry(cell_of(a, cell)).or_default().push(i);
        }
        let r2 = self.params.range_m * self.params.range_m;
        let mut evals: Vec<((ActorId, ActorId), PairEval)> = Vec::new();
        for (i, a) in sorted.iter().enumerate() {
            let (cx, cy) = cell_of(a, cell);
            let mut near: Vec<usize> = Vec::new();
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(v) = grid.get(&(cx + dx, cy + dy)) {
                        near.extend(v.iter().copied().filter(|j| *j > i));
                    }
                }
            }
            near.sort_unstable();
            for j in near {
                let b = &sorted[j];
                let key = (a.actor, b.actor);
                if let Some(e) = named.remove(&key) {
                    evals.push((key, e));
                    continue;
                }
                if a.lane.is_some() && a.lane == b.lane {
                    continue;
                }
                let (dx, dy) = (b.x_m - a.x_m, b.y_m - a.y_m);
                if dx * dx + dy * dy > r2 {
                    continue;
                }
                if let Some(e) =
                    evaluate_pair_motion(a, b, (yaw[i], acc[i]), (yaw[j], acc[j]), &self.params)
                {
                    evals.push((key, e));
                }
            }
        }
        // Named pairs farther apart than one grid cell's neighbourhood are still measured.
        evals.extend(named);
        evals.sort_by_key(|(k, _)| *k);
        for (key, e) in evals {
            if !e.any() && e.ty != EncounterType::Collision {
                continue;
            }
            let o = self.open.entry(key).or_insert(Open {
                begin_s: t_s,
                last_s: t_s,
                ty: e.ty,
                ty_rank: 0,
                min_ttc: None,
                max_drac: None,
                max_mdrac: None,
                min_ppet: None,
            });
            o.update(t_s, &e);
        }
        self.close_idle(t_s);
    }

    /// Closes every encounter idle for longer than the extra time at `now_s`.
    pub fn close_idle(&mut self, now_s: f64) {
        let extra = self.params.extra_time_s;
        let done: Vec<(ActorId, ActorId)> = self
            .open
            .iter()
            .filter(|(_, o)| now_s - o.last_s > extra)
            .map(|(k, _)| *k)
            .collect();
        for k in done {
            self.close(k);
        }
    }

    /// Closes every open encounter (the end of a run).
    pub fn close_all(&mut self) {
        let keys: Vec<(ActorId, ActorId)> = self.open.keys().copied().collect();
        for k in keys {
            self.close(k);
        }
    }

    fn close(&mut self, k: (ActorId, ActorId)) {
        if let Some(o) = self.open.remove(&k) {
            self.encounters += 1;
            if o.is_conflict(&self.params) {
                self.log.push(Conflict {
                    a: k.0,
                    b: k.1,
                    begin_s: o.begin_s,
                    end_s: o.last_s,
                    ty: o.ty,
                    min_ttc: o.min_ttc,
                    max_drac: o.max_drac,
                    max_mdrac: o.max_mdrac,
                    min_ppet: o.min_ppet,
                });
            }
        }
    }

    /// The conflict log so far, taken: in closing order, ties by pair.
    pub fn take_conflicts(&mut self) -> Vec<Conflict> {
        core::mem::take(&mut self.log)
    }
}

fn ordered(a: ActorId, b: ActorId) -> (ActorId, ActorId) {
    if a <= b { (a, b) } else { (b, a) }
}

fn cell_of(a: &SsmActor, cell: f64) -> (i64, i64) {
    ((a.x_m / cell).floor() as i64, (a.y_m / cell).floor() as i64)
}

/// The measures of a named following pair: `gap / Δv` and `Δv² / (2·gap)` while closing.
#[must_use]
pub fn evaluate_following(
    follower: &SsmActor,
    leader: &SsmActor,
    gap_m: f64,
    p: &SsmParams,
) -> Option<PairEval> {
    if gap_m <= 0.0 {
        return None;
    }
    let closing = follower.speed_mps - leader.speed_mps;
    if closing <= 0.0 {
        return None;
    }
    let ttc = gap_m / closing;
    Some(PairEval {
        ty: EncounterType::Following,
        ttc_s: Some(ttc),
        drac_mps2: Some(closing * closing / (2.0 * gap_m)),
        mdrac_mps2: mdrac_closing(closing, ttc, p.reaction_s),
        ppet_s: None,
    })
}

fn mdrac_closing(closing: f64, ttc: f64, reaction_s: f64) -> Option<f64> {
    (ttc > reaction_s).then(|| closing / (2.0 * (ttc - reaction_s)))
}

#[derive(Debug, Clone, Copy)]
struct Body {
    u: (f64, f64),
    n: (f64, f64),
    c: (f64, f64),
    v: (f64, f64),
    half_l: f64,
    half_w: f64,
}

fn body(a: &SsmActor) -> Body {
    let (s, c) = math::sin_cos(a.heading_rad);
    let u = (c, s);
    let speed = a.speed_mps.max(0.0);
    Body {
        u,
        n: (-s, c),
        // The reference point is the rear bumper: the centre is half a length ahead.
        c: (a.x_m + c * a.length_m / 2.0, a.y_m + s * a.length_m / 2.0),
        v: (c * speed, s * speed),
        half_l: a.length_m / 2.0,
        half_w: a.width_m / 2.0,
    }
}

fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

/// The interval of `t ≥ 0` over which the two rectangles overlap, with the closing speed on
/// the axis that separates them longest; `None` if they never do. Without extrapolation,
/// only `t = 0`.
fn overlap_interval(a: &Body, b: &Body, extrapolate: bool) -> Option<(f64, f64, f64)> {
    let d = (b.c.0 - a.c.0, b.c.1 - a.c.1);
    let w = (b.v.0 - a.v.0, b.v.1 - a.v.1);
    let mut lo = f64::NEG_INFINITY;
    let mut hi = f64::INFINITY;
    let mut closing = 0.0;
    for n in [a.u, a.n, b.u, b.n] {
        let r = a.half_l * dot(a.u, n).abs()
            + a.half_w * dot(a.n, n).abs()
            + b.half_l * dot(b.u, n).abs()
            + b.half_w * dot(b.n, n).abs();
        let p0 = dot(d, n);
        let pv = if extrapolate { dot(w, n) } else { 0.0 };
        if pv.abs() < 1e-9 {
            if p0.abs() >= r {
                return None;
            }
            continue;
        }
        let t1 = (-r - p0) / pv;
        let t2 = (r - p0) / pv;
        let (t_in, t_out) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
        if t_in > lo {
            lo = t_in;
            closing = pv.abs();
        }
        hi = hi.min(t_out);
    }
    if hi < 0.0 || lo > hi {
        return None;
    }
    Some((lo.max(0.0), hi, closing))
}

/// One vehicle's occupation of the conflict area: `(d_in, d_out)` travelled distances of
/// its reference point when the front reaches the area and when the rear leaves it.
fn occupation(d_in: f64, d_out: f64, v: f64) -> Option<(f64, f64)> {
    if d_out < 0.0 {
        return None;
    }
    if v < STANDING_MPS {
        return if d_in <= 0.0 {
            Some((0.0, f64::INFINITY))
        } else {
            Some((f64::INFINITY, f64::INFINITY))
        };
    }
    Some((d_in.max(0.0) / v, d_out / v))
}

/// The deceleration that brings a vehicle `d` from the area at `v` there no earlier than
/// `t`, or stops it short when that comes first.
fn delay_decel(d: f64, v: f64, t: f64) -> Option<f64> {
    if d <= 0.0 {
        return None;
    }
    let stop = v * v / (2.0 * d);
    if !t.is_finite() || v * t > 2.0 * d {
        return Some(stop);
    }
    Some((2.0 * (v * t - d) / (t * t)).max(0.0))
}

/// The body of `a` after `t` seconds with yaw rate and acceleration `m = (w, acc)`: the
/// path's curvature `w / v` is held (constant turn rate at the current speed), the speed
/// changes at `acc` and a braking vehicle stays where it stops (constant initial
/// acceleration, Fazekas et al. 2017); a straight line at constant speed when both are 0.
fn body_at(a: &SsmActor, m: (f64, f64), t: f64) -> Body {
    let (w, acc) = m;
    let v = a.speed_mps.max(0.0);
    let tt = if acc < 0.0 { t.min(v / -acc) } else { t };
    let d = (v * tt + 0.5 * acc * tt * tt).max(0.0);
    let speed = (v + acc * tt).max(0.0);
    let k = if v > STANDING_MPS { w / v } else { 0.0 };
    let (x, y, h) = if k.abs() < 1e-9 {
        let (s, c) = math::sin_cos(a.heading_rad);
        (a.x_m + c * d, a.y_m + s * d, a.heading_rad)
    } else {
        let h = a.heading_rad + k * d;
        let (s0, c0) = math::sin_cos(a.heading_rad);
        let (s1, c1) = math::sin_cos(h);
        (a.x_m + (s1 - s0) / k, a.y_m - (c1 - c0) / k, h)
    };
    body(&SsmActor {
        x_m: x,
        y_m: y,
        heading_rad: h,
        speed_mps: speed,
        ..*a
    })
}

/// The first overlap of two bodies on their extrapolated paths within `horizon_s`, by a
/// 0.1 s sweep refined by bisection to 1 ms; with the bodies then.
fn curved_overlap(
    a: &SsmActor,
    b: &SsmActor,
    wa: (f64, f64),
    wb: (f64, f64),
    horizon_s: f64,
) -> Option<(f64, Body, Body)> {
    let hit = |t: f64| {
        let (x, y) = (body_at(a, wa, t), body_at(b, wb, t));
        overlap_interval(&x, &y, false).map(|_| (x, y))
    };
    let dt = 0.1;
    let steps = (horizon_s / dt).ceil() as u32;
    let mut lo = 0.0;
    for k in 0..=steps {
        let t = f64::from(k) * dt;
        if hit(t).is_some() {
            if k == 0 {
                let (x, y) = hit(0.0)?;
                return Some((0.0, x, y));
            }
            let mut hi = t;
            while hi - lo > 1e-3 {
                let mid = 0.5 * (lo + hi);
                if hit(mid).is_some() {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let (x, y) = hit(hi)?;
            return Some((hi, x, y));
        }
        lo = t;
    }
    None
}

/// The measures of a pair on constant-turn-rate paths: TTC from the curved sweep, the type
/// from the angle at the collision, DRAC and MDRAC from the closing speed then.
fn evaluate_turning(
    a: &SsmActor,
    b: &SsmActor,
    wa: (f64, f64),
    wb: (f64, f64),
    p: &SsmParams,
) -> Option<PairEval> {
    let (ttc, xa, xb) = curved_overlap(a, b, wa, wb, p.turning_horizon_s)?;
    if ttc <= 0.0 {
        return Some(PairEval {
            ty: EncounterType::Collision,
            ttc_s: Some(0.0),
            drac_mps2: None,
            mdrac_mps2: None,
            ppet_s: None,
        });
    }
    let cos_t = dot(xa.u, xb.u).clamp(-1.0, 1.0);
    let sin_t = cross(xa.u, xb.u);
    let angle_deg = math::atan2(sin_t.abs(), cos_t).to_degrees();
    let ty = if angle_deg >= ONCOMING_DEG {
        EncounterType::Oncoming
    } else if angle_deg >= SSAM_CROSSING_DEG {
        EncounterType::Crossing
    } else if angle_deg >= SSAM_REAR_END_DEG {
        EncounterType::Merging
    } else {
        let d = (xb.c.0 - xa.c.0, xb.c.1 - xa.c.1);
        let lat = dot(d, xa.n).abs();
        if lat < xa.half_w + xb.half_l * sin_t.abs() + xb.half_w * cos_t.abs() {
            EncounterType::Following
        } else {
            EncounterType::Merging
        }
    };
    let w = (xb.v.0 - xa.v.0, xb.v.1 - xa.v.1);
    let closing = math::hypot(w.0, w.1);
    Some(PairEval {
        ty,
        ttc_s: Some(ttc),
        drac_mps2: Some(closing / (2.0 * ttc)),
        mdrac_mps2: mdrac_closing(closing, ttc, p.reaction_s),
        ppet_s: None,
    })
}

/// As [`evaluate_pair`], with each vehicle's yaw rate, rad/s: a pair either of which is
/// turning faster than `min_yaw_rps` is extrapolated on constant-turn-rate arcs.
#[must_use]
pub fn evaluate_pair_turning(
    a: &SsmActor,
    b: &SsmActor,
    wa: f64,
    wb: f64,
    p: &SsmParams,
) -> Option<PairEval> {
    evaluate_pair_motion(a, b, (wa, 0.0), (wb, 0.0), p)
}

/// As [`evaluate_pair`], with each vehicle's `(yaw rate, acceleration)`: a pair either of
/// which turns faster than `min_yaw_rps` or accelerates or brakes harder than
/// `min_acc_mps2` is swept on its extrapolated path (arc at the current curvature,
/// constant acceleration, stopping where it stops).
#[must_use]
pub fn evaluate_pair_motion(
    a: &SsmActor,
    b: &SsmActor,
    ma: (f64, f64),
    mb: (f64, f64),
    p: &SsmParams,
) -> Option<PairEval> {
    let turn = |w: f64| {
        if p.min_yaw_rps.is_finite() && w.abs() > p.min_yaw_rps { w } else { 0.0 }
    };
    let accel = |x: f64| {
        if p.min_acc_mps2.is_finite() && x.abs() > p.min_acc_mps2 { x } else { 0.0 }
    };
    let ma = (turn(ma.0), accel(ma.1));
    let mb = (turn(mb.0), accel(mb.1));
    if p.extrapolate && (ma != (0.0, 0.0) || mb != (0.0, 0.0)) {
        return evaluate_turning(a, b, ma, mb, p);
    }
    evaluate_pair(a, b, p)
}

/// The measures of a pair that is not a named following pair, by trajectory extrapolation.
#[must_use]
pub fn evaluate_pair(a: &SsmActor, b: &SsmActor, p: &SsmParams) -> Option<PairEval> {
    let ba = body(a);
    let bb = body(b);
    let cos_t = dot(ba.u, bb.u).clamp(-1.0, 1.0);
    let sin_t = cross(ba.u, bb.u);
    let angle_deg = math::atan2(sin_t.abs(), cos_t).to_degrees();
    let hit = overlap_interval(&ba, &bb, p.extrapolate);
    if let Some((t, _, _)) = hit
        && t <= 0.0
    {
        return Some(PairEval {
            ty: EncounterType::Collision,
            ttc_s: Some(0.0),
            drac_mps2: None,
            mdrac_mps2: None,
            ppet_s: None,
        });
    }
    let ttc = hit.map(|(t, _, _)| t);
    let closing = hit.map_or(0.0, |(_, _, c)| c);
    if angle_deg < SSAM_REAR_END_DEG || angle_deg >= ONCOMING_DEG {
        let ttc = ttc?;
        let ty = if angle_deg >= ONCOMING_DEG {
            EncounterType::Oncoming
        } else {
            // Overlapping laterally now (in a's frame, with b's bounding extent): one is
            // behind the other.
            let d = (bb.c.0 - ba.c.0, bb.c.1 - ba.c.1);
            let lat = dot(d, ba.n).abs();
            let b_half = bb.half_l * sin_t.abs() + bb.half_w * cos_t.abs();
            if lat < ba.half_w + b_half {
                EncounterType::Following
            } else {
                EncounterType::Merging
            }
        };
        return Some(PairEval {
            ty,
            ttc_s: Some(ttc),
            drac_mps2: Some(closing / (2.0 * ttc)),
            mdrac_mps2: mdrac_closing(closing, ttc, p.reaction_s),
            ppet_s: None,
        });
    }
    let ty = if angle_deg < SSAM_CROSSING_DEG {
        EncounterType::Merging
    } else {
        EncounterType::Crossing
    };
    // The conflict area: where the two centre lines meet, widened by both bodies' widths.
    let dp = (b.x_m - a.x_m, b.y_m - a.y_m);
    let s_a = cross(dp, bb.u) / sin_t;
    let s_b = cross(dp, ba.u) / sin_t;
    let (sn, cs) = (sin_t.abs(), cos_t.abs());
    let e_a = (b.width_m + a.width_m * cs) / (2.0 * sn);
    let e_b = (a.width_m + b.width_m * cs) / (2.0 * sn);
    let (ina, outa) = (s_a - e_a - a.length_m, s_a + e_a);
    let (inb, outb) = (s_b - e_b - b.length_m, s_b + e_b);
    let occ = if ina <= p.range_m && inb <= p.range_m {
        occupation(ina, outa, a.speed_mps).zip(occupation(inb, outb, b.speed_mps))
    } else {
        None
    };
    match (ttc, occ) {
        (Some(ttc), occ) => {
            // The earlier arrival leads; the later one must wait for it to leave.
            let crossing = occ.and_then(|((ta, xa), (tb, xb))| {
                let (d, v, t_exit) = if ta <= tb {
                    (inb, b.speed_mps, xa)
                } else {
                    (ina, a.speed_mps, xb)
                };
                let drac = delay_decel(d, v, t_exit)?;
                let mdrac = delay_decel(d - v * p.reaction_s, v, t_exit - p.reaction_s)
                    .filter(|_| t_exit > p.reaction_s);
                Some((drac, mdrac))
            });
            let (drac, mdrac) = crossing.map_or_else(
                || (Some(closing / (2.0 * ttc)), mdrac_closing(closing, ttc, p.reaction_s)),
                |(d, m)| (Some(d), m),
            );
            Some(PairEval {
                ty,
                ttc_s: Some(ttc),
                drac_mps2: drac,
                mdrac_mps2: mdrac,
                ppet_s: None,
            })
        }
        (None, Some(((ta, xa), (tb, xb)))) => {
            let ppet = if ta <= tb { tb - xa } else { ta - xb };
            // A standing vehicle that never arrives, or never leaves, predicts nothing.
            if !ppet.is_finite() {
                return None;
            }
            Some(PairEval {
                ty,
                ttc_s: None,
                drac_mps2: None,
                mdrac_mps2: None,
                ppet_s: Some(ppet.max(0.0)),
            })
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::FRAC_PI_2;

    fn car(id: u32, x: f64, y: f64, heading: f64, v: f64) -> SsmActor {
        SsmActor {
            actor: ActorId::new(id),
            x_m: x,
            y_m: y,
            heading_rad: heading,
            speed_mps: v,
            length_m: 4.0,
            width_m: 2.0,
            lane: None,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Hand-computed crossing. Car A eastbound along y = 0 with its rear bumper at x = −30
    /// (front at −26), 10 m/s; car B northbound along x = 0 with its rear bumper at
    /// y = −20 (front at −16), 8 m/s; both 4 × 2 m.
    ///
    /// The overlap needs, on x: |(0 − (−28)) + (−10)t| < 2 + 1 → t ∈ (2.5, 3.1); on y:
    /// |(−18 − 0) + 8t| < 1 + 2 → t ∈ (1.875, 2.625). So TTC = 2.5 s (A's front reaches B's
    /// left side). The conflict area is the 2 × 2 m square at the origin: A occupies it
    /// over [(30 − 1 − 4)/10, (30 + 1)/10] = [2.5, 3.1] s, B over [(20 − 1 − 4)/8,
    /// (20 + 1)/8] = [1.875, 2.625] s. B leads; A must not arrive before 2.625 s from 25 m
    /// at 10 m/s: v·T = 26.25 > 2·25 is false, so DRAC = 2(26.25 − 25)/2.625² = 0.36281 m/s².
    /// MDRAC with R = 1: d' = 15, T' = 1.625, v·T' = 16.25 ≤ 30, so 2(16.25 − 15)/1.625² =
    /// 0.94675 m/s².
    #[test]
    fn a_hand_computed_crossing_case_matches() {
        let a = car(1, -30.0, 0.0, 0.0, 10.0);
        let b = car(2, 0.0, -20.0, FRAC_PI_2, 8.0);
        let e = evaluate_pair(&a, &b, &SsmParams::default()).expect("a crossing conflict");
        assert_eq!(e.ty, EncounterType::Crossing);
        assert!(close(e.ttc_s.unwrap(), 2.5), "{e:?}");
        assert!(close(e.drac_mps2.unwrap(), 2.0 * (26.25 - 25.0) / (2.625 * 2.625)), "{e:?}");
        assert!(close(e.mdrac_mps2.unwrap(), 2.0 * (16.25 - 15.0) / (1.625 * 1.625)), "{e:?}");
        assert_eq!(e.ppet_s, None);
        // Symmetric in the pair.
        let r = evaluate_pair(&b, &a, &SsmParams::default()).unwrap();
        assert_eq!(r.ty, e.ty);
        assert!(close(r.ttc_s.unwrap(), 2.5));
        assert!(close(r.drac_mps2.unwrap(), e.drac_mps2.unwrap()));
    }

    /// The validation's fault: without extrapolation the same crossing is missed.
    #[test]
    fn dropping_extrapolation_misses_the_crossing_case() {
        let a = car(1, -30.0, 0.0, 0.0, 10.0);
        let b = car(2, 0.0, -20.0, FRAC_PI_2, 8.0);
        let p = SsmParams {
            extrapolate: false,
            ..SsmParams::default()
        };
        let e = evaluate_pair(&a, &b, &p);
        assert!(e.is_none_or(|e| e.ttc_s.is_none()), "{e:?}");
    }

    /// Hand-computed PPET: B as above but 12 m further back (rear at y = −32), so it
    /// occupies the square over [(32 − 5)/8, 33/8] = [3.375, 4.125] s, after A has left at
    /// 3.1 s: PPET = 0.275 s and no TTC.
    #[test]
    fn a_crossing_that_misses_has_a_predicted_pet() {
        let a = car(1, -30.0, 0.0, 0.0, 10.0);
        let b = car(2, 0.0, -32.0, FRAC_PI_2, 8.0);
        let e = evaluate_pair(&a, &b, &SsmParams::default()).expect("a PPET");
        assert_eq!(e.ty, EncounterType::Crossing);
        assert_eq!(e.ttc_s, None);
        assert!(close(e.ppet_s.unwrap(), 27.0 / 8.0 - 3.1), "{e:?}");
    }

    /// Hand-computed merging at 45°: A eastbound along y = 0 from rear x = −20 at 10 m/s;
    /// B on a 45° approach whose centre line meets A's at the origin, rear bumper 20 m
    /// before it, at 10 m/s. Both 4 × 2 m. With θ = 45°: e = (2 + 2·cos45)/(2·sin45) =
    /// 1 + √2 = 2.41421 m for both; each enters the area after (20 − 2.41421 − 4)/10 =
    /// 1.35858 s and leaves after (20 + 2.41421)/10 = 2.24142 s. Identical intervals, so a
    /// collision is predicted and, the arrival being simultaneous, a leads: B must not
    /// arrive before 2.24142 s from 13.58579 m at 10 m/s: v·T = 22.4142 ≤ 27.1716, so DRAC =
    /// 2(22.4142 − 13.58579)/2.24142² = 3.51460 m/s². The TTC is the rectangles' own first
    /// overlap, checked against an independent polygon-intersection sweep at 0.1 ms.
    #[test]
    fn a_hand_computed_merging_case_matches() {
        let r = core::f64::consts::FRAC_1_SQRT_2;
        let a = car(1, -20.0, 0.0, 0.0, 10.0);
        let b = car(2, -20.0 * r, -20.0 * r, core::f64::consts::FRAC_PI_4, 10.0);
        let e = evaluate_pair(&a, &b, &SsmParams::default()).expect("a merging conflict");
        assert_eq!(e.ty, EncounterType::Merging);
        let ext = 1.0 + core::f64::consts::SQRT_2;
        let d = 20.0 - ext - 4.0;
        let t = (20.0 + ext) / 10.0;
        assert!(close(e.drac_mps2.unwrap(), 2.0 * (10.0 * t - d) / (t * t)), "{e:?}");
        // The first overlap by brute force, with a test that shares no code with the
        // separating-axis sweep: a corner inside the other body, or two edges crossing.
        let (ba, bb) = (body(&a), body(&b));
        let mut first = None;
        for k in 0..40_000 {
            let t = f64::from(k) * 1e-4;
            if polygons_overlap(&corners(&ba, t), &corners(&bb, t)) {
                first = Some(t);
                break;
            }
        }
        let first = first.expect("they meet");
        assert!((e.ttc_s.unwrap() - first).abs() < 2e-4, "{e:?} against {first}");
        // Not before either front reaches the conflict area (1.35858 s).
        assert!(e.ttc_s.unwrap() >= d / 10.0 - 1e-9, "{e:?}");
    }

    fn corners(b: &Body, t: f64) -> [(f64, f64); 4] {
        let c = (b.c.0 + b.v.0 * t, b.c.1 + b.v.1 * t);
        let p = |l: f64, w: f64| {
            (
                c.0 + b.u.0 * l * b.half_l + b.n.0 * w * b.half_w,
                c.1 + b.u.1 * l * b.half_l + b.n.1 * w * b.half_w,
            )
        };
        [p(1.0, 1.0), p(-1.0, 1.0), p(-1.0, -1.0), p(1.0, -1.0)]
    }

    fn inside(q: (f64, f64), poly: &[(f64, f64); 4]) -> bool {
        (0..4).all(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % 4]);
            cross((b.0 - a.0, b.1 - a.1), (q.0 - a.0, q.1 - a.1)) > 0.0
        })
    }

    fn segments_cross(p: (f64, f64), q: (f64, f64), r: (f64, f64), s: (f64, f64)) -> bool {
        let o = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
            cross((b.0 - a.0, b.1 - a.1), (c.0 - a.0, c.1 - a.1))
        };
        o(p, q, r) * o(p, q, s) < 0.0 && o(r, s, p) * o(r, s, q) < 0.0
    }

    fn polygons_overlap(a: &[(f64, f64); 4], b: &[(f64, f64); 4]) -> bool {
        a.iter().any(|q| inside(*q, b))
            || b.iter().any(|q| inside(*q, a))
            || (0..4).any(|i| {
                (0..4).any(|j| segments_cross(a[i], a[(i + 1) % 4], b[j], b[(j + 1) % 4]))
            })
    }

    /// Head-on: two cars on one line 40 m apart (front to front 32 m) closing at 20 m/s:
    /// TTC 1.6 s, DRAC = 20 / (2·1.6) = 6.25 m/s², MDRAC = 20 / (2·0.6).
    #[test]
    fn an_oncoming_pair_on_one_line_is_oncoming() {
        let a = car(1, 0.0, 0.0, 0.0, 10.0);
        let b = car(2, 40.0, 0.0, core::f64::consts::PI, 10.0);
        let e = evaluate_pair(&a, &b, &SsmParams::default()).expect("oncoming");
        assert_eq!(e.ty, EncounterType::Oncoming);
        assert!(close(e.ttc_s.unwrap(), 1.6), "{e:?}");
        assert!(close(e.drac_mps2.unwrap(), 6.25));
        assert!(close(e.mdrac_mps2.unwrap(), 20.0 / 1.2));
        // Passing in adjacent lanes 3.5 m apart: no conflict.
        let c = car(3, 40.0, 3.5, core::f64::consts::PI, 10.0);
        assert_eq!(evaluate_pair(&a, &c, &SsmParams::default()), None);
    }

    /// A rear-end across a lane boundary, found by geometry, has the same TTC as the
    /// bumper gap over the closing speed: rear bumper of the leader 10 m ahead of the
    /// follower's front, closing at 5 m/s, 2 s.
    #[test]
    fn a_geometric_rear_end_is_following_with_the_bumper_gap() {
        let f = car(1, 0.0, 0.0, 0.0, 15.0);
        let l = car(2, 14.0, 0.3, 0.0, 10.0);
        let e = evaluate_pair(&f, &l, &SsmParams::default()).expect("following");
        assert_eq!(e.ty, EncounterType::Following);
        assert!(close(e.ttc_s.unwrap(), 2.0), "{e:?}");
        assert!(close(e.drac_mps2.unwrap(), 25.0 / 20.0));
    }

    /// The named following pair is today's same-lane measure: gap / Δv and Δv² / 2·gap.
    #[test]
    fn a_named_following_pair_is_the_same_lane_measure() {
        let f = car(1, 0.0, 0.0, 0.0, 12.0);
        let l = car(2, 20.0, 0.0, 0.0, 8.0);
        let e = evaluate_following(&f, &l, 20.0, &SsmParams::default()).unwrap();
        assert_eq!(e.ttc_s, Some(20.0 / 4.0));
        assert_eq!(e.drac_mps2, Some(16.0 / 40.0));
        assert_eq!(e.mdrac_mps2, Some(4.0 / (2.0 * 4.0)));
        assert_eq!(evaluate_following(&l, &f, 20.0, &SsmParams::default()), None);
    }

    /// The conflict log: the crossing pair, stepped at 10 Hz until it has passed, makes one
    /// crossing conflict with its begin, its end and the minimum TTC at the first step;
    /// the encounter is closed only after the extra time.
    #[test]
    fn the_conflict_log_has_one_crossing_conflict_with_begin_and_end() {
        let mut tr = SsmTracker::new(SsmParams::default());
        for k in 0..=200 {
            let t = f64::from(k) * 0.1;
            // Both brake hard to a stop just short of the conflict area after 1 s, so the
            // collision never happens.
            let (ax, by) = if t < 1.0 {
                (-30.0 + 10.0 * t, -20.0 + 8.0 * t)
            } else {
                (-20.0, -12.0)
            };
            let v = if t < 1.0 { (10.0, 8.0) } else { (0.0, 0.0) };
            let a = car(1, ax, 0.0, 0.0, v.0);
            let b = car(2, 0.0, by, FRAC_PI_2, v.1);
            tr.step(t, &[b, a], &[]);
            if k == 30 {
                assert!(tr.take_conflicts().is_empty(), "closed only after the extra time");
            }
        }
        let log = tr.take_conflicts();
        assert_eq!(log.len(), 1, "{log:?}");
        let c = &log[0];
        assert_eq!(c.ty, EncounterType::Crossing);
        assert_eq!((c.a, c.b), (ActorId::new(1), ActorId::new(2)));
        assert!(close(c.begin_s, 0.0));
        let (t_min, v_min) = c.min_ttc.unwrap();
        assert!(c.ttc_below(SSM_TTC_S));
        assert!(t_min < 1.0 && v_min <= 2.5 + 1e-9, "{c:?}");
        assert!(c.end_s >= 0.9 && c.end_s < 1.0 + 1e-9, "{c:?}");
        assert_eq!(tr.encounters(), 1);
    }

    /// Two same-lane actors are measured only through a named following pair.
    #[test]
    fn same_lane_actors_are_left_to_the_named_pairs() {
        let mut f = car(1, 0.0, 0.0, 0.0, 15.0);
        let mut l = car(2, 14.0, 0.0, 0.0, 10.0);
        f.lane = Some(7);
        l.lane = Some(7);
        let mut tr = SsmTracker::new(SsmParams::default());
        tr.step(0.0, &[f, l], &[]);
        tr.close_all();
        assert!(tr.take_conflicts().is_empty());
        assert_eq!(tr.encounters(), 0);
        tr.step(
            1.0,
            &[f, l],
            &[Follow {
                follower: f.actor,
                leader: l.actor,
                gap_m: 14.0,
            }],
        );
        tr.close_all();
        let log = tr.take_conflicts();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].min_ttc, Some((1.0, 14.0 / 5.0)));
    }

    /// Determinism: the same actors in any order give the identical log.
    #[test]
    fn the_log_does_not_depend_on_the_order_of_the_actors() {
        let run = |rev: bool| {
            let mut tr = SsmTracker::new(SsmParams::default());
            for k in 0..30 {
                let t = f64::from(k) * 0.1;
                let mut v = vec![
                    car(1, -30.0 + 10.0 * t, 0.0, 0.0, 10.0),
                    car(2, 0.0, -20.0 + 8.0 * t, FRAC_PI_2, 8.0),
                    car(3, 40.0 - 10.0 * t, 0.2, core::f64::consts::PI, 10.0),
                    car(4, -50.0 + 12.0 * t, 0.1, 0.0, 12.0),
                ];
                if rev {
                    v.reverse();
                }
                tr.step(t, &v, &[]);
            }
            tr.close_all();
            format!("{:?}", tr.take_conflicts())
        };
        assert_eq!(run(false), run(true));
    }

    /// A car turning left on a 10 m radius (8 m/s, 0.8 rad/s) past a car standing 12 m
    /// ahead across its straight-line path: the straight line predicts a crossing conflict
    /// at (11.1 − 4.5) / 8 = 0.825 s; on the arc the turning car's outer front corner stays
    /// within 11.79 m of the turn centre (0, 10), so it never reaches the standing car's
    /// side at x = 11.1 while y is within ±2.25 m: no conflict. Straight again (yaw rate
    /// zero), the arc case is the straight one.
    #[test]
    fn a_turning_car_is_extrapolated_on_its_arc() {
        let mut a = car(1, 0.0, 0.0, 0.0, 8.0);
        a.length_m = 4.5;
        a.width_m = 1.8;
        let mut b = car(2, 12.0, -2.25, FRAC_PI_2, 0.0);
        b.length_m = 4.5;
        b.width_m = 1.8;
        let p = SsmParams::default();
        let straight = evaluate_pair_turning(&a, &b, 0.0, 0.0, &p).expect("straight hit");
        assert_eq!(straight.ty, EncounterType::Crossing);
        assert!(close(straight.ttc_s.unwrap(), 0.825), "{straight:?}");
        assert_eq!(evaluate_pair_turning(&a, &b, 0.8, 0.0, &p), None);
        // On the arc it does hit a car standing on the arc: at (10, 10) facing east, which
        // the reference point reaches after a quarter turn (π/2 / 0.8 = 1.96 s).
        let c = car(3, 9.0, 10.0, 0.0, 0.0);
        let e = evaluate_pair_turning(&a, &c, 0.8, 0.0, &p).expect("arc hit");
        assert!(e.ttc_s.unwrap() < 1.96 && e.ttc_s.unwrap() > 0.5, "{e:?}");
        // The tracker derives the yaw rate from the previous heading.
        let mut tr = SsmTracker::new(p);
        for k in 0..12 {
            let t = f64::from(k) * 0.1;
            let h = 0.8 * t;
            let ak = SsmActor {
                x_m: 10.0 * math::sin(h),
                y_m: 10.0 - 10.0 * math::cos(h),
                heading_rad: h,
                ..a
            };
            tr.step(t, &[ak, b], &[]);
        }
        tr.close_all();
        let log = tr.take_conflicts();
        assert!(log.iter().all(|c| c.min_ttc.is_none_or(|(t, _)| t < 0.05)), "{log:?}");
    }

    /// The hand-computed crossing with the northbound car braking at 8 m/s²: it stops after
    /// 1 s and 4 m, its front at y = −12, short of A's path at y = −1: no conflict. At
    /// constant speed it is the 2.5 s crossing.
    #[test]
    fn a_car_braking_short_of_the_crossing_is_no_conflict() {
        let a = car(1, -30.0, 0.0, 0.0, 10.0);
        let b = car(2, 0.0, -20.0, FRAC_PI_2, 8.0);
        let p = SsmParams::default();
        assert_eq!(evaluate_pair_motion(&a, &b, (0.0, 0.0), (0.0, -8.0), &p), None);
        let e = evaluate_pair_motion(&a, &b, (0.0, 0.0), (0.0, 0.0), &p).unwrap();
        assert!(close(e.ttc_s.unwrap(), 2.5));
    }

    #[test]
    fn sumo_codes_round_trip() {
        for ty in EncounterType::ALL {
            assert_eq!(EncounterType::from_sumo_code(ty.sumo_code()), Some(ty));
        }
        assert_eq!(EncounterType::from_sumo_code(4), None);
        assert_eq!(EncounterType::from_sumo_code(0), None);
    }
}
