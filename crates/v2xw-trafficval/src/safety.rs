//! Group 3: safety surrogates.
//!
//! The FHWA Surrogate Safety Assessment Model (SSAM; Gettman, Pu, Sayed & Shelby 2008,
//! FHWA-HRT-08-051) reads conflicts out of simulated trajectories with two measures, and so
//! does this module:
//!
//! * **time-to-collision**, `TTC = gap / (v_follower − v_leader)` for a follower closing on
//!   its leader; SSAM's default conflict threshold is TTC ≤ 1.5 s. With it the deceleration
//!   rate to avoid the crash, `DRAC = Δv² / 2·gap` (Archer 2005 reads 3.35 m/s² as the
//!   conflict threshold, secondary);
//! * **post-encroachment time** at the junctions' conflict zones (where two movements'
//!   paths cross): the time from one vehicle leaving the zone to a vehicle on the crossing
//!   path entering it; SSAM records PET up to 5 s, and PET ≤ 0 is two vehicles in the zone
//!   at once.
//!
//! What is *held*: no collision (no negative gap behind a leader), no unavoidable one
//! (DRAC above the follower's emergency braking), no PET ≤ 0 at a crossing. Real streets
//! have conflicts, and a model with none would be too clean; but no published conflict
//! rate was found that this simulated street could defensibly be held to — conflict
//! rates depend on volumes, geometry and the observers' threshold — so the rates are
//! **reported** beside SSAM's thresholds, per vehicle-kilometre and per 1,000 junction
//! entries, and the reason is stated rather than a band invented.

use std::collections::{BTreeMap, BTreeSet};

use v2xw_core::ids::{ActorId, LaneId};
use v2xw_mobility::audit::AuditActor;
use v2xw_mobility::calibration::Summary;
use v2xw_mobility::intersection::zones::ConflictZones;
use v2xw_world::World;

use crate::calib::{GridVariant, grid_study};
use crate::{Check, Group, Lab, Outcome, Row, Variant};

/// SSAM's default maximum TTC for a conflict, seconds.
pub const SSAM_TTC_S: f64 = 1.5;
/// SSAM's default maximum PET, seconds.
pub const SSAM_PET_S: f64 = 5.0;
/// DRAC conflict threshold, m/s² (Archer 2005, secondary).
pub const DRAC_CONFLICT_MPS2: f64 = 3.35;

type ZoneKey = (LaneId, LaneId, i64);

#[derive(Debug, Clone, Default)]
struct ZoneState {
    inside: [BTreeSet<ActorId>; 2],
    last_exit: [Option<f64>; 2],
}

/// The observer.
#[derive(Debug, Clone)]
pub struct SafetyObserver {
    zones: ConflictZones,
    lane_len: BTreeMap<LaneId, f64>,
    /// Active TTC conflicts: (follower, leader) → minimum TTC so far.
    active: BTreeMap<(ActorId, ActorId), f64>,
    conflict_min_ttc: Vec<f64>,
    closing_s: f64,
    under_ttc_s: f64,
    drac_over_s: f64,
    collisions: u64,
    unavoidable: u64,
    examples: Vec<String>,
    veh_m: f64,
    veh_s: f64,
    zone_state: BTreeMap<ZoneKey, ZoneState>,
    in_zone: BTreeMap<ActorId, BTreeSet<(ZoneKey, usize)>>,
    prev_lane: BTreeMap<ActorId, LaneId>,
    pet: Vec<f64>,
    simultaneous: u64,
    junction_entries: u64,
}

/// What it measured.
#[derive(Debug, Clone, Default)]
pub struct SafetyStats {
    /// Vehicle-kilometres driven.
    pub vehicle_km: f64,
    /// Vehicle-hours.
    pub vehicle_h: f64,
    /// TTC conflicts (episodes with TTC ≤ 1.5 s), each with its minimum TTC, seconds.
    pub conflict_min_ttc_s: Summary,
    /// Conflicts with a minimum TTC under 0.5 s.
    pub severe_conflicts: usize,
    /// Share of closing car-following time spent under 1.5 s TTC.
    pub share_closing_under_ttc: f64,
    /// Vehicle-seconds with DRAC over 3.35 m/s².
    pub drac_over_s: f64,
    /// Steps where a follower's gap behind its leader was negative.
    pub collisions: u64,
    /// Steps where the deceleration needed exceeded the follower's emergency braking.
    pub unavoidable: u64,
    /// PET at crossing conflict zones, seconds (≤ 5 s recorded).
    pub pet_s: Summary,
    /// PET under 1 s.
    pub pet_under_1s: usize,
    /// Entries into a zone already held by a crossing vehicle (PET ≤ 0).
    pub simultaneous: u64,
    /// Junction-connector entries.
    pub junction_entries: u64,
    /// A few examples of a collision or an unavoidable one.
    pub examples: Vec<String>,
}

impl SafetyObserver {
    /// An observer for `world`.
    pub fn new(world: &World) -> Self {
        Self {
            zones: ConflictZones::build(world),
            lane_len: world.roads.lanes().iter().map(|l| (l.id, l.length_m)).collect(),
            active: BTreeMap::new(),
            conflict_min_ttc: Vec::new(),
            closing_s: 0.0,
            under_ttc_s: 0.0,
            drac_over_s: 0.0,
            collisions: 0,
            unavoidable: 0,
            examples: Vec::new(),
            veh_m: 0.0,
            veh_s: 0.0,
            zone_state: BTreeMap::new(),
            in_zone: BTreeMap::new(),
            prev_lane: BTreeMap::new(),
            pet: Vec::new(),
            simultaneous: 0,
            junction_entries: 0,
        }
    }

    /// One step's vehicles, at `t_s`, `dt_s` after the last.
    pub fn observe(&mut self, world: &World, t_s: f64, dt_s: f64, actors: &[AuditActor]) {
        // Who is on each lane, by front position.
        let mut on: BTreeMap<LaneId, Vec<(f64, usize)>> = BTreeMap::new();
        for (i, a) in actors.iter().enumerate() {
            on.entry(a.lane).or_default().push((a.s_m, i));
            self.veh_m += a.speed_mps * dt_s;
            self.veh_s += dt_s;
        }
        for v in on.values_mut() {
            v.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        }
        let mut still = BTreeSet::new();
        for (i, f) in actors.iter().enumerate() {
            if f.changing.is_some() {
                continue;
            }
            let lane = &on[&f.lane];
            let pos = lane.iter().position(|(_, j)| *j == i).expect("indexed");
            let leader = if pos + 1 < lane.len() {
                let l = &actors[lane[pos + 1].1];
                Some((l, l.s_m - l.length_m - f.s_m))
            } else {
                f.route_next.and_then(|nl| {
                    on.get(&nl).and_then(|v| v.first()).map(|(_, j)| {
                        let l = &actors[*j];
                        let rest = self.lane_len.get(&f.lane).copied().unwrap_or(0.0) - f.s_m;
                        (l, rest + l.s_m - l.length_m)
                    })
                })
            };
            let Some((l, gap)) = leader else { continue };
            if l.changing.is_some() {
                continue;
            }
            if gap < -0.05 {
                self.collisions += 1;
                if self.examples.len() < 5 {
                    self.examples.push(format!(
                        "t {t_s:.1} s: actor {} {:.2} m into actor {} on lane {}",
                        f.actor.index(),
                        -gap,
                        l.actor.index(),
                        f.lane.index()
                    ));
                }
                continue;
            }
            let dv = f.speed_mps - l.speed_mps;
            if dv <= 0.01 {
                continue;
            }
            self.closing_s += dt_s;
            let gap = gap.max(1e-3);
            let ttc = gap / dv;
            let drac = dv * dv / (2.0 * gap);
            if drac > DRAC_CONFLICT_MPS2 {
                self.drac_over_s += dt_s;
            }
            if drac > f.class.spec().emergency_decel_mps2 {
                self.unavoidable += 1;
                if self.examples.len() < 5 {
                    self.examples.push(format!(
                        "t {t_s:.1} s: actor {} needs {drac:.1} m/s² to stop {gap:.2} m behind actor {} (closing {dv:.1} m/s)",
                        f.actor.index(),
                        l.actor.index()
                    ));
                }
            }
            if ttc <= SSAM_TTC_S {
                self.under_ttc_s += dt_s;
                let e = self.active.entry((f.actor, l.actor)).or_insert(f64::INFINITY);
                *e = e.min(ttc);
                still.insert((f.actor, l.actor));
            }
        }
        // Conflicts that ended this step.
        let ended: Vec<(ActorId, ActorId)> = self.active.keys().filter(|k| !still.contains(k)).copied().collect();
        for k in ended {
            if let Some(m) = self.active.remove(&k) {
                self.conflict_min_ttc.push(m);
            }
        }

        // Post-encroachment time at crossing zones.
        let mut present = BTreeSet::new();
        for a in actors {
            present.insert(a.actor);
            let internal = world.try_lane(a.lane).is_some_and(|l| l.junction.is_some());
            if internal && self.prev_lane.get(&a.actor) != Some(&a.lane) {
                self.junction_entries += 1;
            }
            self.prev_lane.insert(a.actor, a.lane);
            let mut now: BTreeSet<(ZoneKey, usize)> = BTreeSet::new();
            if internal {
                let rear = a.s_m - a.length_m;
                for z in self.zones.of(a.lane) {
                    if z.merge || !z.holds_self(rear, a.s_m) {
                        continue;
                    }
                    let (key, side) = if a.lane < z.other {
                        ((a.lane, z.other, (z.s_self * 2.0).round() as i64), 0)
                    } else {
                        ((z.other, a.lane, (z.s_other * 2.0).round() as i64), 1)
                    };
                    now.insert((key, side));
                }
            }
            let before = self.in_zone.remove(&a.actor).unwrap_or_default();
            for (key, side) in now.difference(&before) {
                let st = self.zone_state.entry(*key).or_default();
                let other = 1 - side;
                if !st.inside[other].is_empty() {
                    self.simultaneous += 1;
                } else if let Some(exit) = st.last_exit[other] {
                    let pet = t_s - exit;
                    if pet <= SSAM_PET_S {
                        self.pet.push(pet);
                    }
                }
                st.inside[*side].insert(a.actor);
            }
            for (key, side) in before.difference(&now) {
                let st = self.zone_state.entry(*key).or_default();
                st.inside[*side].remove(&a.actor);
                st.last_exit[*side] = Some(t_s);
            }
            if !now.is_empty() {
                self.in_zone.insert(a.actor, now);
            }
        }
        // Vehicles gone from the world leave their zones.
        let gone: Vec<ActorId> = self.in_zone.keys().filter(|k| !present.contains(k)).copied().collect();
        for g in gone {
            for (key, side) in self.in_zone.remove(&g).unwrap_or_default() {
                let st = self.zone_state.entry(key).or_default();
                st.inside[side].remove(&g);
                st.last_exit[side] = Some(t_s);
            }
        }
        self.prev_lane.retain(|k, _| present.contains(k));
    }

    /// The figures.
    pub fn stats(&self) -> SafetyStats {
        let mut ttc = self.conflict_min_ttc.clone();
        ttc.extend(self.active.values().copied());
        SafetyStats {
            vehicle_km: self.veh_m / 1000.0,
            vehicle_h: self.veh_s / 3600.0,
            severe_conflicts: ttc.iter().filter(|t| **t < 0.5).count(),
            conflict_min_ttc_s: Summary::of(&ttc),
            share_closing_under_ttc: if self.closing_s > 0.0 { self.under_ttc_s / self.closing_s } else { 0.0 },
            drac_over_s: self.drac_over_s,
            collisions: self.collisions,
            unavoidable: self.unavoidable,
            pet_under_1s: self.pet.iter().filter(|p| **p < 1.0).count(),
            pet_s: Summary::of(&self.pet),
            simultaneous: self.simultaneous,
            junction_entries: self.junction_entries,
            examples: self.examples.clone(),
        }
    }
}

/// The checks of this group.
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "safety/time-to-collision",
            group: Group::Safety,
            title: "No car hits, or cannot avoid hitting, the car ahead; TTC conflicts are measured as SSAM measures them",
            procedure: "The Midtown-shaped grid study (5 × 8 blocks of 274 × 80 m, 25 mph, \
                signals, crosswalks, 120 pedestrians, 0.8 veh/s for 300 s; see group 2) is \
                read for every follower closing on a leader in its lane or the next lane of \
                its route: TTC, DRAC, the conflict episodes with TTC ≤ 1.5 s and their \
                minimum TTC.",
            faults: &["one vehicle's published position pushed 12 m forward into the car ahead, every step from 60 s"],
            run: ttc_check,
        },
        Check {
            id: "safety/post-encroachment",
            group: Group::Safety,
            title: "No vehicle enters a crossing zone another vehicle still holds; PET is measured as SSAM measures it",
            procedure: "The same study: for every pair of crossing junction movements, the \
                zone their paths share (`ConflictZones`); a vehicle entering it while a \
                crossing vehicle is inside is PET ≤ 0, otherwise PET is the time since the \
                last crossing vehicle left, recorded up to SSAM's 5 s.",
            faults: &["junction control switched off (no signals, no yielding)"],
            run: pet_check,
        },
    ]
}

/// Teleports actor index 0's published front 12 m forward from step 600 on.
fn push_forward(step: u64, actors: &mut Vec<AuditActor>) {
    if step < 600 {
        return;
    }
    // The first vehicle that has someone ahead of it on its lane.
    let lanes: BTreeMap<LaneId, Vec<usize>> = actors.iter().enumerate().fold(BTreeMap::new(), |mut m, (i, a)| {
        m.entry(a.lane).or_default().push(i);
        m
    });
    for idx in lanes.values() {
        if idx.len() >= 2 {
            let mut v: Vec<usize> = idx.clone();
            v.sort_by(|a, b| actors[*a].s_m.total_cmp(&actors[*b].s_m));
            let f = v[0];
            let l = v[1];
            actors[f].s_m = actors[l].s_m - actors[l].length_m + 0.5;
            return;
        }
    }
}

fn ttc_check(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let gv = if variant.is_fault() { GridVariant::Tampered } else { GridVariant::Shipped };
    let s = grid_study(lab, gv, Some(push_forward))?.safety;
    let per_km = |n: f64| if s.vehicle_km > 0.0 { n / s.vehicle_km } else { f64::NAN };
    let mut notes = vec![format!(
        "{:.1} vehicle-km and {:.1} vehicle-hours observed; {} TTC conflicts.",
        s.vehicle_km,
        s.vehicle_h,
        s.conflict_min_ttc_s.n
    )];
    notes.extend(s.examples.iter().cloned());
    Ok(Outcome {
        rows: vec![
            Row::zero("vehicle-steps with a negative gap behind the leader (collision)", s.collisions, "no collision"),
            Row::zero(
                "vehicle-steps needing more than the class's emergency braking (unavoidable)",
                s.unavoidable,
                "DRAC above emergency deceleration: a crash no driver could prevent",
            ),
            Row::reported(
                "TTC ≤ 1.5 s conflicts per vehicle-km",
                per_km(s.conflict_min_ttc_s.n as f64),
                "SSAM default threshold (Gettman et al. 2008); no field rate this street could be held to was found",
            )
            .n(s.conflict_min_ttc_s.n),
            Row::reported(
                "severe conflicts (TTC < 0.5 s) per vehicle-km",
                per_km(s.severe_conflicts as f64),
                "SSAM; reported",
            ),
            Row::reported(
                "share of closing car-following time under TTC 1.5 s",
                s.share_closing_under_ttc,
                "reported",
            ),
            Row::reported("conflict minimum TTC, median, s", s.conflict_min_ttc_s.p50, "SSAM: ≤ 1.5 s by definition"),
            Row::reported(
                "vehicle-seconds with DRAC > 3.35 m/s² per vehicle-hour",
                if s.vehicle_h > 0.0 { s.drac_over_s / s.vehicle_h } else { f64::NAN },
                "Archer 2005 DRAC conflict threshold (secondary); reported",
            ),
        ],
        notes,
    })
}

fn pet_check(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let gv = if variant.is_fault() { GridVariant::NoJunctionControl } else { GridVariant::Shipped };
    let s = grid_study(lab, gv, None)?.safety;
    let per_k = |n: f64| if s.junction_entries > 0 { 1000.0 * n / s.junction_entries as f64 } else { f64::NAN };
    Ok(Outcome {
        rows: vec![
            Row::zero(
                "entries into a crossing zone a crossing vehicle still holds (PET ≤ 0)",
                s.simultaneous,
                "UVC §11-202(a)1 / NY VTL §1111(a)1: yield to vehicles lawfully within the intersection",
            ),
            Row::reported(
                "PET ≤ 5 s events per 1,000 junction entries",
                per_k(s.pet_s.n as f64),
                "SSAM maximum PET 5 s; reported",
            )
            .n(s.pet_s.n),
            Row::reported("PET < 1 s per 1,000 junction entries", per_k(s.pet_under_1s as f64), "Allen et al. 1978 read PET < 1 s as serious (secondary); reported"),
            Row::reported("PET, 15th percentile, s", s.pet_s.p15, "reported"),
        ],
        notes: vec![format!("{} junction-connector entries.", s.junction_entries)],
    })
}
