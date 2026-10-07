//! `mobility/demand/gtfs` — public transport from a GTFS feed.
//!
//! The feed's routes, stops and timetables become vehicles that run the line through its
//! stops, stop at each one, dwell, and leave — on the same roads, in the same traffic and
//! under the same signals as everyone else:
//!
//! 1. **Read** the feed ([`gtfs`]) for one service day (the scenario's date), keeping the
//!    calls at stops inside the world.
//! 2. **Place** each distinct stop pattern on the lane graph with the HMM matcher
//!    ([`mapping`]), which also gives the route through the stops.
//! 3. **Schedule.** Each trip (and each instance of a `frequencies.txt` trip) departs its
//!    first stop inside the world at its timetabled time, converted to run time with the
//!    scenario's local start time. A trip already under way when the run starts joins at
//!    its next stop.
//! 4. **Run.** [`TransitDemand`] spawns each vehicle on its matched route (the
//!    planned-route seam of [`v2xw_mobility::demand::hooks`]), follows it every step, brakes
//!    it to a stop at each stop point — the deceleration recomputed each step as
//!    `v² / 2d` so it comes to rest at the pole, starting once that reaches the comfortable
//!    rate — holds it for the dwell ([`dwell`]), and, when `hold_early` is set, until the
//!    timetabled departure if it is early (timepoint holding, TCQSM ch. 4). Every call is
//!    logged with its scheduled and actual times, and [`TransitDemand::report`] gives
//!    on-time performance by the TCQSM definition (1 min early to 5 min late).
//!
//! **Street-level rail.** Light rail and streetcars (`route_type` 0) are placed on the
//! road lanes they share — Portland's Streetcar runs in mixed traffic on 10th and 11th
//! Avenues, MAX on the transit mall's lanes — and run as the `bus` vehicle class, since the
//! engine has no rail class: right length class, wrong dynamics. Rail in its own right of
//! way, off the road graph, is not placed.

pub mod dwell;
pub mod gtfs;
pub mod mapping;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_core::time::{SimTime, ns_to_secs, secs_to_ns};
use v2xw_mobility::demand::SpeedLaw;
use v2xw_mobility::demand::hooks::ActorProgress;
use v2xw_mobility::traits::Demand;
use v2xw_mobility::views::{MobilityCommand, TripRequest};
use v2xw_mobility::{MobCtx, VehicleClass};
use v2xw_world::{ClassMask, World};

use crate::error::{DemandError, Result};
use crate::graph::LaneGraph;
use dwell::DwellSpec;
use mapping::MatchParams;

/// The model id (also the scenario kind).
pub const MODEL_ID: &str = "mobility/demand/gtfs";

/// The transit demand's parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TransitParams {
    /// The feed directory (unzipped GTFS).
    pub feed: PathBuf,
    /// The service date, `YYYYMMDD`; `None` takes the scenario's date.
    pub date: Option<u32>,
    /// The local time of day the run starts, seconds after midnight.
    pub start_s_of_day: f64,
    /// The run's length, seconds.
    pub duration_s: f64,
    /// Route types to run (empty: buses, trolleybuses and street-level rail —
    /// 0, 3, 5, 11 and the extended 700–799 and 900–999).
    pub route_types: Vec<u16>,
    /// Only these routes, by short name or id (empty: all).
    pub routes: Vec<String>,
    /// The dwell model.
    pub dwell: DwellSpec,
    /// Hold an early vehicle until its timetabled departure.
    pub hold_early: bool,
    /// The comfortable deceleration a vehicle brakes to a stop at, m/s².
    pub decel_mps2: f64,
    /// A vehicle at rest within this distance of the stop point has arrived, metres.
    pub stop_tolerance_m: f64,
    /// The stop matcher.
    pub matching: MatchParams,
    /// At most this many vehicle trips (0: all).
    pub max_trips: usize,
    /// The seed of the dwell draws.
    pub seed: u64,
}

impl Default for TransitParams {
    fn default() -> Self {
        Self {
            feed: PathBuf::new(),
            date: None,
            start_s_of_day: 8.0 * 3600.0,
            duration_s: 3600.0,
            route_types: Vec::new(),
            routes: Vec::new(),
            dwell: DwellSpec::default(),
            hold_early: true,
            decel_mps2: 1.2,
            stop_tolerance_m: 4.0,
            matching: MatchParams::default(),
            max_trips: 0,
            seed: 0x7_2A51,
        }
    }
}

fn runs_on_road(route_type: u16) -> bool {
    matches!(route_type, 0 | 3 | 5 | 11) || (700..800).contains(&route_type) || (900..1000).contains(&route_type)
}

/// One scheduled call of a vehicle trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Call {
    /// The GTFS stop id.
    pub stop_id: String,
    /// The stop's name.
    pub name: String,
    /// Index into the trip's route.
    pub route_index: usize,
    /// Where on that lane the vehicle stops, metres (front bumper).
    pub s_m: f64,
    /// Scheduled arrival, run seconds.
    pub arrival_s: f64,
    /// Scheduled departure, run seconds.
    pub departure_s: f64,
}

/// One vehicle trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitTrip {
    /// The GTFS trip id (with `#k` for a frequency instance).
    pub trip_id: String,
    /// The line's name.
    pub route: String,
    /// Its route type.
    pub route_type: u16,
    /// Departure from the first call, run seconds.
    pub depart_s: f64,
    /// The lanes driven.
    pub lanes: Vec<LaneId>,
    /// Distance from the route start to the start of each lane, metres.
    pub cum_m: Vec<f64>,
    /// The calls.
    pub calls: Vec<Call>,
}

/// What [`build_schedule`] made of a feed.
#[derive(Debug, Clone, Default)]
pub struct Schedule {
    /// The vehicle trips, by departure.
    pub trips: Vec<TransitTrip>,
    /// Stop patterns that could not be placed at all.
    pub unplaced_patterns: usize,
    /// Distinct stops placed.
    pub placed_stops: usize,
}

/// Builds the vehicle trips of a feed on `world` (steps 1-3 of the module docs).
///
/// # Errors
/// Feed reading errors.
pub fn build_schedule(world: &World, p: &TransitParams, date: u32) -> Result<Schedule> {
    let proj = world.projection();
    let b = world.bbox;
    let corner = |x: f64, y: f64| {
        // Inverse of the ENU projection, near enough for a box filter: degrees per metre.
        let o = proj.origin();
        (
            o.lat_deg + y / proj.metres_per_degree_latitude(),
            o.lon_deg + x / proj.metres_per_degree_longitude(),
        )
    };
    let (s, w) = corner(b.min.x, b.min.y);
    let (n, e) = corner(b.max.x, b.max.y);
    let types = if p.route_types.is_empty() {
        Vec::new()
    } else {
        p.route_types.clone()
    };
    let feed = gtfs::Feed::load(
        &p.feed,
        &gtfs::Filter {
            bbox: Some([s, w, n, e]),
            date: Some(date),
            route_types: types,
        },
    )?;
    let graph = LaneGraph::new(world, ClassMask::BUS);
    let mut patterns: BTreeMap<Vec<u32>, Option<mapping::Matched>> = BTreeMap::new();
    let mut out: Vec<TransitTrip> = Vec::new();
    for (trip_id, calls) in &feed.stop_times {
        let trip = &feed.trips[trip_id];
        let Some(route) = feed.routes.get(&trip.route_id) else {
            continue;
        };
        if !runs_on_road(route.route_type) && p.route_types.is_empty() {
            continue;
        }
        if !p.routes.is_empty() && !p.routes.iter().any(|r| *r == route.name || *r == route.id) {
            continue;
        }
        if calls.len() < 2 {
            continue;
        }
        let pattern: Vec<u32> = calls.iter().map(|c| c.stop).collect();
        let matched = patterns
            .entry(pattern.clone())
            .or_insert_with(|| {
                let pts: Vec<Vec3> = pattern
                    .iter()
                    .map(|i| {
                        let st = &feed.stops[&feed.stop_ids[*i as usize]];
                        let (x, y) = proj.to_enu(st.lat, st.lon);
                        Vec3::new(x, y, 0.0)
                    })
                    .collect();
                mapping::match_pattern(world, &graph, &pts, &p.matching)
            })
            .clone();
        let Some(m) = matched else { continue };
        if m.stops.len() < 2 {
            continue;
        }
        // Times along the matched stretch, interpolating calls without times.
        let times: Vec<(f64, f64)> = interpolate_times(calls);
        let instances: Vec<(String, f64)> = match feed.frequencies.get(trip_id) {
            None => vec![(trip_id.clone(), 0.0)],
            Some(fs) => {
                let first = times[m.stops[0].stop].1;
                let mut v = Vec::new();
                for f in fs {
                    let mut t = f64::from(f.start_s);
                    let mut k = 0;
                    while t < f64::from(f.end_s) {
                        v.push((format!("{trip_id}#{k}"), t - first));
                        t += f64::from(f.headway_s);
                        k += 1;
                    }
                }
                v
            }
        };
        for (id, shift) in instances {
            let mut calls_run: Vec<Call> = m
                .stops
                .iter()
                .map(|ps| {
                    let c = calls[ps.stop];
                    let st = &feed.stops[&feed.stop_ids[c.stop as usize]];
                    let (arr, dep) = times[ps.stop];
                    let lane_len = graph.length_m(ps.node);
                    Call {
                        stop_id: st.id.clone(),
                        name: st.name.clone(),
                        route_index: ps.route_index,
                        s_m: ps.s_m.clamp((14.0f64).min(lane_len), (lane_len - 1.0).max(0.0)),
                        arrival_s: arr + shift - p.start_s_of_day,
                        departure_s: dep + shift - p.start_s_of_day,
                    }
                })
                .collect();
            // Join under way: drop calls already departed at the run start.
            let first_live = calls_run.iter().position(|c| c.departure_s >= 0.0);
            let Some(first_live) = first_live else { continue };
            calls_run.drain(..first_live);
            if calls_run.len() < 2 || calls_run[0].departure_s > p.duration_s {
                continue;
            }
            let offset = calls_run[0].route_index;
            let nodes = &m.route[offset..];
            for c in &mut calls_run {
                c.route_index -= offset;
            }
            let lanes: Vec<LaneId> = nodes.iter().map(|n| graph.lane(*n)).collect();
            let mut cum = Vec::with_capacity(nodes.len());
            let mut acc = 0.0;
            for n in nodes {
                cum.push(acc);
                acc += graph.length_m(*n);
            }
            out.push(TransitTrip {
                trip_id: id,
                route: route.name.clone(),
                route_type: route.route_type,
                depart_s: calls_run[0].departure_s.max(0.0),
                lanes,
                cum_m: cum,
                calls: calls_run,
            });
        }
    }
    out.sort_by(|a, b| a.depart_s.total_cmp(&b.depart_s).then(a.trip_id.cmp(&b.trip_id)));
    if p.max_trips > 0 {
        out.truncate(p.max_trips);
    }
    let placed_stops = patterns
        .values()
        .flatten()
        .flat_map(|m| m.stops.iter().map(|s| s.node))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    Ok(Schedule {
        trips: out,
        unplaced_patterns: patterns.values().filter(|m| m.is_none()).count(),
        placed_stops,
    })
}

/// Arrival and departure of every call, seconds of the service day, a call without times
/// interpolated by position between the timed calls around it (GTFS allows untimed
/// intermediate stops).
fn interpolate_times(calls: &[gtfs::StopTime]) -> Vec<(f64, f64)> {
    let timed: Vec<Option<(f64, f64)>> = calls
        .iter()
        .map(|c| match (c.arrival_s, c.departure_s) {
            (Some(a), Some(d)) => Some((f64::from(a), f64::from(d))),
            (Some(a), None) => Some((f64::from(a), f64::from(a))),
            (None, Some(d)) => Some((f64::from(d), f64::from(d))),
            (None, None) => None,
        })
        .collect();
    let mut out = vec![(0.0, 0.0); calls.len()];
    let mut last: Option<(usize, f64)> = None;
    for i in 0..calls.len() {
        if let Some(t) = timed[i] {
            if let Some((li, lt)) = last {
                for (k, slot) in out.iter_mut().enumerate().take(i).skip(li + 1) {
                    let f = (k - li) as f64 / (i - li) as f64;
                    let v = lt + f * (t.0 - lt);
                    *slot = (v, v);
                }
            }
            out[i] = t;
            last = Some((i, t.1));
        } else if last.is_none() {
            out[i] = (0.0, 0.0);
        }
    }
    // Untimed calls after the last timed one take its time.
    if let Some((li, lt)) = last {
        for slot in out.iter_mut().skip(li + 1) {
            *slot = (lt, lt);
        }
    }
    out
}

/// One call as it happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallRecord {
    /// The vehicle trip.
    pub trip_id: String,
    /// The stop.
    pub stop_id: String,
    /// Scheduled arrival, run seconds.
    pub scheduled_s: f64,
    /// Actual arrival, run seconds (`None`: passed without stopping).
    pub arrived_s: Option<f64>,
    /// Actual departure, run seconds.
    pub departed_s: Option<f64>,
}

/// On-time performance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TransitReport {
    /// Calls served (stopped at).
    pub served: usize,
    /// Calls passed without a stop.
    pub missed: usize,
    /// Share of served calls between 1 min early and 5 min late (TCQSM's on-time window).
    pub on_time_share: f64,
    /// Mean arrival delay of served calls, seconds (negative: early).
    pub mean_delay_s: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct VehState {
    next: usize,
    dwell_until: Option<f64>,
    braking: bool,
    seen: bool,
}

/// The running transit demand (step 4 of the module docs).
#[derive(Debug, Clone)]
pub struct TransitDemand {
    params: TransitParams,
    schedule: Schedule,
    cursor: usize,
    next_seq: u64,
    by_seq: BTreeMap<u64, usize>,
    state: BTreeMap<u64, VehState>,
    actor_of: BTreeMap<u64, ActorId>,
    records: CallLog,
    card: ModelCard,
}

/// A shared handle on the call log, readable after the demand has been handed to the
/// engine (single-threaded; the lock is never contended).
pub type CallLog = std::sync::Arc<std::sync::Mutex<Vec<CallRecord>>>;

impl TransitDemand {
    /// The demand over a built schedule.
    pub fn new(params: TransitParams, schedule: Schedule) -> Self {
        let card = card(&params, &schedule);
        Self {
            params,
            schedule,
            cursor: 0,
            next_seq: 0,
            by_seq: BTreeMap::new(),
            state: BTreeMap::new(),
            actor_of: BTreeMap::new(),
            records: CallLog::default(),
            card,
        }
    }

    /// Reads the feed and builds the demand.
    ///
    /// # Errors
    /// Feed errors; [`DemandError::Invalid`] when no trip could be placed.
    pub fn from_feed(world: &World, params: TransitParams, date: u32) -> Result<Self> {
        let schedule = build_schedule(world, &params, date)?;
        if schedule.trips.is_empty() {
            return Err(DemandError::Invalid(format!(
                "no transit trip of {} on {date} could be placed on this world \
                 ({} stop patterns unplaced)",
                params.feed.display(),
                schedule.unplaced_patterns
            )));
        }
        Ok(Self::new(params, schedule))
    }

    /// The schedule.
    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    /// A handle on the call log, which stays readable after the demand has been handed
    /// to the engine.
    pub fn log(&self) -> CallLog {
        self.records.clone()
    }

    /// On-time performance of the calls logged so far.
    pub fn report(&self) -> TransitReport {
        report(&self.records.lock().map(|r| r.clone()).unwrap_or_default())
    }

    fn class_of(&self, _trip: &TransitTrip) -> VehicleClass {
        VehicleClass::Bus
    }
}

/// On-time performance of a call log.
pub fn report(records: &[CallRecord]) -> TransitReport {
    let served: Vec<&CallRecord> = records.iter().filter(|r| r.arrived_s.is_some()).collect();
    let missed = records.len() - served.len();
    let delays: Vec<f64> = served
        .iter()
        .map(|r| r.arrived_s.unwrap_or(0.0) - r.scheduled_s)
        .collect();
    let on_time = delays.iter().filter(|d| **d >= -60.0 && **d <= 300.0).count();
    let n = delays.len().max(1) as f64;
    TransitReport {
        served: served.len(),
        missed,
        on_time_share: on_time as f64 / n,
        mean_delay_s: v2xw_core::math::sum_ordered(delays.iter().copied()) / n,
    }
}

impl v2xw_core::model::Model for TransitDemand {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Demand for TransitDemand {
    fn spawns_in(&mut self, ctx: &mut dyn MobCtx, _from: SimTime, to: SimTime) -> Vec<TripRequest> {
        let mut out = Vec::new();
        while self.cursor < self.schedule.trips.len() {
            let i = self.cursor;
            let t = &self.schedule.trips[i];
            let depart = secs_to_ns(t.depart_s.max(0.0));
            if depart >= to {
                break;
            }
            self.cursor += 1;
            let seq = self.next_seq;
            self.next_seq += 1;
            let class = self.class_of(t);
            let desired_speed_mps = {
                let mut rng = ctx.rng(RngDomain::DesiredSpeed, EntityRef::custom(MODEL_ID, seq));
                SpeedLaw::ClassSpeedDev.draw(class, &mut rng)
            };
            self.by_seq.insert(seq, i);
            out.push(TripRequest {
                seq,
                t: depart,
                origin: t.lanes[0],
                origin_s_m: 0.0,
                destination: *t.lanes.last().expect("a route"),
                class,
                desired_speed_mps,
            });
        }
        out
    }

    fn planned_route(&self, _world: &World, trip: &TripRequest) -> Option<Vec<LaneId>> {
        let t = &self.schedule.trips[*self.by_seq.get(&trip.seq)?];
        (t.lanes.first() == Some(&trip.origin)).then(|| t.lanes.clone())
    }

    fn wants_progress(&self) -> bool {
        true
    }

    fn observe(&mut self, t: SimTime, progress: &[ActorProgress]) -> Vec<MobilityCommand> {
        let now = ns_to_secs(t);
        let mut out = Vec::new();
        let log = self.records.clone();
        let Ok(mut records) = log.lock() else {
            return out;
        };
        let present: std::collections::BTreeSet<u64> = progress.iter().map(|a| a.seq).collect();
        for a in progress {
            let Some(trip_idx) = self.by_seq.get(&a.seq).copied() else {
                continue;
            };
            self.actor_of.insert(a.seq, a.actor);
            let trip = &self.schedule.trips[trip_idx];
            let st = self.state.entry(a.seq).or_default();
            st.seen = true;
            // Calls behind the vehicle were missed (a detour, or it overshot one).
            while let Some(c) = trip.calls.get(st.next) {
                if a.route_index > c.route_index && st.dwell_until.is_none() {
                    records.push(CallRecord {
                        trip_id: trip.trip_id.clone(),
                        stop_id: c.stop_id.clone(),
                        scheduled_s: c.arrival_s,
                        arrived_s: None,
                        departed_s: None,
                    });
                    st.next += 1;
                    st.braking = false;
                } else {
                    break;
                }
            }
            let Some(call) = trip.calls.get(st.next) else {
                continue;
            };
            if let Some(until) = st.dwell_until {
                if now >= until {
                    if let Some(r) = records
                        .iter_mut()
                        .rev()
                        .find(|r| r.trip_id == trip.trip_id && r.stop_id == call.stop_id && r.departed_s.is_none())
                    {
                        r.departed_s = Some(now);
                    }
                    st.dwell_until = None;
                    st.next += 1;
                    st.braking = false;
                }
                continue;
            }
            let here = trip.cum_m.get(a.route_index).copied().unwrap_or(0.0) + a.s_m;
            let d = trip.cum_m.get(call.route_index).copied().unwrap_or(0.0) + call.s_m - here;
            let tol = self.params.stop_tolerance_m;
            let v = a.speed_mps;
            if v < 0.3 && d.abs() <= tol.max(1.0) + if d < 0.0 { 8.0 } else { 0.0 } {
                // Arrived: dwell, and hold an early vehicle to its timetable.
                let mut rng = RngStream::derive(
                    self.params.seed,
                    RngDomain::plugin(MODEL_ID),
                    EntityRef::custom(MODEL_ID, (a.seq << 16) | st.next as u64),
                );
                let dwell = self
                    .params
                    .dwell
                    .dwell_s((call.departure_s - call.arrival_s).max(0.0), &mut rng);
                let mut until = now + dwell;
                if self.params.hold_early {
                    until = until.max(call.departure_s);
                }
                st.dwell_until = Some(until);
                st.braking = false;
                records.push(CallRecord {
                    trip_id: trip.trip_id.clone(),
                    stop_id: call.stop_id.clone(),
                    scheduled_s: call.arrival_s,
                    arrived_s: Some(now),
                    departed_s: None,
                });
                out.push(MobilityCommand::Stop {
                    actor: a.actor,
                    until: Some(secs_to_ns(until)),
                });
                continue;
            }
            if d > tol {
                let needed = v * v / (2.0 * d.max(0.5));
                if needed >= self.params.decel_mps2 || (st.braking && v >= 0.3) {
                    st.braking = true;
                    out.push(MobilityCommand::Brake {
                        actor: a.actor,
                        decel_mps2: needed.clamp(0.3, 3.0),
                        hold_until: None,
                    });
                } else if st.braking && v < 0.3 {
                    // Stopped short (a queue, a red light): the forced stop would hold it
                    // there for good, so release it to creep up to the pole.
                    st.braking = false;
                    out.push(MobilityCommand::Stop {
                        actor: a.actor,
                        until: Some(t),
                    });
                }
            }
        }
        // Vehicles that have left.
        let gone: Vec<u64> = self
            .state
            .iter()
            .filter(|(s, st)| st.seen && !present.contains(s))
            .map(|(s, _)| *s)
            .collect();
        for s in gone {
            self.state.remove(&s);
            self.actor_of.remove(&s);
        }
        out
    }
}

fn card(p: &TransitParams, s: &Schedule) -> ModelCard {
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "Public transport from a GTFS feed: stop patterns placed on lanes by HMM map \
         matching, vehicles run on timetable, stopping and dwelling at each stop in traffic.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    let mut decel = Parameter::new(
        "decel_mps2",
        "m/s^2",
        serde_json::json!(p.decel_mps2),
        Source::todo_calibrate("comfortable stopping deceleration"),
    );
    decel.calibration = Some(
        "the agency's AVL trajectories approaching stops (deceleration onset and rate)"
            .to_string(),
    );
    card.parameters = vec![decel];
    card.parameters.extend(p.dwell.parameters());
    card.parameters.push(Parameter::new(
        "matching",
        "-",
        serde_json::to_value(p.matching).unwrap_or_default(),
        Source::new(
            SourceKind::Paper,
            "P. Newson, J. Krumm, Hidden Markov map matching through noise and sparseness, \
             ACM SIGSPATIAL GIS 2009, 336-343",
        ),
    ));
    card.assumptions = vec![
        format!(
            "{} vehicle trips, {} stops placed, {} stop patterns unplaced",
            s.trips.len(),
            s.placed_stops,
            s.unplaced_patterns
        ),
        "Street-level rail runs as the bus class on the road lanes it shares.".to_string(),
        "Times in the feed are local; the scenario states the local start time.".to_string(),
    ];
    card.sources = vec![
        Source::new(
            SourceKind::Standard,
            "GTFS Schedule Reference, gtfs.org/schedule/reference",
        ),
        Source::new(
            SourceKind::Standard,
            "TCRP Report 165, Transit Capacity and Quality of Service Manual, 3rd ed., TRB \
             2013 (dwell, timepoint holding, on-time window -1/+5 min)",
        ),
    ];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec![
            RngDomain::plugin(MODEL_ID).as_str().to_string(),
            RngDomain::DesiredSpeed.as_str().to_string(),
        ],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec!["transit::tests::a_bus_serves_its_stops".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::model::Model;
    use v2xw_core::rng::RngRegistry;
    use v2xw_mobility::{EngineParams, Mobility, MobilityCtx, NativeMobility};

    /// A synthetic feed on the procedural grid (null island): one line, four stops along
    /// a straight street, two departures.
    fn write_feed(dir: &std::path::Path, world: &World) {
        std::fs::create_dir_all(dir).unwrap();
        let g = LaneGraph::new(world, ClassMask::BUS);
        // A long straight run: follow straight successors from a long lane.
        let mut path = vec![g.endpoints(80.0)[3]];
        for _ in 0..12 {
            let cur = *path.last().unwrap();
            let lane = g.lane(cur);
            let next = world
                .successors(lane)
                .iter()
                .find(|c| c.permitted && c.direction == v2xw_world::TurnDirection::Straight)
                .map(|c| c.via.unwrap_or(c.to_lane));
            match next.and_then(|l| g.node(l)) {
                Some(n) => path.push(n),
                None => break,
            }
        }
        let road: Vec<_> = path.iter().filter(|n| !g.is_internal(**n)).copied().collect();
        assert!(road.len() >= 4, "a straight street of four blocks");
        let proj = world.projection();
        let o = proj.origin();
        let mut stops = String::from("stop_id,stop_name,stop_lat,stop_lon\n");
        let mut times = String::from("trip_id,arrival_time,departure_time,stop_id,stop_sequence\n");
        for (k, n) in road.iter().take(4).enumerate() {
            let lane = world.lane(g.lane(*n));
            let p = lane.offset_point(lane.length_m * 0.6, -4.0);
            let lat = o.lat_deg + p.y / proj.metres_per_degree_latitude();
            let lon = o.lon_deg + p.x / proj.metres_per_degree_longitude();
            stops += &format!("s{k},Stop {k},{lat},{lon}\n");
            for (trip, base) in [("t1", 8 * 3600 + 10), ("t2", 8 * 3600 + 300)] {
                let a = base + k as u32 * 40;
                let f = |t: u32| format!("{:02}:{:02}:{:02}", t / 3600, (t / 60) % 60, t % 60);
                times += &format!("{trip},{},{},s{k},{}\n", f(a), f(a + 10), k + 1);
            }
        }
        std::fs::write(dir.join("stops.txt"), stops).unwrap();
        std::fs::write(dir.join("stop_times.txt"), times).unwrap();
        std::fs::write(dir.join("routes.txt"), "route_id,route_short_name,route_type\nr1,1,3\n").unwrap();
        std::fs::write(
            dir.join("trips.txt"),
            "route_id,service_id,trip_id\nr1,wk,t1\nr1,wk,t2\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("calendar.txt"),
            "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nwk,1,1,1,1,1,0,0,20260101,20271231\n",
        )
        .unwrap();
    }

    #[test]
    fn a_bus_serves_its_stops() {
        let w = crate::graph::tests::grid();
        let dir = std::env::temp_dir().join(format!("v2xw-gtfs-test-{}", std::process::id()));
        write_feed(&dir, &w);
        let params = TransitParams {
            feed: dir.clone(),
            start_s_of_day: 8.0 * 3600.0,
            duration_s: 900.0,
            ..TransitParams::default()
        };
        let demand = TransitDemand::from_feed(&w, params, 20261007).expect("placed");
        let log = demand.log();
        assert_eq!(demand.schedule().trips.len(), 2);
        assert_eq!(demand.schedule().trips[0].calls.len(), 4);
        demand.card().validate().expect("card");
        let rng = RngRegistry::new(3);
        let ep = EngineParams::default();
        let mut engine = NativeMobility::new(ep);
        {
            let mut ctx = MobilityCtx::new(0, &w, &rng);
            engine.init(&mut ctx, Box::new(demand)).expect("init");
        }
        // The demand is inside the engine now; its call log is still readable.
        let mut t = 0u64;
        while t < secs_to_ns(700.0) {
            let mut ctx = MobilityCtx::new(t, &w, &rng);
            t = engine.step(&mut ctx, ep.step).t;
        }
        let _ = std::fs::remove_dir_all(&dir);
        let records = log.lock().unwrap().clone();
        let r = report(&records);
        // Eight calls; the buses must have stopped at most of them and dwelt at each.
        assert!(r.served >= 6, "{r:?}\n{records:#?}");
        for c in records.iter().filter(|c| c.arrived_s.is_some()) {
            if let (Some(a), Some(d)) = (c.arrived_s, c.departed_s) {
                assert!(d - a >= 9.9, "dwell at {} was {} s", c.stop_id, d - a);
            }
        }
    }
}
