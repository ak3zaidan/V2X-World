//! Traffic cross-validation against SUMO.
//!
//! # Method
//!
//! 1. The network is SUMO's: `netgenerate --grid` for the grid, `netconvert --osm-files`
//!    on a Midtown Manhattan extract of `worlds/cache/manhattan.osm.xml`. The world is
//!    imported from that same `net.xml`, so both simulators drive identical lane geometry
//!    and [`SumoIds`] maps every lane one to one.
//! 2. One trip list is drawn (exponential headways at the stated rate, origins and
//!    destinations uniform over passenger-admitting normal edges), and every trip is routed
//!    once by this engine's free-flow Dijkstra. **Both simulators drive those routes**:
//!    this engine through [`Demand::planned_route`], SUMO through [`SumoMobility`], which
//!    inserts each trip with `route.add` + `vehicle.add`. Differences are therefore in the
//!    driving models, not in routing.
//! 3. Both runs are measured by the same [`Observer`] on the published states, so the
//!    measurement code cannot differ either.
//!
//! # Measures
//!
//! * **Network flow, density, space-mean speed** by Edie's generalised definitions over
//!   the measurement window: `q = VKT / (L·T)`, `k = VHT / (L·T)`, `v = VKT / VHT`, with
//!   `L` the world's total lane length (Edie 1963, "Discussion of traffic stream
//!   measurements and definitions", Proc. 2nd Int. Symp. on the Theory of Traffic Flow).
//! * **Travel time** from actual insertion to arrival, per trip, paired by trip.
//! * **Queues**: vehicles below 0.1 m/s, SUMO's halting threshold (SUMO documentation,
//!   "Simulation/Output/Lane Area Detectors (E2)", `jamThreshold`/halting speed 0.1 m/s).
//! * **Per-edge hourly entries**, compared by the GEH statistic
//!   `GEH = sqrt(2 (M − C)² / (M + C))`, with the customary acceptance "GEH < 5 on 85 % of
//!   links" (UK Highways Agency, Design Manual for Roads and Bridges, Vol. 12 §2 Part 1).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use v2xw_core::card::{Family, ModelCard};
use v2xw_core::ids::{ActorId, EdgeId, LaneId};
use v2xw_core::model::Model;
use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, SimTime};
use v2xw_mobility::ctx::MobCtx;
use v2xw_mobility::routing::dijkstra::DijkstraParams;
use v2xw_mobility::traits::{Demand, Mobility};
use v2xw_mobility::views::{DespawnCause, MobilityUpdate, TripRequest};
use v2xw_mobility::{
    Dijkstra, EngineParams, FreeFlowCost, MobilityCtx, NativeMobility, VehicleClass,
};
use v2xw_world::World;
use v2xw_world::sumo::{SumoImportReport, SumoOptions, import_sumo_net};

use crate::SumoIds;
use crate::sumo::{SumoConfig, SumoMobility};

/// A trip list replayed with fixed routes.
pub struct ListDemand {
    card: ModelCard,
    trips: Vec<(TripRequest, Vec<LaneId>)>,
    by_seq: BTreeMap<u64, usize>,
    cursor: usize,
}

impl ListDemand {
    /// The demand for `trips` (sorted by time) and their routes.
    pub fn new(trips: Vec<(TripRequest, Vec<LaneId>)>) -> Self {
        let by_seq = trips
            .iter()
            .enumerate()
            .map(|(i, (t, _))| (t.seq, i))
            .collect();
        Self {
            card: ModelCard::new(
                "mobility/demand/fixed-list",
                Family::Mobility,
                "1.0.0",
                "A fixed list of trips with fixed routes, for cross-validation.",
            ),
            trips,
            by_seq,
            cursor: 0,
        }
    }
}

impl Model for ListDemand {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Demand for ListDemand {
    fn spawns_in(
        &mut self,
        _ctx: &mut dyn MobCtx,
        _from: SimTime,
        to: SimTime,
    ) -> Vec<TripRequest> {
        let mut out = Vec::new();
        while let Some((t, _)) = self.trips.get(self.cursor) {
            if t.t >= to {
                break;
            }
            out.push(t.clone());
            self.cursor += 1;
        }
        out
    }

    fn planned_route(&self, _world: &World, trip: &TripRequest) -> Option<Vec<LaneId>> {
        self.by_seq.get(&trip.seq).map(|i| self.trips[*i].1.clone())
    }
}

/// A deterministic generator (SplitMix64, Steele, Lea & Flood, OOPSLA 2014).
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Draws and routes trips: exponential headways at `rate_vph` for `demand_s` seconds.
pub fn make_trips(
    world: &World,
    ids: &SumoIds,
    rate_vph: f64,
    demand_s: f64,
    seed: u64,
) -> Vec<(TripRequest, Vec<LaneId>)> {
    let class = VehicleClass::Passenger;
    let mask = class.class_mask();
    let edges: Vec<LaneId> = ids
        .lanes_of
        .values()
        .filter_map(|ls| ls.iter().copied().find(|l| world.lane(*l).admits(mask)))
        .filter(|l| world.lane(*l).length_m > 25.0 && world.lane(*l).kind.is_motorised())
        .collect();
    let mut rng = SplitMix(seed);
    let router = Dijkstra::new(DijkstraParams::for_classes(mask));
    let cost = FreeFlowCost::new(world);
    let mut out = Vec::new();
    let mut t = 0.0;
    let mut seq = 0u64;
    if edges.len() < 2 {
        return out;
    }
    loop {
        t += -(1.0 - rng.unit()).ln() * 3600.0 / rate_vph;
        if t >= demand_s {
            break;
        }
        let o = edges[(rng.next() % edges.len() as u64) as usize];
        let d = edges[(rng.next() % edges.len() as u64) as usize];
        if world.lane(o).edge == world.lane(d).edge {
            continue;
        }
        let at = (t * 1e3).round() as SimTime * 1_000_000;
        let Some(route) = router.search(world, o, d, at, &cost) else {
            continue;
        };
        // Short trips say little about either model.
        if route.length_m < 200.0 {
            continue;
        }
        out.push((
            TripRequest {
                seq,
                t: at,
                origin: o,
                origin_s_m: 0.0,
                destination: d,
                class,
                desired_speed_mps: class.spec().desired_speed_mps(),
            },
            route.lanes,
        ));
        seq += 1;
    }
    out
}

/// What one run measured.
#[derive(Debug, Clone, Default)]
pub struct Measured {
    /// Which simulator.
    pub label: String,
    /// Trips offered.
    pub offered: u64,
    /// Trips that entered the network.
    pub inserted: u64,
    /// Trips that arrived.
    pub completed: u64,
    /// Travel time per completed trip, by trip seq.
    pub travel_s: BTreeMap<u64, f64>,
    /// Insertion delay per inserted trip.
    pub insert_delay_s: Vec<f64>,
    /// Vehicle-km travelled in the window.
    pub vkt: f64,
    /// Vehicle-hours in the window.
    pub vht: f64,
    /// Window length, s.
    pub window_s: f64,
    /// Network lane length, km.
    pub lane_km: f64,
    /// Mean vehicles halting in the window.
    pub halting_mean: f64,
    /// Max vehicles halting at once.
    pub halting_max: u64,
    /// Halting vehicle-seconds over the whole run, per completed trip.
    pub wait_per_trip_s: f64,
    /// Entries per normal edge in the window.
    pub edge_entries: BTreeMap<EdgeId, u64>,
    /// Time-weighted mean speed per edge in the window: (sum v dt, sum dt).
    pub edge_speed: BTreeMap<EdgeId, (f64, f64)>,
    /// 60-s bins: (vehicles, mean speed, halting).
    pub bins: Vec<(f64, f64, f64)>,
    /// What went wrong, if anything.
    pub error: Option<String>,
    /// Wall-clock seconds the run took.
    pub wall_s: f64,
    /// Stops over the whole run: a vehicle that had moved faster than 1 m/s falling below
    /// the halting threshold.
    pub stops: u64,
    /// Lane changes over the whole run (lane index changing within one edge).
    pub lane_changes: u64,
    /// Vehicle-km over the whole run.
    pub vkt_all: f64,
    /// Junction-internal lanes over the whole run, moving vehicles only: (sum v dt, sum dt).
    pub junction_speed: (f64, f64),
    /// Halting vehicle-seconds by place: within 30 m of a normal lane's end (a queue at
    /// the stop line), elsewhere on a normal lane, inside a junction.
    pub halt_where_s: [f64; 3],
    /// Vehicles still in the network when the run ended.
    pub left_in_network: u64,
}

impl Measured {
    /// Edie flow, veh/h per lane.
    pub fn flow(&self) -> f64 {
        self.vkt / (self.lane_km * self.window_s / 3600.0).max(1e-9)
    }
    /// Edie density, veh/km per lane.
    pub fn density(&self) -> f64 {
        self.vht / (self.lane_km * self.window_s / 3600.0).max(1e-9)
    }
    /// Space-mean speed, km/h.
    pub fn speed_kmh(&self) -> f64 {
        self.vkt / self.vht.max(1e-9)
    }
    /// Stops per inserted trip.
    pub fn stops_per_trip(&self) -> f64 {
        self.stops as f64 / self.inserted.max(1) as f64
    }

    /// Lane changes per vehicle-km over the whole run.
    pub fn lane_changes_per_km(&self) -> f64 {
        self.lane_changes as f64 / self.vkt_all.max(1e-9)
    }

    /// Mean speed of moving vehicles on junction-internal lanes, km/h.
    pub fn junction_kmh(&self) -> f64 {
        3.6 * self.junction_speed.0 / self.junction_speed.1.max(1e-9)
    }

    /// Mean travel time, s.
    pub fn mean_tt(&self) -> f64 {
        mean(self.travel_s.values().copied())
    }
    /// A travel-time percentile.
    pub fn tt_pct(&self, p: f64) -> f64 {
        let mut v: Vec<f64> = self.travel_s.values().copied().collect();
        v.sort_by(f64::total_cmp);
        if v.is_empty() {
            return f64::NAN;
        }
        v[((v.len() - 1) as f64 * p).round() as usize]
    }
}

fn mean(it: impl Iterator<Item = f64>) -> f64 {
    let (s, n) = it.fold((0.0, 0u64), |(s, n), x| (s + x, n + 1));
    if n == 0 { f64::NAN } else { s / n as f64 }
}

/// The halting threshold, m/s (SUMO E2 detector default; see the module docs).
pub const HALTING_MPS: f64 = 0.1;

/// Accumulates [`Measured`] from published updates.
pub struct Observer<'a> {
    world: &'a World,
    ids: &'a SumoIds,
    m: Measured,
    requested: BTreeMap<u64, f64>,
    spawn: BTreeMap<ActorId, (u64, f64)>,
    last_edge: BTreeMap<ActorId, EdgeId>,
    window: (f64, f64),
    halting_samples: (f64, f64),
    wait_total: f64,
    bin: BTreeMap<u64, (f64, f64, f64, f64)>,
    last_lane: BTreeMap<ActorId, (EdgeId, u8)>,
    moving: BTreeMap<ActorId, bool>,
    in_network: u64,
}

impl<'a> Observer<'a> {
    /// An observer over `[window.0, window.1)` seconds.
    pub fn new(
        world: &'a World,
        ids: &'a SumoIds,
        trips: &[(TripRequest, Vec<LaneId>)],
        window: (f64, f64),
        label: &str,
    ) -> Self {
        let lane_km = world
            .roads
            .lanes()
            .iter()
            .filter(|l| l.kind.is_motorised())
            .map(|l| l.length_m)
            .sum::<f64>()
            / 1000.0;
        Self {
            world,
            ids,
            m: Measured {
                label: label.to_string(),
                offered: trips.len() as u64,
                window_s: window.1 - window.0,
                lane_km,
                ..Measured::default()
            },
            requested: trips
                .iter()
                .map(|(t, _)| (t.seq, t.t as f64 / 1e9))
                .collect(),
            spawn: BTreeMap::new(),
            last_edge: BTreeMap::new(),
            window,
            halting_samples: (0.0, 0.0),
            wait_total: 0.0,
            bin: BTreeMap::new(),
            last_lane: BTreeMap::new(),
            moving: BTreeMap::new(),
            in_network: 0,
        }
    }

    /// One step's update; `seq_of` maps a spawn to its trip seq.
    pub fn observe(&mut self, u: &MobilityUpdate, dt_s: f64) {
        let t1 = u.t as f64 / 1e9;
        for s in &u.spawned {
            self.m.inserted += 1;
            self.spawn.insert(s.actor, (s.seq, t1));
            if let Some(r) = self.requested.get(&s.seq) {
                self.m.insert_delay_s.push((t1 - r).max(0.0));
            }
        }
        for (a, cause) in &u.despawned {
            if let Some((seq, t0)) = self.spawn.remove(a)
                && *cause == DespawnCause::TripComplete
            {
                self.m.completed += 1;
                self.m.travel_s.insert(seq, t1 - t0);
            }
            self.last_edge.remove(a);
            self.last_lane.remove(a);
            self.moving.remove(a);
        }
        let in_window = t1 > self.window.0 && t1 <= self.window.1;
        let mut halting = 0u64;
        let mut speed_sum = 0.0;
        for (a, k) in &u.states {
            let v = k.vel.x.hypot(k.vel.y);
            speed_sum += v;
            self.m.vkt_all += v * dt_s / 1000.0;
            let lane = k.lane.map(|l| (self.world.lane(l.lane), l.s_m));
            if v < HALTING_MPS {
                halting += 1;
                self.wait_total += dt_s;
                // SUMO's internal lanes have no world lane, so no lane is inside a junction.
                let place = match lane {
                    None => 2,
                    Some((l, _)) if l.junction.is_some() => 2,
                    Some((l, s)) if l.length_m - s <= 30.0 => 0,
                    _ => 1,
                };
                self.m.halt_where_s[place] += dt_s;
                if self.moving.insert(*a, false) == Some(true) {
                    self.m.stops += 1;
                }
            } else if v > 1.0 {
                self.moving.insert(*a, true);
            }
            if lane.is_none_or(|(l, _)| l.junction.is_some()) {
                if v >= HALTING_MPS {
                    self.m.junction_speed.0 += v * dt_s;
                    self.m.junction_speed.1 += dt_s;
                }
            } else if let Some((l, _)) = lane
                && let Some((e, i)) = self.last_lane.insert(*a, (l.edge, l.index))
                && e == l.edge
                && i != l.index
            {
                self.m.lane_changes += 1;
            }
            let edge = k
                .lane
                .map(|l| self.world.lane(l.lane).edge)
                .filter(|e| self.ids.edge_id.contains_key(e));
            if let Some(e) = edge {
                if self.last_edge.get(a) != Some(&e) {
                    self.last_edge.insert(*a, e);
                    if in_window {
                        *self.m.edge_entries.entry(e).or_default() += 1;
                    }
                }
                if in_window {
                    let es = self.m.edge_speed.entry(e).or_default();
                    es.0 += v * dt_s;
                    es.1 += dt_s;
                }
            }
            if in_window {
                self.m.vkt += v * dt_s / 1000.0;
                self.m.vht += dt_s / 3600.0;
            }
        }
        if in_window {
            self.halting_samples.0 += halting as f64 * dt_s;
            self.halting_samples.1 += dt_s;
            self.m.halting_max = self.m.halting_max.max(halting);
        }
        self.in_network = u.states.len() as u64;
        let b = self.bin.entry((t1 / 60.0) as u64).or_default();
        b.0 += u.states.len() as f64 * dt_s;
        b.1 += speed_sum * dt_s;
        b.2 += halting as f64 * dt_s;
        b.3 += dt_s;
    }

    /// The result.
    pub fn finish(mut self) -> Measured {
        self.m.left_in_network = self.in_network;
        self.m.halting_mean = self.halting_samples.0 / self.halting_samples.1.max(1e-9);
        self.m.wait_per_trip_s = self.wait_total / (self.m.inserted.max(1) as f64);
        self.m.bins = self
            .bin
            .values()
            .map(|(n, v, h, t)| {
                let n_mean = n / t.max(1e-9);
                (n_mean, if *n > 0.0 { v / n } else { 0.0 }, h / t.max(1e-9))
            })
            .collect();
        self.m
    }
}

/// One study: a world and its SUMO network.
pub struct Study {
    /// Name.
    pub name: String,
    /// The world, imported from `net`.
    pub world: World,
    /// The SUMO network.
    pub net: PathBuf,
    /// The importer's report.
    pub import: SumoImportReport,
    /// SUMO ids.
    pub ids: SumoIds,
    /// How the network was made.
    pub provenance: String,
}

fn run_tool(cmd: &mut Command) -> Result<String, String> {
    if let Some(home) = crate::sumo_home() {
        cmd.env("SUMO_HOME", home);
    }
    let o = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    if o.status.success() {
        Ok(log)
    } else {
        Err(format!("{cmd:?} failed: {log}"))
    }
}

fn import(name: &str, net: PathBuf, provenance: String) -> Result<Study, String> {
    let opts = SumoOptions {
        pedestrian_layer: false,
        ..SumoOptions::default()
    }
    .imported_at("1970-01-01T00:00:00Z");
    let (world, report) =
        import_sumo_net(&net, &opts).map_err(|e| format!("import {}: {e}", net.display()))?;
    let ids = SumoIds::new(&world);
    Ok(Study {
        name: name.to_string(),
        world,
        net,
        import: report,
        ids,
        provenance,
    })
}

/// A 5 × 5 signalised grid, 200 m blocks, 2 lanes per direction, 50 km/h.
///
/// # Errors
/// netgenerate or the import failing.
pub fn grid_study(home: &Path, dir: &Path) -> Result<Study, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let net = dir.join("grid.net.xml");
    let args = [
        "--grid",
        "--grid.number",
        "5",
        "--grid.length",
        "200",
        "--default.lanenumber",
        "2",
        "--default.speed",
        "13.89",
        "--default-junction-type",
        "traffic_light",
        "--no-turnarounds",
        "true",
        "--seed",
        "1",
    ];
    run_tool(
        Command::new(crate::sumo_tool(home, "netgenerate"))
            .args(args)
            .arg("-o")
            .arg(&net),
    )?;
    import("grid", net, format!("netgenerate {}", args.join(" ")))
}

/// The Midtown Manhattan extract: the `bbox` (min lon, min lat, max lon, max lat) of
/// `osm`, passenger roads only.
///
/// # Errors
/// netconvert or the import failing.
pub fn manhattan_study(
    home: &Path,
    osm: &Path,
    dir: &Path,
    bbox: [f64; 4],
) -> Result<Study, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let net = dir.join("manhattan.net.xml");
    let b = format!("{},{},{},{}", bbox[0], bbox[1], bbox[2], bbox[3]);
    let args = [
        "--keep-edges.in-geo-boundary",
        &b,
        "--keep-edges.by-vclass",
        "passenger",
        "--remove-edges.isolated",
        "true",
        "--geometry.remove",
        "true",
        "--junctions.join",
        "true",
        "--tls.guess-signals",
        "true",
        "--tls.join",
        "true",
        "--no-turnarounds",
        "true",
        "--ramps.guess",
        "false",
    ];
    run_tool(
        Command::new(crate::sumo_tool(home, "netconvert"))
            .arg("--osm-files")
            .arg(osm)
            .args(args)
            .arg("-o")
            .arg(&net),
    )?;
    import(
        "manhattan",
        net,
        format!(
            "netconvert --osm-files manhattan.osm.xml {}",
            args.join(" ")
        ),
    )
}

/// The run's timing.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Mobility step.
    pub step: Duration,
    /// Seconds of demand.
    pub demand_s: f64,
    /// Total seconds simulated.
    pub total_s: f64,
    /// Measurement window start (warm-up end).
    pub warm_s: f64,
}

/// Runs this engine's native mobility on the trips.
pub fn run_native(
    study: &Study,
    trips: &[(TripRequest, Vec<LaneId>)],
    timing: Timing,
    seed: u64,
) -> Measured {
    let started = std::time::Instant::now();
    let world = &study.world;
    let rng = RngRegistry::new(seed);
    // A scenario's shipped defaults (`v2xw_engine::wiring::native_mobility` with no
    // `traffic` section and no highway preset), not the bare engine defaults a unit test
    // sees: strategic lane choice on, gridlock resolution at 300 s (SUMO's
    // `--time-to-teleport` default, which the SUMO runs here keep too).
    let mut engine = NativeMobility::new(v2xw_mobility::rules::TrafficRules::of_highway_preset(None).apply(EngineParams {
        step: timing.step,
        strategic_lane_choice: true,
        time_to_teleport_s: 300.0,
        ..EngineParams::default()
    }));
    let mut obs = Observer::new(
        world,
        &study.ids,
        trips,
        (timing.warm_s, timing.demand_s),
        "this engine (native)",
    );
    {
        let mut ctx = MobilityCtx::new(0, world, &rng);
        if let Err(e) = engine.init(&mut ctx, Box::new(ListDemand::new(trips.to_vec()))) {
            let mut m = obs.finish();
            m.error = Some(e.to_string());
            return m;
        }
    }
    let horizon = (timing.total_s * 1e9) as SimTime;
    let mut t = 0;
    let dt_s = timing.step.as_secs_f64();
    // Diagnostics: `V2XW_XVAL_TRACE=<trip seq>` prints that trip's lanes and halts.
    let trace: Option<u64> = std::env::var("V2XW_XVAL_TRACE").ok().and_then(|v| v.parse().ok());
    let mut traced: Option<ActorId> = None;
    let mut last: Option<(LaneId, bool)> = None;
    while t < horizon {
        let mut ctx = MobilityCtx::new(t, world, &rng);
        let u = engine.step(&mut ctx, timing.step);
        t = u.t;
        if let Some(q) = trace {
            if let Some(s) = u.spawned.iter().find(|s| s.seq == q) {
                traced = Some(s.actor);
                if let Some((_, r)) = trips.iter().find(|(t, _)| t.seq == q) {
                    let e: Vec<String> = r.iter().map(|l| format!("{}", world.lane(*l).edge)).collect();
                    eprintln!("  trace route {}", e.join(" "));
                }
            }
            if let Some(a) = traced
                && let Some((_, c)) = u.despawned.iter().find(|(x, _)| *x == a)
            {
                eprintln!("  trace t={:.1} despawned {c:?}", t as f64 / 1e9);
            }
            if let Some(a) = traced
                && let Some((_, k)) = u.states.iter().find(|(x, _)| *x == a)
            {
                let v = k.vel.x.hypot(k.vel.y);
                let lane = k.lane.map_or(LaneId::new(u32::MAX), |l| l.lane);
                let now = (lane, v < HALTING_MPS);
                if last != Some(now) {
                    let (len, j, e) = k.lane.map_or((0.0, false, EdgeId::new(u32::MAX)), |l| {
                        let w = world.lane(l.lane);
                        (w.length_m, w.junction.is_some(), w.edge)
                    });
                    eprintln!(
                        "  trace t={:.1} lane={} edge={} s={:.1}/{len:.1} junction={j} v={v:.2} halt={}",
                        t as f64 / 1e9, lane, e, k.lane.map_or(0.0, |l| l.s_m), now.1
                    );
                    last = Some(now);
                }
            }
        }
        obs.observe(&u, dt_s);
    }
    let mut m = obs.finish();
    if engine.dropped_trips() > 0 {
        m.error = Some(format!(
            "{} trips dropped by the engine",
            engine.dropped_trips()
        ));
    }
    m.wall_s = started.elapsed().as_secs_f64();
    m
}

/// Runs SUMO over TraCI on the same trips and routes.
pub fn run_sumo(
    study: &Study,
    home: &Path,
    trips: &[(TripRequest, Vec<LaneId>)],
    timing: Timing,
    seed: u64,
    label: &str,
    extra: &[&str],
) -> (Measured, Option<String>) {
    let started = std::time::Instant::now();
    let world = &study.world;
    let rng = RngRegistry::new(seed);
    let mut cfg = SumoConfig::new(home, &study.net);
    cfg.shift_m = study.import.shift_m;
    cfg.seed = seed;
    cfg.extra_args = extra.iter().map(|s| (*s).to_string()).collect();
    let mut sumo = SumoMobility::new(cfg, study.ids.clone());
    let mut obs = Observer::new(
        world,
        &study.ids,
        trips,
        (timing.warm_s, timing.demand_s),
        label,
    );
    {
        let mut ctx = MobilityCtx::new(0, world, &rng);
        let _ = sumo.init(&mut ctx, Box::new(ListDemand::new(trips.to_vec())));
    }
    let horizon = (timing.total_s * 1e9) as SimTime;
    let mut t = 0;
    let dt_s = timing.step.as_secs_f64();
    while t < horizon && sumo.error.is_none() {
        let mut ctx = MobilityCtx::new(t, world, &rng);
        let u = sumo.step(&mut ctx, timing.step);
        t = u.t;
        obs.observe(&u, dt_s);
    }
    let version = sumo.sumo_version().map(str::to_string);
    let mut m = obs.finish();
    m.error = sumo.error.clone();
    if sumo.counters.refused > 0 {
        let note = format!("{} trips refused by SUMO", sumo.counters.refused);
        m.error = Some(m.error.map_or(note.clone(), |e| format!("{e}; {note}")));
    }
    sumo.shutdown();
    m.wall_s = started.elapsed().as_secs_f64();
    (m, version)
}

/// GEH between two hourly counts.
pub fn geh(m: f64, c: f64) -> f64 {
    if m + c <= 0.0 {
        0.0
    } else {
        (2.0 * (m - c).powi(2) / (m + c)).sqrt()
    }
}

/// Per-edge comparison of `a` against `b`: (share of edges with GEH < 5, edges compared,
/// Pearson r of edge mean speeds, mean |Δ speed| km/h).
pub fn edge_agreement(a: &Measured, b: &Measured) -> (f64, usize, f64, f64) {
    let hours = a.window_s / 3600.0;
    let edges: Vec<&EdgeId> = a
        .edge_entries
        .keys()
        .chain(b.edge_entries.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut ok = 0;
    for e in &edges {
        let m = *a.edge_entries.get(e).unwrap_or(&0) as f64 / hours;
        let c = *b.edge_entries.get(e).unwrap_or(&0) as f64 / hours;
        if geh(m, c) < 5.0 {
            ok += 1;
        }
    }
    let pairs: Vec<(f64, f64)> = a
        .edge_speed
        .iter()
        .filter_map(|(e, (sv, st))| {
            let (bv, bt) = b.edge_speed.get(e)?;
            (*st > 30.0 && *bt > 30.0).then(|| (sv / st * 3.6, bv / bt * 3.6))
        })
        .collect();
    let n = pairs.len() as f64;
    let (mx, my) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let cov: f64 = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let vx: f64 = pairs.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let vy: f64 = pairs.iter().map(|p| (p.1 - my).powi(2)).sum();
    let r = cov / (vx * vy).sqrt();
    let mad = pairs.iter().map(|p| (p.0 - p.1).abs()).sum::<f64>() / n;
    (
        if edges.is_empty() {
            f64::NAN
        } else {
            ok as f64 / edges.len() as f64
        },
        edges.len(),
        r,
        mad,
    )
}

/// Paired travel-time comparison: (trips completed by both, mean ratio a/b, median ratio).
pub fn paired_tt(a: &Measured, b: &Measured) -> (usize, f64, f64) {
    let mut ratios: Vec<f64> = a
        .travel_s
        .iter()
        .filter_map(|(k, ta)| b.travel_s.get(k).map(|tb| ta / tb.max(1e-9)))
        .collect();
    ratios.sort_by(f64::total_cmp);
    let n = ratios.len();
    if n == 0 {
        return (0, f64::NAN, f64::NAN);
    }
    (n, ratios.iter().sum::<f64>() / n as f64, ratios[n / 2])
}

/// The export round trip on a procedural world: export → netconvert → import, and what
/// survived.
#[derive(Debug, Clone)]
pub struct RoundTrip {
    /// Normal lanes before.
    pub lanes_before: usize,
    /// Normal lanes after.
    pub lanes_after: usize,
    /// Normal-lane length before, km.
    pub km_before: f64,
    /// After.
    pub km_after: f64,
    /// Signalised junctions before / after.
    pub signals: (usize, usize),
    /// Edges before / after.
    pub edges: (usize, usize),
}

/// Exports this crate's procedural TR 36.885 grid to SUMO and imports it back.
///
/// # Errors
/// World build, netconvert or import failing.
pub fn export_round_trip(home: &Path, dir: &Path) -> Result<RoundTrip, String> {
    let world = v2xw_world::procedural::grid(
        &v2xw_world::procedural::GridParams::tr36885_urban().with_size(3, 3),
        &v2xw_world::ImportOptions::default().imported_at("1970-01-01T00:00:00Z"),
    )
    .map_err(|e| e.to_string())?;
    let ids = SumoIds::new(&world);
    let (net, _log) = crate::netexport::export_net(
        &world,
        &ids,
        &crate::sumo_tool(home, "netconvert"),
        dir,
        "procedural-grid",
    )?;
    let back = import("roundtrip", net, String::new())?;
    let stats = |w: &World| {
        let normal: Vec<_> = w
            .roads
            .lanes()
            .iter()
            .filter(|l| l.junction.is_none() && l.kind.is_motorised())
            .collect();
        let sig = w
            .roads
            .junctions()
            .iter()
            .filter(|j| {
                matches!(
                    j.control,
                    v2xw_world::model::JunctionControl::Signalised { .. }
                )
            })
            .count();
        let edges = SumoIds::new(w).edge_id.len();
        (
            normal.len(),
            normal.iter().map(|l| l.length_m).sum::<f64>() / 1000.0,
            sig,
            edges,
        )
    };
    let a = stats(&world);
    let b = stats(&back.world);
    Ok(RoundTrip {
        lanes_before: a.0,
        lanes_after: b.0,
        km_before: a.1,
        km_after: b.1,
        signals: (a.2, b.2),
        edges: (a.3, b.3),
    })
}
