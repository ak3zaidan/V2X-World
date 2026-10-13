//! `v2xw check-traffic <run dir>` — the traffic invariants of a finished run, read from its
//! recording rather than from inside the engine.
//!
//! The in-engine auditor (`v2xw_mobility::audit`) sees the drivers' own state; this check
//! sees what the owner sees: the recorded poses, lanes and signal lamps the Studio replays.
//! A vehicle the auditor thinks is lawfully held at a red but whose lamp the recording shows
//! green is a defect only this view catches.
//!
//! The run directory must hold `recording.mcap` (a run with `--record-level full`, the
//! default) and `scenario.resolved.yaml`, from which the world is rebuilt to map lanes to
//! junctions and signal movements.
//!
//! # What is checked
//!
//! | row | fails when |
//! |---|---|
//! | `stopped_on_green` | a queue head stands at its stop line longer than [`CheckOptions::green_stop_s`] while its own movement shows a protected green |
//! | `stopped_on_permitted_green` | as above for a permitted (yield) green, over [`CheckOptions::permitted_stop_s`] |
//! | `stopped_at_clear_junction` | a queue head at an unsignalised junction stands longer than [`CheckOptions::clear_stop_s`] while nothing is in or approaching the junction |
//! | `phantom_stop` | a vehicle stands mid-lane, nobody ahead within 30 m, longer than [`CheckOptions::phantom_stop_s`] |
//! | `long_stop` | any vehicle stands longer than one cycle of its junction's plan plus [`CheckOptions::cycle_margin_s`] (120 s off a signal) |
//! | `red_entry` | a vehicle enters a junction while its movement's recorded lamp is red |
//! | `lamp_phase_unknown` | the recorded lamps of a plan match none of its phases |
//! | `overlap` | two vehicles on one lane closer than the shorter body, or any two reference points within 1 m |
//! | `jump` | a step longer than the speed explains (gridlock teleports excused by count) |
//! | `decel` / `accel` | a speed change over 9 m/s² braking or 6 m/s² accelerating |
//! | `jerk` | rate of steps with a jerk over 30 m/s³, under one in a thousand |
//! | `speeding` | a moving vehicle over 1.25 × its lane's limit + 2 m/s |
//! | `ped_jump` / `ped_speed_jump` / `ped_heading_flip` | a pedestrian's step that no walker makes |
//!
//! Plus measurements with no pass mark: stop-line saturation headway and start-up lost time
//! (HCM 7th ed. ch. 19: base 1,900 pc/h/ln is a 1.89 s headway; start-up lost time 2 s),
//! the longest stop, mean speeds per road class, gridlock teleports.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use v2xw_core::ids::LaneId;
use v2xw_mobility::VehicleClass;
use v2xw_record::reader::Reader;
use v2xw_record::wire::U32_NONE;
use v2xw_record::wire::snapshot::{DeltaBody, KeyframeBody};
use v2xw_record::writer::{TOPIC_DELTA, TOPIC_KEYFRAME};
use v2xw_wasm::scene::Scene;
use v2xw_world::model::{LaneKind, TurnDirection};
use v2xw_world::{SignalState, World};

use crate::{CliError, Result};

/// What `v2xw check-traffic` takes.
#[derive(Debug, Clone)]
pub struct CheckOptions {
    /// The run directory.
    pub dir: PathBuf,
    /// Seconds a queue head may stand on its own protected green: 1 s of perception-
    /// reaction (AASHTO 2018 §3.2.2 uses 2.5 s for an unexpected event; a queue head
    /// watching the lamp reacts in about 1 s, HCM start-up lost time 2 s) with margin.
    pub green_stop_s: f64,
    /// Seconds a queue head may stand on a permitted (yield) green: it waits for a gap in
    /// the opposing flow, so only a wait no opposing flow explains fails.
    pub permitted_stop_s: f64,
    /// Seconds a queue head may stand at an unsignalised junction with the junction clear.
    pub clear_stop_s: f64,
    /// Seconds a vehicle may stand mid-lane with nobody ahead.
    pub phantom_stop_s: f64,
    /// Margin over one signal cycle before any stop is a long stop.
    pub cycle_margin_s: f64,
    /// Examples kept per row.
    pub examples: usize,
    /// An actor whose every step is printed on stderr.
    pub trace: Option<u32>,
    /// Whether the run's rules let a vehicle turn right on a steady red after a full stop:
    /// such an entry is not a red entry. [`check_traffic`] reads it from the scenario
    /// (`traffic.right_turn_on_red`, else the highway preset's jurisdiction).
    pub right_turn_on_red: bool,
}

impl CheckOptions {
    /// The defaults for `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        CheckOptions {
            dir: dir.into(),
            green_stop_s: 6.0,
            permitted_stop_s: 90.0,
            clear_stop_s: 8.0,
            phantom_stop_s: 15.0,
            cycle_margin_s: 30.0,
            examples: 8,
            trace: None,
            right_turn_on_red: false,
        }
    }
}

/// One row of the check.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CheckRow {
    /// The row's name.
    pub name: &'static str,
    /// What was counted.
    pub count: u64,
    /// The largest count that passes.
    pub limit: u64,
    /// Whether it passed.
    pub ok: bool,
    /// The first few cases, in words.
    pub examples: Vec<String>,
}

/// Stop-line discharge measured from queues standing at green onset.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Discharge {
    /// Queues of six or more measured.
    pub queues: u64,
    /// Mean headway of the fifth and later queued vehicles, seconds.
    pub saturation_headway_s: Option<f64>,
    /// Mean of `Σ_{i≤4} (h_i − h_s)`, seconds.
    pub startup_lost_time_s: Option<f64>,
    /// Mean headway of each of the first eight positions, seconds.
    pub headway_by_position_s: Vec<f64>,
}

/// The whole check.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TrafficCheck {
    /// The run directory.
    pub dir: String,
    /// Simulated seconds covered.
    pub run_s: f64,
    /// Steps read.
    pub steps: u64,
    /// Distinct vehicles seen.
    pub vehicles: u64,
    /// Distinct pedestrians seen.
    pub pedestrians: u64,
    /// The rows.
    pub rows: Vec<CheckRow>,
    /// Stop-line discharge.
    pub discharge: Discharge,
    /// Longest standstill of any vehicle, seconds, and who.
    pub longest_stop_s: f64,
    /// Where the longest standstill was.
    pub longest_stop: String,
    /// Mean moving speed per road class, m/s, and the mean limit.
    pub speeds: BTreeMap<String, (f64, f64)>,
    /// Gridlock teleports and removals the engine logged.
    pub gridlock_teleports: u64,
    /// Whether every row passed.
    pub ok: bool,
}

const STOP_MPS: f64 = 0.1;

/// How far before the junction a stop line stands, metres: the mobility engine's
/// `STOP_LINE_OFFSET_M`.
const STOP_LINE_M: f64 = 2.0;

/// How recently a vehicle must have stood on its approach for a right turn on red to count
/// as one made after a full stop, seconds. **A choice**: long enough for the gap the
/// driver waits for, short enough that a car that stood a cycle ago does not excuse a
/// turn it rolled through.
const RTOR_STOP_WITHIN_S: f64 = 15.0;

fn ped_class(idx: u8) -> bool {
    VehicleClass::ALL.get(idx as usize) == Some(&VehicleClass::Pedestrian)
}

fn class_of(idx: u8) -> VehicleClass {
    VehicleClass::ALL
        .get(idx as usize)
        .copied()
        .unwrap_or(VehicleClass::Passenger)
}

/// Lane geometry for projection.
struct Geo {
    cum: Vec<Vec<f64>>,
}

impl Geo {
    fn new(world: &World) -> Self {
        let cum = world
            .roads
            .lanes()
            .iter()
            .map(|l| {
                let mut c = Vec::with_capacity(l.centreline.len());
                let mut acc = 0.0;
                for (i, p) in l.centreline.iter().enumerate() {
                    if i > 0 {
                        let q = l.centreline[i - 1];
                        acc += ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt();
                    }
                    c.push(acc);
                }
                c
            })
            .collect();
        Geo { cum }
    }

    /// `(s, lateral)` of `(x, y)` on lane `l`, and the polyline's length.
    fn project(&self, world: &World, l: LaneId, x: f64, y: f64) -> (f64, f64, f64) {
        let lane = world.roads.lane(l);
        let pts = &lane.centreline;
        let cum = &self.cum[l.as_usize()];
        let total = cum.last().copied().unwrap_or(0.0);
        let mut best = (f64::INFINITY, 0.0, 0.0);
        for i in 1..pts.len() {
            let (a, b) = (pts[i - 1], pts[i]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = dx * dx + dy * dy;
            let u = if len2 > 0.0 {
                (((x - a.x) * dx + (y - a.y) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (px, py) = (a.x + u * dx, a.y + u * dy);
            let d2 = (x - px).powi(2) + (y - py).powi(2);
            if d2 < best.0 {
                let side = if dx * (y - a.y) - dy * (x - a.x) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                best = (d2, cum[i - 1] + u * len2.sqrt(), side * d2.sqrt());
            }
        }
        (best.1, best.2, total)
    }
}

/// Signal knowledge: per plan, per phase, the state each wire group shows; per internal
/// lane, its plan and index.
struct Signals {
    /// wire id → (plan index, per-phase j2735 code).
    groups: BTreeMap<u32, (usize, Vec<u8>)>,
    /// plan index → wire ids.
    plan_groups: Vec<Vec<u32>>,
    /// internal lane → (plan index, movement index).
    movement: BTreeMap<LaneId, (usize, usize)>,
    /// Current lamp per wire id.
    lamp: BTreeMap<u32, u8>,
    /// Current matched phase per plan (`None` unknown or ambiguous).
    phase: Vec<Option<usize>>,
    /// Per plan, each movement's state from each lamp change on: `(t, states)`.
    hist: Vec<Vec<(f64, Vec<Option<SignalState>>)>>,
    cycle: Vec<f64>,
}

impl Signals {
    fn new(world: &World) -> Self {
        let mut groups = BTreeMap::new();
        let mut plan_groups = vec![Vec::new(); world.signals.len()];
        for g in world.group_signals() {
            let plan = if g.wire_id >= 65536 {
                (g.wire_id / 65536 - 1) as usize
            } else {
                g.wire_id as usize
            };
            if plan >= plan_groups.len() {
                continue;
            }
            plan_groups[plan].push(g.wire_id);
            groups.insert(
                g.wire_id,
                (
                    plan,
                    g.timeline.iter().map(|(s, _)| s.j2735_phase()).collect(),
                ),
            );
        }
        let mut movement = BTreeMap::new();
        for (p, plan) in world.signals.iter().enumerate() {
            for (i, l) in plan.controlled.iter().enumerate() {
                movement.entry(*l).or_insert((p, i));
            }
        }
        Signals {
            groups,
            plan_groups,
            movement,
            lamp: BTreeMap::new(),
            phase: vec![None; world.signals.len()],
            hist: vec![Vec::new(); world.signals.len()],
            cycle: world.signals.iter().map(|p| p.cycle_s).collect(),
        }
    }

    /// Re-matches the phases of the plans whose lamps changed; returns the plans that
    /// match no phase.
    fn update(&mut self, world: &World, changed: &BTreeSet<usize>, t: f64) -> Vec<usize> {
        let mut bad = Vec::new();
        for &p in changed {
            let wires = &self.plan_groups[p];
            let n = wires
                .first()
                .and_then(|w| self.groups.get(w))
                .map_or(0, |g| g.1.len());
            let mut matches = (0..n).filter(|&ph| {
                wires.iter().all(|w| {
                    let want = self.groups[w].1[ph];
                    self.lamp.get(w).is_none_or(|got| *got == want)
                })
            });
            let first = matches.next();
            let more = matches.next();
            if first.is_none() && !wires.is_empty() {
                bad.push(p);
            }
            // Several phases can show the same lamps (an all-red after each green); the
            // movement states of all of them are then asked for in `state`.
            self.phase[p] = if more.is_some() {
                first.map(|f| f + 1_000_000)
            } else {
                first
            };
            let states = world.signals[p]
                .controlled
                .iter()
                .map(|l| self.state(world, *l))
                .collect();
            self.hist[p].push((t, states));
        }
        bad
    }

    /// The state of movement `via` at an earlier `t` (at or after the first lamp read).
    fn state_at(&self, via: LaneId, t: f64) -> Option<SignalState> {
        let (p, i) = *self.movement.get(&via)?;
        let h = &self.hist[p];
        let k = h.partition_point(|(at, _)| *at <= t);
        h.get(k.checked_sub(1)?).and_then(|(_, v)| v[i])
    }

    /// The state of movement `via` now: `None` when the lamps do not tell.
    fn state(&self, world: &World, via: LaneId) -> Option<SignalState> {
        self.state_in(world, via, &self.lamp, &self.phase)
    }

    /// [`Signals::state`] under the lamps and phases of another instant.
    fn state_in(
        &self,
        world: &World,
        via: LaneId,
        lamp: &BTreeMap<u32, u8>,
        phases: &[Option<usize>],
    ) -> Option<SignalState> {
        let (p, i) = *self.movement.get(&via)?;
        let ph = phases[p]?;
        let plan = &world.signals[p];
        if ph >= 1_000_000 {
            // Ambiguous: the states every matching phase agrees on.
            let wires = &self.plan_groups[p];
            let mut agreed: Option<SignalState> = None;
            for (k, phase) in plan.phases.iter().enumerate() {
                let fits = wires
                    .iter()
                    .all(|w| lamp.get(w).is_none_or(|got| *got == self.groups[w].1[k]));
                if fits {
                    let s = phase.states[i];
                    if agreed.is_some_and(|a| a != s) {
                        return None;
                    }
                    agreed = Some(s);
                }
            }
            return agreed;
        }
        plan.phases.get(ph).map(|phase| phase.states[i])
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    t: f64,
    x: f64,
    y: f64,
    z: f64,
    lane: u32,
    speed: f64,
    accel: f64,
    heading: f64,
    class: u8,
}

/// One standstill being timed.
#[derive(Debug, Clone)]
struct Stop {
    since: f64,
    lane: u32,
    /// Seconds at the head of the queue, per kind of signal state of each movement from
    /// the lane: movement → seconds on protected green, on permitted green.
    green: BTreeMap<LaneId, (f64, f64)>,
    /// Seconds at an unsignalised head with the junction clear, longest stretch.
    clear_run: f64,
    clear_max: f64,
    /// Seconds standing mid-lane with nobody ahead, longest stretch.
    phantom_run: f64,
    phantom_max: f64,
    /// Where it stands, for the examples.
    s: f64,
    cycle: f64,
}

struct Row {
    name: &'static str,
    count: u64,
    limit: u64,
    examples: Vec<String>,
}

struct Rows {
    rows: Vec<Row>,
    max_examples: usize,
}

impl Rows {
    fn add(&mut self, name: &'static str, ex: impl FnOnce() -> String) {
        let max = self.max_examples;
        let r = self
            .rows
            .iter_mut()
            .find(|r| r.name == name)
            .expect("declared row");
        r.count += 1;
        if r.examples.len() < max {
            r.examples.push(ex());
        }
    }
}

/// Runs the check.
///
/// # Errors
/// The run directory lacks its recording or scenario, or either cannot be read.
pub fn check_traffic(opts: &CheckOptions) -> Result<TrafficCheck> {
    let dir = &opts.dir;
    let scenario_path = dir.join("scenario.resolved.yaml");
    let scenario = v2xw_engine::Scenario::load(&scenario_path)?;
    let world = v2xw_engine::wiring::build_world(&scenario)?;
    let rules = v2xw_mobility::rules::TrafficRules::of_highway_preset(
        scenario.world.highway_preset.map(|p| p.label()),
    );
    let opts = CheckOptions {
        right_turn_on_red: scenario
            .traffic
            .as_ref()
            .and_then(|t| t.right_turn_on_red)
            .unwrap_or(rules.right_turn_on_red),
        ..opts.clone()
    };
    check_recording(&world, &dir.join("recording.mcap"), &opts)
}

/// The check over one recording of a run on `world`.
///
/// # Errors
/// The recording cannot be read.
pub fn check_recording(
    world: &World,
    recording: &Path,
    opts: &CheckOptions,
) -> Result<TrafficCheck> {
    let geo = Geo::new(world);
    let mut sig = Signals::new(world);
    let mut rows = Rows {
        rows: [
            ("stopped_on_green", 0),
            ("stopped_on_permitted_green", 0),
            ("stopped_at_clear_junction", 0),
            ("phantom_stop", 0),
            ("long_stop", 0),
            ("red_entry", 0),
            ("lamp_phase_unknown", 0),
            ("overlap", 0),
            ("jump", 0),
            ("decel", 0),
            ("accel", 0),
            ("jerk", 0),
            ("speeding", 0),
            ("ped_jump", 0),
            ("ped_speed_jump", 0),
            ("ped_heading_flip", 0),
        ]
        .into_iter()
        .map(|(name, limit)| Row {
            name,
            count: 0,
            limit,
            examples: Vec::new(),
        })
        .collect(),
        max_examples: opts.examples,
    };

    // Per lane: the junction its end feeds, the internal lanes it can take, whether any is
    // signalised.
    let lanes = world.roads.lanes();
    let mut vias: Vec<Vec<LaneId>> = vec![Vec::new(); lanes.len()];
    for c in world.roads.connections() {
        if let Some(v) = c.via {
            vias[c.from_lane.as_usize()].push(v);
        }
    }
    let junction_of_lane = |l: LaneId| -> Option<usize> {
        vias[l.as_usize()]
            .first()
            .and_then(|v| world.roads.lane(*v).junction)
            .map(|j| j.as_usize())
    };
    let junction_count = world.roads.junctions().len();
    let mut signalised_junction = vec![false; junction_count];
    for plan in &world.signals {
        if plan.junction.as_usize() < junction_count {
            signalised_junction[plan.junction.as_usize()] = true;
        }
    }
    // Incoming non-internal lanes per junction.
    let mut incoming: Vec<Vec<LaneId>> = vec![Vec::new(); junction_count];
    for (i, v) in vias.iter().enumerate() {
        if let Some(j) = v.first().and_then(|v| world.roads.lane(*v).junction) {
            if j.as_usize() < junction_count {
                incoming[j.as_usize()].push(LaneId::new(i as u32));
            }
        }
    }

    let mut reader = Reader::open(recording)?;
    let mut scene = Scene::new();
    let mut prev: BTreeMap<u32, Sample> = BTreeMap::new();
    let mut prev_jerk_a: BTreeMap<u32, f64> = BTreeMap::new();
    let mut stops: BTreeMap<u32, Stop> = BTreeMap::new();
    // Where and when each vehicle last stood: a right turn on red is lawful only after a
    // full stop at the line.
    let mut last_stood: BTreeMap<u32, (u32, f64)> = BTreeMap::new();
    let mut seen_veh: BTreeSet<u32> = BTreeSet::new();
    let mut seen_ped: BTreeSet<u32> = BTreeSet::new();
    let mut steps = 0u64;
    let mut veh_steps = 0u64;
    let mut t_first = f64::NAN;
    let mut t_last = 0.0;
    let mut longest = (0.0f64, String::new());
    let mut speed_acc: BTreeMap<String, (f64, f64, u64)> = BTreeMap::new();
    let mut teleports = 0u64;
    let mut speeding_steps: BTreeMap<u32, u32> = BTreeMap::new();
    let mut flagged: BTreeSet<(u32, &'static str, u64)> = BTreeSet::new();
    // Discharge: per approach lane, the queue at green onset and its entry times.
    let mut lane_green_prev: BTreeMap<u32, bool> = BTreeMap::new();
    let mut queues: BTreeMap<u32, (f64, Vec<u32>, Vec<f64>)> = BTreeMap::new();
    let mut headways: Vec<(f64, Vec<f64>)> = Vec::new();
    let mut err: Option<CliError> = None;
    // The lamps of the previous frame: a vehicle that crossed its stop line during a step
    // decided on the lamp at the step's start.

    let mut frame_cb = |f: v2xw_record::reader::RecordedFrame| -> v2xw_record::Result<()> {
        let mut changed_plans: BTreeSet<usize> = BTreeSet::new();
        if f.topic == TOPIC_KEYFRAME {
            let kf = KeyframeBody::decode(f.frame.body())?;
            for s in &kf.signals {
                if sig.lamp.insert(s.signal_id, s.phase) != Some(s.phase) {
                    if let Some((p, _)) = sig.groups.get(&s.signal_id) {
                        changed_plans.insert(*p);
                    }
                }
            }
            scene.load_keyframe(&kf);
        } else if f.topic == TOPIC_DELTA {
            let d = DeltaBody::decode(f.frame.body())?;
            for s in &d.signals {
                if sig.lamp.insert(s.signal_id, s.phase) != Some(s.phase) {
                    if let Some((p, _)) = sig.groups.get(&s.signal_id) {
                        changed_plans.insert(*p);
                    }
                }
            }
            scene.apply_delta(&d)?;
        } else {
            return Ok(());
        }
        let t = v2xw_core::time::ns_to_secs(scene.sim_time_ns());
        if prev.values().next().is_some_and(|p| p.t >= t) {
            // A keyframe repeating the step a delta already gave.
            return Ok(());
        }
        for p in sig.update(world, &changed_plans, t) {
            let key = (p as u32, "lamp_phase_unknown", 0);
            if flagged.insert(key) {
                rows.add("lamp_phase_unknown", || {
                    format!(
                        "plan {p} at t {t:.1} s: lamps {:?}",
                        sig.plan_groups[p]
                            .iter()
                            .map(|w| sig.lamp.get(w))
                            .collect::<Vec<_>>()
                    )
                });
            }
        }
        steps += 1;
        if t_first.is_nan() {
            t_first = t;
        }
        t_last = t;
        let o = scene.origin();
        let mut now: BTreeMap<u32, Sample> = BTreeMap::new();
        for i in 0..scene.actor_count() {
            let id = scene.actor_id()[i];
            if id == U32_NONE {
                continue;
            }
            now.insert(
                id,
                Sample {
                    t,
                    x: f64::from(scene.x_mm()[i]) / 1000.0 + o[0],
                    y: f64::from(scene.y_mm()[i]) / 1000.0 + o[1],
                    z: f64::from(scene.z_mm()[i]) / 1000.0 + o[2],
                    lane: scene.lane_id()[i],
                    speed: f64::from(scene.speed_cq()[i]) / v2xw_record::quant::SPEED_SCALE,
                    accel: f64::from(scene.accel_cq()[i]) / v2xw_record::quant::ACCEL_SCALE,
                    heading: v2xw_record::quant::brad_to_rad(scene.heading_brad()[i]),
                    class: scene.class_idx()[i],
                },
            );
        }

        // Per lane, vehicles by s.
        let mut by_lane: BTreeMap<u32, Vec<(f64, f64, u32, f64)>> = BTreeMap::new();
        let mut on_internal: BTreeMap<usize, u32> = BTreeMap::new();
        let mut grid: BTreeMap<(i64, i64), Vec<u32>> = BTreeMap::new();
        for (id, a) in &now {
            if ped_class(a.class) {
                seen_ped.insert(*id);
                if let Some(lane) = (a.lane != U32_NONE && (a.lane as usize) < lanes.len())
                    .then(|| world.roads.lane(LaneId::new(a.lane)))
                {
                    if matches!(lane.kind, LaneKind::Crossing | LaneKind::Internal) {
                        if let Some(j) = lane.junction {
                            *on_internal.entry(j.as_usize()).or_default() += 1;
                        }
                    }
                }
                continue;
            }
            seen_veh.insert(*id);
            veh_steps += 1;
            grid.entry(((a.x / 2.0).floor() as i64, (a.y / 2.0).floor() as i64))
                .or_default()
                .push(*id);
            if a.lane == U32_NONE || a.lane as usize >= lanes.len() {
                continue;
            }
            let l = LaneId::new(a.lane);
            let lane = world.roads.lane(l);
            if lane.kind == LaneKind::Internal {
                if let Some(j) = lane.junction {
                    *on_internal.entry(j.as_usize()).or_default() += 1;
                }
            }
            let (s, lat, _) = geo.project(world, l, a.x, a.y);
            let len = class_of(a.class).dims().length_m;
            by_lane.entry(a.lane).or_default().push((s, lat, *id, len));
        }
        for v in by_lane.values_mut() {
            v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
        }

        // Overlaps: same lane, consecutive along s.
        for (lane, v) in &by_lane {
            for w in v.windows(2) {
                let (f, l) = (w[0], w[1]);
                let d = l.0 - f.0;
                if d < f.3.min(l.3) - 0.5 && (f.1 - l.1).abs() < 1.2 {
                    let key = (f.2.min(l.2) ^ (f.2.max(l.2) << 16), "overlap", 0);
                    if flagged.insert(key) {
                        rows.add("overlap", || {
                            format!(
                                "t {t:.1} s lane {lane}: actors {} and {} {d:.2} m apart",
                                f.2, l.2
                            )
                        });
                    }
                }
            }
        }
        // Coincident reference points anywhere (junctions included).
        for ((gx, gy), ids) in &grid {
            for &a in ids {
                for dx in 0..=1 {
                    for dy in -1..=1 {
                        if dx == 0 && dy < 0 {
                            continue;
                        }
                        let Some(others) = grid.get(&(gx + dx, gy + dy)) else {
                            continue;
                        };
                        for &b in others {
                            if (dx == 0 && dy == 0 && b <= a) || a == b {
                                continue;
                            }
                            let (pa, pb) = (now[&a], now[&b]);
                            let d = ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt();
                            if d < 1.0 && (pa.z - pb.z).abs() < 2.0 {
                                let key = (a.min(b) ^ (a.max(b) << 16), "overlap", 1);
                                if flagged.insert(key) {
                                    rows.add("overlap", || format!("t {t:.1} s: actors {a} and {b} {d:.2} m apart (lanes {} and {})", pa.lane, pb.lane));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Green onsets for discharge, per approach lane: any movement from it permits.
        let mut lane_green_now: BTreeMap<u32, bool> = BTreeMap::new();
        for (lane, v) in &by_lane {
            let l = LaneId::new(*lane);
            if vias[l.as_usize()]
                .iter()
                .any(|m| sig.movement.contains_key(m))
            {
                let green = vias[l.as_usize()]
                    .iter()
                    .any(|m| sig.state(world, *m) == Some(SignalState::Green));
                let was = lane_green_prev.get(lane).copied().unwrap_or(true);
                if green && !was {
                    let len = world.roads.lane(l).length_m;
                    let mut q: Vec<(f64, u32)> = v
                        .iter()
                        .filter(|e| now[&e.2].speed < 1.0 && len - e.0 < 80.0)
                        .map(|e| (e.0, e.2))
                        .collect();
                    q.sort_by(|a, b| b.0.total_cmp(&a.0));
                    // A queue is contiguous from the line: no gap over 12 m between bodies.
                    let mut ids = Vec::new();
                    let mut last_s = len;
                    for (s, id) in q {
                        if last_s - s > 12.0 + 8.0 {
                            break;
                        }
                        ids.push(id);
                        last_s = s;
                    }
                    if ids.len() >= 6 {
                        queues.insert(*lane, (t, ids, Vec::new()));
                    }
                }
                lane_green_now.insert(*lane, green);
            }
        }
        for (lane, g) in lane_green_now {
            lane_green_prev.insert(lane, g);
        }

        for (id, a) in &now {
            if a.speed < STOP_MPS && a.lane != U32_NONE {
                last_stood.insert(*id, (a.lane, a.t));
            }
            let Some(p) = prev.get(id).copied() else {
                continue;
            };
            let dt = a.t - p.t;
            if dt <= 0.0 || dt > 1.0 {
                continue;
            }
            let ped = ped_class(a.class);
            let dist = ((a.x - p.x).powi(2) + (a.y - p.y).powi(2)).sqrt();
            let allowed = p.speed.max(a.speed) * dt * 1.5 + 0.75;
            if dist > allowed {
                let name = if ped { "ped_jump" } else { "jump" };
                rows.add(name, || format!("actor {id} at t {t:.1} s moved {dist:.2} m in {dt:.2} s at {:.2}->{:.2} m/s (lanes {}->{})", p.speed, a.speed, p.lane, a.lane));
                continue;
            }
            let acc = (a.speed - p.speed) / dt;
            if ped {
                if acc.abs() > 3.0 {
                    rows.add("ped_speed_jump", || {
                        format!(
                            "pedestrian {id} at t {t:.1} s: {:.2}->{:.2} m/s",
                            p.speed, a.speed
                        )
                    });
                }
                if p.speed > 0.3 && a.speed > 0.3 {
                    let mut dh = (a.heading - p.heading).abs() % std::f64::consts::TAU;
                    if dh > std::f64::consts::PI {
                        dh = std::f64::consts::TAU - dh;
                    }
                    if dh > std::f64::consts::FRAC_PI_2 {
                        rows.add("ped_heading_flip", || {
                            format!(
                                "pedestrian {id} at t {t:.1} s turned {:.0}°",
                                dh.to_degrees()
                            )
                        });
                    }
                }
                continue;
            }
            if acc < -9.0 {
                rows.add("decel", || {
                    format!(
                        "actor {id} at t {t:.1} s lane {}: {:.2}->{:.2} m/s ({acc:.1} m/s²)",
                        a.lane, p.speed, a.speed
                    )
                });
            } else if acc > 6.0 {
                rows.add("accel", || {
                    format!(
                        "actor {id} at t {t:.1} s lane {}: {:.2}->{:.2} m/s ({acc:.1} m/s²)",
                        a.lane, p.speed, a.speed
                    )
                });
            }
            if let Some(pa) = prev_jerk_a.get(id) {
                let jerk = (acc - pa) / dt;
                if jerk.abs() > 30.0 {
                    rows.add("jerk", || {
                        format!("actor {id} at t {t:.1} s: {jerk:.0} m/s³")
                    });
                }
            }
            prev_jerk_a.insert(*id, acc);

            // Entries into a junction: red entries, discharge.
            if a.lane != p.lane && a.lane != U32_NONE && (a.lane as usize) < lanes.len() {
                let l = LaneId::new(a.lane);
                if world.roads.lane(l).kind == LaneKind::Internal {
                    // The lamp the driver saw where the law reads it: as the front crossed
                    // the stop line, `STOP_LINE_M` before the junction (the recorded lane is
                    // the front bumper's), at the speed it had, and a step before that.
                    let crossed = t - dt - STOP_LINE_M / p.speed.max(0.3);
                    let was = sig.state_at(l, crossed);
                    if let Some(state) = sig.state(world, l) {
                        let turned_on_red_after_stop = opts.right_turn_on_red
                            && last_stood.get(id).is_some_and(|(lane, at)| {
                                *lane == p.lane && t - at <= RTOR_STOP_WITHIN_S
                            })
                            && world.roads.connections().iter().any(|c| {
                                c.via == Some(l)
                                    && matches!(
                                        c.direction,
                                        TurnDirection::Right | TurnDirection::SlightRight
                                    )
                            });
                        if state == SignalState::Red
                            && was == Some(SignalState::Red)
                            && p.speed > 0.0
                            && !turned_on_red_after_stop
                        {
                            rows.add("red_entry", || format!("actor {id} at t {t:.1} s entered {} from {} on red at {:.1} m/s", a.lane, p.lane, a.speed));
                        }
                    }
                    if let Some(q) = queues.get_mut(&p.lane) {
                        if q.1.get(q.2.len()) == Some(id) {
                            q.2.push(t);
                        }
                    }
                }
            }

            // Speed per road class.
            if a.lane != U32_NONE && (a.lane as usize) < lanes.len() {
                let lane = world.roads.lane(LaneId::new(a.lane));
                if lane.kind != LaneKind::Internal && a.speed > 0.5 {
                    let rc = world.roads.edge(lane.edge).road_class.label().to_string();
                    let e = speed_acc.entry(rc).or_default();
                    e.0 += a.speed;
                    e.1 += lane.speed_limit_mps;
                    e.2 += 1;
                    if a.speed > 1.25 * lane.speed_limit_mps + 2.0 {
                        let n = speeding_steps.entry(*id).or_default();
                        *n += 1;
                        if *n == 10 {
                            rows.add("speeding", || {
                                format!(
                                    "actor {id} at t {t:.1} s: {:.1} m/s on a {:.1} m/s lane {}",
                                    a.speed, lane.speed_limit_mps, a.lane
                                )
                            });
                        }
                    }
                }
            }
        }

        // Standstills.
        for (id, a) in &now {
            if ped_class(a.class) {
                continue;
            }
            let dt = prev.get(id).map_or(0.0, |p| a.t - p.t).clamp(0.0, 1.0);
            if a.speed > STOP_MPS || a.lane == U32_NONE || a.lane as usize >= lanes.len() {
                if let Some(st) = stops.remove(id) {
                    close_stop(
                        world,
                        &mut rows,
                        &mut longest,
                        *id,
                        &st,
                        t,
                        Some(a.lane),
                        opts,
                        &sig,
                    );
                }
                continue;
            }
            let l = LaneId::new(a.lane);
            let lane = world.roads.lane(l);
            let entry = by_lane
                .get(&a.lane)
                .and_then(|v| v.iter().position(|e| e.2 == *id));
            let (s, len) = entry.map_or((0.0, lane.length_m), |k| {
                (by_lane[&a.lane][k].0, lane.length_m)
            });
            let ahead = entry
                .and_then(|k| by_lane[&a.lane].get(k + 1))
                .map(|e| e.0 - s);
            let j = junction_of_lane(l);
            let cycle = j
                .and_then(|j| {
                    world
                        .signals
                        .iter()
                        .position(|p| p.junction.as_usize() == j)
                })
                .map_or(90.0, |p| sig.cycle[p]);
            let st = stops.entry(*id).or_insert_with(|| Stop {
                since: t,
                lane: a.lane,
                green: BTreeMap::new(),
                clear_run: 0.0,
                clear_max: 0.0,
                phantom_run: 0.0,
                phantom_max: 0.0,
                s,
                cycle,
            });
            if st.lane != a.lane {
                st.lane = a.lane;
                st.green.clear();
            }
            st.s = s;
            let head = ahead.is_none() && len - s < 12.0 && lane.kind != LaneKind::Internal;
            if head {
                for m in &vias[l.as_usize()] {
                    match sig.state(world, *m) {
                        Some(SignalState::Green) => st.green.entry(*m).or_default().0 += dt,
                        Some(SignalState::GreenYield) => st.green.entry(*m).or_default().1 += dt,
                        _ => {}
                    }
                }
                if let Some(j) = j.filter(|j| !signalised_junction[*j]) {
                    let busy = on_internal.get(&j).copied().unwrap_or(0) > 0
                        || incoming[j].iter().any(|il| {
                            *il != l
                                && by_lane.get(&il.index()).is_some_and(|v| {
                                    let ilen = world.roads.lane(*il).length_m;
                                    v.iter().any(|e| ilen - e.0 < 40.0 && now[&e.2].speed > 0.5)
                                })
                        });
                    if busy {
                        st.clear_run = 0.0;
                    } else {
                        st.clear_run += dt;
                        st.clear_max = st.clear_max.max(st.clear_run);
                    }
                }
            }
            let mid =
                ahead.is_none_or(|d| d > 30.0) && len - s > 30.0 && lane.kind != LaneKind::Internal;
            if mid {
                st.phantom_run += dt;
                st.phantom_max = st.phantom_max.max(st.phantom_run);
            } else {
                st.phantom_run = 0.0;
            }
        }
        for id in prev.keys() {
            if !now.contains_key(id) {
                if let Some(st) = stops.remove(id) {
                    close_stop(
                        world,
                        &mut rows,
                        &mut longest,
                        *id,
                        &st,
                        t,
                        None,
                        opts,
                        &sig,
                    );
                }
                prev_jerk_a.remove(id);
            }
        }
        if let Some(a) = opts.trace.and_then(|who| now.get(&who)) {
            let (s, lat, len) = if a.lane != U32_NONE && (a.lane as usize) < lanes.len() {
                geo.project(world, LaneId::new(a.lane), a.x, a.y)
            } else {
                (f64::NAN, f64::NAN, f64::NAN)
            };
            let states: Vec<String> = if a.lane != U32_NONE && (a.lane as usize) < lanes.len() {
                vias[a.lane as usize]
                    .iter()
                    .map(|m| {
                        // What stands on the movement's exit: (count, rearmost actor, its s, its speed).
                        let exit = world
                            .roads
                            .connections()
                            .iter()
                            .find(|c| c.via == Some(*m))
                            .map(|c| c.to_lane);
                        let on_exit = exit
                            .and_then(|e| by_lane.get(&e.index()))
                            .map(|v| (v.len(), v.first().map(|f| (f.2, f.0, now[&f.2].speed))));
                        let inside = by_lane.get(&m.index()).map_or(0, Vec::len);
                        format!(
                            "{}:{:?} in {inside} exit {:?} {on_exit:?}",
                            m.index(),
                            sig.state(world, *m),
                            exit.map(|e| e.index())
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };
            eprintln!(
                "trace t {t:.1} lane {} s {s:.2}/{len:.1} lat {lat:.2} v {:.2} a {:.2} xy ({:.2}, {:.2}) {}",
                a.lane,
                a.speed,
                a.accel,
                a.x,
                a.y,
                states.join(" ")
            );
            // The people near it on a crossing, who may be what it yields to.
            for (pid, p) in &now {
                if !ped_class(p.class) || p.lane == U32_NONE || (p.lane as usize) >= lanes.len() {
                    continue;
                }
                let d = ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
                let kind = world.lane(LaneId::new(p.lane)).kind;
                if d <= 30.0 && kind == v2xw_world::LaneKind::Crossing {
                    eprintln!(
                        "trace t {t:.1}   ped {pid} on crossing lane {} at {d:.1} m xy ({:.2}, {:.2}) v {:.2}",
                        p.lane, p.x, p.y, p.speed
                    );
                }
            }
        }
        prev = now;
        Ok(())
    };
    let mut rec_cb = |r: v2xw_record::reader::RecordedRecord| -> v2xw_record::Result<()> {
        if r.channel == "traffic.control" {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&r.json) {
                let n = v
                    .get("teleported")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    + v.get("removed")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                teleports = teleports.max(n);
            }
        }
        Ok(())
    };
    if let Err(e) = reader.for_each_message(&mut frame_cb, &mut rec_cb) {
        err = Some(e.into());
    }
    drop(frame_cb);
    if let Some(e) = err {
        return Err(e);
    }
    let t_end = t_last;
    for (id, st) in std::mem::take(&mut stops) {
        close_stop(
            world,
            &mut rows,
            &mut longest,
            id,
            &st,
            t_end,
            None,
            opts,
            &sig,
        );
    }
    for (_, (onset, _, times)) in std::mem::take(&mut queues) {
        if times.len() >= 6 {
            headways.push((onset, times));
        }
    }

    // Discharge statistics (HCM 7th ed. §19.2): h_1 from the green onset to the first
    // vehicle over the line, h_i between successive ones; the saturation headway is the
    // mean from the fifth on, and the start-up lost time is Σ_{i≤4} (h_i − h_s).
    let mut discharge = Discharge::default();
    {
        let mut by_pos: Vec<(f64, u64)> = vec![(0.0, 0); 8];
        let mut hs: Vec<f64> = Vec::new();
        let mut lost: Vec<f64> = Vec::new();
        for (onset, times) in &headways {
            let mut h = vec![times[0] - onset];
            h.extend(times.windows(2).map(|w| w[1] - w[0]));
            // A queue broken by a vehicle turning on a later arrow is not saturated flow.
            if h.iter().skip(1).any(|x| *x > 6.0) {
                continue;
            }
            discharge.queues += 1;
            for (i, x) in h.iter().enumerate().take(8) {
                by_pos[i].0 += x;
                by_pos[i].1 += 1;
            }
            let tail: Vec<f64> = h.iter().skip(4).copied().collect();
            if !tail.is_empty() {
                let s = tail.iter().sum::<f64>() / tail.len() as f64;
                hs.push(s);
                lost.push(h.iter().take(4).map(|x| x - s).sum::<f64>());
            }
        }
        discharge.headway_by_position_s = by_pos
            .iter()
            .filter(|p| p.1 > 0)
            .map(|p| v2xw_core::math::q3(p.0 / p.1 as f64))
            .collect();
        if !hs.is_empty() {
            discharge.saturation_headway_s = Some(v2xw_core::math::q3(
                hs.iter().sum::<f64>() / hs.len() as f64,
            ));
            discharge.startup_lost_time_s = Some(v2xw_core::math::q3(
                lost.iter().sum::<f64>() / lost.len() as f64,
            ));
        }
    }

    let jerk_limit = veh_steps / 1000;
    let jump_limit = teleports;
    let rows_out: Vec<CheckRow> = rows
        .rows
        .into_iter()
        .map(|r| {
            let limit = match r.name {
                "jerk" => jerk_limit,
                "jump" => jump_limit,
                _ => r.limit,
            };
            CheckRow {
                name: r.name,
                count: r.count,
                limit,
                ok: r.count <= limit,
                examples: r.examples,
            }
        })
        .collect();
    let ok = rows_out.iter().all(|r| r.ok);
    Ok(TrafficCheck {
        dir: recording
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        run_s: if t_first.is_nan() {
            0.0
        } else {
            t_last - t_first
        },
        steps,
        vehicles: seen_veh.len() as u64,
        pedestrians: seen_ped.len() as u64,
        rows: rows_out,
        discharge,
        longest_stop_s: v2xw_core::math::q3(longest.0),
        longest_stop: longest.1,
        speeds: speed_acc
            .into_iter()
            .map(|(k, (v, lim, n))| {
                (
                    k,
                    (
                        v2xw_core::math::q3(v / n as f64),
                        v2xw_core::math::q3(lim / n as f64),
                    ),
                )
            })
            .collect(),
        gridlock_teleports: teleports,
        ok,
    })
}

#[allow(clippy::too_many_arguments)]
fn close_stop(
    world: &World,
    rows: &mut Rows,
    longest: &mut (f64, String),
    id: u32,
    st: &Stop,
    t: f64,
    next_lane: Option<u32>,
    opts: &CheckOptions,
    _sig: &Signals,
) {
    let dur = t - st.since;
    let lane = st.lane;
    let len = world.roads.lane(LaneId::new(lane)).length_m;
    let where_ = format!(
        "actor {id} on lane {lane} ({:.1} m before its end) from t {:.1} s for {dur:.1} s",
        len - st.s,
        st.since
    );
    if dur > longest.0 {
        *longest = (dur, where_.clone());
    }
    // The movement it took, if it left into a junction; otherwise the worst of them.
    let took = next_lane
        .map(LaneId::new)
        .filter(|l| st.green.contains_key(l));
    let (g, p) = match took {
        Some(m) => st.green[&m],
        None => st
            .green
            .values()
            .fold((f64::INFINITY, f64::INFINITY), |acc, v| {
                (acc.0.min(v.0), acc.1.min(v.1))
            }),
    };
    let (g, p) = if st.green.is_empty() {
        (0.0, 0.0)
    } else {
        (g, p)
    };
    if g > opts.green_stop_s {
        rows.add("stopped_on_green", || {
            format!("{where_}: {g:.1} s of it on a protected green (next lane {next_lane:?})")
        });
    } else if p > opts.permitted_stop_s {
        rows.add("stopped_on_permitted_green", || {
            format!("{where_}: {p:.1} s of it on a permitted green (next lane {next_lane:?})")
        });
    }
    if st.clear_max > opts.clear_stop_s {
        rows.add("stopped_at_clear_junction", || {
            format!(
                "{where_}: {:.1} s with the junction clear (next lane {next_lane:?})",
                st.clear_max
            )
        });
    }
    if st.phantom_max > opts.phantom_stop_s {
        rows.add("phantom_stop", || {
            format!(
                "{where_}: {:.1} s mid-lane with nobody ahead",
                st.phantom_max
            )
        });
    }
    if dur > st.cycle + opts.cycle_margin_s {
        rows.add("long_stop", || where_.clone());
    }
}

/// The check as text.
pub fn format(c: &TrafficCheck) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "check-traffic {}: {:.1} s, {} steps, {} vehicles, {} pedestrians — {}",
        c.dir,
        c.run_s,
        c.steps,
        c.vehicles,
        c.pedestrians,
        if c.ok { "PASS" } else { "FAIL" }
    );
    for r in &c.rows {
        let _ = writeln!(
            s,
            "  {:<28} {:>6} (limit {:>4}) {}",
            r.name,
            r.count,
            r.limit,
            if r.ok { "ok" } else { "FAIL" }
        );
        for e in &r.examples {
            let _ = writeln!(s, "      {e}");
        }
    }
    let d = &c.discharge;
    let _ = writeln!(
        s,
        "  discharge: {} queues, saturation headway {:?} s, start-up lost time {:?} s, by position {:?}",
        d.queues, d.saturation_headway_s, d.startup_lost_time_s, d.headway_by_position_s
    );
    let _ = writeln!(
        s,
        "  longest stop {:.1} s: {}",
        c.longest_stop_s, c.longest_stop
    );
    for (k, (v, l)) in &c.speeds {
        let _ = writeln!(
            s,
            "  speed {k}: mean {v:.1} m/s against a mean limit of {l:.1}"
        );
    }
    let _ = writeln!(s, "  gridlock teleports/removals: {}", c.gridlock_teleports);
    s
}
