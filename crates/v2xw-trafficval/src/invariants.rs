//! Group 4: invariants and metamorphic properties.
//!
//! * Every shipped scenario's traffic, run as the kernel runs it, with the auditor's
//!   safety classes at zero.
//! * Peak density on the procedural grid: the same, at a demand far above the shipped one,
//!   with red meaning no entry.
//! * Metamorphic: more demand never raises the mean speed; a closure reroutes the traffic
//!   that would have used the lane, nobody enters it, and nobody jumps.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use v2xw_core::ids::{ActorId, LaneId};
use v2xw_engine::Scenario;
use v2xw_mobility::audit::{AuditActor, AuditParams, AuditReport, Check as Class};
use v2xw_mobility::rules::TrafficRules;
use v2xw_mobility::{EngineParams, IntersectionMode, NativeMobility};
use v2xw_world::World;

use crate::study::{self, Event, StudyResult, Tamper};
use crate::{Check, Config, Group, Lab, Outcome, Row, Variant};

/// The classes that must be zero on every run: a collision, a rule broken or a physically
/// impossible motion (the gate of `v2xw-mobility/tests/traffic_invariants.rs`), plus the
/// pedestrian classes and the world's own.
pub const SAFETY: [Class; 23] = [
    Class::Overlap,
    Class::GapBelowMinimum,
    Class::LateralOffset,
    Class::InBuilding,
    Class::OutsideJunction,
    Class::RedEntry,
    Class::AmberEntry,
    Class::ConflictZone,
    Class::LaneChangeNearJunction,
    Class::QueueJump,
    Class::IllegalTransition,
    Class::Teleport,
    Class::StepSpeed,
    Class::HeadingFlip,
    Class::SpeedJump,
    Class::AccelBound,
    Class::MidRoadDespawn,
    Class::WorldInternalOutsideJunction,
    Class::WorldLaneInBuilding,
    Class::WorldConflictingGreens,
    Class::PedestrianOverlap,
    Class::OccupiedCrosswalkEntry,
    Class::PedestrianDontWalkEntry,
];

/// Classes held to a rate rather than zero: under one vehicle-step in a thousand.
pub const RATE: [Class; 2] = [Class::Jerk, Class::HeadingJump];

/// The rows an audit report gives: the safety classes' total, the rate classes, and the
/// standstill class.
pub fn audit_rows(label: &str, a: &AuditReport) -> (Vec<Row>, Vec<String>) {
    let total: u64 = SAFETY.iter().map(|c| a.count(*c)).sum();
    let nonzero: Vec<String> = SAFETY
        .iter()
        .filter(|c| a.count(**c) > 0)
        .map(|c| format!("{} {}", c.label(), a.count(*c)))
        .collect();
    let steps = a.stats.vehicle_steps.max(1) as f64;
    let rate: u64 = RATE.iter().map(|c| a.count(*c)).sum();
    let mut rows = vec![
        Row::zero(
            format!("{label}: safety-class violations (vehicle-steps)"),
            total,
            "v2xw_mobility::audit: collisions, red and amber entries, conflict zones, queue jumps, teleports, impossible motion, pedestrians hit",
        )
        .n(a.stats.vehicle_steps as usize),
        Row::held(
            format!("{label}: jerk and heading-rate exceedances per vehicle-step"),
            rate as f64 / steps,
            (0.0, 1e-3),
            "tests/traffic_invariants.rs: under one vehicle-step in a thousand",
        ),
        Row::zero(format!("{label}: vehicles standing past the standstill limit"), a.count(Class::Standstill), "no gridlock"),
    ];
    rows.push(Row::reported(format!("{label}: peak vehicles"), a.stats.peak_vehicles as f64, "—"));
    let mut notes = Vec::new();
    if !nonzero.is_empty() {
        notes.push(format!("{label}: {}", nonzero.join(", ")));
        for e in a.examples.iter().filter(|e| SAFETY.contains(&e.check)).take(4) {
            notes.push(format!(
                "{label} example: {} at t {:.1} s, actor {:?}, other {:?}, lane {:?}, ({:.1}, {:.1}): {}",
                e.check.label(),
                e.t_s,
                e.actor,
                e.other,
                e.lane,
                e.x_m,
                e.y_m,
                e.detail
            ));
        }
    }
    (rows, notes)
}

/// The engine a scenario study runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// As the kernel builds it.
    Shipped,
    /// Junction control off.
    NoJunctionControl,
}

/// Overrides of a scenario before it runs.
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    /// Demand, veh/h (switches a scenario with none to Poisson).
    pub rate_veh_per_h: Option<f64>,
    /// Simulated seconds.
    pub seconds: Option<f64>,
    /// Pedestrians.
    pub pedestrians: Option<u32>,
}

/// Loads `path` with `ov` applied.
///
/// # Errors
/// The loader's.
pub fn load(path: &Path, ov: &Overrides) -> Result<Scenario, String> {
    let mut s = Scenario::load(path).map_err(|e| format!("{}: {e}", path.display()))?;
    apply(&mut s, ov);
    Ok(s)
}

/// Applies `ov` to `s`.
pub fn apply(s: &mut Scenario, ov: &Overrides) {
    if let Some(r) = ov.rate_veh_per_h {
        s.actors.vehicles.demand.rate_veh_per_h = Some(r);
        if s.actors.vehicles.demand.kind == "mobility/demand/none" {
            s.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
        }
    }
    if let Some(d) = ov.seconds {
        s.time.duration_s = d;
    }
    if let Some(p) = ov.pedestrians {
        s.actors.vru.pedestrians = p;
    }
}

/// The world a scenario names, imported once per process.
///
/// # Errors
/// The importer's.
pub fn world_of(lab: &mut Lab, s: &Scenario) -> Result<Arc<World>, String> {
    let key = format!("world-{}", v2xw_engine::wiring::world_cache_key(s).map_err(|e| e.to_string())?);
    let s = s.clone();
    lab.study(&key, move |_| {
        let started = std::time::Instant::now();
        let w = v2xw_engine::wiring::build_world(&s).map_err(|e| e.to_string())?;
        eprintln!(
            "    world: {} lanes, {} junctions, {} signal plans, {} buildings ({:.0} s wall clock)",
            w.roads.lanes().len(),
            w.roads.junctions().len(),
            w.signals.len(),
            w.buildings.len(),
            started.elapsed().as_secs_f64()
        );
        Ok(Arc::new(w))
    })
}

/// Runs scenario `s` (already overridden) with `engine`, events and tamper.
///
/// # Errors
/// The world's, the demand's or the engine's.
pub fn scenario_run(
    lab: &mut Lab,
    s: &Scenario,
    engine: Engine,
    events: &[Event],
    tamper: Option<Tamper>,
    watch: impl FnMut(f64, &[AuditActor]),
) -> Result<StudyResult, String> {
    let world = world_of(lab, s)?;
    let rules = TrafficRules::of_highway_preset(s.world.highway_preset.map(|p| p.label()));
    let mobility = match engine {
        Engine::Shipped => v2xw_engine::wiring::native_mobility(s),
        Engine::NoJunctionControl => {
            let params = rules.apply(EngineParams {
                step: s.time.mobility_step(),
                intersections: IntersectionMode::None,
                junction_clearance: false,
                crosswalk_yield: false,
                ..EngineParams::default()
            });
            NativeMobility::new(params).with_vru_population(v2xw_mobility::engine::VruPopulation {
                pedestrians: s.actors.vru.pedestrians,
                cyclists: s.actors.vru.cyclists,
            })
        }
    };
    let demand = v2xw_engine::wiring::build_demand(s, &world).map_err(|e| e.to_string())?;
    let audit = AuditParams {
        right_turn_on_red: rules.right_turn_on_red,
        ..AuditParams::default()
    };
    study::run(
        &world,
        mobility,
        demand,
        s.seed,
        s.time.mobility_step(),
        s.time.duration_s,
        audit,
        events,
        tamper,
        watch,
    )
}

/// A cached scenario study keyed by a label.
///
/// # Errors
/// As [`scenario_run`].
pub fn scenario_study(
    lab: &mut Lab,
    key: &str,
    s: &Scenario,
    engine: Engine,
    tamper: Option<Tamper>,
) -> Result<StudyResult, String> {
    let k = format!("scenario-{key}-{engine:?}-{}", tamper.is_some());
    if let Some(r) = lab.get::<StudyResult>(&k) {
        return Ok(r);
    }
    let r = scenario_run(lab, s, engine, &[], tamper, |_, _| {})?;
    lab.study(&k, |_| Ok(r.clone()))
}

/// One vehicle's published position jumps 30 m along its lane for one step at step 300.
pub fn teleport(step: u64, actors: &mut Vec<AuditActor>) {
    if step == 300 {
        if let Some(a) = actors.iter_mut().find(|a| a.speed_mps > 1.0) {
            a.s_m += 30.0;
            a.pos.x += 30.0;
        }
    }
}

/// The scenarios of `scenarios/` that put traffic on the road, by file name.
pub fn shipped_scenarios(cfg: &Config) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(dir) = std::fs::read_dir(cfg.root.join("scenarios")) {
        for e in dir.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "yaml") {
                out.push(p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
            }
        }
    }
    out.sort();
    out
}

/// The checks of this group.
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "invariants/shipped-scenarios",
            group: Group::Invariants,
            title: "Every shipped scenario's traffic holds every safety invariant",
            procedure: "Each `scenarios/*.yaml` is loaded, its world, demand and mobility \
                built by `v2xw_engine::wiring` exactly as the kernel builds them, and its \
                traffic stepped with its own seed and step for 120 s (60 s in quick mode) \
                under the auditor, with the jurisdiction's right-turn-on-red rule. Scenarios \
                on an OpenStreetMap world are run in full mode only (group 5 runs each city \
                in quick mode too).",
            faults: &["one vehicle's published position jumps 30 m for one step at 30 s, in every scenario"],
            run: shipped,
        },
        Check {
            id: "invariants/peak-density",
            group: Group::Invariants,
            title: "At peak density the grid stays collision-free, red means no entry, and nothing gridlocks",
            procedure: "`phase1-grid.yaml` at 20,000 veh/h offered (667 times its shipped \
                30 veh/h; the thinned-Poisson demand places what fits) for 180 s (90 s in \
                quick mode).",
            faults: &["junction control switched off (no signals, no yielding)"],
            run: peak_density,
        },
        Check {
            id: "invariants/demand-monotone",
            group: Group::Invariants,
            title: "More demand never raises the mean speed",
            procedure: "`phase1-grid.yaml` at 1,000, 4,000, 10,000 and 20,000 veh/h, 180 s \
                each (90 s in quick mode), the same seed; the auditor's network mean speed. \
                The largest rise from one demand to the next is held under 3 %: the \
                arrivals differ between runs, so a flat stretch moves by noise.",
            faults: &["the 20,000 veh/h run with junction control switched off"],
            run: demand_monotone,
        },
        Check {
            id: "invariants/closure",
            group: Group::Invariants,
            title: "A lane closure reroutes the traffic bound for it; nobody enters it and nobody jumps",
            procedure: "`phase1-grid.yaml` at 6,000 veh/h; at 40 s the driving lane nearest \
                the grid's centre is closed (`MobilityCommand::Closure`). Entries into the \
                lane are counted from vehicles that were not already committed to it (on \
                it, or on a connector into it) when it closed, from 10 s after the closure; \
                vehicles whose next lane was the closed one at the closure and later drove \
                elsewhere count as rerouted; the auditor counts teleports, illegal \
                transitions and mid-road despawns. 120 s (90 s in quick mode).",
            faults: &[
                "the closure is never sent to the engine (the check still treats the lane as closed)",
                "one vehicle's published position jumps 30 m for one step at 30 s",
            ],
            run: closure,
        },
    ]
}

fn shipped(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let cfg = lab.cfg.clone();
    let seconds = cfg.secs(120, 60) as f64;
    let mut rows = Vec::new();
    let mut notes = Vec::new();
    for name in shipped_scenarios(&cfg) {
        let path = cfg.root.join("scenarios").join(&name);
        let s = match load(
            &path,
            &Overrides {
                seconds: Some(seconds),
                ..Overrides::default()
            },
        ) {
            Ok(s) => s,
            Err(e) => {
                notes.push(format!("{name}: does not load: {e}"));
                rows.push(Row::zero(format!("{name}: loads"), 1, "every shipped scenario loads"));
                continue;
            }
        };
        if s.actors.vehicles.demand.kind == "mobility/demand/none" && s.actors.vru.pedestrians == 0 {
            notes.push(format!("{name}: no traffic (no demand, no pedestrians); skipped."));
            continue;
        }
        let osm = !matches!(s.world.source, v2xw_world::WorldSourceSpec::Procedural { .. });
        if osm && cfg.quick {
            notes.push(format!("{name}: OpenStreetMap world, skipped in quick mode."));
            continue;
        }
        let tamper: Option<Tamper> = variant.is_fault().then_some(teleport as Tamper);
        let key = name.trim_end_matches(".yaml").to_string();
        match scenario_study(lab, &key, &s, Engine::Shipped, tamper) {
            Ok(r) => {
                let (mut rr, nn) = audit_rows(&key, &r.audit);
                // Only the safety total and the rate are held per scenario; the rest is noted.
                rows.append(&mut rr);
                notes.extend(nn);
            }
            Err(e) => {
                notes.push(format!("{name}: could not run: {e}"));
                rows.push(Row::zero(format!("{key}: runs"), 1, "every shipped scenario runs"));
            }
        }
    }
    Ok(Outcome { rows, notes })
}

fn grid_scenario(lab: &Lab, rate: f64, seconds: f64) -> Result<Scenario, String> {
    load(
        &lab.cfg.root.join("scenarios/phase1-grid.yaml"),
        &Overrides {
            rate_veh_per_h: Some(rate),
            seconds: Some(seconds),
            pedestrians: None,
        },
    )
}

fn peak_density(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let seconds = lab.cfg.secs(180, 90) as f64;
    let s = grid_scenario(lab, 20_000.0, seconds)?;
    let engine = if variant.is_fault() { Engine::NoJunctionControl } else { Engine::Shipped };
    let r = scenario_study(lab, &format!("grid-20000-{seconds}"), &s, engine, None)?;
    let a = &r.audit;
    let (mut rows, notes) = audit_rows("phase1-grid at 20,000 veh/h", a);
    rows.insert(0, Row::zero("junction entries on red", a.count(Class::RedEntry), "UVC §11-202(c) / MUTCD §4D.04: steady red, do not enter"));
    rows.insert(1, Row::zero("two conflicting movements in one zone at once", a.count(Class::ConflictZone), "UVC §11-202(a)1"));
    rows.insert(2, Row::zero("overlapping vehicles", a.count(Class::Overlap), "no collision"));
    rows.push(Row::held("signalised junction entries", a.stats.signalised_entries as f64, (100.0, f64::INFINITY), "the run must exercise the signals"));
    Ok(Outcome { rows, notes })
}

fn demand_monotone(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let seconds = lab.cfg.secs(180, 90) as f64;
    let rates = [1_000.0, 4_000.0, 10_000.0, 20_000.0];
    let mut speeds = Vec::new();
    for (i, rate) in rates.iter().enumerate() {
        let s = grid_scenario(lab, *rate, seconds)?;
        let engine = if variant.is_fault() && i == rates.len() - 1 { Engine::NoJunctionControl } else { Engine::Shipped };
        let r = scenario_study(lab, &format!("grid-{rate}-{seconds}"), &s, engine, None)?;
        speeds.push((*rate, r.audit.stats.mean_speed_mps, r.audit.stats.peak_vehicles));
    }
    let mut worst = f64::NEG_INFINITY;
    for w in speeds.windows(2) {
        worst = worst.max((w[1].1 - w[0].1) / w[0].1);
    }
    let mut rows = vec![Row::held(
        "largest rise in mean speed from one demand to the next, fraction",
        worst,
        (f64::NEG_INFINITY, 0.03),
        "fundamental diagram: speed is non-increasing in density (Greenshields 1935; Edie 1961)",
    )];
    for (rate, v, peak) in &speeds {
        rows.push(Row::reported(format!("mean speed at {rate:.0} veh/h, m/s (peak {peak} vehicles)"), *v, "—"));
    }
    Ok(Outcome { rows, notes: vec![] })
}

/// The driving lane (not a connector) whose midpoint is nearest the world's centre.
fn central_lane(world: &World) -> Option<LaneId> {
    let lanes: Vec<&v2xw_world::Lane> = world
        .roads
        .lanes()
        .iter()
        .filter(|l| l.kind == v2xw_world::LaneKind::Driving && l.junction.is_none() && l.length_m > 30.0)
        .collect();
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for l in &lanes {
        for p in &l.centreline {
            minx = minx.min(p.x);
            miny = miny.min(p.y);
            maxx = maxx.max(p.x);
            maxy = maxy.max(p.y);
        }
    }
    let (cx, cy) = (0.5 * (minx + maxx), 0.5 * (miny + maxy));
    lanes
        .iter()
        .min_by(|a, b| {
            let d = |l: &v2xw_world::Lane| {
                let m = &l.centreline[l.centreline.len() / 2];
                (m.x - cx).powi(2) + (m.y - cy).powi(2)
            };
            d(a).total_cmp(&d(b)).then(a.id.cmp(&b.id))
        })
        .map(|l| l.id)
}

#[derive(Debug, Clone, Default)]
struct ClosureWatch {
    committed: BTreeSet<ActorId>,
    bound: BTreeSet<ActorId>,
    snap_taken: bool,
    known: BTreeSet<ActorId>,
    entries: u64,
    entered: BTreeSet<ActorId>,
    rerouted: BTreeSet<ActorId>,
    examples: Vec<String>,
}

fn closure(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let seconds = lab.cfg.secs(120, 90) as f64;
    let s = grid_scenario(lab, 6_000.0, seconds)?;
    let world = world_of(lab, &s)?;
    let lane = central_lane(&world).ok_or("no driving lane")?;
    let close_at = 40.0;
    let events = if variant == Variant::Fault(0) {
        vec![]
    } else {
        vec![Event::Closure {
            at_s: close_at,
            lane,
            closed: true,
        }]
    };
    let tamper: Option<Tamper> = (variant == Variant::Fault(1)).then_some(teleport as Tamper);
    let internal: BTreeMap<LaneId, bool> = world.roads.lanes().iter().map(|l| (l.id, l.junction.is_some())).collect();
    let mut w = ClosureWatch::default();
    let r = scenario_run(lab, &s, Engine::Shipped, &events, tamper, |t, actors| {
        if t < close_at {
            for a in actors {
                w.known.insert(a.actor);
            }
            return;
        }
        if !w.snap_taken {
            w.snap_taken = true;
            for a in actors {
                let on_connector_into = internal.get(&a.lane).copied().unwrap_or(false) && a.route_next == Some(lane);
                if a.lane == lane || on_connector_into {
                    w.committed.insert(a.actor);
                }
                if a.route_next == Some(lane) && a.lane != lane {
                    w.bound.insert(a.actor);
                }
            }
        }
        for a in actors {
            if a.lane == lane && !w.committed.contains(&a.actor) && t >= close_at + 10.0 && w.entered.insert(a.actor) {
                w.entries += 1;
                if w.examples.len() < 3 {
                    w.examples.push(format!("actor {} entered the closed lane at {t:.1} s", a.actor.index()));
                }
            }
            if w.bound.contains(&a.actor) && a.lane != lane && a.route_next.is_some_and(|n| n != lane) && !internal.get(&a.lane).copied().unwrap_or(false) {
                w.rerouted.insert(a.actor);
            }
        }
    })?;
    let a = &r.audit;
    let mut notes = vec![format!(
        "closed lane {} at {close_at} s; {} vehicles were committed to it, {} were bound for it next.",
        lane.index(),
        w.committed.len(),
        w.bound.len()
    )];
    notes.extend(w.examples.clone());
    Ok(Outcome {
        rows: vec![
            Row::zero("entries into the closed lane by uncommitted vehicles", w.entries, "MobilityCommand::Closure: closed lanes cost infinity to the router"),
            Row::zero(
                "teleports, illegal transitions, step-speed jumps and mid-road despawns",
                a.count(Class::Teleport) + a.count(Class::IllegalTransition) + a.count(Class::StepSpeed) + a.count(Class::MidRoadDespawn),
                "a reroute is a new path from where the car is, never a jump",
            ),
            Row::reported("vehicles bound for the lane that drove elsewhere", w.rerouted.len() as f64, "—"),
            Row::zero("safety-class violations", SAFETY.iter().map(|c| a.count(*c)).sum::<u64>(), "v2xw_mobility::audit"),
        ],
        notes,
    })
}
