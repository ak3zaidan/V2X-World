//! `mobility/sumo-traci`: SUMO drives the traffic, over TraCI, while this engine runs radio,
//! security, backend and UI on the states it publishes (04-models.md §2.8, the `high`
//! co-simulation tier).
//!
//! # What SUMO does and what this engine does
//!
//! SUMO owns every vehicle's motion: car following, lane changing, junction behaviour and
//! its own signal programmes. This plug-in launches `sumo --remote-port`, advances it by
//! the engine's mobility step, and reads back each vehicle's front-bumper position, speed,
//! angle, acceleration and lane in one batched TraCI message per step. Demand reaches SUMO
//! two ways, which may be combined:
//!
//! * the `.rou.xml` files of [`SumoConfig::route_files`], which SUMO reads and routes
//!   itself;
//! * this engine's [`Demand`], whose trips are routed by this engine's router (or the
//!   demand's own planned route) and inserted with `route.add` + `vehicle.add`, so both
//!   simulators drive *the same routes* — what the cross-validation needs.
//!
//! # Frames
//!
//! SUMO's network coordinates are metres; a world imported from that `net.xml` is the same
//! frame translated by the importer's `shift_m` ([`v2xw_world::sumo::SumoImportReport`]),
//! which [`SumoConfig::shift_m`] carries. SUMO publishes the **front bumper** and an angle
//! in degrees clockwise from north; this engine publishes the rear reference point (the
//! rear bumper, standing in for the rear axle, exactly as `NativeMobility` does —
//! `v2xw-mobility` `engine.rs`, "the published reference point is the rear bumper") and a
//! heading counter-clockwise from east. So `heading = π/2 − angle·π/180` and
//! `pos = front − length·(cos h, sin h)`.
//!
//! # Determinism
//!
//! SUMO is deterministic for a fixed `--seed` and a single simulation thread (SUMO
//! documentation, "Simulation/Randomness"); this plug-in passes the scenario seed, sorts
//! everything it publishes by actor id, and assigns actor ids in sorted SUMO-id order of
//! first appearance, so two runs with the same seed publish the same states.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use v2xw_core::card::{Family, ModelCard, Source, SourceKind, Tier};
use v2xw_core::geom::{Dims, LanePos, Vec3};
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::kinematics::Kinematics;
use v2xw_core::model::Model;
use v2xw_core::time::{Duration, SimTime};
use v2xw_mobility::ctx::MobCtx;
use v2xw_mobility::error::Result as MobResult;
use v2xw_mobility::routing::dijkstra::DijkstraParams;
use v2xw_mobility::traits::{Demand, Mobility};
use v2xw_mobility::views::{
    ActorSpawn, DespawnCause, DriverProfile, MobilityCommand, MobilityUpdate, Route,
};
use v2xw_mobility::{Dijkstra, FreeFlowCost, VehicleClass};

use crate::SumoIds;
use crate::traci::{Content, Get, Traci, cmd, var};

/// The model id.
pub const MODEL_ID: &str = "mobility/sumo-traci";

/// How to run SUMO.
#[derive(Debug, Clone)]
pub struct SumoConfig {
    /// The `sumo` binary.
    pub sumo_binary: PathBuf,
    /// The network (`-n`). Must be the network the world was imported from.
    pub net_file: PathBuf,
    /// Route files SUMO reads itself (`--route-files`), possibly none.
    pub route_files: Vec<PathBuf>,
    /// What to add to a SUMO coordinate to get a world coordinate (the importer's
    /// `shift_m`).
    pub shift_m: (f64, f64),
    /// SUMO's `--seed`.
    pub seed: u64,
    /// Further SUMO options, appended verbatim (e.g. `--default.carfollowmodel IDM`).
    pub extra_args: Vec<String>,
    /// The TCP port; `0` picks a free one.
    pub port: u16,
}

impl SumoConfig {
    /// A configuration for `net_file` with SUMO under `home` (see [`crate::sumo_home`]).
    pub fn new(home: &std::path::Path, net_file: impl Into<PathBuf>) -> Self {
        Self {
            sumo_binary: crate::sumo_tool(home, "sumo"),
            net_file: net_file.into(),
            route_files: Vec::new(),
            shift_m: (0.0, 0.0),
            seed: 1,
            extra_args: Vec::new(),
            port: 0,
        }
    }
}

/// What the plug-in counted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SumoCounters {
    /// Engine trips inserted into SUMO.
    pub inserted: u64,
    /// Engine trips SUMO refused or this engine could not route.
    pub refused: u64,
    /// Steps taken.
    pub steps: u64,
    /// Vehicles seen on a SUMO lane this world has no lane for (internal lanes excluded).
    pub unmapped_lane_samples: u64,
}

struct Tracked {
    id: ActorId,
    length_m: f64,
    width_m: f64,
    heading: f64,
}

/// SUMO as this engine's mobility model.
pub struct SumoMobility {
    card: ModelCard,
    cfg: SumoConfig,
    child: Option<Child>,
    conn: Option<Traci>,
    ids: SumoIds,
    demand: Option<Box<dyn Demand>>,
    router: Dijkstra,
    tracked: BTreeMap<String, Tracked>,
    by_actor: BTreeMap<ActorId, String>,
    kin: BTreeMap<ActorId, Kinematics>,
    next_actor: u32,
    t: SimTime,
    stops: BTreeMap<ActorId, Option<SimTime>>,
    /// Commands waiting for the next step.
    queued: Vec<(u8, u8, String, Content)>,
    /// What happened, for reports.
    pub counters: SumoCounters,
    /// The last error SUMO or the socket reported; once set, the plug-in stops stepping.
    pub error: Option<String>,
    error_reported: bool,
}

/// The model card.
pub fn card() -> ModelCard {
    let mut c = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "SUMO drives every vehicle over TraCI; this engine publishes SUMO's states as its \
         ground truth so radio, security, backend and UI run on SUMO traffic.",
    );
    c.tier = vec![Tier::High];
    c.sources.push(Source::new(
        SourceKind::Code,
        "SUMO TraCI protocol and tools/traci/connection.py, SUMO 1.27.1 (eclipse-sumo wheel)",
    ));
    c.sources.push(Source::new(
        SourceKind::Code,
        "04-models.md §2.8: SUMO via TraCI is the high-fidelity mobility tier",
    ));
    c.assumptions.push(
        "The world was imported from the same net.xml SUMO runs, so SUMO edge ids are world \
         edge names and the frames differ by the importer's shift."
            .to_string(),
    );
    c.limitations.push(
        "Signal states are SUMO's own programme and are not published back to the engine's \
         signal channel; SPaT generated by the engine follows the world's plan, which is \
         the same programme only when SUMO's tlLogic was imported unchanged."
            .to_string(),
    );
    c.limitations.push(
        "Vehicles inside a junction are on SUMO internal lanes, which the world folds into \
         connectors; their lane position is left empty there."
            .to_string(),
    );
    c.limitations.push(
        "Pedestrians and SUMO's own persons are not read.".to_string(),
    );
    c
}

impl SumoMobility {
    /// A plug-in for `cfg` on the world whose SUMO ids are `ids`.
    pub fn new(cfg: SumoConfig, ids: SumoIds) -> Self {
        Self {
            card: card(),
            cfg,
            child: None,
            conn: None,
            ids,
            demand: None,
            router: Dijkstra::new(DijkstraParams::for_classes(VehicleClass::Passenger.class_mask())),
            tracked: BTreeMap::new(),
            by_actor: BTreeMap::new(),
            kin: BTreeMap::new(),
            next_actor: 0,
            t: 0,
            stops: BTreeMap::new(),
            queued: Vec::new(),
            counters: SumoCounters::default(),
            error: None,
            error_reported: false,
        }
    }

    /// The SUMO version string, once connected.
    pub fn sumo_version(&self) -> Option<&str> {
        self.conn.as_ref().map(|c| c.sumo_version.as_str())
    }

    fn launch(&mut self, step: Duration) -> Result<(), String> {
        let port = if self.cfg.port == 0 {
            std::net::TcpListener::bind("127.0.0.1:0")
                .and_then(|l| l.local_addr())
                .map(|a| a.port())
                .map_err(|e| format!("no free port: {e}"))?
        } else {
            self.cfg.port
        };
        let mut c = Command::new(&self.cfg.sumo_binary);
        if let Some(home) = self.cfg.sumo_binary.parent().and_then(std::path::Path::parent) {
            c.env("SUMO_HOME", home);
        }
        c.arg("-n").arg(&self.cfg.net_file);
        if !self.cfg.route_files.is_empty() {
            let files: Vec<String> = self
                .cfg
                .route_files
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            c.arg("--route-files").arg(files.join(","));
        }
        c.arg("--remote-port")
            .arg(port.to_string())
            .arg("--step-length")
            .arg(format!("{}", step.as_secs_f64()))
            .arg("--seed")
            // SUMO's --seed is a 32-bit integer; a 64-bit scenario seed is folded into it.
            .arg(((self.cfg.seed ^ (self.cfg.seed >> 32)) & 0x7fff_ffff).to_string())
            .arg("--no-step-log")
            .arg("true")
            .arg("--no-warnings")
            .arg("true")
            .arg("--duration-log.disable")
            .arg("true")
            .args(&self.cfg.extra_args)
            .stdout(Stdio::null())
            // SUMO's errors (a bad network, a route it refuses) are the only explanation a
            // failed launch has, so they reach the terminal; warnings are off above.
            .stderr(Stdio::inherit());
        let child = c
            .spawn()
            .map_err(|e| format!("{}: {e}", self.cfg.sumo_binary.display()))?;
        self.child = Some(child);
        let conn = Traci::connect(port, std::time::Duration::from_secs(30)).map_err(|e| {
            format!("could not connect to SUMO on port {port} (did it exit? its errors are above): {e}")
        })?;
        self.conn = Some(conn);
        Ok(())
    }

    fn insert_trips(&mut self, ctx: &mut dyn MobCtx, from: SimTime, to: SimTime) {
        let Some(demand) = self.demand.as_mut() else {
            return;
        };
        let trips = demand.spawns_in(ctx, from, to);
        if trips.is_empty() {
            return;
        }
        let world = ctx.world();
        let mut sets = Vec::new();
        for trip in &trips {
            let lanes = demand
                .planned_route(world, trip)
                .filter(|r| v2xw_mobility::demand::hooks::planned_route_is_drivable(world, trip.class, r, false))
                .or_else(|| {
                    Dijkstra::new(DijkstraParams::for_classes(trip.class.class_mask()))
                        .search(world, trip.origin, trip.destination, trip.t, &FreeFlowCost::new(world))
                        .map(|r| r.lanes)
                });
            let edges = lanes.map(|l| self.ids.edges_of_route(world, &l)).unwrap_or_default();
            if edges.is_empty() {
                self.counters.refused += 1;
                continue;
            }
            let rid = format!("v2xw_r{}", trip.seq);
            let vid = format!("v2xw_{}", trip.seq);
            sets.push((cmd::SET_ROUTE, var::ADD, rid.clone(), Content::new().str_list(&edges)));
            let vtype = if trip.class == VehicleClass::Bicycle {
                "DEFAULT_BIKETYPE"
            } else {
                "DEFAULT_VEHTYPE"
            };
            let lane_index = world.lane(trip.origin).index;
            let add = Content::new()
                .compound(14)
                .str(&rid)
                .str(vtype)
                .str("now")
                .str(&lane_index.to_string())
                .str(&format!("{:.2}", trip.origin_s_m))
                .str("max")
                .str("current")
                .str("max")
                .str("current")
                .str("")
                .str("")
                .str("")
                .int(0)
                .int(0);
            sets.push((cmd::SET_VEHICLE, var::ADD_FULL, vid.clone(), add));
            sets.push((
                cmd::SET_VEHICLE,
                var::MAXSPEED,
                vid,
                Content::new().double(trip.desired_speed_mps),
            ));
        }
        let Some(conn) = self.conn.as_mut() else { return };
        match conn.set_batch(&sets) {
            Ok(results) => {
                // Three commands per trip; a trip counts as inserted when its add held.
                for chunk in results.chunks(3) {
                    if chunk.get(1).is_some_and(Result::is_ok) {
                        self.counters.inserted += 1;
                    } else {
                        self.counters.refused += 1;
                    }
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn read_state(&mut self, t1: SimTime, dt_s: f64) -> Result<MobilityUpdate, String> {
        let conn = self.conn.as_mut().ok_or("not connected")?;
        let list = conn
            .get(cmd::GET_VEHICLE, var::ID_LIST, "")
            .map_err(|e| e.to_string())?;
        let mut present: Vec<String> = match list {
            crate::traci::Value::StrList(v) => v,
            other => return Err(format!("vehicle id list: {other:?}")),
        };
        present.sort();
        let new: Vec<&String> = present.iter().filter(|v| !self.tracked.contains_key(*v)).collect();
        let mut gets = Vec::with_capacity(present.len() * 6 + new.len() * 3);
        for v in &new {
            for vr in [var::LENGTH, var::WIDTH, var::VEHICLECLASS] {
                gets.push(Get { cmd: cmd::GET_VEHICLE, var: vr, object: (*v).clone() });
            }
        }
        for v in &present {
            for vr in [var::POSITION, var::SPEED, var::ANGLE, var::ACCELERATION, var::LANE_ID, var::LANEPOSITION] {
                gets.push(Get { cmd: cmd::GET_VEHICLE, var: vr, object: v.clone() });
            }
        }
        let values = conn.get_batch(&gets).map_err(|e| e.to_string())?;
        let mut update = MobilityUpdate::empty(t1);
        let mut k = 0;
        let mut new_classes = BTreeMap::new();
        for v in &new {
            let length = values[k].f().unwrap_or(5.0);
            let width = values[k + 1].f().unwrap_or(1.8);
            let class = values[k + 2]
                .s()
                .and_then(|c| serde_json::from_value::<VehicleClass>(serde_json::Value::String(c.to_string())).ok())
                .unwrap_or_default();
            k += 3;
            let id = ActorId::new(self.next_actor);
            self.next_actor += 1;
            self.tracked.insert((*v).clone(), Tracked { id, length_m: length, width_m: width, heading: f64::NAN });
            self.by_actor.insert(id, (*v).clone());
            new_classes.insert(id, class);
        }
        // Departures: everything tracked and no longer present.
        let gone: Vec<String> = self
            .tracked
            .keys()
            .filter(|k| present.binary_search(k).is_err())
            .cloned()
            .collect();
        for g in gone {
            if let Some(tr) = self.tracked.remove(&g) {
                self.by_actor.remove(&tr.id);
                self.kin.remove(&tr.id);
                update.despawned.push((tr.id, DespawnCause::TripComplete));
            }
        }
        for v in &present {
            let pos = &values[k];
            let speed = values[k + 1].f().unwrap_or(0.0);
            let angle = values[k + 2].f().unwrap_or(0.0);
            let acc = values[k + 3].f().unwrap_or(0.0);
            let lane = values[k + 4].s().unwrap_or("").to_string();
            let lane_pos = values[k + 5].f().unwrap_or(0.0);
            k += 6;
            let Some(tr) = self.tracked.get_mut(v) else { continue };
            let (fx, fy) = match pos {
                crate::traci::Value::Pos(x, y) => (*x + self.cfg.shift_m.0, *y + self.cfg.shift_m.1),
                _ => continue,
            };
            let h = std::f64::consts::FRAC_PI_2 - angle.to_radians();
            let h = h.sin().atan2(h.cos());
            let yaw = if tr.heading.is_finite() && dt_s > 0.0 {
                let d = h - tr.heading;
                d.sin().atan2(d.cos()) / dt_s
            } else {
                0.0
            };
            tr.heading = h;
            let (c, s) = (h.cos(), h.sin());
            let lane_pos_world = if lane.starts_with(':') {
                None
            } else if let Some(l) = self.ids.lane_of.get(&lane) {
                Some(LanePos { lane: *l, s_m: lane_pos, d_m: 0.0 })
            } else {
                self.counters.unmapped_lane_samples += 1;
                None
            };
            let kin = Kinematics {
                t: t1,
                pos: Vec3::new(fx - tr.length_m * c, fy - tr.length_m * s, 0.0),
                vel: Vec3::new(speed * c, speed * s, 0.0),
                acc: Vec3::new(acc * c, acc * s, 0.0),
                heading_rad: h,
                yaw_rate_rad_s: yaw,
                lane: lane_pos_world,
                dims: Dims::new(tr.length_m, tr.width_m, 1.5),
            }
            .quantized();
            let id = tr.id;
            if let Some(class) = new_classes.get(&id) {
                let spec = class.spec();
                update.spawned.push(ActorSpawn {
                    actor: id,
                    t: t1,
                    class: *class,
                    kinematics: kin,
                    route: Route::empty(),
                    driver: DriverProfile {
                        desired_speed_mps: spec.desired_speed_mps(),
                        max_accel_mps2: spec.accel_mps2,
                        comfort_decel_mps2: spec.decel_mps2,
                        time_headway_s: 1.0,
                        min_gap_m: spec.min_gap_m,
                    },
                    seq: v
                        .strip_prefix("v2xw_")
                        .and_then(|n| n.parse().ok())
                        .unwrap_or((1u64 << 40) + u64::from(id.index())),
                });
            }
            self.kin.insert(id, kin);
        }
        update.states = self.kin.iter().map(|(a, k)| (*a, *k)).collect();
        update.spawned.sort_by_key(|s| s.actor);
        update.despawned.sort_by_key(|d| d.0);
        Ok(update)
    }

    fn release_stops(&mut self, t: SimTime) {
        let due: Vec<ActorId> = self
            .stops
            .iter()
            .filter(|(_, u)| u.is_some_and(|u| u <= t))
            .map(|(a, _)| *a)
            .collect();
        for a in due {
            self.stops.remove(&a);
            if let Some(v) = self.by_actor.get(&a) {
                self.queued.push((cmd::SET_VEHICLE, var::SPEED, v.clone(), Content::new().double(-1.0)));
            }
        }
    }

    /// Closes the connection and waits for SUMO to exit.
    pub fn shutdown(&mut self) {
        if let Some(mut c) = self.conn.take() {
            let _ = c.close();
        }
        if let Some(mut ch) = self.child.take() {
            // SUMO exits on close; kill it if it has not within a second.
            for _ in 0..20 {
                if matches!(ch.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let _ = ch.kill();
            let _ = ch.wait();
        }
    }
}

impl Drop for SumoMobility {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Model for SumoMobility {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Mobility for SumoMobility {
    fn tier(&self) -> Tier {
        Tier::High
    }

    fn init(&mut self, ctx: &mut dyn MobCtx, demand: Box<dyn Demand>) -> MobResult<()> {
        self.demand = Some(demand);
        self.t = ctx.now();
        let _ = &self.router;
        // The step length is not known until the first step; SUMO is launched then.
        Ok(())
    }

    fn step(&mut self, ctx: &mut dyn MobCtx, dt: Duration) -> MobilityUpdate {
        let update = self.step_inner(ctx, dt);
        if let Some(e) = &self.error
            && !self.error_reported
        {
            self.error_reported = true;
            eprintln!("mobility/sumo-traci stopped: {e}");
        }
        update
    }

    fn command(&mut self, ctx: &mut dyn MobCtx, cmd_: MobilityCommand) {
        self.command_inner(ctx, cmd_);
    }

    fn kinematics(&self, a: ActorId) -> Option<&Kinematics> {
        self.kin.get(&a)
    }
}

impl SumoMobility {
    fn step_inner(&mut self, ctx: &mut dyn MobCtx, dt: Duration) -> MobilityUpdate {
        let t0 = self.t;
        let t1 = t0 + dt;
        if self.error.is_some() {
            self.t = t1;
            return MobilityUpdate::empty(t1);
        }
        if self.conn.is_none()
            && let Err(e) = self.launch(dt)
        {
            self.error = Some(e);
            self.t = t1;
            return MobilityUpdate::empty(t1);
        }
        self.insert_trips(ctx, t0, t1);
        self.release_stops(t0);
        if !self.queued.is_empty() {
            let q = std::mem::take(&mut self.queued);
            if let Some(c) = self.conn.as_mut()
                && let Err(e) = c.set_batch(&q)
            {
                self.error = Some(e.to_string());
            }
        }
        let stepped = self
            .conn
            .as_mut()
            .map(|c| c.step_to(t1 as f64 / 1e9).map_err(|e| e.to_string()));
        if let Some(Err(e)) = stepped {
            self.error = Some(e);
            self.t = t1;
            return MobilityUpdate::empty(t1);
        }
        self.t = t1;
        self.counters.steps += 1;
        match self.read_state(t1, dt.as_secs_f64()) {
            Ok(u) => u,
            Err(e) => {
                self.error = Some(e);
                MobilityUpdate::empty(t1)
            }
        }
    }

    fn command_inner(&mut self, ctx: &mut dyn MobCtx, cmd_: MobilityCommand) {
        let world = ctx.world();
        match cmd_ {
            MobilityCommand::SpeedCap { actor, v_mps } => {
                if let Some(v) = self.by_actor.get(&actor) {
                    // SUMO has no "lift the cap"; the vType's own maximum (55.56 m/s for
                    // DEFAULT_VEHTYPE, SUMO vType defaults) is the uncapped value.
                    self.queued.push((cmd::SET_VEHICLE, var::MAXSPEED, v.clone(), Content::new().double(v_mps.unwrap_or(55.56))));
                }
            }
            MobilityCommand::Stop { actor, until } => {
                if let Some(v) = self.by_actor.get(&actor) {
                    self.queued.push((cmd::SET_VEHICLE, var::SPEED, v.clone(), Content::new().double(0.0)));
                    self.stops.insert(actor, until);
                }
            }
            MobilityCommand::Despawn { actor } => {
                if let Some(v) = self.by_actor.get(&actor) {
                    // REMOVE_VAPORIZED = 3 (tools/traci/constants.py).
                    self.queued.push((cmd::SET_VEHICLE, var::REMOVE, v.clone(), Content::new().byte(3)));
                }
            }
            MobilityCommand::Reroute { actor, to } => {
                if let (Some(v), Some(e)) = (self.by_actor.get(&actor), self.ids.edge_of_lane(world, to)) {
                    self.queued.push((cmd::SET_VEHICLE, var::CHANGETARGET, v.clone(), Content::new().str(e)));
                }
            }
            MobilityCommand::Closure { lane, closed } => {
                if let Some(l) = self.ids.sumo_lane.get(&lane) {
                    let (v, list) = if closed {
                        (var::LANE_DISALLOWED, vec!["all".to_string()])
                    } else {
                        (var::LANE_ALLOWED, vec!["all".to_string()])
                    };
                    self.queued.push((cmd::SET_LANE, v, l.clone(), Content::new().str_list(&list)));
                }
            }
            _ => {}
        }
    }
}

/// The set of world lanes SUMO currently reports vehicles on, for tests and reports.
pub fn occupied_lanes(update: &MobilityUpdate) -> BTreeSet<LaneId> {
    update.states.iter().filter_map(|(_, k)| k.lane.map(|l| l.lane)).collect()
}
