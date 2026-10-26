//! SUMO-format outputs (adoption package WP-06, `docs/design/frameworks/sumo.md` P7).
//!
//! Each writer reads the finished recording — `gt.kinematics`, and the `gt.spawn` and
//! `gt.despawn` records a run writes when a `sumo-*` exporter is named
//! ([`crate::export::records_lifecycle`]) — and writes one file in the format SUMO's own
//! output of that name has, so SUMO's tool chain (`xml2csv.py`, `attributeStats.py`,
//! `routeSampler.py`, `duarouter --weight-files`, `traceExporter.py`) reads an engine run
//! as it reads a SUMO run.
//!
//! **Model card.** Re-implemented from the SUMO documentation (sumo.dlr.de/docs,
//! "Simulation/Output/FCDOutput", "TripInfo", "Lane- or Edge-based Traffic Measures",
//! "Summary", "Induction Loops Detectors (E1)", "Lanearea Detectors (E2)") and the
//! element and attribute declarations of SUMO's XSDs (`fcd_file.xsd`, `tripinfo_file.xsd`,
//! `meandata_file.xsd`, `summary_file.xsd`, `det_e1_file.xsd`, `det_e2_file.xsd`, SUMO
//! 1.27.1). SUMO was read for behaviour only and no code was copied. The constants are
//! SUMO's documented defaults: the halting speed 0.1 m/s (waiting time, `halting`), the
//! E2 jam thresholds 1 s / 1.39 m/s / 10 m (`timeThreshold`, `speedThreshold`,
//! `jamThreshold`), output precision 2 (`--precision`).
//!
//! # Frames and conventions
//!
//! * Ids: a vehicle's SUMO id is its actor id. Lanes and edges are named by
//!   [`v2xw_interop::SumoIds`], the mapping the network export (`netexport`) writes, so an
//!   edge id in `edgedata.xml` is an edge of the `net.xml` the same world exports to; a
//!   junction connector has no SUMO id (netconvert builds its own internal lanes), so a
//!   sample on one carries no `lane` and counts on no edge.
//! * Coordinates: SUMO's network frame, which is the world frame less the importer's
//!   shift for a world read from a `net.xml`. FCD `x`, `y` and `pos` are the **front
//!   bumper** (SUMO's reference point); this engine records the rear bumper, so the front
//!   is the recorded point plus the vehicle length along the heading (`pos` is capped at
//!   the lane's length). `angle` is degrees clockwise from north.
//! * A sample stands for the time from its instant to the actor's next sample (to its
//!   despawn for the last one, or one recording step at the end of the run): the
//!   time-weighted measures (`sampledSeconds`, `waitingTime`, `timeLoss`, occupancy) are
//!   sums over those spans.
//! * `timeLoss` is the time lost against the ideal speed `min(lane limit, desired speed)`,
//!   SUMO's `vmax = min(lane speed × speedFactor, vType maxSpeed)` in this engine's terms;
//!   `speedFactor` is written as the desired speed over the depart lane's limit.
//! * Edge and lane measures assign a vehicle to the lane its recorded (rear) point is on.
//!
//! # What is not written, and why
//!
//! `rerouteNo` and `stopTime` are 0 and `collisions`, `teleports` and `stopped` in the
//! summary are 0: the recording carries no reroute, scheduled stop, collision or teleport
//! record. A provider that does not report a trip's requested depart time gets SUMO's
//! error value `-1` for `departDelay`. A vehicle that left for a reason other than the end
//! of its route is `vaporized` (`route-blocked` → `teleport`, `commanded` → `traci`,
//! `lifetime-expired` → `vaporizer`); one still running at the end is written only with
//! `opts.write_unfinished` (SUMO's `--tripinfo-output.write-unfinished`), as `end`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;
use v2xw_core::ids::LaneId;
use v2xw_interop::SumoIds;
use v2xw_metrics::channels::GtKinematicsView;
use v2xw_record::dataset::views::{GtDespawnView, GtSpawnView};
use v2xw_world::World;

/// The SUMO exporter ids, and the file each writes.
pub const IDS: [(&str, &str); 7] = [
    ("sumo-fcd", "fcd.xml"),
    ("sumo-tripinfo", "tripinfo.xml"),
    ("sumo-edgedata", "edgedata.xml"),
    ("sumo-lanedata", "lanedata.xml"),
    ("sumo-summary", "summary.xml"),
    ("sumo-e1", "e1.xml"),
    ("sumo-e2", "e2.xml"),
];

/// SUMO's halting speed, m/s (`SUMO_const_haltingSpeed`): at or below it a vehicle is
/// waiting (tripinfo `waitingTime`, edgeData `waitingTime`, summary `halting`).
pub const HALTING_SPEED_MPS: f64 = 0.1;

/// Whether `id` is one of the SUMO exporters.
#[must_use]
pub fn is_sumo(id: &str) -> bool {
    IDS.iter().any(|(x, _)| *x == id)
}

/// One virtual detector of `sumo-e1` or `sumo-e2`.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectorSpec {
    /// Its id in the output.
    pub id: String,
    /// The SUMO lane id it is on (`<edge>_<index>`).
    pub lane: String,
    /// Its position along the lane, metres (an E2's upstream end).
    pub pos_m: f64,
    /// An E2's length, metres.
    pub length_m: f64,
}

/// A SUMO exporter's options, with SUMO's defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct SumoOptions {
    /// Aggregation or sampling period, seconds: FCD `--device.fcd.period`, meandata
    /// `period`, summary `--summary-output.period`, detector `period`. `None`: every
    /// recorded step for FCD and the summary, the whole run for the aggregates.
    pub period_s: Option<f64>,
    /// Decimal places of every float (`--precision`, default 2).
    pub precision: usize,
    /// tripinfo: also write the vehicles still running at the end, `vaporized="end"`.
    pub write_unfinished: bool,
    /// The detectors of `sumo-e1` / `sumo-e2`.
    pub detectors: Vec<DetectorSpec>,
    /// E2 `timeThreshold`, seconds (default 1).
    pub time_threshold_s: f64,
    /// E2 `speedThreshold`, m/s (default 1.39, i.e. 5 km/h).
    pub speed_threshold_mps: f64,
    /// E2 `jamThreshold`, metres (default 10).
    pub jam_threshold_m: f64,
}

impl Default for SumoOptions {
    fn default() -> Self {
        Self {
            period_s: None,
            precision: 2,
            write_unfinished: false,
            detectors: Vec::new(),
            time_threshold_s: 1.0,
            speed_threshold_mps: 1.39,
            jam_threshold_m: 10.0,
        }
    }
}

type OptError = (String, String);

/// The options of exporter `id`, from its `opts`.
///
/// # Errors
/// `(field, why)` for an unknown option, an out-of-range value or a malformed detector.
pub fn options_of(id: &str, opts: &Value) -> Result<SumoOptions, OptError> {
    let mut o = SumoOptions::default();
    let Some(map) = opts.as_object() else {
        if opts.is_null() {
            return Ok(o);
        }
        return Err(("opts".into(), "is not a mapping".into()));
    };
    let detectors = matches!(id, "sumo-e1" | "sumo-e2");
    for (k, v) in map {
        let num = |lo: f64, hi: f64| -> Result<f64, OptError> {
            v.as_f64()
                .filter(|x| x.is_finite() && *x >= lo && *x <= hi)
                .ok_or_else(|| (format!("opts.{k}"), format!("must be a number in [{lo}, {hi}]")))
        };
        match k.as_str() {
            "profile" => {}
            "period_s" => o.period_s = Some(num(0.001, 604_800.0)?),
            "precision" => o.precision = num(0.0, 9.0)? as usize,
            "write_unfinished" if id == "sumo-tripinfo" => {
                o.write_unfinished = v
                    .as_bool()
                    .ok_or_else(|| (format!("opts.{k}"), "must be true or false".into()))?;
            }
            "time_threshold_s" if id == "sumo-e2" => o.time_threshold_s = num(0.0, 3600.0)?,
            "speed_threshold_mps" if id == "sumo-e2" => {
                o.speed_threshold_mps = num(0.0, 100.0)?;
            }
            "jam_threshold_m" if id == "sumo-e2" => o.jam_threshold_m = num(0.0, 1000.0)?,
            "detectors" if detectors => o.detectors = detectors_of(id, v)?,
            other => {
                return Err((
                    format!("opts.{other}"),
                    format!("is not an option of the {id} exporter"),
                ));
            }
        }
    }
    if detectors && o.detectors.is_empty() {
        return Err((
            "opts.detectors".into(),
            "names no detector: give at least one {id, lane, pos_m}".into(),
        ));
    }
    Ok(o)
}

fn detectors_of(id: &str, v: &Value) -> Result<Vec<DetectorSpec>, OptError> {
    let list = v
        .as_array()
        .ok_or_else(|| ("opts.detectors".to_string(), "must be a list".to_string()))?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (i, d) in list.iter().enumerate() {
        let f = |k: &str| format!("opts.detectors[{i}].{k}");
        let s = |k: &str| {
            d.get(k)
                .and_then(Value::as_str)
                .filter(|x| !x.is_empty())
                .map(str::to_string)
                .ok_or_else(|| (f(k), "is required, a non-empty string".to_string()))
        };
        let det_id = s("id")?;
        let lane = s("lane")?;
        let pos_m = d
            .get("pos_m")
            .and_then(Value::as_f64)
            .filter(|x| x.is_finite() && *x >= 0.0)
            .ok_or_else(|| (f("pos_m"), "is required, metres along the lane, ≥ 0".to_string()))?;
        let length_m = if id == "sumo-e2" {
            d.get("length_m")
                .and_then(Value::as_f64)
                .filter(|x| x.is_finite() && *x > 0.0)
                .ok_or_else(|| (f("length_m"), "is required for an E2, metres, > 0".to_string()))?
        } else {
            0.0
        };
        if let Some(obj) = d.as_object()
            && let Some(k) = obj
                .keys()
                .find(|k| !matches!(k.as_str(), "id" | "lane" | "pos_m" | "length_m"))
        {
            return Err((f(k), "is not a detector field".to_string()));
        }
        if !seen.insert(det_id.clone()) {
            return Err((f("id"), format!("'{det_id}' is used twice")));
        }
        out.push(DetectorSpec {
            id: det_id,
            lane,
            pos_m,
            length_m,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// The recording, as tracks.

/// One recorded state.
#[derive(Debug, Clone, Copy)]
struct Sample {
    t: u64,
    x: f64,
    y: f64,
    z: Option<f64>,
    speed: f64,
    acc: Option<f64>,
    heading: f64,
    lane: Option<LaneId>,
    s: f64,
    /// Seconds this sample stands for.
    dt: f64,
}

#[derive(Debug, Clone, Default)]
struct Track {
    class: String,
    samples: Vec<Sample>,
    spawn: Option<GtSpawnView>,
    despawn: Option<GtDespawnView>,
}

impl Track {
    fn is_person(&self) -> bool {
        self.class == "pedestrian"
    }
    fn length(&self) -> f64 {
        self.spawn.as_ref().and_then(|s| s.length_m).unwrap_or(0.0)
    }
    fn depart(&self) -> Option<u64> {
        self.spawn
            .as_ref()
            .map(|s| s.t)
            .or_else(|| self.samples.first().map(|s| s.t))
    }
    fn desired(&self) -> Option<f64> {
        self.spawn.as_ref().and_then(|s| s.desired_speed_mps)
    }
}

/// The recording, read once for every SUMO exporter.
#[derive(Debug, Clone, Default)]
pub struct Tracks {
    by_actor: BTreeMap<u32, Track>,
    /// Every distinct sample instant, ascending.
    times: Vec<u64>,
    /// The recording step, nanoseconds: the smallest gap between two sample instants.
    step_ns: u64,
    /// The end of the run, nanoseconds.
    end_ns: u64,
}

impl Tracks {
    /// Reads the tracks out of a recording's records.
    ///
    /// # Errors
    /// A message when a record does not fit its channel's view.
    pub fn from_records(records: &[v2xw_record::RecordedRecord]) -> Result<Self, String> {
        fn view<T: serde::de::DeserializeOwned>(
            r: &v2xw_record::RecordedRecord,
        ) -> Result<T, String> {
            serde_json::from_slice(&r.json)
                .map_err(|e| format!("a record on {} does not fit its view: {e}", r.channel))
        }
        let mut by_actor: BTreeMap<u32, Track> = BTreeMap::new();
        let mut times = BTreeSet::new();
        let mut last = 0u64;
        for r in records {
            match r.channel.as_str() {
                "gt.kinematics" => {
                    let k: GtKinematicsView = view(r)?;
                    times.insert(k.t);
                    last = last.max(k.t);
                    let tr = by_actor.entry(k.actor.0).or_default();
                    if let Some(c) = k.class.as_ref()
                        && tr.class.is_empty()
                    {
                        tr.class.clone_from(c);
                    }
                    tr.samples.push(Sample {
                        t: k.t,
                        x: k.x_m,
                        y: k.y_m,
                        z: k.z_m,
                        speed: k.speed_mps.max(0.0),
                        acc: k.acc_mps2,
                        heading: k.heading_rad.unwrap_or(0.0),
                        lane: k.lane.map(LaneId::new),
                        s: k.lane_pos_m.unwrap_or(0.0),
                        dt: 0.0,
                    });
                }
                "gt.spawn" => {
                    let v: GtSpawnView = view(r)?;
                    let tr = by_actor.entry(v.actor.0).or_default();
                    if let Some(c) = v.class.as_ref() {
                        tr.class.clone_from(c);
                    }
                    tr.spawn = Some(v);
                }
                "gt.despawn" => {
                    let v: GtDespawnView = view(r)?;
                    last = last.max(v.t);
                    let actor = v.actor.0;
                    by_actor.entry(actor).or_default().despawn = Some(v);
                }
                _ => {}
            }
        }
        let times: Vec<u64> = times.into_iter().collect();
        let step_ns = times
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|d| *d > 0)
            .min()
            .unwrap_or(1_000_000_000);
        let end_ns = last.saturating_add(step_ns);
        for tr in by_actor.values_mut() {
            tr.samples.sort_by_key(|s| s.t);
            tr.samples.dedup_by_key(|s| s.t);
            let stop = tr.despawn.as_ref().map_or(end_ns, |d| d.t);
            let n = tr.samples.len();
            for i in 0..n {
                let next = if i + 1 < n {
                    tr.samples[i + 1].t
                } else {
                    stop.max(tr.samples[i].t)
                };
                tr.samples[i].dt = (next.saturating_sub(tr.samples[i].t)) as f64 / 1e9;
            }
        }
        Ok(Self {
            by_actor,
            times,
            step_ns,
            end_ns,
        })
    }

    /// The end of the run, seconds.
    #[must_use]
    pub fn end_s(&self) -> f64 {
        self.end_ns as f64 / 1e9
    }
}

// ---------------------------------------------------------------------------------------
// Shared helpers.

struct Ctx<'a> {
    world: &'a World,
    ids: SumoIds,
    shift: (f64, f64),
    p: usize,
}

impl Ctx<'_> {
    fn f(&self, v: f64) -> String {
        fmt(v, self.p)
    }
    fn t(&self, ns: u64) -> String {
        fmt(ns as f64 / 1e9, self.p)
    }
    fn lane_id(&self, l: Option<LaneId>) -> Option<&str> {
        l.and_then(|l| self.ids.sumo_lane.get(&l)).map(String::as_str)
    }
    fn edge_id(&self, l: Option<LaneId>) -> Option<&str> {
        l.filter(|l| (l.0 as usize) < self.world.roads.lanes().len())
            .and_then(|l| self.ids.edge_of_lane(self.world, l))
    }
    fn lane_len(&self, l: LaneId) -> f64 {
        self.world.lane(l).length_m
    }
    fn limit(&self, l: Option<LaneId>) -> Option<f64> {
        l.filter(|l| (l.0 as usize) < self.world.roads.lanes().len())
            .map(|l| self.world.lane(l).speed_limit_mps)
            .filter(|v| *v > 0.0)
    }
    /// The ideal speed of a sample: `min(lane limit, desired speed)`.
    fn vmax(&self, s: &Sample, desired: Option<f64>) -> Option<f64> {
        match (self.limit(s.lane), desired) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
        .filter(|v| *v > 0.0)
    }
    fn time_loss(&self, s: &Sample, desired: Option<f64>) -> f64 {
        self.vmax(s, desired)
            .map_or(0.0, |v| s.dt * (1.0 - s.speed / v).max(0.0))
    }
}

/// A float with `p` decimals, never `-0.00`.
fn fmt(v: f64, p: usize) -> String {
    let v = if v.is_finite() { v } else { -1.0 };
    let s = format!("{v:.p$}");
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

/// XML attribute escaping.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c => o.push(c),
        }
    }
    o
}

fn header(root: &str, xsd: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- generated by v2xw from a recording \
         (SUMO-format output, WP-06) -->\n<{root} \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xsi:noNamespaceSchemaLocation=\"http://sumo.dlr.de/xsd/{xsd}\">\n"
    )
}

/// The aggregation intervals `[begin, end)` in nanoseconds.
fn intervals(period_s: Option<f64>, end_ns: u64) -> Vec<(u64, u64)> {
    match period_s {
        None => vec![(0, end_ns)],
        Some(p) => {
            let p = ((p * 1e9).round() as u64).max(1);
            let mut out = Vec::new();
            let mut b = 0;
            while b < end_ns {
                out.push((b, (b + p).min(end_ns)));
                b += p;
            }
            if out.is_empty() {
                out.push((0, end_ns));
            }
            out
        }
    }
}

fn on_period(t: u64, period_s: Option<f64>) -> bool {
    period_s.is_none_or(|p| {
        let p = ((p * 1e9).round() as u64).max(1);
        t % p == 0
    })
}

fn angle_deg(heading_rad: f64) -> f64 {
    (90.0 - heading_rad.to_degrees()).rem_euclid(360.0)
}

// ---------------------------------------------------------------------------------------
// The writers.

/// Writes exporter `id`'s file into `dir` and returns its path and row count.
///
/// # Errors
/// A message for an unknown id, a detector on a lane the world has no SUMO id for, or a
/// file that cannot be written.
pub fn write(
    id: &str,
    opts: &SumoOptions,
    tracks: &Tracks,
    world: &World,
    dir: &Path,
) -> Result<(std::path::PathBuf, usize), String> {
    let ctx = Ctx {
        world,
        ids: SumoIds::new(world),
        shift: crate::wiring::sumo_shift(world),
        p: opts.precision,
    };
    let (xml, rows) = match id {
        "sumo-fcd" => fcd(&ctx, opts, tracks),
        "sumo-tripinfo" => tripinfo(&ctx, opts, tracks),
        "sumo-edgedata" => meandata(&ctx, opts, tracks, false),
        "sumo-lanedata" => meandata(&ctx, opts, tracks, true),
        "sumo-summary" => summary(&ctx, opts, tracks),
        "sumo-e1" => e1(&ctx, opts, tracks)?,
        "sumo-e2" => e2(&ctx, opts, tracks)?,
        other => return Err(format!("'{other}' is not a SUMO exporter")),
    };
    let file = IDS
        .iter()
        .find(|(x, _)| *x == id)
        .map_or("out.xml", |(_, f)| f);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(file);
    std::fs::write(&path, xml).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((path, rows))
}

/// FCD: one `timestep` per recorded instant, a `vehicle` (or `person`) per actor in it.
fn fcd(c: &Ctx, o: &SumoOptions, tr: &Tracks) -> (String, usize) {
    let mut at: BTreeMap<u64, Vec<(u32, &Track, &Sample)>> = BTreeMap::new();
    for (a, t) in &tr.by_actor {
        for s in &t.samples {
            if on_period(s.t, o.period_s) {
                at.entry(s.t).or_default().push((*a, t, s));
            }
        }
    }
    let mut x = header("fcd-export", "fcd_file.xsd");
    let mut rows = 0;
    for (t, list) in &at {
        let _ = writeln!(x, "    <timestep time=\"{}\">", c.t(*t));
        // Vehicles first, then persons: the XSD's sequence order.
        for persons in [false, true] {
            for (a, track, s) in list.iter().filter(|(_, k, _)| k.is_person() == persons) {
                let len = if persons { 0.0 } else { track.length() };
                let fx = s.x + len * s.heading.cos() - c.shift.0;
                let fy = s.y + len * s.heading.sin() - c.shift.1;
                let _ = write!(
                    x,
                    "        <{} id=\"{a}\" x=\"{}\" y=\"{}\"",
                    if persons { "person" } else { "vehicle" },
                    c.f(fx),
                    c.f(fy)
                );
                if let Some(z) = s.z {
                    let _ = write!(x, " z=\"{}\"", c.f(z));
                }
                let _ = write!(
                    x,
                    " angle=\"{}\" type=\"{}\" speed=\"{}\"",
                    c.f(angle_deg(s.heading)),
                    esc(&track.class),
                    c.f(s.speed)
                );
                if let Some(l) = s.lane
                    && let Some(lid) = c.lane_id(Some(l))
                {
                    let pos = (s.s + len).min(c.lane_len(l)).max(0.0);
                    if persons {
                        let e = c.edge_id(Some(l)).unwrap_or("");
                        let _ = write!(x, " pos=\"{}\" edge=\"{}\"", c.f(pos), esc(e));
                    } else {
                        let _ = write!(x, " pos=\"{}\" lane=\"{}\"", c.f(pos), esc(lid));
                    }
                }
                if !persons && let Some(acc) = s.acc {
                    let _ = write!(x, " acceleration=\"{}\"", c.f(acc));
                }
                x.push_str("/>\n");
                rows += 1;
            }
        }
        x.push_str("    </timestep>\n");
    }
    x.push_str("</fcd-export>\n");
    (x, rows)
}

/// One finished (or unfinished) trip's tripinfo numbers.
#[derive(Debug, Clone)]
struct Trip {
    actor: u32,
    depart: u64,
    arrival: u64,
    route_length: f64,
    waiting: f64,
    waiting_count: u64,
    time_loss: f64,
    vaporized: Option<&'static str>,
}

fn trip_of(c: &Ctx, a: u32, t: &Track, end_ns: u64) -> Option<Trip> {
    let depart = t.depart()?;
    let (arrival, vaporized) = match t.despawn.as_ref() {
        Some(d) => (
            d.t,
            match d.cause.as_str() {
                "trip-complete" => None,
                "route-blocked" => Some("teleport"),
                "commanded" => Some("traci"),
                _ => Some("vaporizer"),
            },
        ),
        None => (end_ns, Some("end")),
    };
    let mut route_length = 0.0;
    let mut waiting = 0.0;
    let mut waiting_count = 0;
    let mut was_waiting = false;
    let mut time_loss = 0.0;
    let desired = t.desired();
    for (i, s) in t.samples.iter().enumerate() {
        // Trapezoidal distance between this sample and the next.
        let v_next = t.samples.get(i + 1).map_or(s.speed, |n| n.speed);
        route_length += 0.5 * (s.speed + v_next) * s.dt;
        let w = s.speed <= HALTING_SPEED_MPS;
        if w {
            waiting += s.dt;
            if !was_waiting {
                waiting_count += 1;
            }
        }
        was_waiting = w;
        time_loss += c.time_loss(s, desired);
    }
    Some(Trip {
        actor: a,
        depart,
        arrival,
        route_length,
        waiting,
        waiting_count,
        time_loss,
        vaporized,
    })
}

/// tripinfo: one `tripinfo` per vehicle that left, in arrival order.
fn tripinfo(c: &Ctx, o: &SumoOptions, tr: &Tracks) -> (String, usize) {
    let mut trips: Vec<(Trip, &Track)> = tr
        .by_actor
        .iter()
        .filter(|(_, t)| !t.is_person() && (t.despawn.is_some() || o.write_unfinished))
        .filter_map(|(a, t)| trip_of(c, *a, t, tr.end_ns).map(|x| (x, t)))
        .collect();
    trips.sort_by_key(|(x, _)| (x.arrival, x.actor));
    let mut x = header("tripinfos", "tripinfo_file.xsd");
    for (trip, t) in &trips {
        let first = t.samples.first();
        let last = t.samples.last();
        let len = t.length();
        let lane = |s: Option<&Sample>| s.and_then(|s| c.lane_id(s.lane)).unwrap_or("");
        let pos = |s: Option<&Sample>| {
            s.map_or(0.0, |s| {
                s.lane
                    .map_or(s.s + len, |l| (s.s + len).min(c.lane_len(l)))
                    .max(0.0)
            })
        };
        let delay = t
            .spawn
            .as_ref()
            .and_then(|s| s.requested_t)
            .map_or(-1.0, |r| trip.depart.saturating_sub(r) as f64 / 1e9);
        let speed_factor = match (t.desired(), c.limit(first.and_then(|s| s.lane))) {
            (Some(d), Some(l)) if d > 0.0 => d / l,
            _ => 1.0,
        };
        let _ = write!(
            x,
            "    <tripinfo id=\"{}\" depart=\"{}\" departLane=\"{}\" departPos=\"{}\" \
             departSpeed=\"{}\" departDelay=\"{}\" arrival=\"{}\" arrivalLane=\"{}\" \
             arrivalPos=\"{}\" arrivalSpeed=\"{}\" duration=\"{}\" routeLength=\"{}\" \
             waitingTime=\"{}\" waitingCount=\"{}\" stopTime=\"{}\" timeLoss=\"{}\" \
             rerouteNo=\"0\" devices=\"tripinfo_{}\" vType=\"{}\" speedFactor=\"{}\"",
            trip.actor,
            c.t(trip.depart),
            esc(lane(first)),
            c.f(pos(first)),
            c.f(first.map_or(0.0, |s| s.speed)),
            c.f(delay),
            c.t(trip.arrival),
            esc(lane(last)),
            c.f(pos(last)),
            c.f(last.map_or(0.0, |s| s.speed)),
            c.f(trip.arrival.saturating_sub(trip.depart) as f64 / 1e9),
            c.f(trip.route_length),
            c.f(trip.waiting),
            trip.waiting_count,
            c.f(0.0),
            c.f(trip.time_loss),
            trip.actor,
            esc(&t.class),
            c.f(speed_factor.max(10f64.powi(-(c.p as i32)))),
        );
        if let Some(v) = trip.vaporized {
            let _ = write!(x, " vaporized=\"{v}\"");
        }
        x.push_str("/>\n");
    }
    x.push_str("</tripinfos>\n");
    (x, trips.len())
}

/// One edge's or lane's measures in one interval.
#[derive(Debug, Clone, Default)]
struct Measures {
    sampled: f64,
    dist: f64,
    occupied: f64,
    waiting: f64,
    time_loss: f64,
    departed: u64,
    arrived: u64,
    vaporized: u64,
    entered: u64,
    left: u64,
    lc_from: u64,
    lc_to: u64,
}

/// edgeData / laneData (SUMO "Lane- or Edge-based Traffic Measures").
fn meandata(c: &Ctx, o: &SumoOptions, tr: &Tracks, lanes: bool) -> (String, usize) {
    let ivs = intervals(o.period_s, tr.end_ns);
    let idx = |t: u64| ivs.iter().position(|(b, e)| t >= *b && t < *e);
    let mut m: Vec<BTreeMap<String, Measures>> = vec![BTreeMap::new(); ivs.len()];
    let key = |l: Option<LaneId>| -> Option<String> {
        if lanes {
            c.lane_id(l).map(str::to_string)
        } else {
            c.edge_id(l).map(str::to_string)
        }
    };
    for t in tr.by_actor.values().filter(|t| !t.is_person()) {
        let desired = t.desired();
        let len = t.length();
        let mut prev: Option<&Sample> = None;
        for s in &t.samples {
            let here = key(s.lane);
            let Some(i) = idx(s.t) else {
                prev = Some(s);
                continue;
            };
            if let Some(k) = here.as_ref() {
                let e = m[i].entry(k.clone()).or_default();
                // The span this sample stands for, cut at the interval's end.
                let dt = s.dt.min((ivs[i].1 - s.t) as f64 / 1e9);
                e.sampled += dt;
                e.dist += s.speed * dt;
                e.occupied += len * dt;
                if s.speed <= HALTING_SPEED_MPS {
                    e.waiting += dt;
                }
                e.time_loss += c
                    .vmax(s, desired)
                    .map_or(0.0, |v| dt * (1.0 - s.speed / v).max(0.0));
            }
            match prev {
                None => {
                    if let Some(k) = here.as_ref() {
                        m[i].entry(k.clone()).or_default().departed += 1;
                    }
                }
                Some(p) => {
                    let before = key(p.lane);
                    if before != here {
                        // A lane change inside one edge, for lane data; else a move.
                        let same_edge = lanes
                            && before.is_some()
                            && here.is_some()
                            && c.edge_id(p.lane) == c.edge_id(s.lane);
                        if let Some(k) = before.as_ref() {
                            let e = m[i].entry(k.clone()).or_default();
                            if same_edge {
                                e.lc_from += 1;
                            } else {
                                e.left += 1;
                            }
                        }
                        if let Some(k) = here.as_ref() {
                            let e = m[i].entry(k.clone()).or_default();
                            if same_edge {
                                e.lc_to += 1;
                            } else {
                                e.entered += 1;
                            }
                        }
                    }
                }
            }
            prev = Some(s);
        }
        if let (Some(d), Some(last)) = (t.despawn.as_ref(), t.samples.last())
            && let (Some(i), Some(k)) = (idx(d.t).or_else(|| idx(last.t)), key(last.lane))
        {
            let e = m[i].entry(k).or_default();
            if d.cause == "trip-complete" {
                e.arrived += 1;
            } else {
                e.vaporized += 1;
            }
        }
    }
    // Length and lane count of every key.
    let geom = |k: &str| -> (f64, f64, f64) {
        if lanes {
            let l = c.ids.lane_of[k];
            let lane = c.world.lane(l);
            (lane.length_m, 1.0, lane.speed_limit_mps)
        } else {
            let ls = &c.ids.lanes_of[k];
            let lane = c.world.lane(ls[0]);
            (lane.length_m, ls.len() as f64, lane.speed_limit_mps)
        }
    };
    let mut x = header("meandata", "meandata_file.xsd");
    let mut rows = 0;
    let id = if lanes { "lanedata" } else { "edgedata" };
    for (i, (b, e)) in ivs.iter().enumerate() {
        let span = (e - b) as f64 / 1e9;
        let _ = writeln!(
            x,
            "    <interval begin=\"{}\" end=\"{}\" id=\"{id}\">",
            c.t(*b),
            c.t(*e)
        );
        let mut by_edge: BTreeMap<String, Vec<(&String, &Measures)>> = BTreeMap::new();
        for (k, v) in &m[i] {
            let edge = if lanes {
                c.edge_id(Some(c.ids.lane_of[k])).unwrap_or("").to_string()
            } else {
                k.clone()
            };
            by_edge.entry(edge).or_default().push((k, v));
        }
        for (edge, list) in &by_edge {
            if lanes {
                let _ = writeln!(x, "        <edge id=\"{}\">", esc(edge));
            }
            for (k, v) in list {
                let (len, n, limit) = geom(k);
                let tag = if lanes { "lane" } else { "edge" };
                let pad = if lanes { "            " } else { "        " };
                let _ = write!(
                    x,
                    "{pad}<{tag} id=\"{}\" sampledSeconds=\"{}\"",
                    esc(k),
                    c.f(v.sampled)
                );
                if v.sampled > 0.0 {
                    let speed = v.dist / v.sampled;
                    let density = v.sampled / span / (len / 1000.0);
                    if speed > 0.0 {
                        let _ = write!(x, " traveltime=\"{}\"", c.f(len / speed));
                    }
                    let _ = write!(x, " density=\"{}\"", c.f(density));
                    if !lanes {
                        let _ = write!(x, " laneDensity=\"{}\"", c.f(density / n));
                    }
                    let _ = write!(
                        x,
                        " occupancy=\"{}\" waitingTime=\"{}\" timeLoss=\"{}\" speed=\"{}\"",
                        c.f(100.0 * v.occupied / (span * len * n)),
                        c.f(v.waiting),
                        c.f(v.time_loss),
                        c.f(speed)
                    );
                    if limit > 0.0 {
                        let _ = write!(x, " speedRelative=\"{}\"", c.f(speed / limit));
                    }
                }
                let _ = write!(
                    x,
                    " departed=\"{}\" arrived=\"{}\" entered=\"{}\" left=\"{}\"",
                    v.departed, v.arrived, v.entered, v.left
                );
                if lanes {
                    let _ = write!(
                        x,
                        " laneChangedFrom=\"{}\" laneChangedTo=\"{}\"",
                        v.lc_from, v.lc_to
                    );
                }
                let _ = writeln!(x, " vaporized=\"{}\"/>", v.vaporized);
                rows += 1;
            }
            if lanes {
                x.push_str("        </edge>\n");
            }
        }
        x.push_str("    </interval>\n");
    }
    x.push_str("</meandata>\n");
    (x, rows)
}

/// summary: the network's state at every recorded step.
fn summary(c: &Ctx, o: &SumoOptions, tr: &Tracks) -> (String, usize) {
    struct Life<'a> {
        track: &'a Track,
        depart: u64,
        end: Option<u64>,
        requested: Option<u64>,
        arrived: bool,
    }
    let lives: Vec<Life> = tr
        .by_actor
        .values()
        .filter(|t| !t.is_person())
        .filter_map(|t| {
            Some(Life {
                track: t,
                depart: t.depart()?,
                end: t.despawn.as_ref().map(|d| d.t),
                requested: t.spawn.as_ref().and_then(|s| s.requested_t),
                arrived: t.despawn.as_ref().is_some_and(|d| d.cause == "trip-complete"),
            })
        })
        .collect();
    let mut cursor = vec![0usize; lives.len()];
    let mut x = header("summary", "summary_file.xsd");
    let mut rows = 0;
    for &t in tr.times.iter().filter(|t| on_period(**t, o.period_s)) {
        let (mut inserted, mut waiting, mut running, mut ended, mut arrived) = (0, 0, 0, 0, 0);
        let (mut halting, mut n_speed, mut sum_speed, mut sum_rel, mut n_rel) =
            (0u64, 0u64, 0.0, 0.0, 0u64);
        let (mut tt_sum, mut tt_n, mut delay_sum, mut delay_n) = (0.0, 0u64, 0.0, 0u64);
        for (i, l) in lives.iter().enumerate() {
            if l.depart > t {
                if l.requested.is_some_and(|r| r <= t) {
                    waiting += 1;
                }
                continue;
            }
            inserted += 1;
            if let Some(r) = l.requested {
                delay_sum += l.depart.saturating_sub(r) as f64 / 1e9;
                delay_n += 1;
            }
            if let Some(e) = l.end
                && e <= t
            {
                ended += 1;
                if l.arrived {
                    arrived += 1;
                }
                tt_sum += e.saturating_sub(l.depart) as f64 / 1e9;
                tt_n += 1;
                continue;
            }
            running += 1;
            // The vehicle's newest sample at or before `t`.
            let s = &l.track.samples;
            while cursor[i] + 1 < s.len() && s[cursor[i] + 1].t <= t {
                cursor[i] += 1;
            }
            if let Some(cur) = s.get(cursor[i]).filter(|x| x.t <= t) {
                n_speed += 1;
                sum_speed += cur.speed;
                if cur.speed <= HALTING_SPEED_MPS {
                    halting += 1;
                }
                if let Some(v) = c.limit(cur.lane) {
                    sum_rel += cur.speed / v;
                    n_rel += 1;
                }
            }
        }
        let mean = |s: f64, n: u64| if n == 0 { -1.0 } else { s / n as f64 };
        let _ = writeln!(
            x,
            "    <step time=\"{}\" loaded=\"{}\" inserted=\"{inserted}\" running=\"{running}\" \
             waiting=\"{waiting}\" ended=\"{ended}\" arrived=\"{arrived}\" collisions=\"0\" \
             teleports=\"0\" halting=\"{halting}\" stopped=\"0\" meanWaitingTime=\"{}\" \
             meanTravelTime=\"{}\" meanSpeed=\"{}\" meanSpeedRelative=\"{}\"/>",
            c.t(t),
            inserted + waiting,
            c.f(mean(delay_sum, delay_n)),
            c.f(mean(tt_sum, tt_n)),
            c.f(mean(sum_speed, n_speed)),
            c.f(mean(sum_rel, n_rel)),
        );
        rows += 1;
    }
    x.push_str("</summary>\n");
    (x, rows)
}

fn detector_lane(c: &Ctx, d: &DetectorSpec, e2: bool) -> Result<LaneId, String> {
    let l = *c.ids.lane_of.get(&d.lane).ok_or_else(|| {
        format!(
            "detector '{}' names lane '{}', which this world has no SUMO lane of",
            d.id, d.lane
        )
    })?;
    let len = c.lane_len(l);
    let end = d.pos_m + if e2 { d.length_m } else { 0.0 };
    if end > len + 1e-6 {
        return Err(format!(
            "detector '{}' ends at {end:.2} m on lane '{}', which is {len:.2} m long",
            d.id, d.lane
        ));
    }
    Ok(l)
}

/// E1 induction loops: a vehicle enters when its front reaches the loop and has passed
/// when its rear leaves it (SUMO `nVehContrib` counts the vehicles that passed).
fn e1(c: &Ctx, o: &SumoOptions, tr: &Tracks) -> Result<(String, usize), String> {
    struct Pass {
        enter: f64,
        exit: Option<f64>,
        passed: bool,
        speed: f64,
        length: f64,
    }
    let ivs = intervals(o.period_s, tr.end_ns);
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- generated by v2xw from a recording \
         (SUMO-format output, WP-06) -->\n<detector \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xsi:noNamespaceSchemaLocation=\"http://sumo.dlr.de/xsd/det_e1_file.xsd\">\n",
    );
    let mut rows = 0;
    let mut per_det: Vec<(String, Vec<Pass>)> = Vec::new();
    for d in &o.detectors {
        let lane = detector_lane(c, d, false)?;
        let pos = d.pos_m;
        let mut passes = Vec::new();
        for t in tr.by_actor.values().filter(|t| !t.is_person()) {
            let len = t.length();
            let mut open: Option<Pass> = None;
            let mut prev: Option<&Sample> = None;
            for s in &t.samples {
                let on = s.lane == Some(lane);
                let ts = s.t as f64 / 1e9;
                let p_on = prev.is_some_and(|p| p.lane == Some(lane));
                if on {
                    let (r, f) = (s.s, s.s + len);
                    if open.is_none() {
                        let entered = match prev.filter(|_| p_on) {
                            // The front crossed the loop since the last sample.
                            Some(p) if p.s + len < pos && f >= pos => {
                                let tp = p.t as f64 / 1e9;
                                Some(tp + (ts - tp) * (pos - p.s - len) / (f - p.s - len))
                            }
                            Some(_) => None,
                            // Came onto the lane already over the loop.
                            None if r <= pos && f >= pos => Some(ts),
                            None => None,
                        };
                        if let Some(enter) = entered {
                            open = Some(Pass {
                                enter,
                                exit: None,
                                passed: false,
                                speed: s.speed,
                                length: len,
                            });
                        }
                    }
                    if let Some(pass) = open.as_mut()
                        && r > pos
                    {
                        let exit = match prev.filter(|_| p_on) {
                            Some(p) if p.s <= pos && r > p.s => {
                                let tp = p.t as f64 / 1e9;
                                tp + (ts - tp) * (pos - p.s) / (r - p.s)
                            }
                            _ => ts,
                        };
                        pass.exit = Some(exit.max(pass.enter));
                        pass.passed = true;
                        passes.push(open.take().expect("an open passage"));
                    }
                } else if p_on && let Some(mut pass) = open.take() {
                    // Off the lane while over the loop: forward onto the next edge is a
                    // passage; a lane change onto a sibling lane is not.
                    let forward = c.edge_id(s.lane) != c.edge_id(Some(lane));
                    pass.exit = Some(ts.max(pass.enter));
                    pass.passed = forward;
                    passes.push(pass);
                }
                prev = Some(s);
            }
            if let Some(mut pass) = open.take() {
                let stop = t.despawn.as_ref().map_or(tr.end_ns, |d| d.t);
                pass.exit = Some((stop as f64 / 1e9).max(pass.enter));
                passes.push(pass);
            }
        }
        per_det.push((d.id.clone(), passes));
    }
    for (b, e) in &ivs {
        let (b, e) = (*b as f64 / 1e9, *e as f64 / 1e9);
        let span = e - b;
        for (id, passes) in &per_det {
            let done: Vec<&Pass> = passes
                .iter()
                .filter(|p| p.passed && p.exit.is_some_and(|x| x >= b && x < e))
                .collect();
            let entered = passes.iter().filter(|p| p.enter >= b && p.enter < e).count();
            let occ: f64 = passes
                .iter()
                .map(|p| (p.exit.unwrap_or(e).min(e) - p.enter.max(b)).max(0.0))
                .sum();
            let n = done.len();
            let mean = |f: &dyn Fn(&Pass) -> f64| {
                if n == 0 {
                    -1.0
                } else {
                    done.iter().map(|p| f(p)).sum::<f64>() / n as f64
                }
            };
            let harmonic = if n == 0 || done.iter().any(|p| p.speed <= 0.0) {
                -1.0
            } else {
                n as f64 / done.iter().map(|p| 1.0 / p.speed).sum::<f64>()
            };
            let _ = writeln!(
                x,
                "    <interval begin=\"{}\" end=\"{}\" id=\"{}\" nVehContrib=\"{n}\" \
                 flow=\"{}\" occupancy=\"{}\" speed=\"{}\" harmonicMeanSpeed=\"{}\" \
                 length=\"{}\" nVehEntered=\"{entered}\"/>",
                c.f(b),
                c.f(e),
                esc(id),
                c.f(n as f64 * 3600.0 / span),
                c.f(100.0 * occ / span),
                c.f(mean(&|p| p.speed)),
                c.f(harmonic),
                c.f(mean(&|p| p.length)),
            );
            rows += 1;
        }
    }
    x.push_str("</detector>\n");
    Ok((x, rows))
}

/// E2 lane-area detectors, with SUMO's jam and halting definitions.
fn e2(c: &Ctx, o: &SumoOptions, tr: &Tracks) -> Result<(String, usize), String> {
    let ivs = intervals(o.period_s, tr.end_ns);
    let step = tr.step_ns as f64 / 1e9;
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- generated by v2xw from a recording \
         (SUMO-format output, WP-06) -->\n<detector \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xsi:noNamespaceSchemaLocation=\"http://sumo.dlr.de/xsd/det_e2_file.xsd\">\n",
    );
    let mut rows = 0;
    // Each detector's per-interval aggregates, written interval-major like SUMO.
    let mut lines: Vec<Vec<String>> = vec![Vec::new(); ivs.len()];
    for d in &o.detectors {
        let lane = detector_lane(c, d, true)?;
        let (lo, hi) = (d.pos_m, d.pos_m + d.length_m);
        // Per instant: the vehicles on the area, with (actor, rear, front, speed, dt).
        let mut at: BTreeMap<u64, Vec<(u32, f64, f64, f64, f64, f64)>> = BTreeMap::new();
        for (a, t) in tr.by_actor.iter().filter(|(_, t)| !t.is_person()) {
            let len = t.length();
            let desired = t.desired();
            for s in t.samples.iter().filter(|s| s.lane == Some(lane)) {
                let (r, f) = (s.s, s.s + len.max(0.01));
                if f > lo && r < hi {
                    let loss = c.time_loss(s, desired);
                    at.entry(s.t)
                        .or_default()
                        .push((*a, r.max(lo), f.min(hi), s.speed, s.dt, loss));
                }
            }
        }
        for (i, (b, e)) in ivs.iter().enumerate() {
            let mut sampled = 0.0;
            let mut dist = 0.0;
            let mut loss = 0.0;
            let mut seen = BTreeSet::new();
            let (mut entered, mut left) = (0u64, 0u64);
            let (mut occ_sum, mut occ_max) = (0.0f64, 0.0f64);
            let (mut veh_sum, mut veh_max) = (0u64, 0u64);
            let (mut jam_v_sum, mut jam_m_sum) = (0u64, 0.0f64);
            let (mut maxjam_v_sum, mut maxjam_m_sum) = (0u64, 0.0f64);
            let (mut maxjam_v, mut maxjam_m) = (0u64, 0.0f64);
            let mut steps = 0u64;
            let mut started_halts = 0u64;
            // Halting durations: whole stay on the area, and within this interval.
            let mut halt_total: BTreeMap<u32, f64> = BTreeMap::new();
            let mut halt_interval: BTreeMap<u32, f64> = BTreeMap::new();
            let mut slow_for: BTreeMap<u32, f64> = BTreeMap::new();
            let mut halting_now: BTreeSet<u32> = BTreeSet::new();
            let mut on_before: BTreeSet<u32> = BTreeSet::new();
            let none = Vec::new();
            for t in &tr.times {
                let list = at.get(t).unwrap_or(&none);
                let on: BTreeSet<u32> = list.iter().map(|v| v.0).collect();
                // Halting state carries across the interval boundary.
                let mut slow_next = BTreeMap::new();
                let mut halting_next = BTreeSet::new();
                let mut halting: Vec<(f64, f64)> = Vec::new();
                for &(a, r, f, v, dt, _) in list {
                    if v < o.speed_threshold_mps {
                        let since = slow_for.get(&a).copied().unwrap_or(0.0) + dt;
                        slow_next.insert(a, since);
                        if since >= o.time_threshold_s {
                            halting_next.insert(a);
                            halting.push((r, f));
                            if *t >= *b && *t < *e {
                                *halt_total.entry(a).or_default() += dt;
                                *halt_interval.entry(a).or_default() += dt;
                                if !halting_now.contains(&a) {
                                    started_halts += 1;
                                }
                            } else if *t < *b {
                                *halt_total.entry(a).or_default() += dt;
                            }
                        }
                    }
                }
                slow_for = slow_next;
                halting_now = halting_next;
                if *t < *b || *t >= *e {
                    on_before = on;
                    continue;
                }
                steps += 1;
                entered += on.difference(&on_before).count() as u64;
                left += on_before.difference(&on).count() as u64;
                let mut occ = 0.0;
                for &(a, r, f, v, dt, l) in list {
                    seen.insert(a);
                    sampled += dt;
                    dist += v * dt;
                    loss += l;
                    occ += f - r;
                }
                let occ = 100.0 * occ / d.length_m;
                occ_sum += occ;
                occ_max = occ_max.max(occ);
                veh_sum += list.len() as u64;
                veh_max = veh_max.max(list.len() as u64);
                // Jams: halting vehicles, downstream first, joined when the gap between
                // a leader's rear and its follower's front is under the jam threshold.
                halting.sort_by(|p, q| q.1.total_cmp(&p.1));
                let mut jams: Vec<(u64, f64)> = Vec::new();
                let mut cur: Option<(u64, f64, f64)> = None; // (vehicles, front, rear)
                for (r, f) in halting {
                    cur = match cur {
                        Some((n, front, rear)) if rear - f <= o.jam_threshold_m => {
                            Some((n + 1, front, r))
                        }
                        Some((n, front, rear)) => {
                            jams.push((n, front - rear));
                            Some((1, f, r))
                        }
                        None => Some((1, f, r)),
                    };
                }
                if let Some((n, front, rear)) = cur {
                    jams.push((n, front - rear));
                }
                let (mv, mm) = jams
                    .iter()
                    .fold((0u64, 0.0f64), |(v, m), j| (v.max(j.0), m.max(j.1)));
                jam_v_sum += jams.iter().map(|j| j.0).sum::<u64>();
                jam_m_sum += jams.iter().map(|j| j.1).sum::<f64>();
                maxjam_v_sum += mv;
                maxjam_m_sum += mm;
                maxjam_v = maxjam_v.max(mv);
                maxjam_m = maxjam_m.max(mm);
                on_before = on;
            }
            let span_steps = (((*e - *b) as f64 / 1e9) / step).max(1.0);
            let n_seen = seen.len();
            let per = |s: f64, n: f64| if n > 0.0 { s / n } else { -1.0 };
            let hs: Vec<f64> = seen.iter().map(|a| halt_total.get(a).copied().unwrap_or(0.0)).collect();
            let hi_: Vec<f64> =
                seen.iter().map(|a| halt_interval.get(a).copied().unwrap_or(0.0)).collect();
            let sum = |v: &[f64]| v.iter().sum::<f64>();
            let max = |v: &[f64]| v.iter().copied().fold(0.0, f64::max);
            lines[i].push(format!(
                "    <interval begin=\"{}\" end=\"{}\" id=\"{}\" sampledSeconds=\"{}\" \
                 nVehEntered=\"{entered}\" nVehLeft=\"{left}\" nVehSeen=\"{n_seen}\" \
                 meanSpeed=\"{}\" meanTimeLoss=\"{}\" meanOccupancy=\"{}\" maxOccupancy=\"{}\" \
                 meanMaxJamLengthInVehicles=\"{}\" meanMaxJamLengthInMeters=\"{}\" \
                 maxJamLengthInVehicles=\"{maxjam_v}\" maxJamLengthInMeters=\"{}\" \
                 jamLengthInVehiclesSum=\"{jam_v_sum}\" jamLengthInMetersSum=\"{}\" \
                 meanHaltingDuration=\"{}\" maxHaltingDuration=\"{}\" \
                 haltingDurationSum=\"{}\" meanIntervalHaltingDuration=\"{}\" \
                 maxIntervalHaltingDuration=\"{}\" intervalHaltingDurationSum=\"{}\" \
                 startedHalts=\"{}\" meanVehicleNumber=\"{}\" maxVehicleNumber=\"{veh_max}\"/>",
                c.t(*b),
                c.t(*e),
                esc(&d.id),
                c.f(sampled),
                c.f(per(dist, sampled)),
                c.f(per(loss, n_seen as f64)),
                c.f(occ_sum / span_steps),
                c.f(occ_max),
                c.f(maxjam_v_sum as f64 / span_steps),
                c.f(maxjam_m_sum / span_steps),
                c.f(maxjam_m),
                c.f(jam_m_sum),
                c.f(per(sum(&hs), n_seen as f64).max(0.0)),
                c.f(max(&hs)),
                c.f(sum(&hs)),
                c.f(per(sum(&hi_), n_seen as f64).max(0.0)),
                c.f(max(&hi_)),
                c.f(sum(&hi_)),
                c.f(started_halts as f64),
                c.f(veh_sum as f64 / span_steps),
            ));
            let _ = steps;
        }
    }
    for l in lines.into_iter().flatten() {
        x.push_str(&l);
        x.push('\n');
        rows += 1;
    }
    x.push_str("</detector>\n");
    Ok((x, rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_never_print_a_negative_zero() {
        assert_eq!(fmt(-0.0001, 2), "0.00");
        assert_eq!(fmt(-1.0, 2), "-1.00");
        assert_eq!(fmt(2.345, 1), "2.3");
    }

    #[test]
    fn angles_are_compass_degrees() {
        assert!((angle_deg(0.0) - 90.0).abs() < 1e-9, "east is 90");
        assert!((angle_deg(std::f64::consts::FRAC_PI_2)).abs() < 1e-9, "north is 0");
        assert!((angle_deg(std::f64::consts::PI) - 270.0).abs() < 1e-9, "west is 270");
    }

    #[test]
    fn options_refuse_what_an_exporter_does_not_take() {
        assert!(options_of("sumo-fcd", &serde_json::json!({"period_s": 1.0})).is_ok());
        let e = options_of("sumo-fcd", &serde_json::json!({"write_unfinished": true}))
            .expect_err("fcd has no write_unfinished");
        assert_eq!(e.0, "opts.write_unfinished");
        let e = options_of("sumo-e1", &serde_json::json!({})).expect_err("no detector");
        assert_eq!(e.0, "opts.detectors");
        let e = options_of(
            "sumo-e2",
            &serde_json::json!({"detectors": [{"id": "a", "lane": "e1_0", "pos_m": 1.0}]}),
        )
        .expect_err("an E2 needs a length");
        assert_eq!(e.0, "opts.detectors[0].length_m");
    }

    #[test]
    fn intervals_cover_the_run() {
        assert_eq!(intervals(None, 5), vec![(0, 5)]);
        assert_eq!(
            intervals(Some(2e-9), 5),
            vec![(0, 2), (2, 4), (4, 5)]
        );
    }
}
