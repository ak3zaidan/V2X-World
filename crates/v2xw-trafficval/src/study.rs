//! A study: one simulated run watched by every observer the suite has.
//!
//! The run is stepped exactly as the kernel steps mobility (the scenario's seed and step,
//! the same `MobilityCtx`), and each step's vehicles and pedestrians are handed to:
//!
//! * the calibration harness ([`v2xw_mobility::calibration::CalibrationObserver`]);
//! * the traffic auditor ([`v2xw_mobility::audit::TrafficAuditor`]);
//! * the safety-surrogate observer ([`crate::safety::SafetyObserver`]);
//! * the motion observer here: lateral acceleration through junction turns, and the
//!   acceleration of each vehicle class from standstill.

use std::collections::BTreeMap;

use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::rng::RngRegistry;
use v2xw_core::time::Duration;
use v2xw_mobility::audit::{AuditActor, AuditParams, AuditReport, TrafficAuditor};
use v2xw_mobility::calibration::{CalibrationObserver, CalibrationReport, Summary};
use v2xw_mobility::traits::{Demand, Mobility};
use v2xw_mobility::views::MobilityCommand;
use v2xw_mobility::{MobilityCtx, NativeMobility, VehicleClass};
use v2xw_world::World;

use crate::safety::{SafetyObserver, SafetyStats};

/// Lateral acceleration and launches.
#[derive(Debug, Clone, Default)]
pub struct MotionObserver {
    prev: BTreeMap<ActorId, (LaneId, f64, f64)>,
    /// Per traversal of a junction connector: the actor's peak lateral acceleration.
    turn_peak: BTreeMap<ActorId, (LaneId, f64)>,
    turn_peaks: Vec<f64>,
    /// Launch in progress: start time and class.
    launch: BTreeMap<ActorId, (f64, VehicleClass, bool)>,
    launches: BTreeMap<&'static str, Vec<f64>>,
    accel: Vec<f64>,
}

/// What the motion observer measured.
#[derive(Debug, Clone, Default)]
pub struct MotionStats {
    /// Peak lateral acceleration of each junction-connector traversal faster than 2 m/s,
    /// m/s².
    pub turn_peak_lateral_mps2: Summary,
    /// Mean launch acceleration 0 to 8 m/s by class label, m/s².
    pub launch_by_class: BTreeMap<&'static str, Summary>,
    /// Every vehicle-step's acceleration, m/s².
    pub accel_mps2: Summary,
    /// 5th percentile of the acceleration (hard braking end), m/s².
    pub accel_p05: f64,
    /// 95th percentile, m/s².
    pub accel_p95: f64,
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let i = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn wrap(a: f64) -> f64 {
    let tau = 2.0 * core::f64::consts::PI;
    let mut x = a % tau;
    if x > core::f64::consts::PI {
        x -= tau;
    } else if x < -core::f64::consts::PI {
        x += tau;
    }
    x
}

impl MotionObserver {
    /// One step.
    pub fn observe(&mut self, world: &World, t1_s: f64, dt_s: f64, actors: &[AuditActor]) {
        let mut seen = BTreeMap::new();
        for a in actors {
            self.accel.push(a.accel_mps2);
            let internal = world.try_lane(a.lane).is_some_and(|l| l.junction.is_some());
            if let Some((plane, _ps, ph)) = self.prev.get(&a.actor) {
                // Lateral acceleration on a connector: speed × heading rate.
                if internal && *plane == a.lane && a.speed_mps > 2.0 && a.changing.is_none() {
                    let lat = (a.speed_mps * wrap(a.heading_rad - ph) / dt_s).abs();
                    let e = self.turn_peak.entry(a.actor).or_insert((a.lane, 0.0));
                    if e.0 != a.lane {
                        self.turn_peaks.push(e.1);
                        *e = (a.lane, 0.0);
                    }
                    e.1 = e.1.max(lat);
                }
            }
            if !internal {
                if let Some((_, peak)) = self.turn_peak.remove(&a.actor) {
                    if peak > 0.0 {
                        self.turn_peaks.push(peak);
                    }
                }
            }
            // Launches: from standstill to 8 m/s, never held back.
            match self.launch.get_mut(&a.actor) {
                None if a.speed_mps < 0.1 => {
                    self.launch.insert(a.actor, (t1_s, a.class, true));
                }
                Some(l) if a.speed_mps < 0.1 => {
                    *l = (t1_s, a.class, true);
                }
                Some(l) => {
                    if a.accel_mps2 < 0.0 {
                        l.2 = false;
                    }
                    if a.speed_mps >= 8.0 {
                        if l.2 && t1_s > l.0 {
                            self.launches.entry(a.class.as_str()).or_default().push(8.0 / (t1_s - l.0));
                        }
                        self.launch.remove(&a.actor);
                    }
                }
                None => {}
            }
            seen.insert(a.actor, (a.lane, a.s_m, a.heading_rad));
        }
        self.launch.retain(|k, _| seen.contains_key(k));
        self.prev = seen;
    }

    /// The figures.
    pub fn stats(&self) -> MotionStats {
        let mut acc = self.accel.clone();
        acc.sort_by(f64::total_cmp);
        MotionStats {
            turn_peak_lateral_mps2: Summary::of(&self.turn_peaks),
            launch_by_class: self.launches.iter().map(|(k, v)| (*k, Summary::of(v))).collect(),
            accel_mps2: Summary::of(&self.accel),
            accel_p05: quantile(&acc, 0.05),
            accel_p95: quantile(&acc, 0.95),
        }
    }
}

/// Everything a study measured.
#[derive(Debug, Clone)]
pub struct StudyResult {
    /// Calibration figures.
    pub calib: CalibrationReport,
    /// Invariant classes.
    pub audit: AuditReport,
    /// Safety surrogates.
    pub safety: SafetyStats,
    /// Lateral acceleration and launches.
    pub motion: MotionStats,
    /// Simulated seconds.
    pub seconds: f64,
    /// Trips the demand offered that the engine could not route.
    pub dropped_trips: u64,
}

/// An event a study applies at a time.
#[derive(Debug, Clone)]
pub enum Event {
    /// Close (or reopen) a lane.
    Closure {
        /// When, seconds.
        at_s: f64,
        /// Which lane.
        lane: LaneId,
        /// True closes it.
        closed: bool,
    },
}

/// A hook into the observed stream, for faults that corrupt what is observed (a teleport
/// injected into the published positions, for instance).
pub type Tamper = fn(step: u64, actors: &mut Vec<AuditActor>);

/// Runs `engine` on `world` with `demand` for `seconds`.
///
/// # Errors
/// The engine's `init`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    world: &World,
    mut engine: NativeMobility,
    demand: Box<dyn Demand>,
    seed: u64,
    step: Duration,
    seconds: f64,
    audit: AuditParams,
    events: &[Event],
    tamper: Option<Tamper>,
    mut watch: impl FnMut(f64, &[AuditActor]),
) -> Result<StudyResult, String> {
    let rng = RngRegistry::new(seed);
    {
        let mut ctx = MobilityCtx::new(0, world, &rng);
        engine.init(&mut ctx, demand).map_err(|e| e.to_string())?;
    }
    let mut calib = CalibrationObserver::new(world);
    let mut auditor = TrafficAuditor::new(world, audit);
    auditor.audit_world(world);
    let mut safety = SafetyObserver::new(world);
    let mut motion = MotionObserver::default();
    let horizon = (seconds * 1e9) as u64;
    let mut pending: Vec<&Event> = events.iter().collect();
    let mut t = 0u64;
    let mut k = 0u64;
    while t < horizon {
        let mut ctx = MobilityCtx::new(t, world, &rng);
        pending.retain(|e| match e {
            Event::Closure { at_s, lane, closed } => {
                if (*at_s * 1e9) as u64 <= t {
                    engine.command(
                        &mut ctx,
                        MobilityCommand::Closure {
                            lane: *lane,
                            closed: *closed,
                        },
                    );
                    false
                } else {
                    true
                }
            }
        });
        let update = engine.step(&mut ctx, step);
        let t1 = update.t;
        let mut actors = engine.audit_actors(world, t1);
        if let Some(f) = tamper {
            f(k, &mut actors);
        }
        let people = engine.audit_pedestrians(world);
        let t1_s = t1 as f64 / 1e9;
        let dt_s = (t1 - t) as f64 / 1e9;
        calib.observe(world, t, t1, &actors, &update.despawned, &people);
        auditor.observe_with_pedestrians(world, t, t1, &actors, &update.despawned, &people);
        safety.observe(world, t1_s, dt_s, &actors);
        motion.observe(world, t1_s, dt_s, &actors);
        watch(t1_s, &actors);
        t = t1;
        k += 1;
    }
    Ok(StudyResult {
        calib: calib.report(),
        audit: auditor.report(),
        safety: safety.stats(),
        motion: motion.stats(),
        seconds,
        dropped_trips: engine.dropped_trips(),
    })
}
