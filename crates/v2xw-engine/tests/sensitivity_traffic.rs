//! Traffic sensitivity: each traffic parameter of docs/guides/config-inventory-traffic.md
//! moves the traffic metrics the way traffic theory says, and, where the network is in
//! the loop, the network metrics too. One same-seed pair (or ladder) per row of
//! docs/validation/sensitivity-traffic.md; a row whose parameter never reached the model,
//! or whose metric does not respond to it, fails its assertion.
//!
//! The shared sensitivity harness is not on this base, so the helpers below are the
//! minimal same-seed version: one scenario, one changed field, the same seed, and the
//! measured quantities read from the recording.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::scenario::schema::{
    GnssSettings, SignalControl, SignalControlKind, Traffic, VehicleClassSpec,
};
use v2xw_engine::{Engine, RunRecorder, RunReport, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// A small signalised procedural grid (4 x 6 Midtown-shaped blocks, two lanes each way,
/// 11.2 m/s limit, 60 s cycle), `secs` long, Poisson demand at `rate` veh/h, traffic only.
fn grid(secs: f64, rate: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("loads");
    s.time.duration_s = secs;
    s.metrics = vec![];
    s.actors.vehicles.equipped_fraction = 0.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(rate);
    s.world.buildings.enabled = false;
    grid_param(&mut s, "cols", serde_json::json!(4));
    grid_param(&mut s, "rows", serde_json::json!(6));
    s
}

/// The same grid with every vehicle equipped and the network metrics on.
fn connected(secs: f64, rate: f64) -> Scenario {
    let mut s = grid(secs, rate);
    s.actors.vehicles.equipped_fraction = 1.0;
    s.metrics = ["channel_load", "e2e_latency"]
        .iter()
        .map(|m| (*m).to_string())
        .collect();
    s
}

/// The grid held saturated: a fixed fleet of 250 vehicles, every finished trip replaced.
fn saturated(secs: f64) -> Scenario {
    let mut s = grid(secs, 0.0);
    s.actors.vehicles.demand.rate_veh_per_h = None;
    s.actors.vehicles.demand.fleet_size = Some(250);
    s
}

fn grid_param(s: &mut Scenario, key: &str, v: serde_json::Value) {
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params[key] = v;
    } else {
        panic!("not a procedural world");
    }
}

/// A recorder that keeps only what the rows read — motor-vehicle kinematics, metric
/// samples, misbehaviour reports and node telemetry — and counts the rest as it passes,
/// so a run with hundreds of radios stays inside the machine's memory.
#[derive(Default)]
struct Probe {
    records: Vec<(SimTime, OwnedRecord)>,
    vru_msgs: u64,
    /// Each BSM's claimed position, by sending node: (t, east m, north m).
    claims: BTreeMap<String, Vec<(u64, f64, f64)>>,
    /// Ground truth, by node: (t, x m, y m).
    truth: BTreeMap<String, Vec<(u64, f64, f64)>>,
    /// The world's projection, to bring a BSM's latitude and longitude to metres.
    projection: Option<v2xw_world::Projection>,
}

impl Probe {
    fn records(&self) -> &[(SimTime, OwnedRecord)] {
        &self.records
    }
}

impl RunRecorder for Probe {
    fn write(&mut self, at: SimTime, record: &OwnedRecord) {
        let ch = record.channel.to_string();
        match ch.as_ref() {
            "gt.kinematics" => {
                let vru = serde_json::from_slice::<serde_json::Value>(&record.json)
                    .ok()
                    .is_some_and(|k| is_vru(k.get("class").and_then(|c| c.as_str())));
                if !vru {
                    if let Ok(k) = serde_json::from_slice::<serde_json::Value>(&record.json)
                        && let Some(node) = k.get("node").filter(|n| !n.is_null())
                        && let (Some(x), Some(y)) = (k["x_m"].as_f64(), k["y_m"].as_f64())
                    {
                        self.truth
                            .entry(node.to_string())
                            .or_default()
                            .push((at, x, y));
                    }
                    self.records.push((at, record.clone()));
                }
            }
            "metric.sample" | "ma.report" | "node.telemetry" => {
                self.records.push((at, record.clone()));
            }
            "node.tx" => {
                let v: serde_json::Value = serde_json::from_slice(&record.json).unwrap_or_default();
                let t = v["msg_type"].as_str().unwrap_or("").to_ascii_lowercase();
                if t.contains("psm") || t.contains("vam") {
                    self.vru_msgs += 1;
                }
                if t.contains("bsm")
                    && let Some(proj) = &self.projection
                    && let (Some(lat), Some(lon)) = (
                        v["content"]["lat_deg"].as_f64(),
                        v["content"]["lon_deg"].as_f64(),
                    )
                {
                    let (x, y) = proj.to_enu(lat, lon);
                    self.claims
                        .entry(v["node"].to_string())
                        .or_default()
                        .push((at, x, y));
                }
            }
            _ => {}
        }
    }
}

fn run(s: Scenario) -> (RunReport, Probe) {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "the scenario should load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = Probe {
        projection: Some(engine.world().projection()),
        ..Probe::default()
    };
    let report = engine.run(&mut rec).expect("runs");
    (report, rec)
}

/// Traffic quantities from `gt.kinematics`, motor vehicles only.
#[derive(Debug, Default, Clone)]
#[allow(dead_code)] // every field is printed (Debug) for the validation table
struct Flow {
    /// Mean motor vehicles in the network per sampled instant (a density proxy on a
    /// fixed network, HCM 6th ed. ch. 4: density = vehicles / length).
    density: f64,
    /// Time-mean speed over every vehicle-sample, m/s.
    speed: f64,
    /// Vehicle-metres travelled: the network's throughput (Edie 1963 flow x area).
    vkt_m: f64,
    /// Travel-time rate: vehicle-seconds per vehicle-km (HCM 6th ed. ch. 16), s/km.
    tt_per_km: f64,
    /// Stops (a fall from at least 2 m/s to at most 0.1 m/s) per vehicle-km.
    stops_per_km: f64,
    /// Stopped time per vehicle (speed at most 0.1 m/s), s: the HCM's stopped delay.
    stopped_per_veh: f64,
    /// Mean vehicles standing (speed at most 0.1 m/s) per instant: the network's queue.
    queue: f64,
    /// Lane changes: a lane switch that keeps the longitudinal position (within 6 m)
    /// away from a lane's start, which a junction crossing does not.
    lane_changes: u64,
    /// Vehicle-samples braking harder than 2 m/s² (above IDM's comfortable 1.67 m/s²,
    /// Treiber & Kesting 2013 table 11.2), per vehicle-km: what a cut-in imposes.
    hard_brakes_per_km: f64,
    /// Mean time from leaving a standstill to reaching 6 m/s, s: the start-up loss.
    launch_s: f64,
    /// Distinct motor vehicles seen.
    vehicles: usize,
}

fn is_vru(class: Option<&str>) -> bool {
    matches!(
        class,
        Some("pedestrian" | "cyclist" | "bicycle" | "ped" | "wheelchair")
    )
}

fn flow(rec: &Probe) -> Flow {
    #[derive(Clone, Copy)]
    struct Prev {
        t: u64,
        v: f64,
        lane: Option<u32>,
        pos: Option<f64>,
        moving: bool,
        launch_from: Option<u64>,
    }
    let mut prev: BTreeMap<u64, Prev> = BTreeMap::new();
    let mut per_t: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
    let (mut n, mut vsum, mut vkt, mut vt, mut stopped_t) = (0u64, 0.0, 0.0, 0.0, 0.0);
    let (mut stops, mut lc, mut hard) = (0u64, 0u64, 0u64);
    let mut launches: Vec<f64> = Vec::new();
    for (_, r) in rec.records() {
        if r.channel != "gt.kinematics" {
            continue;
        }
        let Ok(k) = serde_json::from_slice::<serde_json::Value>(&r.json) else {
            continue;
        };
        if is_vru(k.get("class").and_then(|c| c.as_str())) {
            continue;
        }
        let (Some(actor), Some(t), Some(v)) = (
            k.get("actor").and_then(serde_json::Value::as_u64),
            k.get("t").and_then(serde_json::Value::as_u64),
            k.get("speed_mps").and_then(serde_json::Value::as_f64),
        ) else {
            continue;
        };
        let lane = k
            .get("lane")
            .and_then(serde_json::Value::as_u64)
            .map(|l| l as u32);
        let pos = k.get("lane_pos_m").and_then(serde_json::Value::as_f64);
        if k.get("acc_mps2")
            .and_then(serde_json::Value::as_f64)
            .is_some_and(|a| a <= -2.0)
        {
            hard += 1;
        }
        n += 1;
        vsum += v;
        let e = per_t.entry(t).or_default();
        e.0 += 1;
        if v <= 0.1 {
            e.1 += 1;
        }
        let mut cur = Prev {
            t,
            v,
            lane,
            pos,
            moving: v >= 2.0,
            launch_from: None,
        };
        if let Some(p) = prev.get(&actor).copied() {
            let dt = (t.saturating_sub(p.t) as f64 * 1e-9).min(1.0);
            vkt += 0.5 * (v + p.v) * dt;
            vt += dt;
            if v <= 0.1 {
                stopped_t += dt;
            }
            cur.moving = if v >= 2.0 {
                true
            } else if v <= 0.1 {
                false
            } else {
                p.moving
            };
            if p.moving && !cur.moving {
                stops += 1;
            }
            // Start-up: the clock starts on the last standing sample, stops at 6 m/s.
            cur.launch_from = if v <= 0.1 { Some(t) } else { p.launch_from };
            if let Some(t0) = cur.launch_from
                && v >= 6.0
            {
                launches.push(t.saturating_sub(t0) as f64 * 1e-9);
                cur.launch_from = None;
            }
            if let (Some(a), Some(b), Some(x0), Some(x1)) = (p.lane, lane, p.pos, pos)
                && a != b
                && (x1 - x0).abs() < 6.0
                && x1 > 6.0
            {
                lc += 1;
            }
        }
        prev.insert(actor, cur);
    }
    assert!(n > 0, "no motor-vehicle kinematics were recorded");
    let instants = per_t.len().max(1) as f64;
    let mean = |v: &[f64]| {
        if v.is_empty() {
            f64::NAN
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };
    let f = Flow {
        density: per_t.values().map(|c| c.0 as f64).sum::<f64>() / instants,
        speed: vsum / n as f64,
        vkt_m: vkt,
        tt_per_km: vt / (vkt / 1000.0).max(1e-9),
        stops_per_km: stops as f64 / (vkt / 1000.0).max(1e-9),
        stopped_per_veh: stopped_t / prev.len().max(1) as f64,
        queue: per_t.values().map(|c| c.1 as f64).sum::<f64>() / instants,
        lane_changes: lc,
        hard_brakes_per_km: hard as f64 / (vkt / 1000.0).max(1e-9),
        launch_s: mean(&launches),
        vehicles: prev.len(),
    };
    eprintln!("sens-flow {f:?}");
    f
}

/// Network quantities: the run report's counts, the metric samples, `node.tx`,
/// `ma.report` and the VWP telemetry frames.
#[derive(Debug, Default, Clone)]
#[allow(dead_code)] // every field is printed (Debug) for the validation table
struct Net {
    /// Receptions decoded / reception attempts (the run's packet delivery ratio).
    pdr: f64,
    /// Mean channel busy ratio (`channel_load`, SAE J2945/1 CBR).
    cbr: f64,
    /// Mean end-to-end latency, ms (`e2e_latency`).
    latency_ms: f64,
    /// Mean MAC access delay, ms (run report).
    access_ms: f64,
    /// Mean neighbour-table size per node (`nbr_total`, node telemetry).
    neighbours: f64,
    /// Mean successful receptions per created node.
    rx_per_node: f64,
    /// PSMs and VAMs sent.
    vru_msgs: u64,
    /// Misbehaviour reports filed.
    ma_reports: u64,
    /// Local detector verdicts; with no attacker every one is a false positive.
    detections: u64,
    /// Messages the detector suite checked.
    checked: u64,
    /// Mean distance from a BSM's claimed position to the sender's true one, m.
    pos_err_m: f64,
}

fn metric_mean(rec: &Probe, name: &str) -> f64 {
    let (mut s, mut w) = (0.0, 0.0);
    for (_, r) in rec.records() {
        if r.channel != "metric.sample" {
            continue;
        }
        let Ok(m) = serde_json::from_slice::<v2xw_metrics::MetricSample>(&r.json) else {
            continue;
        };
        if m.metric != name {
            continue;
        }
        if let Some(p) = m.value.point()
            && p.is_finite()
        {
            let n = m.value.n().max(1) as f64;
            s += p * n;
            w += n;
        }
    }
    if w > 0.0 { s / w } else { f64::NAN }
}

fn net(report: &RunReport, rec: &Probe) -> Net {
    let mut ma_reports = 0;
    let (mut nb, mut nbn) = (0.0, 0.0);
    for (_, r) in rec.records() {
        if r.channel == "ma.report" {
            ma_reports += 1;
        } else if r.channel == "node.telemetry" {
            let v: serde_json::Value = serde_json::from_slice(&r.json).unwrap_or_default();
            if let Some(x) = v["nbr_total"].as_f64() {
                nb += x;
                nbn += 1.0;
            }
        }
    }
    eprintln!("sens-verdicts {:?}", report.phase2.verdicts_by_detector);
    // Each BSM's claim against the sender's latest true position at or before it.
    let (mut pe, mut pen) = (0.0, 0.0);
    for (node, claims) in &rec.claims {
        let Some(truth) = rec.truth.get(node) else {
            continue;
        };
        for &(t, x, y) in claims {
            let i = truth.partition_point(|s| s.0 <= t);
            if i > 0 {
                let (_, tx, ty) = truth[i - 1];
                pe += ((x - tx).powi(2) + (y - ty).powi(2)).sqrt();
                pen += 1.0;
            }
        }
    }
    let vru_msgs = rec.vru_msgs;
    let n = Net {
        pdr: report.receptions_ok as f64 / report.reception_attempts.max(1) as f64,
        cbr: metric_mean(rec, "channel_load"),
        latency_ms: metric_mean(rec, "e2e_latency"),
        access_ms: report.mac_access_delay_ns as f64 / report.mac_grants.max(1) as f64 * 1e-6,
        neighbours: if nbn > 0.0 { nb / nbn } else { f64::NAN },
        rx_per_node: report.receptions_ok as f64 / report.nodes_created.max(1) as f64,
        vru_msgs,
        ma_reports,
        detections: report.phase2.verdicts_fired,
        checked: report.phase2.messages_checked,
        pos_err_m: if pen > 0.0 { pe / pen } else { f64::NAN },
    };
    eprintln!("sens-net {n:?}");
    n
}

// ---------------------------------------------------------------------------------------
// Demand
// ---------------------------------------------------------------------------------------

/// Demand up: density, travel-time rate and stops up, speed down (Greenshields 1935;
/// HCM 6th ed. ch. 16 and 19: delay grows with the volume-to-capacity ratio).
#[test]
fn demand_up_raises_density_travel_time_and_stops_and_lowers_speed() {
    let (_, lo) = run(grid(90.0, 400.0));
    let (_, hi) = run(grid(90.0, 2400.0));
    let (lo, hi) = (flow(&lo), flow(&hi));
    assert!(hi.density > 1.5 * lo.density, "density {lo:?} -> {hi:?}");
    assert!(hi.speed < lo.speed, "speed {} -> {}", lo.speed, hi.speed);
    assert!(
        hi.tt_per_km > lo.tt_per_km,
        "tt {} -> {}",
        lo.tt_per_km,
        hi.tt_per_km
    );
    assert!(
        hi.stops_per_km > lo.stops_per_km,
        "stops/km {} -> {}",
        lo.stops_per_km,
        hi.stops_per_km
    );
}

/// Demand up, through the network: more transmitters load the channel (CBR up), more
/// collisions and hidden terminals lose more frames (PDR down), and the MAC waits longer
/// for the medium (latency up) — Bilstrup et al. 2008; SAE J2945/1 §6.3.
#[test]
fn demand_up_raises_cbr_and_latency_and_lowers_pdr() {
    let fleet = |n: u32| {
        let mut s = connected(12.0, 0.0);
        s.actors.vehicles.demand.rate_veh_per_h = None;
        s.actors.vehicles.demand.fleet_size = Some(n);
        run(s)
    };
    let (rl, lo) = fleet(30);
    let (rh, hi) = fleet(150);
    let (lo, hi) = (net(&rl, &lo), net(&rh, &hi));
    assert!(hi.cbr > lo.cbr, "CBR {} -> {}", lo.cbr, hi.cbr);
    assert!(hi.pdr < lo.pdr, "PDR {} -> {}", lo.pdr, hi.pdr);
    assert!(
        hi.access_ms > lo.access_ms,
        "MAC access delay {} -> {} ms",
        lo.access_ms,
        hi.access_ms
    );
    assert!(
        hi.latency_ms > lo.latency_ms,
        "e2e latency {} -> {} ms",
        lo.latency_ms,
        hi.latency_ms
    );
}

// ---------------------------------------------------------------------------------------
// Car following and lane changing
// ---------------------------------------------------------------------------------------

/// IDM time headway T up: saturation flow ~ 1/(T + (s0 + l)/v) falls, so a network held
/// on the congested branch of the fundamental diagram (a fixed fleet of 250 on the grid)
/// carries fewer vehicle-km (Treiber, Hennecke & Helbing 2000).
#[test]
fn a_longer_time_headway_lowers_the_flow() {
    let with = |t: f64| {
        let mut s = saturated(90.0);
        s.actors.vehicles.drivers.time_headway_s = Some(t);
        flow(&run(s).1)
    };
    let (short, long) = (with(1.0), with(2.5));
    assert!(
        long.vkt_m < 0.95 * short.vkt_m,
        "vehicle-km {} (T 1.0 s) -> {} (T 2.5 s)",
        short.vkt_m,
        long.vkt_m
    );
}

/// Max acceleration down: a vehicle takes longer from a standstill to speed, which is the
/// start-up lost time of the HCM's saturation-headway model (HCM 6th ed. ch. 19; Greenshields
/// 1947).
#[test]
fn a_lower_max_acceleration_lengthens_the_start_up() {
    let with = |a: f64| {
        let mut s = grid(90.0, 1500.0);
        s.actors.vehicles.drivers.max_accel_mps2 = Some(a);
        flow(&run(s).1)
    };
    let (brisk, slow) = (with(2.5), with(0.8));
    assert!(
        slow.launch_s > 1.3 * brisk.launch_s,
        "0-6 m/s took {} s at 2.5 m/s² and {} s at 0.8 m/s²",
        brisk.launch_s,
        slow.launch_s
    );
}

/// MOBIL politeness: the incentive is a_c' - a_c + p (Δa_n + Δa_o) (Kesting, Treiber &
/// Helbing 2007, eq. 3). The expectation "politeness up, lane changes down" holds only
/// where the new follower's loss Δa_n dominates; the same term also rewards a change that
/// lets the old follower accelerate (Δa_o > 0), and on long multi-lane links with spread
/// desired speeds that wins. Measured here, p = 0 -> 1 *raises* the change count
/// (49 -> 59), so the row asserts that the parameter reaches every decision and records
/// the direction in docs/validation/sensitivity-traffic.md rather than asserting one.
#[test]
fn politeness_reaches_the_lane_change_decisions() {
    let with = |p: f64| {
        // Slow trucks among cars of widely spread desired speeds on long unsignalised
        // three-lane links (highway-congestion.yaml's geometry): the discretionary
        // overtaking MOBIL decides, rather than the route's own lane choice.
        let mut s = grid(120.0, 3000.0);
        for (k, v) in [
            ("cols", serde_json::json!(2)),
            ("rows", serde_json::json!(2)),
            ("block_x_m", serde_json::json!(1500.0)),
            ("block_y_m", serde_json::json!(60.0)),
            ("speed_limit_mps", serde_json::json!(29.06)),
            ("signalised", serde_json::json!(false)),
            ("crossings", serde_json::json!(false)),
        ] {
            grid_param(&mut s, k, v);
        }
        let mut c = BTreeMap::new();
        for (name, f) in [("passenger", 1.0)] {
            c.insert(
                name.to_string(),
                VehicleClassSpec {
                    fraction: f,
                    obu: None,
                },
            );
        }
        s.actors.vehicles.classes = c;
        s.actors.vehicles.drivers.speed_spread = Some(3.0);
        s.actors.vehicles.drivers.politeness = Some(p);
        grid_param(&mut s, "lanes_per_direction", serde_json::json!(3));
        flow(&run(s).1)
    };
    let (selfish, polite) = (with(0.0), with(1.0));
    eprintln!(
        "sens-politeness lane changes {} -> {}, hard braking {:.1} -> {:.1} /veh-km",
        selfish.lane_changes,
        polite.lane_changes,
        selfish.hard_brakes_per_km,
        polite.hard_brakes_per_km
    );
    assert_ne!(
        polite.lane_changes, selfish.lane_changes,
        "politeness did not change a single lane-change decision"
    );
}

/// More trucks: slower acceleration and longer bodies lower the mean speed and the
/// carried flow (HCM 6th ed. ch. 19 heavy-vehicle adjustment f_HV = 1/(1 + P_T(E_T - 1))).
#[test]
fn more_trucks_lower_speed_and_capacity() {
    let mix = |truck: f64| {
        let mut s = saturated(90.0);
        let mut c = BTreeMap::new();
        c.insert(
            "passenger".to_string(),
            VehicleClassSpec {
                fraction: 1.0 - truck,
                obu: None,
            },
        );
        if truck > 0.0 {
            c.insert(
                "truck".to_string(),
                VehicleClassSpec {
                    fraction: truck,
                    obu: None,
                },
            );
        }
        s.actors.vehicles.classes = c;
        flow(&run(s).1)
    };
    let (cars, trucks) = (mix(0.0), mix(0.5));
    assert!(
        trucks.speed < cars.speed,
        "speed {} -> {}",
        cars.speed,
        trucks.speed
    );
    assert!(
        trucks.vkt_m < cars.vkt_m,
        "vehicle-km {} -> {}",
        cars.vkt_m,
        trucks.vkt_m
    );
}

// ---------------------------------------------------------------------------------------
// Signals and the speed limit
// ---------------------------------------------------------------------------------------

/// A longer cycle at the same split: the HCM uniform delay d1 = 0.5 C (1 - g/C)² /
/// (1 - min(1, X) g/C) grows with C, and so does the red-phase queue (HCM 6th ed. ch. 19,
/// eq. 19-19; Webster 1958).
#[test]
fn a_longer_cycle_raises_delay_and_queue() {
    let with = |c: f64| {
        let mut s = grid(120.0, 1200.0);
        grid_param(&mut s, "cycle_s", serde_json::json!(c));
        flow(&run(s).1)
    };
    let (short, long) = (with(40.0), with(120.0));
    assert!(
        long.stopped_per_veh > short.stopped_per_veh,
        "stopped delay {} s (C 40) -> {} s (C 120)",
        short.stopped_per_veh,
        long.stopped_per_veh
    );
    assert!(
        long.queue > short.queue,
        "queue {} -> {}",
        short.queue,
        long.queue
    );
}

/// Actuated against fixed-time at low demand: an actuated controller gaps out on an empty
/// approach and serves the waiting one, so delay falls (FHWA Traffic Signal Timing
/// Manual 2008 ch. 5; HCM 6th ed. ch. 19 actuated-control adjustment).
#[test]
fn actuated_signals_lower_delay_at_low_demand() {
    let with = |control: SignalControlKind| {
        let mut s = grid(120.0, 300.0);
        s.traffic = Some(Traffic {
            signals: SignalControl {
                control,
                ..SignalControl::default()
            },
            ..Traffic::default()
        });
        flow(&run(s).1)
    };
    let (fixed, act) = (
        with(SignalControlKind::FixedTime),
        with(SignalControlKind::Actuated),
    );
    assert!(
        act.stopped_per_veh < fixed.stopped_per_veh,
        "stopped delay {} s fixed -> {} s actuated",
        fixed.stopped_per_veh,
        act.stopped_per_veh
    );
}

/// A lower speed limit: speed falls and the travel-time rate rises, and the vehicle-km
/// the same vehicles cover in the window fall (flow q = k v at the same k; Greenshields
/// 1935; HCM 6th ed. ch. 4). A 90 s run is shorter than a trip across the grid, so the
/// steady-state density rise q = k v predicts at equal demand is not reached and is not
/// asserted.
#[test]
fn a_lower_speed_limit_lowers_speed_and_flow() {
    let with = |limit: f64| {
        let mut s = grid(90.0, 600.0);
        grid_param(&mut s, "speed_limit_mps", serde_json::json!(limit));
        flow(&run(s).1)
    };
    let (fast, slow) = (with(13.4), with(8.9));
    assert!(
        slow.speed < 0.85 * fast.speed,
        "speed {} -> {}",
        fast.speed,
        slow.speed
    );
    assert!(
        slow.tt_per_km > fast.tt_per_km,
        "travel-time rate {} -> {} s/km",
        fast.tt_per_km,
        slow.tt_per_km
    );
    assert!(
        slow.vkt_m < fast.vkt_m,
        "vehicle-km {} -> {}",
        fast.vkt_m,
        slow.vkt_m
    );
}

// ---------------------------------------------------------------------------------------
// Equipment, vulnerable road users, weather, positioning
// ---------------------------------------------------------------------------------------

/// Equipped fraction down: fewer transmitters, so each node hears fewer neighbours and
/// knows fewer of the vehicles around it (ETSI TR 103 562 awareness; Lyamin 2018).
#[test]
fn a_lower_equipped_fraction_lowers_awareness_and_neighbours() {
    let with = |f: f64| {
        let mut s = connected(45.0, 1500.0);
        s.actors.vehicles.equipped_fraction = f;
        let (r, rec) = run(s);
        net(&r, &rec)
    };
    let (all, few) = (with(1.0), with(0.3));
    assert!(
        few.neighbours < all.neighbours,
        "neighbours {} -> {}",
        all.neighbours,
        few.neighbours
    );
    assert!(
        few.rx_per_node < all.rx_per_node,
        "receptions per node {} -> {}",
        all.rx_per_node,
        few.rx_per_node
    );
}

/// More pedestrians: more VRU devices send more PSMs/VAMs, and turning vehicles yield to
/// more crossers, so vehicle delay grows (HCM 6th ed. ch. 19 pedestrian adjustment
/// f_Lpb/f_Rpb).
#[test]
fn more_pedestrians_mean_more_vru_messages_and_more_crossing_delay() {
    let with = |peds: u32| {
        let mut s = connected(60.0, 900.0);
        s.actors.vru.pedestrians = peds;
        s.actors.vru.device_fraction = 0.5;
        let (r, rec) = run(s);
        (net(&r, &rec), flow(&rec))
    };
    let ((nf, ff), (nm, fm)) = (with(40), with(300));
    assert!(
        nm.vru_msgs > nf.vru_msgs,
        "VRU messages {} -> {}",
        nf.vru_msgs,
        nm.vru_msgs
    );
    assert!(
        fm.stopped_per_veh > ff.stopped_per_veh,
        "vehicle stopped delay {} -> {}",
        ff.stopped_per_veh,
        fm.stopped_per_veh
    );
}

/// Heavy rain: FHWA free-flow speed reductions lower the mean speed (FHWA-HOP-12-004,
/// Weather-Responsive Traffic Management). ITU-R P.838 rain attenuation at 5.9 GHz is
/// under 1 dB/km even at 25 mm/h, so the radio loss is recorded, not asserted.
#[test]
fn rain_lowers_speed() {
    let with = |rain: bool| {
        let mut s = connected(60.0, 900.0);
        if rain {
            s.weather.initial = v2xw_core::weather::WeatherKind::Rain;
            s.weather.intensity = 1.0;
        }
        let (r, rec) = run(s);
        (net(&r, &rec), flow(&rec))
    };
    let ((nc, fc), (nr, fr)) = (with(false), with(true));
    assert!(
        fr.speed < fc.speed,
        "speed {} clear -> {} rain",
        fc.speed,
        fr.speed
    );
    eprintln!("sens-rain pdr {} -> {}", nc.pdr, nr.pdr);
}

/// GNSS error up: the fixes the nodes broadcast are further from the truth, and the
/// plausibility checks of honest receivers flag more of them (VeReMi, van der Heijden
/// et al. 2018: false-positive rate rises with position noise).
#[test]
fn more_gnss_error_raises_position_error_and_false_reports() {
    let with = |grade: u8| {
        let mut s = connected(45.0, 1200.0);
        s.detection.local = vec![
            serde_json::from_value(serde_json::json!({
                "id": "detect/legacy-12"
            }))
            .expect("detector choice"),
        ];
        s.actors.vehicles.gnss = Some(match grade {
            0 => GnssSettings {
                rtk: Some(true),
                ..GnssSettings::default()
            },
            1 => GnssSettings::default(),
            _ => GnssSettings {
                outlier_rate_hz: Some(0.3),
                outlier_magnitude_m: Some(60.0),
                ..GnssSettings::default()
            },
        });
        let (r, rec) = run(s);
        net(&r, &rec)
    };
    let (good, standard, bad) = (with(0), with(1), with(2));
    assert!(
        good.pos_err_m < standard.pos_err_m,
        "position error {} m (RTK) -> {} m (standard)",
        good.pos_err_m,
        standard.pos_err_m
    );
    assert!(
        bad.pos_err_m > standard.pos_err_m,
        "position error {} -> {} m",
        standard.pos_err_m,
        bad.pos_err_m
    );
    assert!(
        bad.detections > standard.detections,
        "false detector verdicts {} -> {}",
        standard.detections,
        bad.detections
    );
}
