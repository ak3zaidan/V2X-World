//! The scenario schema: 03-interfaces.md §13 as Rust types.
//!
//! Every field carries its unit in its name (`duration_s`, `mobility_step_ms`,
//! `tx_power_dbm`) because §13 requires the published schema to state units, and a name is
//! the one place a unit cannot be separated from its value.
//!
//! # Lenient to parse, strict to validate
//!
//! The same rule build decision D11 item 4 settled for model cards applies here, for the
//! same reason: `serde` defaults make a scenario pleasant to write — a file with a seed, a
//! world and a duration is a valid scenario — and [`crate::scenario::validate`] is what
//! enforces the contract. Parsing leniency without validation strictness would defeat it,
//! so no field is defaulted *and* unchecked: everything with a default is either
//! range-checked or cross-checked in `validate`.
//!
//! # Why the world source is `v2xw_world`'s own type
//!
//! `world.source` deserialises straight into [`WorldSourceSpec`], which is the type the
//! importer already takes. A second, scenario-shaped copy of it would be a schema that can
//! express a source the importer cannot build, and the mismatch would surface as a runtime
//! error in the middle of a run rather than as a validation error at load.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use v2xw_core::card::Tier;
use v2xw_core::time::Duration;
use v2xw_core::weather::WeatherKind;
use v2xw_world::{GeoBbox, WorldSourceSpec};

/// The schema version this build writes, and the only one it loads without migrating.
pub const CURRENT_SCHEMA: &str = "v2xw/scenario/1";

/// A complete scenario (03-interfaces.md §13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Schema version, e.g. `v2xw/scenario/1`. Migration is by this string and nothing
    /// else (see [`crate::scenario::migrate`]).
    pub schema: String,
    /// Who wrote it and what it is for.
    #[serde(default)]
    pub meta: Meta,
    /// The master seed. Every sub-stream is derived from it (ADR 0004 §3); a scenario
    /// never sets a sub-stream seed, which is why there is no field for one.
    ///
    /// Accepts `0xdeadbeef`, `"0xdeadbeef"` or a decimal integer.
    #[serde(default, with = "seed_repr")]
    pub seed: u64,
    /// The clock.
    #[serde(default)]
    pub time: Time,
    /// The city the scenario runs on, by its id in the city library
    /// (`worlds/cities.yaml`, [`crate::city`]).
    ///
    /// A scenario says what happens and a city says where, so a scenario file need not
    /// name a world at all: with `city` and no `world`, the world is the city's extract,
    /// box and jurisdiction. `v2xw run --city`, the server's `run.start` and
    /// `scenario.load` and the Studio's city picker set it, replacing whatever world the
    /// file named ([`Scenario::with_city`]). Absent on a scenario that embeds its own world.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// Where the world comes from. Filled from [`Scenario::city`] when the file names a
    /// city and no world.
    pub world: WorldSpec,
    /// What moves in it.
    #[serde(default)]
    pub actors: Actors,
    /// Weather at `t0`; fronts arrive through the [`Scenario::events`] timeline.
    #[serde(default)]
    pub weather: Weather,
    /// The radio stack.
    #[serde(default)]
    pub radio: Radio,
    /// The network stack.
    #[serde(default)]
    pub net: Net,
    /// Which messages are generated and how they are encoded.
    #[serde(default)]
    pub messages: Messages,
    /// The security envelope, protocol and policies.
    #[serde(default)]
    pub security: Security,
    /// Which hardware profiles the nodes run on.
    #[serde(default)]
    pub nodes: Nodes,
    /// Attackers, jammers, compromised infrastructure.
    #[serde(default)]
    pub threats: Threats,
    /// Detectors and the misbehaviour-authority pipeline.
    #[serde(default)]
    pub detection: Detection,
    /// Which metrics to compute: a list of ids, or the single entry `all`.
    #[serde(default)]
    pub metrics: Vec<String>,
    /// Which exporters to run.
    #[serde(default)]
    pub exporters: Vec<ExporterSpec>,
    /// The timeline: things that happen at a stated instant.
    #[serde(default)]
    pub events: Vec<TimelineItem>,
    /// The V2X applications equipped vehicles run (`v2xw_node::apps`). Left out of the
    /// serialised scenario when it is the default, so a scenario that does not mention it
    /// keeps its content hash.
    #[serde(default, skip_serializing_if = "Apps::is_default")]
    pub apps: Apps,
    /// Emission, fuel and battery-electric energy of the traffic (`v2xw_energy`): which
    /// models, which fleet. Absent computes no energy metrics, and is left out of the
    /// serialised scenario so a scenario that does not mention it keeps its content hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy: Option<v2xw_energy::EnergySpec>,
    /// Who drives the traffic: absent (or `provider: native`) is this engine's own
    /// mobility; `provider: sumo` hands every vehicle to SUMO over TraCI
    /// (`v2xw_interop::sumo`) while radio, security, backend and UI run here. Left out of
    /// the serialised scenario when absent, so existing content hashes hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mobility: Option<MobilityProvider>,
    /// A parameter sweep, when this file describes one (08-measurement-and-data.md §4).
    #[serde(default)]
    pub experiment: Option<Experiment>,
    /// City scale (`crate::scale`): a microscopic focus region inside a mesoscopic city,
    /// and the radio matched to it. Left out of the serialised scenario when absent, so a
    /// scenario that does not mention it keeps its content hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<Scale>,
    /// How the traffic behaves at junctions and in jams: right turn on red, rerouting,
    /// lane changing, keeping junctions clear, gridlock resolution, and how the signals are
    /// controlled (fixed-time or actuated, and a green wave). Absent uses the defaults
    /// stated on each field, and is left out of the serialised scenario so a scenario that
    /// does not mention it keeps its content hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traffic: Option<Traffic>,
    /// The sensors of the vehicles and roadside units that send CPMs — a suite per vehicle
    /// class with each sensor's mount and azimuth, the roadside units' suite — and the
    /// occlusion rule (`crate::perception`). Absent keeps the default front suite and the
    /// centre-line rule, and is left out of the serialised scenario so a scenario that
    /// does not mention it keeps its content hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perception: Option<PerceptionSpec>,
}

/// How the traffic behaves at junctions and in jams ([`Scenario::traffic`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Traffic {
    /// Whether a right turn may be made on a steady red after a full stop. Absent follows
    /// the jurisdiction's rule (New York City prohibits it unless signed; most of the US
    /// permits it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_turn_on_red: Option<bool>,
    /// Whether drivers change route when the travel times they see change or a street
    /// closes. On by default.
    #[serde(default = "Traffic::yes")]
    pub dynamic_rerouting: bool,
    /// Congestion-aware rerouting: every vehicle re-plans its route this often, seconds,
    /// on the speeds the lanes have shown over the last three minutes, as SUMO's rerouting
    /// device does. 0 (the default) routes on free-flow times and re-plans only for a
    /// closure. Needs dynamic_rerouting.
    #[serde(default)]
    pub reroute_period_s: f64,
    /// Whether drivers change lane to overtake or to reach the lane their next turn needs
    /// (MOBIL). On by default; off keeps every vehicle in the lane it started in.
    #[serde(default = "Traffic::yes")]
    pub lane_changes: bool,
    /// Whether a driver picks the lane their route needs ahead of a turn and changes into
    /// it, rather than only routing from the lane they happen to be in. On by default;
    /// needs lane_changes.
    #[serde(default = "Traffic::yes")]
    pub strategic_lane_choice: bool,
    /// Whether a driver keeps out of a junction until there is room beyond it and yields to
    /// vehicles already inside it ("don't block the box"). On by default.
    #[serde(default = "Traffic::yes")]
    pub keep_junctions_clear: bool,
    /// Gridlock resolution: a vehicle that has stood still this long, seconds, is moved on
    /// along its route to the next lane with room and counted, as SUMO's time-to-teleport
    /// option does (default 300 s). 0 turns it off.
    #[serde(default = "Traffic::default_gridlock_timeout")]
    pub gridlock_timeout_s: f64,
    /// How the signals are controlled.
    #[serde(default)]
    pub signals: SignalControl,
    // traffic audit
    /// A multiplier on the critical gaps and follow-up times drivers accept at junctions
    /// without signals (HCM 6th ed. ch. 20 two-way-stop table: 4.1 s for a major-road left
    /// turn to 7.1-7.5 s for a minor-road left). Below 1 drivers take shorter gaps, above
    /// 1 they wait for longer ones. Unset is 1, the table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critical_gap_factor: Option<f64>,
    /// The critical headway a permitted left turn on a green accepts in the oncoming
    /// traffic, seconds. Unset is 4.5 s (HCM 6th ed. ch. 31).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permitted_left_gap_s: Option<f64>,
    /// The average sideways acceleration a turn through a junction is driven at, m/s²: a
    /// corner of radius R is taken at sqrt(a·R) at most. Unset is 5.5 m/s² (SUMO
    /// netconvert `--junctions.limit-turn-speed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_lateral_accel_mps2: Option<f64>,
    /// The arc length, m, over which a junction connector's radius is judged for its turn
    /// speed. Unset (or 0) averages the radius over the whole connector, as SUMO
    /// netconvert does; a window (6 m suggested) takes the tightest arc of that length, so
    /// drivers slow for the apex of a corner, not only for its average.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_curve_window_m: Option<f64>,
    /// SUMO LC2013's eagerness for the lane changes a route needs: it scales how far
    /// ahead of the turn the change becomes due (10 s at the look-ahead speed, twice that
    /// to the left). Setting any `lc_*` field drives discretionary lane changes by LC2013's
    /// motivations instead of MOBIL; unset `lc_*` fields take SUMO's default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lc_strategic: Option<f64>,
    /// LC2013's willingness to change lane to let a neighbour that must merge in (0 never,
    /// 1 always unless it costs speed). See `lc_strategic`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lc_cooperative: Option<f64>,
    /// LC2013's eagerness to change lane for speed: the speed-gain threshold is
    /// 0.2 / lc_speed_gain (0 turns speed-gain changes off). See `lc_strategic`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lc_speed_gain: Option<f64>,
    /// LC2013's eagerness to move back right when the right lane is free (0 off). See
    /// `lc_strategic`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lc_keep_right: Option<f64>,
    /// Whether trucks, buses and coaches wait for longer gaps than cars when yielding:
    /// the HCM 6th ed. heavy-vehicle terms (Exhibit 20-11: +1.0 s critical gap on a major
    /// road of under four lanes, +2.0 s on four or more; +0.9 / 1.0 s follow-up), at
    /// unsignalised junctions and on a permitted left. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heavy_vehicle_gap_terms: Option<bool>,
    /// The spread of drivers' critical gaps: each driver's is lognormal around the HCM
    /// value with this coefficient of variation (0.15-0.2 in field studies, SUMO P1;
    /// 0.18 is the realistic preset), at unsignalised junctions, on a permitted left and
    /// on a right turn on red. Unset is 0: every driver the HCM value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critical_gap_cv: Option<f64>,
    /// Seconds of standing after which a yielding driver's impatience is full and its
    /// critical gap has fallen to `impatience_gap_floor` of its own (SUMO
    /// `--time-to-impatience`, 180 s). Unset is 0: no impatience.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_to_impatience_s: Option<f64>,
    /// The critical gap at full impatience as a share of the driver's own. Unset is 0.7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impatience_gap_floor: Option<f64>,
    /// Whether an impatient driver judges an approaching foe as if it would brake at
    /// impatience times 4.5 m/s² (SUMO's blend toward the foe's braking arrival). Unset is
    /// off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impatience_foe_braking: Option<bool>,
    /// How many permitted left turners per lane may complete the turn in the amber that
    /// ends a permissive green, the HCM's "sneakers" (2 per cycle). Each must cross the
    /// stop line before the red. Unset is 0: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissive_sneakers: Option<u8>,
    /// Whether a driver closing on a standing vehicle brakes at no less than the constant
    /// deceleration that stops it at its standstill gap, as at a stop line, instead of
    /// being stopped in one step when the car-following model's braking (which scales
    /// with its acceleration, low for vans and trucks) arrives too late. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standing_leader_stop: Option<bool>,
    /// Whether a trip enters the road already moving, at the highest safe speed there
    /// (SUMO's `departSpeed="max"`) instead of at rest (SUMO's default, `departSpeed="0"`):
    /// a freeway entry merges at speed. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depart_speed_max: Option<bool>,
    /// Whether the bends of a through lane's polyline are driven no faster than the
    /// turn-speed rule allows (`turn_lateral_accel_mps2` over the tightest
    /// `turn_curve_window_m`, 6 m when unset), as a junction connector is. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_bend_speed: Option<bool>,
    /// Kerbside stops: buses at stops, taxis picking up, delivery vans double-parked.
    /// Unset follows the jurisdiction (New York City has its taxis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kerbside: Option<KerbsideSettings>,
    /// Whether a vehicle entering the map from outside it (its origin a lane no road of
    /// the map feeds, at the map's edge) is put on at the start of that lane at its
    /// desired speed, capped by the speed that is safe behind the vehicle ahead and that
    /// can still stop at the lane's end, as SUMO's departSpeed="desired" with
    /// departPos="base" does. Origins inside the network stay at rest. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depart_at_speed: Option<bool>,
    /// The share of drivers who re-route on information, 0-1, drawn once per driver
    /// (SUMO device.rerouting.probability). The rest keep the route they chose and detour
    /// only round a closure they can see. Unset is 1: every driver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reroute_share: Option<f64>,
    /// Where those drivers learn that the road has changed: `navigation` (a navigation
    /// app: network-wide, on speeds smoothed over three minutes, and every closure) or
    /// `v2x` (only a DENM or TIM their own unit received and verified, placing a blocking
    /// event on their route: local and belief-only). Unset is navigation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reroute_source: Option<v2xw_mobility::routing::dynamic::RerouteSource>,
    /// Grade-aware, power-limited acceleration (WP-22): the slope of the draped road and
    /// the class's power, rolling and air resistance cap how fast a vehicle can speed up,
    /// so a loaded truck crawls up a long climb. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle_dynamics: Option<VehicleDynamicsSettings>,
    /// Traffic yields to emergency vehicles (WP-20): a driver with an emergency vehicle
    /// running its beacons close behind on the same carriageway moves over and stops as
    /// the jurisdiction's law says (NY VTL §1144(a): to the right-hand edge, clear of any
    /// intersection). Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emergency_yield: Option<EmergencyYieldSettings>,
}

/// Traffic yields to emergency vehicles ([`Traffic::emergency_yield`],
/// [`v2xw_mobility::EmergencyYieldParams`]); each unset part keeps its default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmergencyYieldSettings {
    /// Whether drivers yield at all. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// How far behind a driver an emergency vehicle with its siren on is noticed, metres
    /// (gap from its front bumper to the driver's rear). Unset is 25 m, SUMO's bluelight
    /// device `reactiondist` default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaction_distance_m: Option<f64>,
    /// The speed an emergency vehicle slows to at a red before it passes on, once the
    /// junction is clear, m/s (UVC §11-106(b)2). Unset is 0: a full stop first, as
    /// fire-service practice has it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red_pass_speed_mps: Option<f64>,
}

impl EmergencyYieldSettings {
    /// The engine's parameters: `base` (the jurisdiction's, which names the yield rule)
    /// with each set field in place.
    pub fn apply(
        self,
        base: v2xw_mobility::EmergencyYieldParams,
    ) -> v2xw_mobility::EmergencyYieldParams {
        v2xw_mobility::EmergencyYieldParams {
            enabled: self.enabled.unwrap_or(base.enabled),
            reaction_distance_m: self.reaction_distance_m.unwrap_or(base.reaction_distance_m),
            red_pass_speed_mps: self.red_pass_speed_mps.unwrap_or(base.red_pass_speed_mps),
            ..base
        }
    }
}

/// Grade-aware, power-limited acceleration ([`Traffic::vehicle_dynamics`],
/// [`v2xw_mobility::VehicleDynamicsParams`]); each unset part is off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VehicleDynamicsSettings {
    /// The slope term: a vehicle's acceleration less g times the sine of the road's
    /// grade, as SUMO's KraussPS has it. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade: Option<bool>,
    /// The power term: no more acceleration than the engine's power gives after rolling
    /// and air resistance, for the classes with a sourced road-load row (passenger car
    /// and tractor-trailer). Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_limit: Option<bool>,
}

impl VehicleDynamicsSettings {
    /// The engine's parameters.
    pub fn params(self) -> v2xw_mobility::VehicleDynamicsParams {
        v2xw_mobility::VehicleDynamicsParams {
            grade: self.grade.unwrap_or(false),
            power_limit: self.power_limit.unwrap_or(false),
        }
    }
}

// traffic audit
/// Kerbside stops ([`Traffic::kerbside`]); each unset field keeps the jurisdiction's value
/// ([`v2xw_mobility::kerbside::KerbsideParams`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KerbsideSettings {
    /// Whether vehicles stop at the kerb at all. Unset is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// The distance from one bus stop to the next along a bus's route, metres. Unset is
    /// 260 m.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus_stop_spacing_m: Option<f64>,
    /// The mean time a bus stands at a stop, seconds. Unset is 60 s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus_dwell_mean_s: Option<f64>,
    /// The share of passenger cars that are taxis or for-hire cars and stop at the kerb
    /// once on their trip. Unset is the jurisdiction's (0, or 0.3 in Midtown Manhattan).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taxi_share: Option<f64>,
    /// The median time a taxi stands at the kerb, seconds. Unset is 30 s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taxi_dwell_median_s: Option<f64>,
    /// The share of delivery vans and trucks that make one delivery stop on their trip.
    /// Unset is 0.6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_stop_share: Option<f64>,
    /// The median time a delivery stop lasts, seconds. Unset is 600 s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_dwell_median_s: Option<f64>,
}

impl KerbsideSettings {
    /// The engine's kerbside parameters: `p` (the jurisdiction's) with these over it.
    pub fn apply(
        &self,
        mut p: v2xw_mobility::kerbside::KerbsideParams,
    ) -> v2xw_mobility::kerbside::KerbsideParams {
        if let Some(v) = self.enabled {
            p.enabled = v;
        }
        if let Some(v) = self.bus_stop_spacing_m {
            p.bus_stop_spacing_m = v;
        }
        if let Some(v) = self.bus_dwell_mean_s {
            p.bus_dwell_mean_s = v;
        }
        if let Some(v) = self.taxi_share {
            p.taxi_share = v;
        }
        if let Some(v) = self.taxi_dwell_median_s {
            p.taxi_dwell_median_s = v;
        }
        if let Some(v) = self.delivery_stop_share {
            p.delivery_stop_share = v;
        }
        if let Some(v) = self.delivery_dwell_median_s {
            p.delivery_dwell_median_s = v;
        }
        p
    }
}

impl Traffic {
    fn yes() -> bool {
        true
    }
    /// SUMO's time-to-teleport default (SUMO user documentation, sumo options).
    pub const DEFAULT_GRIDLOCK_TIMEOUT_S: f64 = 300.0;
    fn default_gridlock_timeout() -> f64 {
        Self::DEFAULT_GRIDLOCK_TIMEOUT_S
    }
}

impl Default for Traffic {
    fn default() -> Self {
        Traffic {
            right_turn_on_red: None,
            dynamic_rerouting: true,
            reroute_period_s: 0.0,
            lane_changes: true,
            strategic_lane_choice: true,
            keep_junctions_clear: true,
            gridlock_timeout_s: Traffic::DEFAULT_GRIDLOCK_TIMEOUT_S,
            signals: SignalControl::default(),
            critical_gap_factor: None,
            permitted_left_gap_s: None,
            turn_lateral_accel_mps2: None,
            turn_curve_window_m: None,
            lc_strategic: None,
            lc_cooperative: None,
            lc_speed_gain: None,
            lc_keep_right: None,
            heavy_vehicle_gap_terms: None,
            critical_gap_cv: None,
            time_to_impatience_s: None,
            impatience_gap_floor: None,
            impatience_foe_braking: None,
            permissive_sneakers: None,
            standing_leader_stop: None,
            depart_speed_max: None,
            lane_bend_speed: None,
            kerbside: None,
            depart_at_speed: None,
            reroute_share: None,
            reroute_source: None,
            vehicle_dynamics: None,
            emergency_yield: None,
        }
    }
}

/// How a junction's signal decides when to change ([`Traffic::signals`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignalControlKind {
    /// The plan's clock: every phase lasts its stated time.
    #[default]
    FixedTime,
    /// Vehicle-actuated (NEMA TS 2): a green runs at least its minimum, is extended while
    /// detectors keep seeing vehicles within the passage time, ends early (gaps out) when
    /// they stop and another approach is waiting, and never runs past its maximum.
    Actuated,
    /// The NEMA dual-ring eight-phase controller (FHWA Signal Timing Manual 2nd ed.,
    /// NCHRP Report 812, §5.1.2 Exhibit 5-3): two rings timing concurrently, meeting at
    /// barriers, with [`SignalControl::nema`] settings (WP-17). Junctions that do not map
    /// to the dual ring keep the single-ring actuated controller.
    Nema,
}

/// Where an actuated controller's vehicle detectors are ([`SignalControl::detector_placement`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DetectorPlacement {
    /// A presence loop covering the last `detector_m` of every approach lane (FHWA Signal
    /// Timing Manual 2e, ch. 5: stop-line presence detection).
    #[default]
    StopLine,
    /// SUMO's `actuated` default (MSActuatedTrafficLightLogic): one induction loop per
    /// approach lane, the lane's speed limit times `detector-gap` = 2 s upstream of the
    /// stop line (at least 1 m, at most the lane), which extends the green while a vehicle
    /// covers it. A vehicle standing in the last `detector_m` still places the call on a
    /// red approach, as a stop-line loop does.
    Sumo,
}

impl DetectorPlacement {
    fn is_default(&self) -> bool {
        *self == Self::StopLine
    }
}

/// Signal control ([`Traffic::signals`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalControl {
    /// Fixed-time, single-ring vehicle-actuated, or the NEMA dual ring.
    #[serde(default)]
    pub control: SignalControlKind,
    /// The shortest an actuated green runs, seconds.
    #[serde(default = "SignalControl::default_min_green")]
    pub min_green_s: f64,
    /// The longest an actuated green runs while another approach waits, seconds.
    #[serde(default = "SignalControl::default_max_green")]
    pub max_green_s: f64,
    /// The gap between detections that ends an actuated green, seconds (the passage time
    /// or unit extension).
    #[serde(default = "SignalControl::default_passage")]
    pub passage_s: f64,
    /// How far back from the stop line the detector sees, metres.
    #[serde(default = "SignalControl::default_detector")]
    pub detector_m: f64,
    /// Where the extension detectors are: a stop-line presence loop (the default) or
    /// SUMO's upstream induction loop at 2 s of travel from the stop line.
    #[serde(default, skip_serializing_if = "DetectorPlacement::is_default")]
    pub detector_placement: DetectorPlacement,
    /// Short-link clearance: a driving lane shorter than this between two signalised
    /// junctions gets a downstream green that starts just before the upstream green that
    /// fills it (`crate::actuated::clear_short_links`), metres. Absent is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_link_clearance_m: Option<f64>,
    /// A green wave: every signal's offset set so that a vehicle at this speed meets green
    /// after green travelling in green_wave_heading_deg. Absent keeps the plans' own
    /// offsets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub green_wave_speed_mps: Option<f64>,
    /// The direction the green wave runs, degrees clockwise from north (90 is eastbound).
    #[serde(default = "SignalControl::default_wave_heading")]
    pub green_wave_heading_deg: f64,
    /// The NEMA dual-ring controller's settings, read only with `control: nema`. Absent
    /// takes every default of the block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nema: Option<NemaSettings>,
}

impl SignalControl {
    // FHWA Traffic Signal Timing Manual (FHWA-HOP-08-024, 2008), chapter 5: minimum green
    // 5-15 s for a through phase, maximum green 30-60 s for a minor-arterial through phase,
    // passage time 2-3 s with stop-line presence detection, and a 40 ft (12.2 m) stop-line
    // loop (recalled; the values are the middle of each range).
    fn default_min_green() -> f64 {
        7.0
    }
    fn default_max_green() -> f64 {
        45.0
    }
    fn default_passage() -> f64 {
        2.5
    }
    fn default_detector() -> f64 {
        12.2
    }
    fn default_wave_heading() -> f64 {
        90.0
    }
}

impl Default for SignalControl {
    fn default() -> Self {
        SignalControl {
            control: SignalControlKind::FixedTime,
            min_green_s: Self::default_min_green(),
            max_green_s: Self::default_max_green(),
            passage_s: Self::default_passage(),
            detector_m: Self::default_detector(),
            detector_placement: DetectorPlacement::StopLine,
            short_link_clearance_m: None,
            green_wave_speed_mps: None,
            green_wave_heading_deg: Self::default_wave_heading(),
            nema: None,
        }
    }
}

/// The order of a barrier group's left-turn phase and its opposing through
/// ([`NemaSettings::lefts`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NemaLeftSequence {
    /// Both lefts before their throughs (1 and 5 lead 2 and 6): "the most common left-turn
    /// phase sequence" (STM 2nd ed. §5.1.3.6, p. 5-9).
    #[default]
    LeadLead,
    /// Both lefts after their throughs.
    LagLag,
    /// One left leads and the opposing left lags (ring 1's left leads).
    LeadLag,
}

/// How a left-turn phase's movement is served ([`NemaSettings::left_turn_mode`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LeftTurnMode {
    /// A protected arrow in the left phase, then a permissive left yielding to the
    /// opposing through (STM 2nd ed. §5.1.3, Exhibit 5-4, p. 5-5). The STM names no
    /// default; this is the choice of this build.
    #[default]
    ProtectedPermissive,
    /// The protected arrow only; the left is held while the opposing through runs.
    Protected,
    /// No left phase: the left yields to the opposing through during the through's green.
    Permissive,
}

/// The face shown for a permissive left turn ([`NemaSettings::permissive_display`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissiveDisplay {
    /// A flashing yellow arrow (MUTCD 2009 §4D.20 ¶03 item 4).
    #[default]
    Fya,
    /// A circular green (MUTCD 2009 §4D.18 ¶03).
    GreenBall,
}

/// Where a coordinated controller's force-offs sit in the cycle
/// ([`NemaSettings::force_off`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ForceOffMode {
    /// Each non-coordinated phase is forced off at its own split's end, so time a phase
    /// leaves unused goes to the coordinated phases (STM 2nd ed. §7.3.4, Exhibit 7-14;
    /// SUMO `fixForceOff=false`, its default).
    #[default]
    Floating,
    /// Force-off points fixed in the cycle, so a later non-coordinated phase may use time
    /// an earlier one left (SUMO `fixForceOff=true`).
    Fixed,
}

/// The controller hardware the coordination offset is referenced to
/// ([`NemaSettings::controller_type`]); SUMO `controllerType`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NemaControllerType {
    /// NEMA TS 2: the offset references the start of the first coordinated phase's green
    /// (SUMO NEMA documentation; its default).
    #[default]
    Ts2,
    /// A Type 170 controller: the offset references the start of the coordinated phase's
    /// yellow (SUMO NEMA documentation).
    Type170,
}

/// The NEMA dual-ring eight-phase controller ([`SignalControl::nema`]).
///
/// The layout is the FHWA Signal Timing Manual 2nd ed. (NCHRP Report 812, 2015): two rings
/// and two barriers, §5.1.2 Exhibit 5-3 (p. 5-4), with §5.1.1's numbering (2/6 main-street
/// throughs, 1/5 main-street lefts, 4/8 side throughs, 3/7 side lefts; a pedestrian phase
/// takes its parallel through's number). The SUMO names in brackets are SUMO's documented
/// parameters (Simulation/NEMA), kept for import parity; SUMO's code was not read. Every
/// default and its page: `docs/design/frameworks/wp16-24-build-plans.md` §WP-17.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NemaSettings {
    /// Ring 1's phases in service order [SUMO `ring1`], NEMA numbers 1-8.
    #[serde(default = "NemaSettings::default_ring1")]
    pub ring1: Vec<u8>,
    /// Ring 2's phases in service order [SUMO `ring2`].
    #[serde(default = "NemaSettings::default_ring2")]
    pub ring2: Vec<u8>,
    /// The pair of phases, one per ring, that end together at the barrier closing the
    /// side-street group [SUMO `barrierPhases`, "usually 4,8"].
    #[serde(default = "NemaSettings::default_barrier")]
    pub barrier_phases: Vec<u8>,
    /// The pair that end together at the barrier closing the main-street group [SUMO
    /// `barrier2Phases`, "usually 2,6"].
    #[serde(default = "NemaSettings::default_barrier2")]
    pub barrier2_phases: Vec<u8>,
    /// Lead-lead, lag-lag or lead-lag lefts.
    #[serde(default)]
    pub lefts: NemaLeftSequence,
    /// Protected-permissive, protected-only or permissive-only lefts.
    #[serde(default)]
    pub left_turn_mode: LeftTurnMode,
    /// The permissive left's face: flashing yellow arrow or circular green.
    #[serde(default)]
    pub permissive_display: PermissiveDisplay,
    /// A left-turn phase's minimum green, seconds (through phases use
    /// `traffic.signals.min_green_s`).
    #[serde(default = "NemaSettings::default_left_min")]
    pub left_min_green_s: f64,
    /// A left-turn phase's maximum green, seconds (through phases use
    /// `traffic.signals.max_green_s`).
    #[serde(default = "NemaSettings::default_left_max")]
    pub left_max_green_s: f64,
    /// A left-turn phase's passage time, seconds (through phases use
    /// `traffic.signals.passage_s`).
    #[serde(default = "NemaSettings::default_left_passage")]
    pub left_passage_s: f64,
    /// Every phase's yellow change interval, seconds. Absent takes each plan's own amber.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yellow_s: Option<f64>,
    /// Every phase's red clearance, seconds. Absent takes each plan's own all-red.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red_clear_s: Option<f64>,
    /// The pedestrian walk interval, seconds. Absent is `world.signals.pedestrian_walk_s`,
    /// else 7 s (MUTCD 2009 §4E.06 ¶11).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_s: Option<f64>,
    /// The walking speed the pedestrian clearance is timed for, m/s. Absent is
    /// `world.signals.pedestrian_speed_mps`, else 1.067 m/s (3.5 ft/s, MUTCD 2009 §4E.06).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ped_speed_mps: Option<f64>,
    /// Phases called every cycle and timed to at least their minimum [SUMO `minRecall`].
    #[serde(default)]
    pub min_recall: Vec<u8>,
    /// Phases called only when no conflicting phase has a call, so the controller rests in
    /// them (STM 2nd ed. Exhibit 6-22, p. 6-22: soft recall on 2 and 6 and no minimum
    /// recall at a fully actuated intersection).
    #[serde(default = "NemaSettings::default_soft_recall")]
    pub soft_recall: Vec<u8>,
    /// Phases called every cycle and held to their maximum [SUMO `maxRecall`].
    #[serde(default)]
    pub max_recall: Vec<u8>,
    /// Phases whose pedestrian walk is served every cycle without a push-button call.
    #[serde(default)]
    pub ped_recall: Vec<u8>,
    /// The coordinated phases (STM 2nd ed. §5.1.1, p. 5-3: 2 and 6). SUMO coordinates its
    /// `barrier2Phases` instead.
    #[serde(default = "NemaSettings::default_coordinated")]
    pub coordinate_phases: Vec<u8>,
    /// Coordinated operation: a common cycle, an offset and force-offs [SUMO
    /// `coordinate-mode`, default false].
    #[serde(default)]
    pub coordination: bool,
    /// Floating or fixed force-offs [SUMO `fixForceOff`].
    #[serde(default)]
    pub force_off: ForceOffMode,
    /// TS 2 or Type 170: where the coordination offset is referenced [SUMO
    /// `controllerType`].
    #[serde(default)]
    pub controller_type: NemaControllerType,
}

impl NemaSettings {
    fn default_ring1() -> Vec<u8> {
        vec![1, 2, 3, 4]
    }
    fn default_ring2() -> Vec<u8> {
        vec![5, 6, 7, 8]
    }
    fn default_barrier() -> Vec<u8> {
        vec![4, 8]
    }
    fn default_barrier2() -> Vec<u8> {
        vec![2, 6]
    }
    // STM 2nd ed. Exhibit 6-4 (p. 6-6): left-turn minimum green 2-5 s on any facility;
    // Exhibit 6-9 (p. 6-10): left-turn maximum green 15-30 s; p. 6-12: setback-detector
    // passage 2-3 s. 4 / 25 / 2.5 lie inside those ranges; the exact values are this
    // build's choice.
    fn default_left_min() -> f64 {
        4.0
    }
    fn default_left_max() -> f64 {
        25.0
    }
    fn default_left_passage() -> f64 {
        2.5
    }
    fn default_soft_recall() -> Vec<u8> {
        vec![2, 6]
    }
    fn default_coordinated() -> Vec<u8> {
        vec![2, 6]
    }
}

impl NemaSettings {
    /// The settings a SUMO `NEMA` `tlLogic`'s `<param>`s state, for WP-23's importer. The
    /// names are SUMO's documented ones (Simulation/NEMA); SUMO's code was not read.
    ///
    /// * `ring1`, `ring2`: phase lists, `0` for an absent phase. SUMO writes lead or lag
    ///   as the order within a barrier group; here that becomes [`NemaSettings::lefts`]
    ///   and the rings are written ascending.
    /// * `barrierPhases`, `barrier2Phases`: the two barriers. With `coordinate-mode`
    ///   true, SUMO's coordinated phases are `barrier2Phases`.
    /// * `minRecall`: SUMO calls every phase when it is unset. `maxRecall` is empty when
    ///   unset. SUMO has no soft recall, so none is set.
    /// * `fixForceOff`: true is fixed force-offs. `controllerType`: `TS2` or `Type 170`.
    ///
    /// Other parameters are not read; the importer counts them.
    ///
    /// # Errors
    ///
    /// A list that does not parse, or a lead/lag pair this build cannot state (ring 1
    /// lagging while ring 2 leads).
    pub fn from_sumo_params(params: &BTreeMap<String, String>) -> Result<Self, String> {
        let list = |key: &str| -> Result<Option<Vec<u8>>, String> {
            let Some(v) = params.get(key) else {
                return Ok(None);
            };
            let mut out = Vec::new();
            for part in v.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                let p: u8 = part
                    .parse()
                    .map_err(|_| format!("{key}: '{part}' is not a phase number"))?;
                if p != 0 {
                    out.push(p);
                }
            }
            Ok(Some(out))
        };
        let flag = |key: &str| {
            params
                .get(key)
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
        };
        let mut n = NemaSettings {
            soft_recall: Vec::new(),
            ..NemaSettings::default()
        };
        // Leading when the barrier group's first phase is its left (odd).
        let mut leads = [true, true];
        for (k, key) in ["ring1", "ring2"].into_iter().enumerate() {
            if let Some(ring) = list(key)? {
                let main: Vec<u8> = ring
                    .iter()
                    .copied()
                    .filter(|p| matches!(p, 1 | 2 | 5 | 6))
                    .collect();
                if main.len() == 2 {
                    leads[k] = main[0] % 2 == 1;
                }
                let mut sorted = ring;
                sorted.sort_unstable();
                sorted.dedup();
                if k == 0 {
                    n.ring1 = sorted;
                } else {
                    n.ring2 = sorted;
                }
            }
        }
        n.lefts = match leads {
            [true, true] => NemaLeftSequence::LeadLead,
            [false, false] => NemaLeftSequence::LagLag,
            [true, false] => NemaLeftSequence::LeadLag,
            [false, true] => {
                return Err(
                    "ring1 lags and ring2 leads; this build states lead-lag as ring 1 leading"
                        .to_string(),
                );
            }
        };
        if let Some(b) = list("barrierPhases")? {
            n.barrier_phases = b;
        }
        if let Some(b) = list("barrier2Phases")? {
            n.barrier2_phases = b;
        }
        n.coordination = flag("coordinate-mode");
        if n.coordination {
            n.coordinate_phases = n.barrier2_phases.clone();
        }
        n.min_recall = match list("minRecall")? {
            Some(v) => v,
            None => n.ring1.iter().chain(&n.ring2).copied().collect(),
        };
        n.max_recall = list("maxRecall")?.unwrap_or_default();
        // Both act only under coordination (the loader refuses them otherwise).
        if n.coordination && flag("fixForceOff") {
            n.force_off = ForceOffMode::Fixed;
        }
        if let Some(t) = params.get("controllerType").filter(|_| n.coordination) {
            let t = t.trim().to_ascii_lowercase().replace(' ', "");
            n.controller_type = match t.as_str() {
                "type170" => NemaControllerType::Type170,
                _ => NemaControllerType::Ts2,
            };
        }
        Ok(n)
    }
}

impl Default for NemaSettings {
    fn default() -> Self {
        NemaSettings {
            ring1: Self::default_ring1(),
            ring2: Self::default_ring2(),
            barrier_phases: Self::default_barrier(),
            barrier2_phases: Self::default_barrier2(),
            lefts: NemaLeftSequence::default(),
            left_turn_mode: LeftTurnMode::default(),
            permissive_display: PermissiveDisplay::default(),
            left_min_green_s: Self::default_left_min(),
            left_max_green_s: Self::default_left_max(),
            left_passage_s: Self::default_left_passage(),
            yellow_s: None,
            red_clear_s: None,
            walk_s: None,
            ped_speed_mps: None,
            min_recall: Vec::new(),
            soft_recall: Self::default_soft_recall(),
            max_recall: Vec::new(),
            ped_recall: Vec::new(),
            coordinate_phases: Self::default_coordinated(),
            coordination: false,
            force_off: ForceOffMode::default(),
            controller_type: NemaControllerType::default(),
        }
    }
}

/// City scale: which traffic tier runs where, and how the radio follows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scale {
    /// The traffic tiers.
    pub traffic: ScaleTraffic,
    /// The radio's place in them.
    #[serde(default)]
    pub radio: ScaleRadio,
}

/// The traffic tiers of a city-scale run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScaleTraffic {
    /// `mobility/tiered/meso-micro` — microscopic inside `region`, a queue network
    /// outside — or `mobility/meso/queue`, the whole world as queues.
    pub model: String,
    /// The microscopic region, a geodetic box. The tiered model needs one.
    #[serde(default)]
    pub region: Option<GeoBbox>,
    /// The queue network's parameters, SUMO MESO's defaults unless set.
    #[serde(default)]
    pub meso: ScaleMeso,
}

/// The queue network's parameters (Eissfeldt 2003; SUMO MESO's options).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScaleMeso {
    /// The longest a queue segment may be, metres (`--meso-edgelength`, 98).
    pub segment_length_m: f64,
    /// Headway from a free segment into a free one, seconds (`--meso-tauff`, 1.13).
    pub tau_ff_s: f64,
    /// Headway from a free segment into a jammed one, seconds (`--meso-taufj`, 1.13).
    pub tau_fj_s: f64,
    /// Headway from a jammed segment into a free one, seconds (`--meso-taujf`, 1.73).
    pub tau_jf_s: f64,
    /// Headway from a jammed segment into a jammed one, seconds (`--meso-taujj`, 1.4).
    pub tau_jj_s: f64,
    /// The occupancy share above which a segment is jammed; absent, SUMO's speed rule.
    pub jam_threshold: Option<f64>,
    /// Whether a signal holds the head of a queue while its movement is not green.
    pub junction_control: bool,
}

impl Default for ScaleMeso {
    fn default() -> Self {
        Self {
            segment_length_m: 98.0,
            tau_ff_s: 1.13,
            tau_fj_s: 1.13,
            tau_jf_s: 1.73,
            tau_jj_s: 1.4,
            jam_threshold: None,
            junction_control: true,
        }
    }
}

/// How the radio follows the traffic tiers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScaleRadio {
    /// Whether the radio's focus region is the microscopic region: the declared tiers
    /// inside it and the abstract propagation tier outside. Ignored when
    /// `radio.tiers.focus` names a region of its own.
    pub follows: bool,
    /// How far outside the microscopic region a vehicle's radio stays on, metres; past it
    /// a vehicle in the queue network neither transmits nor receives. Absent, the radio's
    /// own reach (`radio.range`).
    pub margin_m: Option<f64>,
    /// Whether a vehicle in the margin (queued, within `margin_m` of the region) is also
    /// a receiver. Off, it only transmits: the region hears it, and its own receptions —
    /// most of a city's reception work — are not evaluated.
    pub receive_in_margin: bool,
}

impl Default for ScaleRadio {
    fn default() -> Self {
        Self {
            follows: true,
            margin_m: None,
            receive_in_margin: false,
        }
    }
}

/// `mobility:` — the traffic provider.
///
/// ```yaml
/// world: { source: { kind: sumo-net, path: worlds/my.net.xml } }
/// mobility:
///   provider: sumo
///   route_files: [worlds/my.rou.xml]     # optional: SUMO reads and routes these itself
///   sumo_args: [--default.carfollowmodel, IDM]
/// ```
///
/// SUMO is found through `sumo_home`, else `$SUMO_HOME`, else `$V2XW_SUMO_VENV`. The world
/// must be the `net.xml` SUMO runs (`world.source.kind: sumo-net`): that is what makes a
/// SUMO lane a world lane. The scenario's own demand still applies: its trips are routed
/// here and inserted into SUMO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MobilityProvider {
    /// `native` or `sumo`.
    pub provider: String,
    /// `.rou.xml` files SUMO loads itself.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_files: Vec<String>,
    /// Extra SUMO command-line options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sumo_args: Vec<String>,
    /// SUMO's installation directory (holding `bin/sumo`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sumo_home: Option<String>,
}

/// `seed: 0x…`, `seed: "0x…"` or `seed: 12345`.
mod seed_repr {
    use serde::de::{Deserializer, Error as _, Unexpected};
    use serde::{Deserialize, Serialize, Serializer};

    /// Serialises as hexadecimal, which is how §13 writes it and how a run report reads
    /// best beside a derived stream key.
    pub fn serialize<S: Serializer>(seed: &u64, s: S) -> Result<S::Ok, S::Error> {
        format!("0x{seed:016x}").serialize(s)
    }

    /// Accepts a string with or without `0x`, or a plain integer.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Int(u64),
            Text(String),
        }
        match Repr::deserialize(d)? {
            Repr::Int(v) => Ok(v),
            Repr::Text(t) => {
                let trimmed = t.trim();
                let (body, radix) = match trimmed.strip_prefix("0x").or(trimmed.strip_prefix("0X"))
                {
                    Some(hex) => (hex, 16),
                    None => (trimmed, 10),
                };
                u64::from_str_radix(&body.replace('_', ""), radix).map_err(|_| {
                    D::Error::invalid_value(Unexpected::Str(&t), &"a u64, decimal or 0x-hex")
                })
            }
        }
    }
}

/// Who wrote the scenario and what it derives from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meta {
    /// A short name.
    #[serde(default)]
    pub name: String,
    /// What it is for.
    #[serde(default)]
    pub description: String,
    /// Authors.
    #[serde(default)]
    pub authors: Vec<String>,
    /// Free-form tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// When it was written, ISO-8601. **Author-supplied**: nothing in the engine reads a
    /// wall clock (02-architecture.md §6.1), so the loader cannot fill this in.
    #[serde(default)]
    pub created: String,
    /// A preset id or a path this scenario overlays (see [`crate::scenario::merge`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
}

/// The run's clock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Time {
    /// The civil instant `SimTime` zero corresponds to, RFC 3339. It is what the 1609.2
    /// `generationTime` is stamped from, and it is *scenario data*, not a wall-clock read.
    #[serde(default = "Time::default_t0")]
    pub t0: String,
    /// How long the run lasts, simulated seconds.
    #[serde(default = "Time::default_duration_s")]
    pub duration_s: f64,
    /// The mobility period, milliseconds. ADR 0004 decision 2 allows 10–100.
    #[serde(default = "Time::default_mobility_step_ms")]
    pub mobility_step_ms: u64,
    /// The resolution models are promised, e.g. `1us`. The kernel is always nanoseconds;
    /// this is the guarantee a model may rely on (02-architecture.md §5.1).
    #[serde(default = "Time::default_des_resolution")]
    pub des_resolution: String,
    /// Windows in which radio events are skipped so a long backend experiment can run in
    /// minutes (02-architecture.md §5.4). The manifest records them, and metrics that
    /// depend on radio events are marked `not-observed` inside them.
    #[serde(default)]
    pub time_dilation: Vec<DilationWindow>,
    /// How often every metric provider flushes a window into a `metric.sample`, simulated
    /// seconds; 1 s when left out. A run of days at a minute's period writes a sixtieth
    /// of the samples — the long-run option that keeps `metrics.json` and the page's
    /// series a usable size — and each sample then summarises a minute instead of a
    /// second. Left out of the serialised scenario when not set, so no existing scenario's
    /// hash moves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_period_s: Option<f64>,
    /// The warm-up period, simulated seconds: how long the run fills with traffic before
    /// it is measured. No metric sample is recorded for a window that ends inside it, so
    /// the charts and the metric summaries describe the steady state rather than an empty
    /// map filling up (the transient-deletion practice of Law, *Simulation Modeling and
    /// Analysis*, 5th ed., §9.5.1). The run report's whole-run counters still include it.
    /// Unset is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warmup_s: Option<f64>,
    /// Which calendar the credential system runs on (WP-D2, gap row 43). `real`, the
    /// default, is the deployment's: one-week i-periods counted from the US epoch,
    /// 2015-01-06 04:00 Eastern (03 §6.1; [SMP] §2.1.17 Table 2), so a run starting
    /// 2026-10-16 provisions i = 614. `compressed` is the only way to state a shorter
    /// `security.protocol.params.i_period_s`: its i-period numbers, validity windows and
    /// every calendar-driven wait are not the deployment's, and the manifest and the run
    /// report say so. Left out of the serialised scenario when real, so no existing
    /// scenario's hash moves.
    #[serde(default, skip_serializing_if = "Calendar::is_real")]
    pub calendar: Calendar,
    /// Windows the run fast-forwards through (WP-D2, gap row 44): inside one the traffic
    /// is frozen where it stands and no radio event is generated, and the engine jumps
    /// from one credential-system event to the next — top-up horizons, the RA's
    /// shuffles and pre-generation, the CRL publication and fetches, re-enrolment, root
    /// rollovers — on the mobility-step grid, so weeks of PKI behaviour on the real
    /// calendar fit in a short run. A backend event lands on exactly the step a full run
    /// would have handled it on. The manifest records each window, as a time-dilation
    /// window too (radio skipped) and in its warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fast_forward: Vec<DilationWindow>,
}

/// The calendar the credential system runs on ([`Time::calendar`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Calendar {
    /// The deployment's weekly grid from the 2015 epoch.
    #[default]
    Real,
    /// A shorter i-period, labelled in every output.
    Compressed,
}

impl Calendar {
    /// Whether this is the deployment's calendar.
    #[must_use]
    pub fn is_real(&self) -> bool {
        *self == Calendar::Real
    }
}

impl Time {
    fn default_t0() -> String {
        "2027-03-04T07:00:00Z".to_string()
    }
    fn default_duration_s() -> f64 {
        60.0
    }
    fn default_mobility_step_ms() -> u64 {
        100
    }
    fn default_des_resolution() -> String {
        "1us".to_string()
    }

    /// The metric period as a [`Duration`]: `metric_period_s`, or one second.
    pub fn metric_period(&self) -> Duration {
        self.metric_period_s
            .filter(|s| s.is_finite() && *s > 0.0)
            .map_or(Duration::from_secs(1), Duration::from_secs_f64)
    }

    /// The warm-up period's end, nanoseconds; zero when there is none.
    pub fn warmup_ns(&self) -> u64 {
        self.warmup_s
            .filter(|s| s.is_finite() && *s > 0.0)
            .map_or(0, |s| (s * 1e9).round() as u64)
    }

    /// The mobility period as a [`Duration`].
    pub fn mobility_step(&self) -> Duration {
        Duration::from_millis(self.mobility_step_ms)
    }

    /// Whether `t` (simulated nanoseconds) falls inside a [`Time::fast_forward`] window
    /// (start inclusive, end exclusive), and if so that window's end.
    pub fn fast_forward_end(&self, t: u64) -> Option<u64> {
        self.fast_forward.iter().find_map(|w| {
            let (a, b) = (secs_to_ns(w.from_s), secs_to_ns(w.to_s));
            (t >= a && t < b).then_some(b)
        })
    }

    /// The run horizon in nanoseconds, rounded to the nanosecond grid.
    ///
    /// `duration_s` is a float in the file because authors write `0.5`; the kernel is
    /// integer nanoseconds, so the conversion happens once, here, and every consumer takes
    /// the integer.
    pub fn horizon_ns(&self) -> u64 {
        (self.duration_s * 1e9).round().max(0.0) as u64
    }
}

fn secs_to_ns(s: f64) -> u64 {
    (s * 1e9).round().max(0.0) as u64
}

impl Default for Time {
    fn default() -> Self {
        Time {
            t0: Time::default_t0(),
            duration_s: Time::default_duration_s(),
            mobility_step_ms: Time::default_mobility_step_ms(),
            des_resolution: Time::default_des_resolution(),
            time_dilation: Vec::new(),
            metric_period_s: None,
            warmup_s: None,
            calendar: Calendar::Real,
            fast_forward: Vec::new(),
        }
    }
}

/// A period in which radio events are not generated.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DilationWindow {
    /// When it starts, simulated seconds.
    pub from_s: f64,
    /// When it ends, simulated seconds.
    pub to_s: f64,
}

/// Where the world comes from and what is kept of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldSpec {
    /// The source description, in the importer's own vocabulary.
    pub source: WorldSourceSpec,
    /// The import date, ISO-8601 UTC, **supplied here rather than read from a clock**
    /// (02-architecture.md §6.1). It reaches
    /// [`v2xw_world::ImportOptions::imported_at`] unchanged and is excluded from the
    /// world's content hash.
    #[serde(default)]
    pub imported_at: String,
    /// Building import options.
    #[serde(default)]
    pub buildings: BuildingOptions,
    /// Terrain options.
    #[serde(default)]
    pub terrain: TerrainOptions,
    /// Where an imported world is cached, so a repeated run skips the import.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<String>,
    /// Which named set of `highway=*` class defaults the OSM importer falls back on.
    ///
    /// **There is no default, and an OSM import is refused without it.** A road class's
    /// fallback speed limit is a statement about a jurisdiction, not a physical constant:
    /// the `sumo-german` table gives 404 Midtown driving lanes a limit above 90 km/h,
    /// 393 of them at 100 km/h on named side streets. The scenario must say which
    /// jurisdiction it means, so this is stated here and nowhere else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highway_preset: Option<v2xw_world::osm::HighwayPreset>,
    /// How the OSM importer times the signals it synthesises.
    #[serde(default, skip_serializing_if = "WorldSignalOptions::is_default")]
    pub signals: WorldSignalOptions,
    /// Whether `highway=service` ways (alleys, driveways, parking aisles) become
    /// motor-traffic lanes ([`v2xw_world::osm::OsmLayers::service`]). Default `true`; a
    /// city of the library sets `false`, the street network a city study drives on.
    #[serde(
        default = "crate::scenario::schema::truth",
        skip_serializing_if = "crate::scenario::schema::is_true"
    )]
    pub service_roads: bool,
    /// Whether the OSM importer keeps the trees as a vegetation layer (WP-18: woods and
    /// forests, tree rows, single trees; [`v2xw_world::osm::OsmOptions::vegetation`]),
    /// which `radio.channel_model.foliage` charges. Default `false`, and a world imported
    /// without it keeps its content hash.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vegetation: bool,
    /// How a SUMO `net.xml` import treats its traffic-light programmes (WP-23).
    #[serde(default, skip_serializing_if = "WorldSumoOptions::is_default")]
    pub sumo: WorldSumoOptions,
}

/// How a SUMO `net.xml` import is read (`world.sumo`, WP-23).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldSumoOptions {
    /// `as-file` (the default): an actuated, delay-based or NEMA `<tlLogic>` runs as the
    /// live controller its `type` names, with its phases' `minDur`/`maxDur` and its
    /// `<param>`s ([`v2xw_world::sumo::ProgrammeImport`]). `static`: every programme runs
    /// its phase list as fixed time, as before WP-23.
    #[serde(default)]
    pub signal_programmes: v2xw_world::sumo::ProgrammeImport,
}

impl WorldSumoOptions {
    /// True when nothing differs from the default, so a scenario that never mentions
    /// `world.sumo` serialises exactly as before.
    pub fn is_default(&self) -> bool {
        *self == WorldSumoOptions::default()
    }
}

/// How an imported world's synthesised signal plans are coordinated
/// ([`v2xw_world::osm::SignalDefaults`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldSignalOptions {
    /// Offset the plans along each major road for a green wave.
    #[serde(default = "crate::scenario::schema::truth")]
    pub coordinate: bool,
    /// The green wave's speed, m/s; absent, each link's speed limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progression_speed_mps: Option<f64>,
    // traffic audit
    /// The cycle length of the synthesised signal plans, seconds. Unset is 90 s
    /// (netconvert `--tls.cycle.time`). Imported (OSM) worlds only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle_s: Option<f64>,
    /// The shortest all-red between phases, seconds; the ITE red-clearance formula may
    /// give more. Unset is 0 (netconvert `--tls.allred.time`). Imported worlds only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_red_s: Option<f64>,
    /// The pedestrian walk interval each phase's green must leave room for, seconds.
    /// Unset is 7 s (MUTCD 2009 §4E.06 minimum). Imported worlds only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pedestrian_walk_s: Option<f64>,
    /// The walking speed pedestrian clearance is timed for, m/s. Unset is 1.067 m/s
    /// (3.5 ft/s, MUTCD 2009 §4E.06). Imported worlds only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pedestrian_speed_mps: Option<f64>,
    /// Joins two signalised junctions whose connecting road is at most this long, metres,
    /// into one junction with one signal plan (`netconvert --tls.join`, whose
    /// `--tls.join-dist` default is 20 m). Unset joins none. Imported worlds only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_m: Option<f64>,
}

impl Default for WorldSignalOptions {
    fn default() -> Self {
        WorldSignalOptions {
            coordinate: true,
            progression_speed_mps: None,
            cycle_s: None,
            all_red_s: None,
            pedestrian_walk_s: None,
            pedestrian_speed_mps: None,
            join_m: None,
        }
    }
}

impl WorldSignalOptions {
    /// True when nothing differs from the default, so a scenario that never mentions
    /// signals serialises exactly as before.
    pub fn is_default(&self) -> bool {
        *self == WorldSignalOptions::default()
    }
}

/// How buildings are imported (04-models.md §1.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingOptions {
    /// Import building footprints at all.
    #[serde(default = "crate::scenario::schema::truth")]
    pub enabled: bool,
    /// Keep interior holes in footprints.
    #[serde(default = "crate::scenario::schema::truth")]
    pub keep_holes: bool,
    /// Metres per storey for the `building:levels` rule. **`TODO: calibrate`** upstream
    /// ([`v2xw_world::ImportOptions::metres_per_level`]); a scenario that has a local fit
    /// sets it here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metres_per_level: Option<f64>,
}

impl Default for BuildingOptions {
    fn default() -> Self {
        BuildingOptions {
            enabled: true,
            keep_holes: true,
            metres_per_level: None,
        }
    }
}

/// How terrain is imported.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainOptions {
    /// Path to a DEM raster, when the scenario has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dem: Option<String>,
    /// Lay the world on the DEM (WP-22 P22.4): every lane centreline point, junction,
    /// crossing end, building, site and signal head is lifted by the DEM's height at its
    /// position (`v2xw_world::DrapeOptions::all`; 04-models.md §1.4, "lane z is taken from
    /// the DEM where the source network has no z"), so a lane has the slope
    /// `traffic.vehicle_dynamics.grade` reads. Off by default: the DEM is then only the
    /// ground the radio's knife-edge diffraction reads, and the geometry stays flat. Needs
    /// `dem`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub drape: bool,
}

/// Everything that moves or transmits.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actors {
    /// Vehicle demand and fleet mix.
    #[serde(default)]
    pub vehicles: Vehicles,
    /// Vulnerable road users.
    #[serde(default)]
    pub vru: Vru,
    /// Roadside units.
    #[serde(default)]
    pub rsus: Vec<Rsu>,
    /// Roadside units placed by the engine at the city's busiest signalised
    /// intersections, so a scenario that does not know its city still has units where a
    /// deployment would put them. They are appended to [`Actors::rsus`] once the world is
    /// built ([`crate::city::place_rsus`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rsu_placement: Option<RsuPlacement>,
    /// Backend entities and the links between them.
    #[serde(default)]
    pub backend: Backend,
}

/// Roadside units the engine places itself ([`Actors::rsu_placement`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RsuPlacement {
    /// How many units, at most: one per signalised intersection, the intersections with
    /// the most approach lanes first, no two closer than `min_spacing_m`. A city with
    /// fewer signals gets fewer units.
    pub count: u32,
    /// What each does (as [`Rsu::roles`]).
    #[serde(default)]
    pub roles: Vec<String>,
    /// The hardware profile id of each (as [`Rsu::profile`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// The backhaul of each (as [`Rsu::backhaul`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backhaul: Option<String>,
    /// The backhaul parameters of each (as [`Rsu::backhaul_params`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backhaul_params: Option<serde_json::Value>,
    /// The least distance between two units, metres; 150 m by default, about two
    /// downtown blocks.
    #[serde(default = "RsuPlacement::default_spacing")]
    pub min_spacing_m: f64,
}

impl RsuPlacement {
    fn default_spacing() -> f64 {
        150.0
    }
}

/// Vehicle demand and the fleet mix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vehicles {
    /// The demand model and its parameters.
    #[serde(default)]
    pub demand: DemandSpec,
    /// The classes in the fleet, by name, each with its share.
    #[serde(default)]
    pub classes: BTreeMap<String, VehicleClassSpec>,
    /// What fraction of vehicles carry an OBU. `0.0` is a legal scenario: a pure traffic
    /// run with no radio.
    #[serde(default = "Vehicles::default_equipped")]
    pub equipped_fraction: f64,
    /// The driving-model library: which car-following model each class drives (IDM,
    /// EIDM, Krauss, Gipps, Wiedemann 99, ACC, the V2X-dependent CACC and platoon laws),
    /// how cooperative vehicles age and associate what they hear, and the sub-lane lateral
    /// model ([`v2xw_mobility::carfollowing::library::DrivingSpec`]). Left out of the
    /// serialised scenario when it is the default, so a scenario that does not mention it
    /// keeps its content hash and its behaviour.
    #[serde(
        default,
        skip_serializing_if = "v2xw_mobility::carfollowing::library::DrivingSpec::is_default"
    )]
    pub driving: v2xw_mobility::carfollowing::library::DrivingSpec,
    /// The drivers: how human they are (the realism preset, each driver's own
    /// car-following parameters, reaction time, perception errors, the truck left-lane
    /// restriction and motorcycle lane filtering) and the passenger-car driver as plain
    /// numbers (IDM `a`, `b`, `T`, `s0`; MOBIL politeness and threshold). Left out of the serialised scenario when it is the
    /// default, so a scenario that does not mention it keeps its content hash.
    #[serde(default, skip_serializing_if = "Drivers::is_default")]
    pub drivers: Drivers,
    // traffic audit
    /// The positioning error the vehicles' GNSS receivers make, which every BSM's
    /// position carries. Unset is a standard automotive receiver (Reid 2019 error
    /// percentiles, the legacy engine's outlier and burst rates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gnss: Option<GnssSettings>,
    /// The per-device BSM timing model (`node/bsm-timing/per-device`): each OBU's timing
    /// class sets its transmitted BSMs' secMark phase, position-epoch slips, frozen
    /// repeats and generationTime jitter, as measured on CV-pilot BSMs
    /// (research/synthesis/check-realdata-and-vesnos.md §2.3). Unset is off: every BSM
    /// reports the position at its own generation instant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bsm_timing: Option<BsmTimingSettings>,
}

/// The per-device BSM timing model's settings ([`Vehicles::bsm_timing`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BsmTimingSettings {
    /// Turns the model on. Unset or false is off (every existing run unchanged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// A `v2xw.bsm_timing.v1` calibration file (YAML), as the real-data pipeline writes
    /// it (`data/calibration/bsm-timing-cvpilot.yaml`). Unset is the built-in values,
    /// which are PROVISIONAL (CV-pilot sample statistics,
    /// research/synthesis/check-realdata-and-vesnos.md §2.2-2.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<String>,
    /// The share of OBUs in the `locked` class (secMark on the GNSS fix epoch), 0 to 1;
    /// the other classes share the rest in proportion. Unset is the calibration's own
    /// share (0.25 built in: Tampa's 48 locked of 190 classified IDs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_share: Option<f64>,
    /// The locked class's epoch stamp chain (position on the fix epoch, secMark on it or
    /// 1 ms early, steps at 0 -> -1 transitions). Unset or not enabled is the calibration's
    /// own stamp (the v3 fit's jittered instant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp_chain: Option<StampChainSettings>,
}

/// The locked class's epoch stamp chain ([`BsmTimingSettings::stamp_chain`],
/// `v2xw_node::bsm_timing::StampChain`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StampChainSettings {
    /// Turns the chain on. Unset or false is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// The rate set: `tampa_locked` (default; Tampa's locked IDs, a fit set),
    /// `wyoming_obu` (the one Wyoming OBU, P1-E4's held-out source) or `tampa_chain`
    /// (Tampa's chain devices alone, steps out of state 0 only; use it with
    /// `data/calibration/bsm-timing-cvpilot-chain-device.yaml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// P(the next BSM is stamped 1 ms early | this one is); unset is the profile's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stay_minus1: Option<f64>,
    /// P(the next BSM is stamped on the epoch | this one is); unset is the profile's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stay_zero: Option<f64>,
    /// Lasting position steps at 0 -> -1 transitions, at the profile's rate and size.
    /// Unset is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<bool>,
    /// The reported speed's change with a step, m/s per m of along-track step. Unset is
    /// no speed change; set, it turns the change on (Tampa's chain devices: 0.499).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_per_m: Option<f64>,
    /// The speed change's decay time, s; unset is Tampa's 0.357 (needs `speed_per_m`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_tau_s: Option<f64>,
    /// The share of the speed change the reported position moves with, 0 to 1; unset is
    /// Tampa's 0.759 (needs `speed_per_m`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_follows: Option<f64>,
}

impl StampChainSettings {
    /// Whether the chain runs.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.enabled == Some(true)
    }

    /// The chain these settings name.
    ///
    /// # Errors
    /// A sentence naming the field and what is wrong with it.
    pub fn chain(&self) -> Result<v2xw_node::bsm_timing::StampChain, (&'static str, String)> {
        use v2xw_node::bsm_timing::StampChain;
        let name = self.profile.as_deref().unwrap_or("tampa_locked");
        let mut ch = StampChain::profile(name).ok_or_else(|| {
            (
                "actors.vehicles.bsm_timing.stamp_chain.profile",
                format!(
                    "is {name:?}; the profiles are {}",
                    StampChain::PROFILES.join(", ")
                ),
            )
        })?;
        if let Some(v) = self.stay_minus1 {
            ch.stay_minus1 = v;
        }
        if let Some(v) = self.stay_zero {
            ch.stay_zero = v;
        }
        if self.steps == Some(false) {
            ch.step = None;
        }
        if self.speed_per_m.is_none()
            && (self.speed_tau_s.is_some() || self.position_follows.is_some())
        {
            return Err((
                "actors.vehicles.bsm_timing.stamp_chain.speed_per_m",
                "is unset, so speed_tau_s and position_follows have nothing to act on".into(),
            ));
        }
        if let Some(per_m) = self.speed_per_m {
            let Some(s) = ch.step.as_mut() else {
                return Err((
                    "actors.vehicles.bsm_timing.stamp_chain.speed_per_m",
                    "is set and steps are off: the speed change comes with a step".into(),
                ));
            };
            let d = StampChain::TAMPA_CHAIN_SPEED;
            s.speed = Some(v2xw_node::bsm_timing::StepSpeed {
                per_m_along_mps: per_m,
                tau_s: self.speed_tau_s.unwrap_or(d.tau_s),
                position_follows: self.position_follows.unwrap_or(d.position_follows),
            });
        }
        ch.check("actors.vehicles.bsm_timing.stamp_chain")
            .map_err(|e| ("actors.vehicles.bsm_timing.stamp_chain", e))?;
        Ok(ch)
    }
}

impl BsmTimingSettings {
    /// Whether the model runs.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.enabled == Some(true)
    }

    /// The calibration these settings name, with the locked share applied.
    ///
    /// # Errors
    /// A sentence naming the field and what is wrong with it.
    pub fn calibration(
        &self,
    ) -> Result<v2xw_node::bsm_timing::BsmTimingCalibration, (&'static str, String)> {
        let cal = match &self.calibration {
            None => v2xw_node::bsm_timing::BsmTimingCalibration::provisional(),
            Some(path) => {
                let text = std::fs::read_to_string(path).map_err(|e| {
                    (
                        "actors.vehicles.bsm_timing.calibration",
                        format!("names {path}, which cannot be read: {e}"),
                    )
                })?;
                v2xw_node::bsm_timing::BsmTimingCalibration::from_text(&text).map_err(|e| {
                    (
                        "actors.vehicles.bsm_timing.calibration",
                        format!("names {path}, which is not a v2xw.bsm_timing.v1 file: {e}"),
                    )
                })?
            }
        };
        let cal = match self.locked_share {
            None => cal,
            Some(share) => cal
                .with_locked_share(share)
                .map_err(|e| ("actors.vehicles.bsm_timing.locked_share", e))?,
        };
        match &self.stamp_chain {
            Some(s) if s.is_on() => cal
                .with_stamp_chain(s.chain()?)
                .map_err(|e| ("actors.vehicles.bsm_timing.stamp_chain", e)),
            _ => Ok(cal),
        }
    }
}

// traffic audit
/// The GNSS error model's settings ([`Vehicles::gnss`]); each unset field keeps the
/// model's default ([`v2xw_mobility::gnss::GaussMarkovParams`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GnssSettings {
    /// An RTK-corrected receiver (Reid 2019's OxTS RT3000 rows: 0.26 m at the 68th
    /// percentile) rather than a standard one (about 1.5 m). Unset is standard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtk: Option<bool>,
    /// How often a fix jumps by an outlier, per second. Unset is 0.01.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outlier_rate_hz: Option<f64>,
    /// How far an outlier puts the fix, metres. Unset is 12 m.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outlier_magnitude_m: Option<f64>,
    /// How often a degraded burst (multipath, an urban canyon) starts, per second. Unset
    /// is 0.006.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst_rate_hz: Option<f64>,
    /// The chance each fix is lost entirely. Unset is 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outage_probability: Option<f64>,
    /// The receiver's position-smoothing time constant, seconds: the receiver propagates
    /// its solution with its own Doppler velocity and blends each raw fix in at this time
    /// constant, as deployed receivers do (`gnss/error/gauss-markov` module docs). Unset
    /// or 0 outputs every raw fix, whose white noise makes consecutive 10 Hz BSMs
    /// disagree by metres (R-USFP).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothing_tau_s: Option<f64>,
    /// The receiver's static-hold threshold, m/s: below this measured speed it holds its
    /// position and reports zero velocity. Unset or 0 is off; read only with
    /// `smoothing_tau_s`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_hold_mps: Option<f64>,
    /// The receiver's velocity output noise, m/s per horizontal axis, drawn afresh at every
    /// fix. Unset is the GPS SPS committed velocity accuracy (0.2 m/s at 95 %, σ 0.102 m/s),
    /// which as a white 10 Hz noise makes consecutive BSMs' speeds differ by sd 0.14 m/s,
    /// where CV-pilot locked devices' differ by 0.045 m/s
    /// (research/synthesis/p1-realdata-baseline.md §4). 0 leaves the BSM-level speed and
    /// heading noise to `bsm_timing`'s fitted `output_noise`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity_noise_mps: Option<f64>,
    /// The one-sigma horizontal accuracy the receiver states in each BSM's J2735
    /// `PositionalAccuracy.semiMajor`, metres (the median over devices). Unset or 0
    /// states the model's own honest 95 % radius (about 6.5 m with raw fixes, 5.1 m
    /// smoothed). Deployed OBUs state far less than their real error: the CV-pilot
    /// medians are 0.30 m Tampa, 0.40 m NYC, 0.50 m Wyoming and a constant 2.00 m Orem
    /// (detect-fix3, measured with tools/realdata).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stated_sigma_m: Option<f64>,
    /// The per-device spread of that stated accuracy: a lognormal's log-sd, drawn once
    /// per receiver. Unset or 0 states the median on every device; Tampa's per-device
    /// medians give 0.81.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stated_sigma_log_sd: Option<f64>,
}

impl GnssSettings {
    /// The model's parameters with these over the defaults.
    pub fn params(&self) -> v2xw_mobility::gnss::GaussMarkovParams {
        let mut p = v2xw_mobility::gnss::GaussMarkovParams::default();
        if let Some(v) = self.rtk {
            p.rtk = v;
        }
        if let Some(v) = self.outlier_rate_hz {
            p.outlier_rate = v;
        }
        if let Some(v) = self.outlier_magnitude_m {
            p.outlier_magnitude_m = v;
        }
        if let Some(v) = self.burst_rate_hz {
            p.burst_rate = v;
        }
        if let Some(v) = self.outage_probability {
            p.outage_rate = v;
        }
        if let Some(v) = self.smoothing_tau_s {
            p.smoothing_tau_s = v;
        }
        if let Some(v) = self.static_hold_mps {
            p.static_hold_mps = v;
        }
        if let Some(v) = self.velocity_noise_mps {
            p.sigma_velocity_mps = v;
        }
        if let Some(v) = self.stated_sigma_m {
            p.stated_sigma_m = v;
        }
        if let Some(v) = self.stated_sigma_log_sd {
            p.stated_sigma_log_sd = v;
        }
        p
    }
}

/// How human the drivers are (`actors.vehicles.drivers`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drivers {
    /// The driver population: `calibrated` (the default), `identical`, `heterogeneous`
    /// or `human`.
    #[serde(default)]
    pub realism: DriverRealism,
    /// Overrides how far drivers differ in time gap, acceleration, braking and standstill
    /// gap: a multiplier on the per-class spread (0 none, 1 as fitted to real drivers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_spread: Option<f64>,
    /// Overrides the drivers' reaction time in car following, seconds (0 reacts at once).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaction_time_s: Option<f64>,
    /// Overrides how badly drivers misjudge the gap and the closing speed: a multiplier on
    /// the Human Driver Model's estimation errors (0 none, 1 as published).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perception_error: Option<f64>,
    /// Keeps trucks out of the leftmost lane of a motorway with three or more lanes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truck_lane_restriction: Option<bool>,
    /// Overrides the jurisdiction's rule on motorcycles filtering between stopped cars;
    /// true turns on the sub-lane model if the scenario has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motorcycle_filtering: Option<bool>,
    /// The passenger car's comfortable maximum acceleration `a` (Treiber, Hennecke and
    /// Helbing 2000), m/s². Replaces the model's calibration for cars; every driver still
    /// gets their own style and spread around it. Unset is the model's (1.4 m/s² for the
    /// urban IDM).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_accel_mps2: Option<f64>,
    /// The passenger car's comfortable deceleration `b`, m/s². Unset is the model's
    /// (2.0 m/s²).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_decel_mps2: Option<f64>,
    /// The passenger car's desired time gap to the vehicle ahead `T`, s. Shorter gaps
    /// raise road capacity and shorten the time a warning has to act. Unset is the
    /// model's (about 1.2 to 1.5 s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_headway_s: Option<f64>,
    /// The gap a passenger car keeps to a stopped vehicle `s0`, m. Unset is the model's
    /// (2 m).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_gap_m: Option<f64>,
    /// MOBIL's politeness factor `p` (Kesting, Treiber and Helbing 2007): 0 is a selfish
    /// driver who changes lane whenever it helps them, 1 weighs the followers' loss as
    /// much as their own gain. Unset is 0.5 (Kesting 2007 reports 0.2 to 0.5 as
    /// realistic). Applies to every class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub politeness: Option<f64>,
    /// MOBIL's lane-change threshold `Δa_th`, m/s²: the least advantage worth changing
    /// lane for. Unset is 0.1 m/s² (ibid.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_change_threshold_mps2: Option<f64>,
    // traffic audit
    /// How fast drivers choose to drive on a free road, as a multiple of the posted limit:
    /// the population mean of SUMO's `speedFactor`. Unset is 1.0 (SUMO's default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_factor: Option<f64>,
    /// How far drivers differ in that chosen speed: a multiplier on each class's
    /// deviation (SUMO `speedDev`, 0.1 for a car). 0 gives everyone the mean. Unset is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_spread: Option<f64>,
    /// The share of drivers who run reds: they go on an amber inside their threshold
    /// without checking they reach the line before the red. Unset is 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red_runner_share: Option<f64>,
    /// Rule violations and crash states (WP-08): red-light running after the red onset,
    /// amber running beyond the dilemma zone, failure to yield, and collisions that stop
    /// the vehicles and block the lane until cleared. Unset (or `enabled: false`) is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub violations: Option<Violations>,
    /// The median time a driver takes to move off when the light turns green or a
    /// crosswalk clears, seconds. Unset is 1.08 s (Bonneson 1992's 1.22 s mean start-response, lognormal σ 0.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_reaction_s: Option<f64>,
    /// The median delay before a queued driver moves off after the car in front does,
    /// seconds: what sets the start-up lost time and the saturation flow at a light.
    /// Unset is 0.5 s (calibrated to Greenshields 1947 discharge headways).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_start_delay_s: Option<f64>,
    /// Trucks' and trailers' car-following numbers. Unset is the model's per-class
    /// calibration (SUMO vType `truck`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truck: Option<ClassDriver>,
    /// Buses' and coaches' car-following numbers. Unset is the model's (SUMO `bus`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<ClassDriver>,
    /// Motorcycles' and mopeds' car-following numbers. Unset is the two-wheeler
    /// calibration (2.5 m/s², 3.0 m/s², 1.2 s, 1.5 m).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motorcycle: Option<ClassDriver>,
    /// Cyclists' car-following numbers. Unset is SUMO's bicycle vType.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bicycle: Option<ClassDriver>,
}

/// `actors.vehicles.drivers.violations` ([`v2xw_mobility::drivers::ViolationParams`]);
/// every unset field keeps its default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Violations {
    /// Whether drivers commit violations at all. Unset is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// The share of drivers who enter on red within their tolerance. Unset is 0.6,
    /// calibrated so that the run reproduces 1.0 violation per 10,000 vehicle-cycles
    /// (Bonneson et al., TTI 0-4196-P1, Table 1, Texas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red_run_share: Option<f64>,
    /// The mean of a runner's tolerance after the red onset, seconds (exponential). Unset is
    /// 0.6 s: a median entry 0.42 s into red (TTI 0-4196-P1 Figure 4: under 0.5 s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red_run_tolerance_mean_s: Option<f64>,
    /// The longest tolerance, seconds. Unset is 4.0 s (98 % of violations within 4.0 s,
    /// TTI 0-4196-P1 Table 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red_run_tolerance_max_s: Option<f64>,
    /// The longest amber threshold drawn, seconds: the Zegeer & Deen 1978 logistic's tail
    /// beyond the dilemma zone's far edge. Unset is 8 s; 5.5 keeps the clamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amber_tail_max_s: Option<f64>,
    /// The probability that a driver fails to yield at a junction where they must give way
    /// (SUMO `jmIgnoreFoeProb`). Unset is 0 (todo-calibrate: no field rate was read).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_to_yield_share: Option<f64>,
    /// The fastest foe a failing driver ignores, m/s (SUMO `jmIgnoreFoeSpeed`). Unset is 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore_foe_speed_mps: Option<f64>,
    /// Whether a collision resolves to a crash state. Unset is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crashes: Option<bool>,
    /// How long a crashed vehicle stands before it is cleared, seconds. Unset is 900 s (a
    /// choice, todo-calibrate).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crash_clearance_s: Option<f64>,
}

impl Violations {
    /// The engine's parameters this block asks for.
    pub fn params(&self) -> v2xw_mobility::drivers::ViolationParams {
        let mut p = v2xw_mobility::drivers::ViolationParams::default();
        if let Some(x) = self.enabled {
            p.enabled = x;
        }
        if let Some(x) = self.red_run_share {
            p.red_run_share = x;
        }
        if let Some(x) = self.red_run_tolerance_mean_s {
            p.red_run_tolerance_mean_s = x;
        }
        if let Some(x) = self.red_run_tolerance_max_s {
            p.red_run_tolerance_max_s = x;
        }
        if let Some(x) = self.amber_tail_max_s {
            p.amber_tail_max_s = x;
        }
        if let Some(x) = self.fail_to_yield_share {
            p.fail_to_yield_share = x;
        }
        if let Some(x) = self.ignore_foe_speed_mps {
            p.ignore_foe_speed_mps = x;
        }
        if let Some(x) = self.crashes {
            p.crashes = x;
        }
        if let Some(x) = self.crash_clearance_s {
            p.crash_clearance_s = x;
        }
        p
    }
}

// traffic audit
/// One vehicle class's car-following numbers ([`Drivers::truck`] and its siblings); each
/// unset field keeps the class's own calibration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassDriver {
    /// Comfortable maximum acceleration `a`, m/s².
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_accel_mps2: Option<f64>,
    /// Comfortable deceleration `b`, m/s².
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_decel_mps2: Option<f64>,
    /// Desired time gap to the vehicle ahead `T`, s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_headway_s: Option<f64>,
    /// Gap kept to a stopped vehicle `s0`, m.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_gap_m: Option<f64>,
}

impl ClassDriver {
    /// As the engine's override.
    pub fn as_override(&self) -> v2xw_mobility::DriverOverride {
        v2xw_mobility::DriverOverride {
            max_accel_mps2: self.max_accel_mps2,
            comfort_decel_mps2: self.comfort_decel_mps2,
            time_headway_s: self.time_headway_s,
            min_gap_m: self.min_gap_m,
        }
    }
}

impl Drivers {
    /// Whether this is the block a scenario that does not mention it gets.
    pub fn is_default(&self) -> bool {
        *self == Drivers::default()
    }

    /// The engine's human-driver parameters this block asks for.
    pub fn human_params(&self) -> v2xw_mobility::drivers::HumanDriverParams {
        use v2xw_mobility::drivers::{HDM_DISTANCE_ERROR, HDM_RATE_ERROR_PER_S, HumanDriverParams};
        let mut p = match self.realism {
            DriverRealism::Calibrated | DriverRealism::Identical => HumanDriverParams::default(),
            DriverRealism::Heterogeneous => HumanDriverParams::heterogeneous(),
            DriverRealism::Human => HumanDriverParams::human(),
        };
        if let Some(x) = self.parameter_spread {
            p.parameter_spread = x;
        }
        if let Some(x) = self.reaction_time_s {
            p.reaction_time_s = x;
        }
        if let Some(x) = self.perception_error {
            p.distance_error = HDM_DISTANCE_ERROR * x;
            p.rate_error_per_s = HDM_RATE_ERROR_PER_S * x;
        }
        if let Some(x) = self.truck_lane_restriction {
            p.truck_lane_restriction = x;
        }
        p
    }
}

/// The driver population of `actors.vehicles.drivers.realism`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DriverRealism {
    /// Calibrated city drivers: their own free speed, reaction at a green and one of three
    /// following styles; car following itself is exact.
    #[default]
    Calibrated,
    /// Every driver the textbook model: the limit exactly, no reaction time, one style.
    Identical,
    /// Calibrated, plus every driver their own time gap, acceleration, braking and
    /// standstill gap, spread as fitted to real drivers.
    Heterogeneous,
    /// Heterogeneous, plus a 0.8 s reaction time and the Human Driver Model's perception
    /// errors: traffic that breaks into stop-and-go waves as real traffic does.
    Human,
}

impl Vehicles {
    fn default_equipped() -> f64 {
        1.0
    }
}

impl Default for Vehicles {
    fn default() -> Self {
        Vehicles {
            demand: DemandSpec::default(),
            classes: BTreeMap::new(),
            equipped_fraction: Vehicles::default_equipped(),
            driving: v2xw_mobility::carfollowing::library::DrivingSpec::default(),
            drivers: Drivers::default(),
            gnss: None,
            bsm_timing: None,
        }
    }
}

/// How vehicles arrive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemandSpec {
    /// The demand model's id, e.g. `mobility/demand/poisson`.
    #[serde(default = "DemandSpec::default_kind")]
    pub kind: String,
    /// Vehicles per hour offered to the network, when the model takes a rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_veh_per_h: Option<f64>,
    /// Vehicles per hour per kilometre of driving lane: the arrival rate stated so that it
    /// fits any city. Once the world is built it becomes `rate_veh_per_h` (the rate times
    /// the network's driving-lane length), and it wins over a `rate_veh_per_h` the file
    /// also gives. Midtown's 119 km of driving lane at 25 veh/h/km is 3,000 veh/h.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_veh_per_h_per_lane_km: Option<f64>,
    /// Hold exactly this many vehicles on the road for the whole run: every trip that ends
    /// is replaced by a new one, and a trip that cannot enter or has no route is redrawn.
    /// The arrival rate is then not used. Absent runs the arrival process.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fleet_size: Option<u32>,
    /// The model's own parameters, in the shape its parameter struct deserialises.
    #[serde(default)]
    pub params: serde_json::Value,
}

impl DemandSpec {
    fn default_kind() -> String {
        "mobility/demand/none".to_string()
    }
}

impl Default for DemandSpec {
    fn default() -> Self {
        DemandSpec {
            kind: DemandSpec::default_kind(),
            rate_veh_per_h: None,
            rate_veh_per_h_per_lane_km: None,
            fleet_size: None,
            params: serde_json::Value::Null,
        }
    }
}

/// One class in the fleet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VehicleClassSpec {
    /// Its share of the fleet, `0..=1`. The shares across classes must sum to 1.
    pub fraction: f64,
    /// The hardware profile its OBU runs, or `null` for an unequipped class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obu: Option<String>,
}

/// Vulnerable road users.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vru {
    /// How many pedestrians.
    #[serde(default)]
    pub pedestrians: u32,
    /// How many cyclists.
    #[serde(default)]
    pub cyclists: u32,
    /// What fraction of them carry a device that transmits.
    #[serde(default)]
    pub device_fraction: f64,
    /// How the pedestrians behave: `observed` (the default — walking speeds by age group,
    /// start-up at walk, a share who cross against the signal into a gap, groups, and
    /// mid-block crossing) or `document` (the 1995 social-force defaults of 04-models.md
    /// §2.5: one speed law, everyone waits for walk, nobody crosses mid-block).
    #[serde(default)]
    pub behaviour: VruBehaviour,
    /// Overrides the observed behaviour's mid-block crossing rate, decisions per 100 m of
    /// eligible sidewalk walked; 0 turns mid-block crossing off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midblock_rate_per_100m: Option<f64>,
    /// Overrides the observed behaviour's share of pedestrians who cross against the
    /// signal when the traffic leaves a gap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red_crossing_share: Option<f64>,
    /// Overrides the share of cyclists on e-bikes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ebike_share: Option<f64>,
    /// Overrides the median time a pedestrian wants a traffic gap to leave beyond the time
    /// to walk across (the HCM's start-up and clearance time), seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossing_gap_margin_s: Option<f64>,
    /// Overrides the mean walking speed of pedestrians under 65, m/s. Under `observed`
    /// the 65-and-over mean scales with it (keeping Knoblauch et al. 1996's ratio, 1.25
    /// against 1.51 m/s); under `document` it is the one Gaussian's mean (1.34 m/s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walking_speed_mps: Option<f64>,
}

/// How the pedestrians of `actors.vru` behave.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VruBehaviour {
    /// As observed in the field studies the mobility crate cites.
    #[default]
    Observed,
    /// The social-force model's document defaults.
    Document,
}

/// A roadside unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rsu {
    /// The world site it stands at, by site id.
    ///
    /// Optional, because not every world has a site table: the procedural generator makes
    /// one site per junction with `rsu_at_junctions: true`, and the OSM importer makes
    /// none at all — an extract carries road geometry and buildings, not the mast
    /// inventory of a deployment. A scenario on a real city therefore states the position
    /// itself, in [`Rsu::position_m`]. Exactly one of the two must be given, which
    /// [`crate::scenario::validate`] enforces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<u32>,
    /// Where it stands, world-local ENU metres `[east, north, up]`, when the world has no
    /// site to name.
    ///
    /// The `up` component is the **ground** height; the antenna height the propagation
    /// model reads is the profile's mast, added the same way a site's is
    /// ([`v2xw_world::Site::antenna_position`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_m: Option<[f64; 3]>,
    /// What it does: `crl`, `provisioning-proxy`, `report-forward`, `spat`, `map`, `wsa`,
    /// `cpm` (infrastructure collective perception from its own sensors, `perception.rsu`).
    #[serde(default)]
    pub roles: Vec<String>,
    /// Its hardware profile id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Its backhaul link model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backhaul: Option<String>,
    /// This unit's own backhaul parameters — `rate_mbps`, `down_mbps`, `latency_ms`,
    /// `distance_km`, `hops`, `loss` (the medium card's keys) — over `net.backhaul.params`
    /// when it names the same medium, over the medium's defaults otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backhaul_params: Option<serde_json::Value>,
}

/// Backend entities and their topology.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backend {
    /// The credential-management protocol id, e.g. `proto/scms-camp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    /// The entities, by role name (`ra`, `pca`, `la1`, `la2`, `ma`, `crlg`, …).
    #[serde(default)]
    pub entities: BTreeMap<String, BackendEntity>,
    /// The links between them.
    #[serde(default)]
    pub links: Vec<BackendLink>,
    /// Servers that are down, slow or lying, and when (`v2xw_proto::fault`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<BackendFailureSpec>,
}

/// One backend entity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendEntity {
    /// Its hardware profile id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Its service model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_model: Option<String>,
    /// Its network model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<String>,
}

/// A link between two backend entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendLink {
    /// The entity the link starts at.
    pub from: String,
    /// The entity it ends at.
    pub to: String,
    /// One-way latency, milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<f64>,
    /// Capacity, megabits per second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity_mbps: Option<f64>,
}

/// Weather at `t0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weather {
    /// The kind of weather the run starts in.
    #[serde(default)]
    pub initial: WeatherKind,
    /// How hard it is doing it, on `[0, 1]`.
    ///
    /// Dimensionless on purpose, and the same scale [`v2xw_core::WeatherState::intensity`]
    /// uses: a millimetre-per-hour rate is what a radio attenuation model wants and a
    /// fraction of maximum is what a behavioural model wants, and the two do not convert
    /// without a per-kind maximum that no source publishes. Each model's card declares
    /// what its own `1.0` means.
    #[serde(default)]
    pub intensity: f64,
    /// Meteorological visibility, metres. Absent means unrestricted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility_m: Option<f64>,
    /// The road surface, which need not follow from the kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<v2xw_core::weather::SurfaceCondition>,
}

impl Default for Weather {
    fn default() -> Self {
        Weather {
            initial: WeatherKind::Clear,
            intensity: 0.0,
            visibility_m: None,
            surface: None,
        }
    }
}

/// The radio stack.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Radio {
    /// Which radio access technology.
    #[serde(default)]
    pub rat: Rat,
    /// The fidelity tier of each radio family.
    #[serde(default)]
    pub tiers: RadioTiers,
    /// The model chosen for each family, with its parameters.
    #[serde(default)]
    pub models: BTreeMap<String, ModelChoice>,
    /// The radio each kind of node carries: transmit power, antenna gain, cable loss and
    /// antenna height.
    #[serde(default)]
    pub devices: RadioDevices,
    /// How far a transmission is followed: derived from the link budget.
    #[serde(default)]
    pub range: CandidateRange,
    /// The regulatory region: its band plan, the channel each technology deploys on and
    /// the EIRP limits (`v2xw_radio::regulation`). Unset, the region the technology was
    /// deployed under: `us-2016` for 802.11p (the DSRC band plan SAE J2945/1 was written
    /// for), `us` for LTE-V2X and NR-V2X (FCC 24-123).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<RadioRegion>,
    /// The channel number within the region's band plan. Unset, the region's deployment
    /// channel for the technology.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<u16>,
    /// Transmitters of another technology on a neighbouring channel, where the region
    /// lets the two operate side by side (ITS-G5 beside LTE-V2X in Europe): their
    /// leakage enters this run's receivers attenuated by the adjacent-channel
    /// interference ratio.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjacent_channel: Vec<AdjacentEmitter>,
    /// The run's own channel map (WP-13): which messages leave the safety channel
    /// (`radio.channel`) for a service channel of the region's band plan, as an RSU's
    /// SPaT, MAP and WSA do on a multi-channel deployment (IEEE 1609.4-2016 §6.2.5; SAE
    /// J2945/1 keeps the BSM on channel 172; ETSI EN 302 663 / TS 103 141 in Europe).
    /// A unit hears and sends on the service channel only with a second radio
    /// (`radio.devices.*.radios: 2`). Empty, the default: every message on
    /// `radio.channel`, exactly as before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<ChannelAssignment>,
    /// Whether the run's own channels leak into each other through the transmitter mask
    /// and the receiver's selectivity (the ACIR of `v2xw_radio::regulation`), WP-13.
    /// Unset is on; `false` is ideal channel filters. Only acts with `radio.channels`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_leakage: Option<bool>,
    /// The 802.11p receiver and transmitter settings a researcher varies: data rate (MCS),
    /// noise figure, receiver sensitivity, carrier-sense threshold and capture threshold.
    /// Each one left unset keeps the model's cited default.
    #[serde(default, skip_serializing_if = "PhySettings::is_default")]
    pub phy: PhySettings,
    /// 802.11p medium access (EDCA): which access category each kind of message is queued
    /// in, the contention window and arbitration spacing of the two safety categories, and
    /// the queue length.
    #[serde(default, skip_serializing_if = "MacSettings::is_default")]
    pub mac: MacSettings,
    /// The radio channel as plain numbers: path-loss exponent, shadowing spread and
    /// decorrelation distance, and the small-scale fading law (none, Rayleigh, Rice or
    /// Nakagami-m).
    #[serde(default, skip_serializing_if = "ChannelModelSettings::is_default")]
    pub channel_model: ChannelModelSettings,
    /// Decentralised congestion control as numbers: on or off, the ETSI adaptive target
    /// load and gains, and the SAE J2945/1 rate and power limits. Which algorithm runs is
    /// `radio.models.dcc` (the region's by default).
    #[serde(default, skip_serializing_if = "DccSettings::is_default")]
    pub dcc: DccSettings,
    /// C-V2X sidelink scheduling as numbers (LTE-V2X Mode 4, NR-V2X Mode 2): the
    /// reservation interval, the probability of keeping a resource, the sensing exclusion
    /// threshold, and sensing on or off. The profile, MCS and retransmissions are
    /// `radio.models.sidelink`'s.
    #[serde(default, skip_serializing_if = "SidelinkSettings::is_default")]
    pub sidelink: SidelinkSettings,
}

/// Sidelink semi-persistent scheduling settings (`radio.sidelink`). Unset fields keep the
/// profile's.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidelinkSettings {
    /// How often a radio's reserved resource repeats, ms (the resource reservation
    /// interval, RRI). LTE allows 20, 50, 100 or a multiple of 100 up to 1,000
    /// (TS 36.331 `sl-ResourceReservationPeriod`); NR also 1 to 99. Unset is 100 ms, one
    /// BSM period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation_interval_ms: Option<u32>,
    /// The probability a radio keeps its resource when the reselection counter runs out
    /// (`probResourceKeep`): 0, 0.2, 0.4, 0.6 or 0.8 (TS 36.331). Higher keeps
    /// reservations stable for others to sense, but makes two radios that collide keep
    /// colliding. Unset is the profile's (0.8 under SAE J3161/1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_keep_probability: Option<f64>,
    /// The RSRP above which a resource another radio reserved is excluded from selection,
    /// dBm; it is raised 3 dB at a time until enough candidates remain (TS 36.213
    /// §14.1.1.6). Unset is the profile's (−128 dBm NR, −110 dBm LTE studies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rsrp_threshold_dbm: Option<f64>,
    /// Whether radios sense the channel before choosing a resource. `false` is the random
    /// selection baseline (Ali et al. 2021 §II.1). Unset is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensing: Option<bool>,
    /// The share of the selection window that must survive exclusion before a radio
    /// picks: 20 % (LTE's fixed value), 35 % or 50 % (NR only). Unset is 20 %.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_percent: Option<u8>,
    /// How late after a message is ready its new resource may be (the selection window's
    /// end T2), ms. Shorter cuts latency but leaves fewer resources to pick from; at most
    /// the message interval. Unset is the profile's (100 ms at 10 Hz).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_window_ms: Option<u32>,
    // network audit
    /// The UE receiver noise figure, dB: TR 36.885 Annex A.1.1's 9 dB unless set. It
    /// moves every sidelink noise floor (and the candidate range measured against it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noise_figure_db: Option<f64>,
    /// `sl-ThresS-RSSI-CBR`, dBm: the S-RSSI above which a sub-channel counts as busy in
    /// the channel busy ratio (TS 36.214 §5.1.30, TS 36.331 range -112 to -8 dBm in 2 dB
    /// steps). -94 dBm unless set; the CR limits and every CBR metric read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cbr_threshold_dbm: Option<f64>,
}

impl SidelinkSettings {
    /// Whether nothing is set.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Congestion-control settings (`radio.dcc`). Unset fields keep the standards' tables.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DccSettings {
    /// Whether 802.11p congestion control runs at all. `false` lets every radio send at
    /// its full rate and power however busy the channel is: the baseline a DCC study
    /// compares against. Unset is on (EN 302 571 requires it in Europe).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// The channel load the ETSI adaptive algorithm steers towards, as a fraction of time
    /// busy. Unset is 0.68 (TS 102 687 V1.2.1 Table 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cbr_target: Option<f64>,
    /// The ETSI adaptive smoothing factor α: how much of each new load measurement enters
    /// the running average. Unset is 0.016 (ibid.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_alpha: Option<f64>,
    /// The ETSI adaptive gain β: how hard the allowed duty cycle reacts to the gap from the
    /// target. Unset is 0.0012 (ibid.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_beta: Option<f64>,
    /// The longest interval SAE J2945/1 may stretch a BSM's spacing to under congestion,
    /// ms (`vMax_ITT`). Unset is 600 ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_itt_ms: Option<f64>,
    /// The vehicle density at which SAE J2945/1 starts to stretch the interval
    /// (`B`, vehicles within 100 m). Unset is 25.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density_coefficient: Option<f64>,
    /// Whether SAE J2945/1 also lowers the radiated power as the channel fills. Unset is
    /// on (J2945/1 §6.3.8); J3161/1 for LTE-V2X has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_control: Option<bool>,
    /// The lowest power J2945/1's power control may go to, dBm (`vRP_Min`). Unset is 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_power_dbm: Option<f64>,
    // network audit
    /// ETSI reactive DCC's table (TS 102 687 V1.2.1 Annex A): `a1` (frames up to 1 ms on
    /// air, Table A.1, the default) or `a2` (up to 500 µs, Table A.2, twice the rates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactive_table: Option<String>,
    /// ETSI reactive DCC's state-hold time, ms: how long a state is held before the next
    /// step. 0 unless set (the tables specify no hysteresis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactive_hold_ms: Option<f64>,
}

impl DccSettings {
    /// Whether nothing is set.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// 802.11p physical-layer settings (`radio.phy`). Unset fields keep the defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhySettings {
    /// The data rate every safety frame is sent at, Mbit/s: one of 3, 4.5, 6, 9, 12, 18,
    /// 24 or 27 (the eight 10 MHz OFDM rates of IEEE 802.11-2020 Table 17-4). A higher
    /// rate takes less air time per frame but needs a stronger signal, so it trades range
    /// for channel capacity. Unset is 6 Mbit/s, the SAE J2945/1 default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_rate_mbps: Option<f64>,
    /// The receiver noise figure, dB: how much noise the receiver's own electronics add on
    /// top of the thermal floor of −104 dBm in 10 MHz. Unset is the hardware figure the
    /// PHY card cites (6 dB).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noise_figure_db: Option<f64>,
    /// One receiver sensitivity at every data rate, dBm: a frame weaker than this is not
    /// decoded (Veins' `minPowerLevel`). Unset uses the per-rate table that
    /// `radio.models.phy` selects (fielded units, −92 dBm at 6 Mbit/s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity_dbm: Option<f64>,
    /// The carrier-sense (CCA) threshold, dBm: energy above it makes a radio treat the
    /// channel as busy, defer its transmission and count the time in the channel busy
    /// ratio. Unset is −85 dBm (ETSI EN 302 571 §4.2.10.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cca_threshold_dbm: Option<f64>,
    /// The energy-detect level, dBm: another channel's frame leaking through the channel
    /// filter holds carrier sense busy only above it (and, under SAE J2945/1, counts in the
    /// CBP only above it). Unset is −65 dBm, 20 dB above the 10 MHz minimum sensitivity
    /// (IEEE 802.11-2020 §17.3.10.6); never below `cca_threshold_dbm`. −85 dBm is the
    /// carrier sense before radio-fix4, which held a leak to the preamble level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy_detect_dbm: Option<f64>,
    /// A hard capture threshold, dB: a frame is decoded only while it stays this far above
    /// the interference plus noise (Torrent-Moreno 2009, Eq. 1). Unset uses the SINR trace
    /// against the error model, as Veins does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_threshold_db: Option<f64>,
}

impl PhySettings {
    /// Whether nothing is set.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// 802.11p EDCA settings (`radio.mac`). Unset fields keep EN 302 663 Table C.4.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacSettings {
    /// The access category periodic awareness messages (BSM, CAM, PSM, VAM, CPM, SPaT,
    /// MAP) are queued in: `vo`, `vi`, `be` or `bk`. Unset is `vo`, the highest, which is
    /// where SAE J2945/1 puts the BSM; EN 302 663 Annex C puts the CAM at `vi`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub periodic_access_category: Option<String>,
    /// The access category event messages (DENM, forwarded hazard warnings) are queued
    /// in. Unset is `vo` (EN 302 663 Annex C).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_access_category: Option<String>,
    /// `CWmin` of the voice category, slots: a frame waits a random 0 to `CWmin` idle
    /// slots before it may start. A larger window means fewer collisions and more delay.
    /// Unset is 3 (EN 302 663 Table C.4). Broadcast never doubles it, so there is no
    /// `CWmax` to set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cw_min_vo: Option<u32>,
    /// `AIFSN` of the voice category, slots: the fixed idle wait before the countdown.
    /// Unset is 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aifsn_vo: Option<u32>,
    /// `CWmin` of the video category, slots. Unset is 7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cw_min_vi: Option<u32>,
    /// `AIFSN` of the video category, slots. Unset is 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aifsn_vi: Option<u32>,
    /// How many frames each access category queues before it drops the next. Unset is 64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_length: Option<u32>,
}

impl MacSettings {
    /// Whether nothing is set.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The radio channel as plain numbers (`radio.channel_model`). Unset fields keep the
/// propagation and fading models the tier or `radio.models` chooses.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelModelSettings {
    /// A single-slope path-loss exponent: 2 is free space, about 2.7 to 3.5 a city street.
    /// Setting it, or the shadowing spread, runs the log-distance law
    /// (`propagation/log-distance-shadowing`) with these numbers over a free-space anchor
    /// at 10 m. Unset keeps the measured dual-slope fits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_loss_exponent: Option<f64>,
    /// The log-normal shadowing standard deviation, dB: how much the received power
    /// wanders around the path-loss mean as buildings and trucks come and go. Unset keeps
    /// the fit's own (about 4 dB in line of sight, 6 to 7 dB obstructed, Abbas 2015).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowing_sigma_db: Option<f64>,
    /// How far a vehicle moves before its shadowing is uncorrelated with what it was,
    /// metres (Gudmundson 1991). Unset is the environment's (about 10 m in a city).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowing_decorrelation_m: Option<f64>,
    /// The small-scale fading law: `none`, `rayleigh` (no direct path), `rice` (a direct
    /// path `rice_k_db` stronger than the scatter) or `nakagami` (shape `nakagami_shape`).
    /// Unset keeps the tier's choice (Nakagami m = 3 for 802.11p).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fading: Option<String>,
    /// How the fade of one link changes from frame to frame: `independent` (each frame
    /// draws its own, the default) or `doppler` (one fade per coherence time,
    /// `0.423 / f_D` with `f_D` from both ends' speeds and the carrier, so slow links fade
    /// in bursts and fast links decorrelate; Akki & Haber 1986, Rappaport 2002 Eq. 5.40c).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fading_coherence: Option<String>,
    /// The Nakagami shape `m` when `fading` is `nakagami` (written `m` in the literature): 1 is Rayleigh, larger is
    /// milder. Unset is 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nakagami_shape: Option<f64>,
    /// The Rice factor `K` when `fading` is `rice`, dB: the direct path's power over the
    /// scattered power's. Unset is 6 dB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rice_k_db: Option<f64>,
    /// The Nakagami `m` beyond `nakagami_breakpoint_m`, when `fading` is `nakagami`, for a
    /// channel that gets harsher with range (Veins-style: about 1.5 near, 0.75 far). Needs
    /// the breakpoint. Unset keeps `nakagami_shape` at every distance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nakagami_shape_far: Option<f64>,
    /// The distance at which the Nakagami `m` switches to `nakagami_shape_far`, metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nakagami_breakpoint_m: Option<f64>,
    /// How much each building wall the signal passes through costs (Sommer 2011 β), dB:
    /// 9 dB for the fitted default, about 2.4 dB for light construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub building_wall_loss_db: Option<f64>,
    /// How much each metre of path inside a building costs (Sommer 2011 γ), dB per metre:
    /// 0.4 for the fitted default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub building_loss_db_per_m: Option<f64>,
    /// Whether other vehicles between two antennas attenuate the signal (3GPP TR 37.885
    /// NLOSv blockage). On by default with the city-street law; off makes vehicles
    /// radio-transparent, on charges them under every law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle_blockage: Option<bool>,
    /// Whether a path through trees loses the foliage excess (ITU-R P.833 §3.1 with
    /// GEMV²'s 2.3 dB/m at 5.9 GHz, capped at the diffraction over the canopy:
    /// `obstacle/foliage/p833`). Needs `world.vegetation`. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foliage: Option<bool>,
    /// Whether the log-distance law's shadowing σ grows with the vehicles and buildings
    /// around the link (GEMV², Boban et al. 2014 eq. 10: 3.3 dB on an empty road to
    /// 5.2 dB downtown for line of sight). Unset keeps the preset's fixed σ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density_sigma: Option<bool>,
    /// Whether a building-blocked link also receives the strongest single specular
    /// reflection off a building wall, power-summed with the shadowed path (GEMV², Boban
    /// et al. 2014 §IV-B.3, eqs. 6-8; ITU-R P.2040-3 Table 3 for the walls:
    /// `obstacle/reflection/gemv2-single-bounce`). Needs buildings. Unset is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection: Option<bool>,
}

impl ChannelModelSettings {
    /// Whether nothing is set.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Whether these settings replace the propagation law's slope (the exponent is the
    /// log-distance law's; shadowing applies to every law).
    pub fn tunes_law(&self) -> bool {
        self.path_loss_exponent.is_some()
    }
}

/// One transmitter on an adjacent channel (`radio.adjacent_channel`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdjacentEmitter {
    /// Its technology.
    pub rat: Rat,
    /// Its channel number; unset, the region's deployment channel for its technology.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<u16>,
    /// The adjacent-channel interference ratio, dB; unset, the region's mask for the
    /// emitter's technology combined with the minimum selectivity of this run's receivers
    /// (`v2xw_radio::regulation::adjacent_acir_db`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acir_db: Option<f64>,
    /// Where it is and when it transmits, as a jammer's: `position_m`, `follow_node` or
    /// `path_m` (with `speed_mps`, `loop_path`); `power_dbm` (23 by default), `from_s`,
    /// `to_s`, and `period_ms` with `duty` for an emitter that is not on all the time.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// A regulatory region: the band plan and power limits a run transmits under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RadioRegion {
    /// The United States under FCC 24-123 (2024): LTE-V2X and NR-V2X in 5.895-5.925 GHz,
    /// no DSRC.
    Us,
    /// The US DSRC band plan before FCC 20-164 (47 CFR §90.377, 2017): 802.11p, the BSM
    /// on channel 172.
    #[serde(rename = "us-2016")]
    Us2016,
    /// Europe under ETSI EN 302 571: 10 MHz channels in 5.855-5.925 GHz for ITS-G5,
    /// LTE-V2X and NR-V2X alike.
    Eu,
}

impl RadioRegion {
    /// The radio crate's region.
    #[must_use]
    pub const fn regulation(self) -> v2xw_radio::regulation::Region {
        match self {
            RadioRegion::Us => v2xw_radio::regulation::Region::Us,
            RadioRegion::Us2016 => v2xw_radio::regulation::Region::Us2016,
            RadioRegion::Eu => v2xw_radio::regulation::Region::Eu,
        }
    }
}

/// One entry of the run's own channel map (`radio.channels`, WP-13).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelAssignment {
    /// The IEEE channel number in the region's band plan (US 2016: 172-184; EU 172-184).
    pub channel: u16,
    /// The message sets that go out on it (`spat`, `map`, `wsa`, `srm`, `ssm`, `crl`,
    /// `mbr`, `denm`, `cpm`, ...). The safety broadcasts (`bsm`, `cam`, `psm`, `vam`)
    /// stay on `radio.channel`, which is the one to change to move them.
    #[serde(default)]
    pub messages: Vec<String>,
}

/// The radio hardware of each kind of node (04-models.md §3.7).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadioDevices {
    /// A vehicle's on-board unit.
    #[serde(default)]
    pub obu: ObuRadio,
    /// A roadside unit.
    #[serde(default)]
    pub rsu: RsuRadio,
    /// A pedestrian's or cyclist's device.
    #[serde(default)]
    pub vru: VruRadio,
}

/// A vehicle on-board unit's radio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObuRadio {
    /// Conducted transmit power at the antenna port, dBm; congestion control may lower it.
    #[serde(default = "ObuRadio::tx_power")]
    pub tx_power_dbm: f64,
    /// Antenna gain, dBi, transmitting and receiving.
    #[serde(default = "ObuRadio::gain")]
    pub antenna_gain_dbi: f64,
    /// Loss of the cable between radio and antenna, dB, transmitting and receiving.
    #[serde(default)]
    pub cable_loss_db: f64,
    /// Antenna height above the road, metres; unset, 1.5 m on a car and 3 m on a truck or bus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antenna_height_m: Option<f64>,
    /// The antenna's radiation pattern (`v2xw_radio::antenna`): TR 37.885's per vehicle
    /// type by default — a rooftop antenna on a car or van, front and rear panels on a
    /// truck or bus — every vehicle's on the roof, or none.
    #[serde(default, skip_serializing_if = "ObuAntennaPattern::is_default")]
    pub antenna_pattern: ObuAntennaPattern,
    /// A measured installed pattern (`antenna/sampled`, WP-09): when set it replaces
    /// `antenna_pattern` on every vehicle. Unset, the default, changes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampled_pattern: Option<SampledPattern>,
    /// How many 802.11p radios the unit carries (WP-13): 1, the default (SAE J2945/1's
    /// on-board unit, continuously on the safety channel), or 2, which adds one on the
    /// service channel of `radio.channels`. Acts only with `radio.channels`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radios: Option<u8>,
}

/// A measured antenna pattern every vehicle carries (`radio.devices.obu.sampled_pattern`,
/// `v2xw_radio::antenna::SampledAntenna`). Re-implemented from Eckhoff, Brummer and Sommer,
/// IEEE VNC 2016 §III (sampled azimuth gains, interpolated between samples), with measured
/// vehicle patterns such as Kornek et al., IEEE ICC Workshops 2010.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampledPattern {
    /// The table, as CSV text (a YAML block scalar): a header naming `gain_db` (dB
    /// relative to `antenna_gain_dbi`), `azimuth_deg` (counter-clockwise from the heading)
    /// or `azimuth_cw_deg` (clockwise), and optionally `elevation_deg`; then one sample a
    /// line. Interpolated linearly in dB. No default: the measurement is the scenario's.
    pub csv: String,
    /// The mount's rotation from the heading, degrees counter-clockwise; default 0
    /// (Veins `SampledAntenna1D` rotation, behaviour only).
    #[serde(default)]
    pub rotation_deg: f64,
    /// A per-vehicle random mount rotation on top: `none` (default), `uniform` in
    /// ±`rotation_jitter_deg`, or `normal` with σ = `rotation_jitter_deg` (Veins
    /// `SampledAntenna1D` random rotation, behaviour only). Keyed by seed and node.
    #[serde(default)]
    pub rotation_jitter: RotationJitter,
    /// The width of that rotation, degrees; default 0.
    #[serde(default)]
    pub rotation_jitter_deg: f64,
}

/// How a sampled antenna's mount rotation varies from vehicle to vehicle
/// (`radio.devices.obu.sampled_pattern.rotation_jitter`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RotationJitter {
    /// Every mount at `rotation_deg`.
    #[default]
    None,
    /// Uniform in ±`rotation_jitter_deg` per vehicle.
    Uniform,
    /// Normal with σ = `rotation_jitter_deg` per vehicle.
    Normal,
}

impl RotationJitter {
    /// The radio crate's value.
    #[must_use]
    pub fn to_radio(self) -> v2xw_radio::antenna::RotationJitter {
        match self {
            RotationJitter::None => v2xw_radio::antenna::RotationJitter::None,
            RotationJitter::Uniform => v2xw_radio::antenna::RotationJitter::Uniform,
            RotationJitter::Normal => v2xw_radio::antenna::RotationJitter::Normal,
        }
    }
}

/// A vehicle antenna's radiation pattern (`radio.devices.obu.antenna_pattern`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObuAntennaPattern {
    /// 3GPP TR 37.885 Option 1 by vehicle type: rooftop (Type 2) on a car, van or
    /// motorcycle, front and rear panels (Type 3) on a truck or bus.
    #[default]
    Tr37885,
    /// A rooftop antenna on every vehicle, trucks included.
    Rooftop,
    /// No pattern: the scalar gain in every direction.
    Isotropic,
}

impl ObuAntennaPattern {
    fn is_default(&self) -> bool {
        *self == Self::Tr37885
    }
}

impl ObuRadio {
    fn tx_power() -> f64 {
        23.0
    }
    fn gain() -> f64 {
        3.0
    }
}

impl Default for ObuRadio {
    fn default() -> Self {
        Self {
            tx_power_dbm: Self::tx_power(),
            antenna_gain_dbi: Self::gain(),
            cable_loss_db: 0.0,
            antenna_height_m: None,
            antenna_pattern: ObuAntennaPattern::Tr37885,
            sampled_pattern: None,
            radios: None,
        }
    }
}

/// A roadside unit's radio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RsuRadio {
    /// Conducted transmit power at the antenna port, dBm.
    #[serde(default = "RsuRadio::tx_power")]
    pub tx_power_dbm: f64,
    /// Antenna gain, dBi, transmitting and receiving.
    #[serde(default = "RsuRadio::gain")]
    pub antenna_gain_dbi: f64,
    /// Loss of the cable between radio and antenna, dB, transmitting and receiving.
    #[serde(default)]
    pub cable_loss_db: f64,
    /// Antenna height above the ground, metres; unset, the site's own mast height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antenna_height_m: Option<f64>,
    /// How many 802.11p radios the unit carries (WP-13): 2, the default, one on the
    /// safety channel and one on the service channel of `radio.channels` (the USDOT DSRC
    /// Roadside Unit Specification v4.1 calls for two radios), or 1. Acts only with
    /// `radio.channels`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radios: Option<u8>,
    /// A unit that leaves the safety channel early in some IEEE 1609.4 sync intervals
    /// (802.11p only): it hears nothing there while away. Unset, the default, keeps it on
    /// the channel all the time. Fitted on Tampa's RSU log (`tools/realdata`, P1-E5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_channel: Option<RsuOffChannel>,
}

/// When a roadside unit is away from the safety channel (`radio.devices.rsu.off_channel`).
///
/// IEEE 1609.4-2016 §6.2.5 divides UTC time into 100 ms sync intervals whose first 50 ms
/// are the control-channel interval. Tampa's RSU-received BSMs (CV Pilot THEA, msgCnt-
/// attested losses) lose 50-57 % of free devices' BSMs generated 10-30 ms into the UTC
/// 100 ms interval against 18-24 % in its second half, at every location, with no locked
/// device near: a receiver-side deafness locked to the sync interval's first half. The
/// unit leaves, independently in each interval with `probability`, from `from_ms` after
/// the interval's start for a duration uniform in `[min_duration_ms, max_duration_ms]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RsuOffChannel {
    /// The share of sync intervals the unit leaves in, 0 to 1.
    pub probability: f64,
    /// When it leaves, after the start of the UTC 100 ms sync interval, ms.
    pub from_ms: f64,
    /// The shortest time away, ms.
    pub min_duration_ms: f64,
    /// The longest time away, ms.
    pub max_duration_ms: f64,
}

impl RsuRadio {
    fn tx_power() -> f64 {
        23.0
    }
    fn gain() -> f64 {
        3.0
    }
}

impl Default for RsuRadio {
    fn default() -> Self {
        Self {
            tx_power_dbm: Self::tx_power(),
            antenna_gain_dbi: Self::gain(),
            cable_loss_db: 0.0,
            antenna_height_m: None,
            radios: None,
            off_channel: None,
        }
    }
}

/// A vulnerable road user's device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VruRadio {
    /// Conducted transmit power at the antenna port, dBm.
    #[serde(default = "VruRadio::tx_power")]
    pub tx_power_dbm: f64,
    /// Antenna gain, dBi, transmitting and receiving.
    #[serde(default)]
    pub antenna_gain_dbi: f64,
    /// Loss of the cable between radio and antenna, dB, transmitting and receiving.
    #[serde(default)]
    pub cable_loss_db: f64,
    /// Antenna height above the ground, metres.
    #[serde(default = "VruRadio::height")]
    pub antenna_height_m: f64,
    /// How many 802.11p radios the device carries (WP-13): 1, the default (SAE J2945/9's
    /// personal device on the safety channel), or 2. Acts only with `radio.channels`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radios: Option<u8>,
}

impl VruRadio {
    fn tx_power() -> f64 {
        23.0
    }
    fn height() -> f64 {
        1.5
    }
}

impl Default for VruRadio {
    fn default() -> Self {
        Self {
            tx_power_dbm: Self::tx_power(),
            antenna_gain_dbi: 0.0,
            cable_loss_db: 0.0,
            antenna_height_m: Self::height(),
            radios: None,
        }
    }
}

/// How far a transmission is followed (04-models.md §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRange {
    /// How far below the receiver's noise floor a line-of-sight arrival may fall before
    /// the transmission is no longer followed there, dB.
    #[serde(default = "CandidateRange::margin")]
    pub margin_db: f64,
    /// A cap on the fully evaluated range, metres; beyond it only line-of-sight
    /// interference is counted. Unset, the link budget alone decides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_m: Option<f64>,
}

impl CandidateRange {
    fn margin() -> f64 {
        10.0
    }
}

impl Default for CandidateRange {
    fn default() -> Self {
        Self {
            margin_db: Self::margin(),
            max_m: None,
        }
    }
}

/// Radio access technology (03-interfaces.md §13).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Rat {
    /// IEEE 802.11p / DSRC, the Phase 1 stack.
    #[default]
    #[serde(rename = "dsrc-80211p")]
    Dsrc80211p,
    /// LTE-V2X PC5 sidelink.
    LteV2xPc5,
    /// NR-V2X PC5 sidelink.
    NrV2xPc5,
    /// More than one of the above at once.
    Hybrid,
}

/// The fidelity tier of each radio family (02-architecture.md §7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadioTiers {
    /// Path loss.
    #[serde(default = "RadioTiers::high")]
    pub propagation: Tier,
    /// The physical layer.
    #[serde(default = "RadioTiers::medium")]
    pub phy: Tier,
    /// Medium access.
    #[serde(default = "RadioTiers::medium")]
    pub mac: Tier,
    /// A region run at a higher tier than the rest of the world (§7.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<Focus>,
}

impl RadioTiers {
    fn medium() -> Tier {
        Tier::Medium
    }
    /// Propagation's default: the geometric city-street law, the one best supported by
    /// measurement on a street grid (04-models.md §3.3).
    fn high() -> Tier {
        Tier::High
    }
}

impl Default for RadioTiers {
    fn default() -> Self {
        RadioTiers {
            propagation: Tier::High,
            phy: Tier::Medium,
            mac: Tier::Medium,
            focus: None,
        }
    }
}

/// A focus region and the tier inside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Focus {
    /// The region: `follow:<node id>` or a geodetic box.
    pub region: FocusRegion,
    /// The tier inside it.
    pub tier: Tier,
}

/// Where the focus region is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum FocusRegion {
    /// Centred on a node, moving with it.
    Follow {
        /// Which node.
        node: u32,
        /// The radius around it, metres.
        radius_m: f64,
    },
    /// A fixed geodetic box.
    Bbox {
        /// The box.
        bbox: GeoBbox,
    },
}

/// A model id with its parameter overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelChoice {
    /// The model's registry id.
    pub id: String,
    /// Overrides for its card's defaults. Anything not named here keeps the card's value,
    /// which is what makes the parameter set content-addressable (02-architecture.md §6.5).
    #[serde(default)]
    pub params: serde_json::Value,
}

impl ModelChoice {
    /// A choice with no overrides.
    pub fn new(id: impl Into<String>) -> Self {
        ModelChoice {
            id: id.into(),
            params: serde_json::Value::Null,
        }
    }
}

/// The network stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Net {
    /// Which network layer: `wsmp` or `gn-btp`.
    #[serde(default = "Net::default_layer")]
    pub layer: String,
    /// The fragmenter's model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragmenter: Option<ModelChoice>,
    /// The backhaul link model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backhaul: Option<ModelChoice>,
    /// The cellular uplink model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uu: Option<ModelChoice>,
    /// The backend network model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_net: Option<ModelChoice>,
    /// Multi-hop GeoNetworking for hazard warnings (`gn-btp` only): when a hazard fires,
    /// its DENM is GeoBroadcast and forwarded hop by hop through the equipped vehicles.
    /// Unset, a DENM travels one hop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geonet: Option<Geonet>,
    /// IEEE 1609.4 channel access on `wsmp`: `continuous` (one channel all the time, the
    /// deployed SAE J2945/1 profile; the default) or `alternating` (50 ms on the control
    /// channel, 50 ms on a service channel, a 4 ms guard at each switch: a safety message
    /// waits for the next control-channel interval).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_access: Option<String>,
    /// Which EDCA queue each message waits in: `single` (every frame on AC_VO, the
    /// deployed SAE J2945/1 profile; the default) or `etsi-profiles` (the ETSI DCC
    /// profiles of TS 102 724: hazard DENMs on AC_VO, CAMs on AC_BE, forwarded
    /// GeoNetworking copies and everything else on AC_BK, so a busy channel serves warnings
    /// first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_categories: Option<String>,
    /// Which channel busy ratio the congestion control acts on (`gn-btp` only): `local`
    /// (the station's own measurement; the default) or `global` (the highest of its own,
    /// its neighbours' and their neighbours', carried in every GeoNetworking single-hop
    /// header, ETSI TS 102 636-4-2), so a station also backs off for congestion it cannot
    /// hear itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dcc_cbr: Option<String>,
    /// SCMS traffic through roadside units over the air (WP-19): how the hop between a
    /// relayed vehicle and its unit is modelled. Unset, the constant air link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rsu_ip: Option<RsuIp>,
}

/// The IP service a roadside unit offers vehicles with no modem ([`Net::rsu_ip`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RsuIp {
    /// `constant` (the air hop is a fixed latency and rate, and a vehicle uses any relaying
    /// unit within range; the default) or `sch` (IEEE 1609.3: each relaying unit sends a
    /// signed WSA advertising the SCMS service with a WAVE Routing Advertisement, and a
    /// vehicle uses a unit only after it has verified that unit's WSA).
    #[serde(default = "RsuIp::default_air")]
    pub air: String,
    /// The WSA period under `sch`, ms: 100 (USDOT RSU Specification v4.1 App. D,
    /// `RepeatRate` 50 per 5 s).
    #[serde(default = "RsuIp::default_wsa_interval_ms")]
    pub wsa_interval_ms: u32,
}

impl RsuIp {
    fn default_air() -> String {
        "constant".to_string()
    }
    const fn default_wsa_interval_ms() -> u32 {
        100
    }
}

/// Multi-hop GeoNetworking forwarding of hazard warnings ([`Net::geonet`]), EN 302 636-4-1
/// with the DEN basic service of EN 302 637-3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geonet {
    /// How forwarded copies travel: `on-air` (each forward is a real frame that queues in
    /// the forwarder's MAC, contends, collides, counts in the channel busy ratio and passes
    /// the DCC gate; the default) or `overlay` (a protocol-only run over a disk of
    /// `range_m` at the hazard's instant, with no MAC: optimistic, but instant to compute).
    #[serde(default = "Geonet::default_forwarding")]
    pub forwarding: String,
    /// How a warning spreads inside its area: `cbf` (contention-based: the farthest
    /// receiver rebroadcasts first, the rest stand down) or `simple` (every receiver
    /// rebroadcasts once).
    #[serde(default = "Geonet::default_area_forwarding")]
    pub area_forwarding: String,
    /// How a packet travels towards an area it is not yet in: `greedy` (to the neighbour
    /// nearest the area) or `cbf`.
    #[serde(default = "Geonet::default_non_area_forwarding")]
    pub non_area_forwarding: String,
    /// What a contending vehicle does on hearing a copy: `etsi` (stand down, and take a
    /// later copy as new: many transmissions), `dpd` (stand down, never forward twice) or
    /// `gpc` (stand down only if the copy came from farther along).
    #[serde(default = "Geonet::default_cbf_variant")]
    pub cbf_variant: String,
    /// The most hops a warning may take; 10 by default (`itsGnDefaultHopLimit`).
    #[serde(default = "Geonet::default_hop_limit")]
    pub hop_limit: u32,
    /// The warning area's radius around the hazard, metres; 1 000 m by default.
    #[serde(default = "Geonet::default_area_radius_m")]
    pub area_radius_m: f64,
    /// The single-hop range of the `overlay` forwarding, metres; 500 m by default. On the
    /// air the run's own link budget decides who hears a copy.
    #[serde(default = "Geonet::default_range_m")]
    pub range_m: f64,
    /// How often the originator repeats the warning, ms; 0 sends it once.
    #[serde(default = "Geonet::default_repetition_interval_ms")]
    pub repetition_interval_ms: u64,
    /// How long it keeps repeating, ms. Unset, the overlay repeats for 5 000 ms and the
    /// on-air DEN service for 15 000 ms (the C2C-CC stationary-vehicle profile); any value
    /// given, 5 000 included, is used as given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetition_duration_ms: Option<u64>,
    /// Keep-alive forwarding: vehicles in the area re-send a warning they stop hearing,
    /// so it reaches vehicles that arrive later.
    #[serde(default)]
    pub keep_alive_forwarding: bool,
    /// How long each warning is followed, seconds.
    #[serde(default = "Geonet::default_window_s")]
    pub window_s: f64,
    // network audit
    /// `itsGnGeoBroadcastCbfMinTime`, ms: the CBF timer of the farthest contender
    /// (EN 302 636-4-1 Annex H, 1 ms unless set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cbf_min_time_ms: Option<u64>,
    /// `itsGnGeoBroadcastCbfMaxTime`, ms: the CBF timer of the nearest contender
    /// (Annex H, 100 ms unless set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cbf_max_time_ms: Option<u64>,
    /// `itsGnDefaultMaxCommunicationRange` (`DIST_MAX`), m: the progress at which the CBF
    /// timer reaches its minimum (Annex H, 1 000 m unless set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_comm_range_m: Option<f64>,
}

impl Geonet {
    fn default_forwarding() -> String {
        "on-air".to_string()
    }
    fn default_area_forwarding() -> String {
        "cbf".to_string()
    }
    fn default_non_area_forwarding() -> String {
        "greedy".to_string()
    }
    fn default_cbf_variant() -> String {
        "gpc".to_string()
    }
    fn default_hop_limit() -> u32 {
        10
    }
    fn default_area_radius_m() -> f64 {
        1_000.0
    }
    fn default_range_m() -> f64 {
        500.0
    }
    fn default_repetition_interval_ms() -> u64 {
        1_000
    }
    /// The overlay's repetition duration when `repetition_duration_ms` is unset, ms.
    pub const OVERLAY_REPETITION_DURATION_MS: u64 = 5_000;
    fn default_window_s() -> f64 {
        10.0
    }
}

impl Default for Geonet {
    fn default() -> Self {
        Geonet {
            forwarding: Geonet::default_forwarding(),
            area_forwarding: Geonet::default_area_forwarding(),
            non_area_forwarding: Geonet::default_non_area_forwarding(),
            cbf_variant: Geonet::default_cbf_variant(),
            hop_limit: Geonet::default_hop_limit(),
            area_radius_m: Geonet::default_area_radius_m(),
            range_m: Geonet::default_range_m(),
            repetition_interval_ms: Geonet::default_repetition_interval_ms(),
            repetition_duration_ms: None,
            keep_alive_forwarding: false,
            window_s: Geonet::default_window_s(),
            cbf_min_time_ms: None,
            cbf_max_time_ms: None,
            max_comm_range_m: None,
        }
    }
}

impl Net {
    fn default_layer() -> String {
        "wsmp".to_string()
    }
}

impl Default for Net {
    fn default() -> Self {
        Net {
            layer: Net::default_layer(),
            fragmenter: None,
            backhaul: None,
            uu: None,
            backend_net: None,
            geonet: None,
            channel_access: None,
            access_categories: None,
            dcc_cbr: None,
            rsu_ip: None,
        }
    }
}

/// Which messages are generated and how.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Messages {
    /// Which message sets: `bsm`, `cam`, `denm`, `spat`, `map`, `psm`, `vam`, `cpm`.
    #[serde(default = "Messages::default_sets")]
    pub sets: Vec<String>,
    /// The generator model and its parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<ModelChoice>,
    /// Which codec tier: real `uper` bytes or the validated `size-model` (D2).
    #[serde(default = "Messages::default_codec_tier")]
    pub codec_tier: String,
    /// A fixed awareness-message rate, messages per second: every BSM and every CAM goes
    /// out on this period instead of J2945/1's 10 Hz and the ETSI CAM triggers (heading
    /// 4°, position 4 m, speed 0.5 m/s, EN 302 637-2 §6.1.3), the way ns-3's
    /// `BsmApplication` and Veins' `beaconInterval` send. Congestion control may still
    /// stretch it. Unset keeps the standards' generation rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_hz: Option<f64>,
    // network audit
    /// The SPaT broadcast interval of a roadside unit running `spat`, ms: CTI 4501's
    /// 10 Hz (100 ms) unless set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spat_interval_ms: Option<u64>,
    /// The MAP broadcast interval of a roadside unit running `map`, ms: CTI 4501's 1 Hz
    /// (1 000 ms) unless set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_interval_ms: Option<u64>,
    /// `T_GenCpm`, ms: how often a vehicle running `cpm` checks whether a CPM is due (ETSI
    /// TS 103 324 §6.1.2.3, 100 ms unless set; at most `T_GenCpmMax`, 1 000 ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpm_interval_ms: Option<u64>,
    /// Whether `T_GenCpm` follows the congestion control: the DCC's minimum gap between
    /// transmissions, clamped to `[T_GenCpmMin, T_GenCpmMax]` = [100, 1 000] ms (ETSI TS
    /// 103 324 V2.1.1 §6.1.2.1, Annex D; WP-12). Off unless set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpm_dcc_limited: Option<bool>,
    /// `ObjectPerceptionQualityThreshold`, 0 to 15: a CPM carries only objects whose
    /// §7.1.8.6 quality exceeds it, and states that quality (TS 103 324 §6.1.2.3, Table
    /// F.1 recommends 3; WP-12). Not applied unless set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpm_quality_threshold: Option<u8>,
    /// The most perceived objects one CPM carries, 1 to 255: the selection is ordered by
    /// the §6.1.3.2 object utility and the lowest-utility objects beyond it wait for the
    /// next event (WP-12). The ASN.1 bound 255, in perception order, unless set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpm_max_objects: Option<u32>,
    /// Whether a vehicle running `denm` raises the C2C-CC traffic-condition DENMs (sudden
    /// speed drop / dangerous end of queue, local slow down / traffic jam ahead; RS 2007
    /// R1.6.10; WP-12). Off unless set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denm_traffic_condition: Option<bool>,
    /// Whether a vehicle running `denm` raises the C2C-CC traction-loss DENM (adverse
    /// weather, RS 2002 R1.6.10 §3.3; WP-12). Off unless set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denm_traction_loss: Option<bool>,
    /// The most path-history points a BSM's Part II (and a CAM's low-frequency container)
    /// carries: J2945/1's 15 unless set, at most J2735's 23; 0 sends none. The points are
    /// the Part II's bulk, about 8 to 9 bytes each.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_history_points: Option<u32>,
}

impl Messages {
    fn default_sets() -> Vec<String> {
        vec!["bsm".to_string()]
    }
    fn default_codec_tier() -> String {
        "uper".to_string()
    }
}

impl Default for Messages {
    fn default() -> Self {
        Messages {
            sets: Messages::default_sets(),
            generator: None,
            codec_tier: Messages::default_codec_tier(),
            rate_hz: None,
            spat_interval_ms: None,
            map_interval_ms: None,
            cpm_interval_ms: None,
            cpm_dcc_limited: None,
            cpm_quality_threshold: None,
            cpm_max_objects: None,
            denm_traffic_condition: None,
            denm_traction_loss: None,
            path_history_points: None,
        }
    }
}

/// The security stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Security {
    /// The envelope profile: `ieee1609.2` or `etsi103097`.
    #[serde(default = "Security::default_envelope")]
    pub envelope: String,
    /// The credential-management protocol and its parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<ModelChoice>,
    /// The signature scheme, e.g. `ecdsa-p256`, `hybrid-mldsa44-ecdsa-p256`, `ml-dsa-65`
    /// — any id in `v2xw_sec::scheme::SIGNATURE_IDS`.
    #[serde(default = "Security::default_signature")]
    pub signature: String,
    /// The key-encapsulation scheme for encryption to the PCA and to devices, e.g.
    /// `ecies-p256` or `ml-kem-768` — any id in `v2xw_sec::scheme::KEM_IDS`.
    #[serde(default = "Security::default_kem")]
    pub kem: String,
    /// Committees that replace single authorities of the credential system
    /// (`v2xw_proto::threshold`). Absent: one server per role, two-of-two linkage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<ThresholdSpec>,
    /// `modeled` costs the operations without doing the mathematics; `real` does the
    /// mathematics. Both are recorded in the manifest.
    #[serde(default)]
    pub crypto_mode: CryptoModeSpec,
    /// `verify-all`, `on-demand` or `prioritized`.
    #[serde(default = "Security::default_verification_policy")]
    pub verification_policy: String,
    /// How often a full certificate is attached instead of a digest.
    #[serde(default)]
    pub signer_id_policy: SignerIdPolicySpec,
    /// When a node changes pseudonym.
    #[serde(default)]
    pub pseudonym_change: PseudonymChangeSpec,
    /// The relevance score at or above which an `on-demand` node verifies a message from
    /// a signer it already knows. 0.5 unless set; refused under any other policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_demand_threshold: Option<f64>,
    /// The distance beyond which proximity adds nothing to a `prioritized` node's service
    /// priority, metres. 300 unless set; refused under any other policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prioritized_horizon_m: Option<f64>,
    /// The SCMS deployment the credential lifecycle runs on (`protocol/scms/camp` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scms: Option<ScmsSpec>,
}

/// `security.scms`: the SCMS deployment (WP-D1, 00-gap-analysis §2).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScmsSpec {
    /// Which providers run, what each runs and where. Absent: `us-production-2025`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<DeploymentSpec>,
    /// What each device's local certificate chain file holds (WP-C2, 1609.2.1 §6.3.5):
    /// `all-providers` (default: every CTL provider's CA certificates, the global chain
    /// file) or `home-provider` (only the CA certificates of the provider that certifies
    /// the device, under every CTL root; ASSUMPTION, a device whose chain file predates
    /// the other providers). Under `home-provider` a device learns another provider's
    /// ICA and ACA over the air by P2PCD (IEEE 1609.2 clause 8) before it can verify that
    /// provider's certificates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_file: Option<String>,
    /// Publish the PKI view (WP-E1): `pki.state`, `pki.flow` with every SPDU's bytes and
    /// its recipient-opened plaintext, `pki.case`, `pki.knowledge` and
    /// `node.credentials`. Absent: on. `false` keeps a long run's recording lean; the
    /// Backend view's `backend.state` is published either way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pki_view: Option<bool>,
    /// Record every misbehaviour report's fate (`pki.report_fate`: stored, evicted by the
    /// quota, decayed out, expired, uploaded, delivered, held at the end), what each MA
    /// received (`ma.received`, the fields the offline MA-rule replay reads) and each
    /// device's exposure (`gt.scms_device`). Absent: off, so existing recordings keep
    /// their digests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_records: Option<bool>,
    /// The credential lifecycle's timing and capacity knobs (certificates per week, the
    /// RA shuffle, downloads, enrolment, CRL cadence and distribution, the MA's review),
    /// each a field of its own. Every field left out keeps the cited default, and a field
    /// set here wins over the same key in `security.protocol.params`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<LifecycleSpec>,
    /// The TLS version of every device-to-RA session (`session-scmsAuth`, SCMS Manager EE
    /// profile §2.1.18: TLS 1.2 or 1.3): `1.3` (default) or `1.2`, whose handshake costs
    /// one more round trip before the first request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<String>,
    /// `overdue_CRL_tolerance` (CAMP SCMS 1.2.2 Table 67), seconds: how long past a CRL's
    /// `nextCrl` a device keeps trusting the certificates that CRL governs before it
    /// refuses every message they sign. Two weeks (1,209,600 s) unless set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crl_overdue_tolerance_s: Option<u32>,
    /// Which messages an overdue pseudonym CRL makes a device refuse
    /// (`v2xw_node::rxval::CrlOverdueScope`): `craca` (default), the signers whose CRACA is
    /// the overdue list's; `all`, every message while any held series-1 list is past
    /// `nextCrl` + the tolerance. CAMP SCMS 1.2.2 Table 67 has the device "rejecting all of
    /// them", which the sources leave to either reading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crl_overdue_scope: Option<String>,
    /// The receiver's `generationTime` relevance window (IEEE 1609.2 §5.2,
    /// `v2xw_node::rxval::GenerationTimeWindow`): the oldest a received BSM, CAM, PSM or
    /// VAM may be on the receiver's own clock, seconds (2 unless set, C2C-CC BSP
    /// RS_BSP_168). Older ones are discarded unverified and never reach a detector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rx_beacon_max_age_s: Option<f64>,
    /// The same for every other application message (SPaT, MAP, DENM, CPM, SRM, SSM),
    /// seconds (600 unless set, RS_BSP_168).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rx_max_age_s: Option<f64>,
    /// How far ahead of the receiver's own clock a `generationTime` may be, seconds
    /// (0.04 unless set, RS_BSP_168).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rx_max_future_s: Option<f64>,
}

/// `security.scms.lifecycle`: the credential lifecycle's knobs as typed, published fields.
///
/// The same model reads them as `security.protocol.params` (`phase2::LIFECYCLE_KEYS`); this
/// block is what the settings window shows. Defaults are the US deployment's, cited on each
/// field's guide row.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleSpec {
    /// The i-period, s: how long one set of pseudonym certificates is current (one week,
    /// 604,800 s, CAMP-EE §2.1.5.3.2). Anything shorter needs `time.calendar: compressed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i_period_s: Option<f64>,
    /// A pseudonym certificate's lifetime, s: the i-period plus the overlap (10,140 min,
    /// one hour longer than the week, CAMP-EE §2.1.5.3.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_lifetime_s: Option<f64>,
    /// Pseudonym certificates per i-period, all valid at once (20, CAMP-EE Table
    /// 2.1.2.6.2; at most 60).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certs_per_period: Option<u32>,
    /// i-periods a vehicle holds when the run starts (2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_periods: Option<u32>,
    /// A vehicle asks for its next period when fewer than this many periods, the current
    /// one counted, remain (1; 0 turns top-ups off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topup_below_periods: Option<u32>,
    /// The most i-periods ahead the RA provisions a device (156, three years, CAMP Primer
    /// p.7: 3,120 = 156 × 20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_periods_ahead: Option<u32>,
    /// Certificate requests that release the RA's shuffle before its window closes
    /// (1,000, CAMP 1.2.2 Table 67 `shuffle_threshold`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_shuffle_threshold: Option<u32>,
    /// The longest a certificate request waits in the RA's shuffle, s (2 days, CAMP 1.2.2
    /// Table 67 `max_cert_request_age`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_shuffle_window_s: Option<f64>,
    /// Misbehaviour reports that release the RA's report shuffle (10,000, CAMP-EE
    /// SCMS-765).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_shuffle_threshold: Option<u32>,
    /// The longest a misbehaviour report waits in the RA's shuffle, s (one day, CAMP-EE
    /// SCMS-765).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_shuffle_window_s: Option<f64>,
    /// How long after the RA's acknowledgement a device first polls for its batch, s (60).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_batch_delay_s: Option<f64>,
    /// `download-minWait`, s: the wait between polls of a batch that is not ready (300,
    /// SCMS Manager EE profile Table 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_poll_interval_s: Option<f64>,
    /// `download-maxReqs`: how many polls a device makes before it gives up on a batch
    /// (100, SCMS Manager EE profile Table 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_download_polls: Option<u32>,
    /// How far a download request's generation time may be from the RA's clock, s (5,
    /// CAMP 1.2.2 SCMS-1203 `ra.client-signature-ttl-sec`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_request_tolerance_s: Option<f64>,
    /// The enrolment certificate's lifetime, s (six years, CAMP 1.2.2 Table 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrolment_lifetime_s: Option<f64>,
    /// How long before the enrolment certificate expires the device re-enrols, s (548
    /// days, SCMS Manager re-enrolment standard: 12 to 24 months).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reenrol_lead_s: Option<f64>,
    /// The wait between a re-enrolment request and its download, s (300, the profile's
    /// `download-minWait`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reenrol_download_delay_s: Option<f64>,
    /// A fixed CRL publication cadence, s. Absent: each provider's own rule (the US
    /// presets publish within one business day of a decision, SCMS Manager Provider
    /// Requirements §3.4.9). 0 publishes on decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crl_cadence_s: Option<f64>,
    /// How often a cellular vehicle polls for a new CRL, s (3,600).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crl_fetch_interval_s: Option<f64>,
    /// How often a roadside unit with the `crl` role repeats the CRL over the air, s (5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crl_broadcast_interval_s: Option<f64>,
    /// The Misbehaviour Authority operator's review before a revocation is carried out,
    /// s. 0 is the automatic MA. Absent: 8 h on the real calendar (the pilots' manual
    /// review), automatic on a compressed one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ma_review_s: Option<f64>,
    /// The most investigations the Misbehaviour Authority keeps open at once (1,000, an
    /// assumption: no MA capacity is published); a case beyond it waits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ma_max_open_cases: Option<u32>,
    /// The device's misbehaviour-report storage quota, bytes (65,536, an ASSUMPTION:
    /// SCMS Manager MBR v1.0 §2.2 makes it device-specific). Over it the lowest-weight
    /// report is deleted; 0 keeps no report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_store_quota_bytes: Option<u32>,
    /// The daily weight multiplier of every stored report (0.9, MBR v1.0 §2.3.1
    /// `DailyMultiplier`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_daily_multiplier: Option<f64>,
    /// The Eastern hour the multiplier applies at (5, MBR v1.0 §2.2: "every day, at 5 am
    /// Eastern time").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_decay_hour_et: Option<u32>,
    /// `initialWeight` of a RandomPosition report (1.0, MBR v1.0 §2.3.1.2.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_weight_random_position: Option<f64>,
    /// `initialWeight` of a ConstantPosition report (1.0, MBR v1.0 §2.3.1.2.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_weight_constant_position: Option<f64>,
    /// `initialWeight` of a MaxSpeed report (1.0, MBR v1.0 §2.3.1.3.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_weight_max_speed: Option<f64>,
    /// `initialWeight` of a MaxAcceleration report (1.0, MBR v1.0 §2.3.1.3.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_weight_max_acceleration: Option<f64>,
    /// `initialWeight` of a BrakeAcceleration report (1.0, MBR v1.0 §2.3.1.3.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_weight_brake_acceleration: Option<f64>,
}

impl LifecycleSpec {
    /// Every field that is set, as `(key, value)` in `security.protocol.params` spelling.
    #[must_use]
    pub fn entries(&self) -> Vec<(&'static str, f64)> {
        let u = |v: Option<u32>| v.map(f64::from);
        [
            ("i_period_s", self.i_period_s),
            ("cert_lifetime_s", self.cert_lifetime_s),
            ("certs_per_period", u(self.certs_per_period)),
            ("pool_periods", u(self.pool_periods)),
            ("topup_below_periods", u(self.topup_below_periods)),
            ("max_periods_ahead", u(self.max_periods_ahead)),
            ("cert_shuffle_threshold", u(self.cert_shuffle_threshold)),
            ("cert_shuffle_window_s", self.cert_shuffle_window_s),
            ("report_shuffle_threshold", u(self.report_shuffle_threshold)),
            ("report_shuffle_window_s", self.report_shuffle_window_s),
            ("first_batch_delay_s", self.first_batch_delay_s),
            ("download_poll_interval_s", self.download_poll_interval_s),
            ("max_download_polls", u(self.max_download_polls)),
            (
                "download_request_tolerance_s",
                self.download_request_tolerance_s,
            ),
            ("enrolment_lifetime_s", self.enrolment_lifetime_s),
            ("reenrol_lead_s", self.reenrol_lead_s),
            ("reenrol_download_delay_s", self.reenrol_download_delay_s),
            ("crl_cadence_s", self.crl_cadence_s),
            ("crl_fetch_interval_s", self.crl_fetch_interval_s),
            ("crl_broadcast_interval_s", self.crl_broadcast_interval_s),
            ("ma_review_s", self.ma_review_s),
            ("ma_max_open_cases", u(self.ma_max_open_cases)),
            ("report_store_quota_bytes", u(self.report_store_quota_bytes)),
            ("report_daily_multiplier", self.report_daily_multiplier),
            ("report_decay_hour_et", u(self.report_decay_hour_et)),
            (
                "report_weight_random_position",
                self.report_weight_random_position,
            ),
            (
                "report_weight_constant_position",
                self.report_weight_constant_position,
            ),
            ("report_weight_max_speed", self.report_weight_max_speed),
            (
                "report_weight_max_acceleration",
                self.report_weight_max_acceleration,
            ),
            (
                "report_weight_brake_acceleration",
                self.report_weight_brake_acceleration,
            ),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect()
    }
}

/// `security.scms.deployment`: the providers on the CTL and what each runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentSpec {
    /// `us-production-2025` (two providers on one CTL, the US system), `us-test-ctl-2025`
    /// (the 2025-07-17 test CTL: six roots, five approved), `camp-reference` (one SCMS with
    /// LOP and a central MA, the earlier
    /// topology) or `custom` (the providers listed below).
    #[serde(default = "DeploymentSpec::default_preset")]
    pub preset: String,
    /// Per-provider changes, matched by name to the preset's providers; under `custom`,
    /// the providers themselves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<ProviderSpec>,
    /// Elector count (5). Any other value marks the run as not the US system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub electors: Option<u32>,
    /// Elector signatures a trust list needs (3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quorum: Option<u32>,
    /// Elements specified but not deployed in the US.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<DeploymentOptionsSpec>,
    /// `region`: latency from great-circle distance at fibre speed plus a per-link
    /// overhead (ASSUMPTION); `flat`: the fixed backend link latency on every link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency: Option<String>,
    /// Signal speed in fibre as a fraction of c, under `region` (2/3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fibre_fraction: Option<f64>,
    /// Added to every link under `region`, ms (1, ASSUMPTION).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_link_ms: Option<f64>,
    /// Whether each MA forwards a report about another provider's certificate to that
    /// provider's MA (on).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ma_report_sharing: Option<bool>,
}

impl DeploymentSpec {
    fn default_preset() -> String {
        "us-production-2025".to_string()
    }
}

/// `security.scms.deployment.providers[]`: one provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSpec {
    /// Its neutral label, e.g. `provider-b`.
    pub name: String,
    /// Instances per role (`root`, `ica`, `eca`, `ra`, `aca`, `la1`, `la2`, `ma`, `crlg`,
    /// `pg`); a role left out keeps the preset's count.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<String, u32>,
    /// Its cloud region (ASSUMPTION: no provider publishes one): `us-east`, `us-east-2`,
    /// `us-central`, `us-south`, `us-west`, `us-west-1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// The share of roadside units it certifies (ASSUMPTION: no market share is
    /// published); the shares sum to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fleet_share: Option<f64>,
    /// Whether the electors have approved its root onto the CTL (on). Off models a
    /// provider whose root is not trusted: a test-CTL root on the production system, or
    /// a rogue SCMS; nothing it certifies verifies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_ctl: Option<bool>,
    /// Its MA's decision mode: `manual-review` (the CV-pilot operator review, the
    /// default) or `automatic`. Overrides `security.protocol.params.ma_review_s` for
    /// this provider only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ma_mode: Option<String>,
    /// Under `manual-review`, the operator's review time, seconds (ASSUMPTION: 8 h, one
    /// working shift).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ma_review_s: Option<f64>,
    /// When its CRL Generator publishes after a decision: `sla-1-business-day` (the
    /// SCMS Manager SLA, the US default), `cadence` (the run's `crl_cadence_s`) or
    /// `on-decision`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crl_publication: Option<String>,
}

/// `security.scms.deployment.options`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentOptionsSpec {
    /// The Location Obscurer Proxy (off: not in 1609.2.1 or the US profile). Off makes
    /// the proxy hop transparent: no latency, no queue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lop: Option<bool>,
    /// `original` only: the unified and compact unified butterflies are refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub butterfly: Option<String>,
}

impl Security {
    fn default_envelope() -> String {
        "ieee1609.2".to_string()
    }
    fn default_signature() -> String {
        "ecdsa-p256".to_string()
    }
    fn default_kem() -> String {
        "ecies-p256".to_string()
    }
    fn default_verification_policy() -> String {
        "verify-all".to_string()
    }
}

impl Default for Security {
    fn default() -> Self {
        Security {
            envelope: Security::default_envelope(),
            protocol: None,
            signature: Security::default_signature(),
            kem: Security::default_kem(),
            threshold: None,
            crypto_mode: CryptoModeSpec::default(),
            verification_policy: Security::default_verification_policy(),
            signer_id_policy: SignerIdPolicySpec::default(),
            pseudonym_change: PseudonymChangeSpec::default(),
            on_demand_threshold: None,
            prioritized_horizon_m: None,
            scms: None,
        }
    }
}

/// `security.threshold`: which credential-system roles a t-of-n committee plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdSpec {
    /// The signing protocol for the PCA and the CRL Generator: an id in
    /// `v2xw_proto::threshold::THRESHOLD_IDS` (`frost-p256`, `cggmp21-ecdsa`, `umbrella`).
    #[serde(default = "ThresholdSpec::default_protocol")]
    pub protocol: String,
    /// Committee size.
    #[serde(default = "ThresholdSpec::default_members")]
    pub members: u16,
    /// How many members must act: a `quorum`-of-`members` committee.
    #[serde(default = "ThresholdSpec::default_quorum")]
    pub quorum: u16,
    /// The roles a committee plays: any of `pca`, `crl`, `linkage`.
    #[serde(default = "ThresholdSpec::default_roles")]
    pub roles: Vec<String>,
    /// The linkage committee's size and quorum, when `linkage` is a role (defaults to
    /// `members` and `quorum`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linkage_members: Option<u16>,
    /// See [`ThresholdSpec::linkage_members`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linkage_quorum: Option<u16>,
    /// For `umbrella`: leaf pseudonyms per umbrella credential.
    #[serde(default = "ThresholdSpec::default_leaves")]
    pub umbrella_leaves: u32,
    /// For `umbrella`: the protocol the committee signs umbrellas with.
    #[serde(default = "ThresholdSpec::default_protocol")]
    pub umbrella_inner: String,
    /// Whether the offline phase (FROST commitments, CGGMP21 presignatures) is done ahead
    /// of time, leaving the online round alone.
    #[serde(default)]
    pub preprocessed: bool,
    /// How long a coordinator waits for a round before excluding the silent, ms.
    #[serde(default = "ThresholdSpec::default_timeout")]
    pub round_timeout_ms: f64,
    /// How long it waits before retrying a session that ran out of members, s.
    #[serde(default = "ThresholdSpec::default_retry")]
    pub retry_after_s: f64,
    /// Ask every live member and keep the first quorum (`true`), or exactly a quorum.
    #[serde(default = "crate::scenario::schema::truth")]
    pub ask_all: bool,
    /// One-way latency between a coordinator and a member, ms: separate organisations
    /// over a WAN.
    #[serde(default = "ThresholdSpec::default_member_latency")]
    pub member_latency_ms: f64,
}

impl ThresholdSpec {
    fn default_protocol() -> String {
        "frost-p256".to_string()
    }
    fn default_members() -> u16 {
        5
    }
    fn default_quorum() -> u16 {
        3
    }
    fn default_roles() -> Vec<String> {
        vec!["pca".to_string(), "crl".to_string(), "linkage".to_string()]
    }
    fn default_leaves() -> u32 {
        20
    }
    fn default_timeout() -> f64 {
        500.0
    }
    fn default_retry() -> f64 {
        30.0
    }
    fn default_member_latency() -> f64 {
        20.0
    }
}

/// `actors.backend.failures[]`: a backend server that is down, slow or lying for a window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendFailureSpec {
    /// The server: a role (`pca`, `la1`, `la2`, `ma`, `crlg`, `ra`, …) or a committee
    /// member, `<role>-member-<k>` (`pca-member-0`, `linkage-member-2`, `crl-member-1`);
    /// the home provider's, unless qualified by its provider, `<provider>/<role>`
    /// (`provider-b/crlg`).
    pub node: String,
    /// `offline`, `slow` or `malicious`.
    pub mode: String,
    /// When it starts, s.
    #[serde(default)]
    pub from_s: f64,
    /// When it ends, s; absent for the rest of the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_s: Option<f64>,
    /// For `slow`: the extra service time per message, ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_ms: Option<f64>,
}

/// Whether the cryptography is done or costed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CryptoModeSpec {
    /// Sign and verify are modelled by their cost and their outcome.
    #[default]
    Modeled,
    /// Real ECDSA over real bytes.
    Real,
}

impl From<CryptoModeSpec> for v2xw_core::manifest::CryptoMode {
    fn from(m: CryptoModeSpec) -> Self {
        match m {
            CryptoModeSpec::Modeled => v2xw_core::manifest::CryptoMode::Modeled,
            CryptoModeSpec::Real => v2xw_core::manifest::CryptoMode::Real,
        }
    }
}

/// How often the full certificate is attached (05-protocols.md §2.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerIdPolicySpec {
    /// The cadence, milliseconds. 1000 is the J2945/1 reading, 450 the NYC-pilot one;
    /// 05-protocols.md §2.4 admits both, which is why this is a number and not a flag.
    #[serde(default = "SignerIdPolicySpec::default_cadence")]
    pub full_cert_every_ms: u64,
    /// Attach a digest the rest of the time.
    #[serde(default = "crate::scenario::schema::truth")]
    pub digest_otherwise: bool,
    /// Also attach the certificate to the next BSM or CAM after a station this node has
    /// not heard before appears (the ETSI CAM rule, TS 103 097 V1.3.1 §7.1.1). Off by
    /// default, and left out of a scenario that does not set it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub on_new_neighbour: bool,
}

impl SignerIdPolicySpec {
    fn default_cadence() -> u64 {
        1000
    }
}

impl Default for SignerIdPolicySpec {
    fn default() -> Self {
        SignerIdPolicySpec {
            full_cert_every_ms: SignerIdPolicySpec::default_cadence(),
            digest_otherwise: true,
            on_new_neighbour: false,
        }
    }
}

/// When a node changes pseudonym.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PseudonymChangeSpec {
    /// `time`, `distance`, `c2c-cc`, `mix-zone` or `silent`
    /// (`crate::pseudonym_policy`).
    #[serde(default = "PseudonymChangeSpec::default_strategy")]
    pub strategy: String,
    /// The period, seconds, when the strategy is time-based.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_s: Option<f64>,
    /// The distance, metres, when the strategy is distance-based.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<f64>,
    /// A random silent period after every change, `[min, max]` seconds: no safety message
    /// for a uniform draw from the range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silent_period_s: Option<Vec<f64>>,
    /// The mix zone's radius around a signalised intersection, metres (`mix-zone`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix_zone_radius_m: Option<f64>,
    /// The age a pseudonym must reach before a mix zone changes it, seconds (`mix-zone`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix_zone_min_age_s: Option<f64>,
}

impl PseudonymChangeSpec {
    fn default_strategy() -> String {
        "time".to_string()
    }
}

impl Default for PseudonymChangeSpec {
    fn default() -> Self {
        PseudonymChangeSpec {
            strategy: PseudonymChangeSpec::default_strategy(),
            period_s: Some(300.0),
            distance_m: None,
            silent_period_s: None,
            mix_zone_radius_m: None,
            mix_zone_min_age_s: None,
        }
    }
}

/// Which hardware the nodes run on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nodes {
    /// The profile every equipped vehicle uses unless its class overrides it.
    #[serde(default = "Nodes::default_obu")]
    pub default_obu: String,
    /// Per-class overrides, by vehicle class name.
    #[serde(default)]
    pub per_class: BTreeMap<String, String>,
    /// The compute tier.
    #[serde(default = "RadioTiers::medium")]
    pub compute_tier: Tier,
    /// The backend tier.
    #[serde(default = "RadioTiers::medium")]
    pub backend_tier: Tier,
    // network audit
    /// How many received messages wait for signature verification before new ones are
    /// dropped, per node. 64 unless set (no profile publishes a depth).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_queue_depth: Option<u32>,
    /// How many peer certificates each vehicle or roadside unit holds before it forgets the least
    /// recently used one. 128 unless set (no profile publishes one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_cert_cache: Option<u32>,
    /// How many validated end-entity certificates each device remembers, so a repeat
    /// signer costs only its signature check (least recently used evicted first).
    /// 4,096 unless set (`v2xw_node::rxval::EE_CACHE`; no device publishes one). The peer
    /// certificate cache usually bounds it first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validated_cert_cache: Option<u32>,
    /// The lower edge of the HSM signing time, microseconds, for a profile that publishes
    /// only an upper bound (the CRATON2's "<9 ms"). Set, each signature takes a time drawn
    /// uniformly between this edge and the bound from the node's own random stream; unset,
    /// every signature takes the bound itself (the worst case). No typical value is
    /// published for the CRATON2/SECTON eHSM (searched 2026-10-13: Unex sheets, Autotalks
    /// pages, CMVP #3556/#4395 policies), so the default stays the bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hsm_sign_lower_us: Option<f64>,
}

impl Nodes {
    /// The reference on-board unit `v2xw-node` ships.
    ///
    /// It was the string `node/obu/reference` until the 2026-09-22 audit, which is not a
    /// profile id any build has ever shipped: `v2xw_node::profiles::get` returned nothing
    /// for it and `wiring::build_node` fell through to the reference profile without a
    /// word. So the default *behaved* correctly and *documented* a device that does not
    /// exist, which is the kind of gap a generated settings form turns into a
    /// free-text box offering nonsense. It is now the id itself, and
    /// `crate::scenario::validate` refuses any id that does not ship.
    fn default_obu() -> String {
        v2xw_node::profiles::REFERENCE_OBU.to_string()
    }
}

impl Default for Nodes {
    fn default() -> Self {
        Nodes {
            default_obu: Nodes::default_obu(),
            per_class: BTreeMap::new(),
            compute_tier: Tier::Medium,
            backend_tier: Tier::Medium,
            verify_queue_depth: None,
            peer_cert_cache: None,
            validated_cert_cache: None,
            hsm_sign_lower_us: None,
        }
    }
}

/// Attackers, jammers and compromised infrastructure.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Threats {
    /// The attackers.
    #[serde(default)]
    pub attackers: Vec<Attacker>,
    /// The jammers.
    #[serde(default)]
    pub jammers: Vec<ModelChoice>,
    /// Roadside units under an attacker's control, by their position in `actors.rsus`
    /// (0 is the first unit declared).
    #[serde(default)]
    pub compromised_rsus: Vec<u32>,
    /// What a compromised unit does: `threat/attacker/compromised-rsu` with `params.kind`
    /// `PoisonForwardedReports` (the default), `SuppressForwardedReports` or `FalseCrl`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compromised_rsu_attack: Option<ModelChoice>,
    /// The passive eavesdropper's coverage: `threat/observer/passive-privacy` with
    /// `params.sniffer_fraction` (the share of signalised intersections with a sniffer)
    /// and `params.range_m`. Absent, the observer hears every safety frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eavesdropper: Option<ModelChoice>,
}

/// One attacker population.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attacker {
    /// The attacker model's id.
    pub id: String,
    /// What share of equipped vehicles are attackers. Mutually exclusive with `count` and
    /// `actor_ids`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fraction: Option<f64>,
    /// How many attackers. Mutually exclusive with `fraction` and `actor_ids`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Exactly which actors are attackers. Mutually exclusive with the other two.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actor_ids: Vec<u32>,
    /// The attacker's parameters.
    #[serde(default)]
    pub params: serde_json::Value,
    /// When it is active, simulated seconds `[from, to)`. Empty means the whole run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<DilationWindow>,
}

/// Detectors and the misbehaviour authority.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detection {
    /// Detectors that run on the node.
    #[serde(default)]
    pub local: Vec<ModelChoice>,
    /// The misbehaviour-authority pipeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ma: Option<ModelChoice>,
    /// The responder that acts on the authority's decisions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responder: Option<ModelChoice>,
    /// The perception tier, when the scenario models perception.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perception_tier: Option<Tier>,
}

/// One exporter to run at the end of the run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExporterSpec {
    /// The exporter's id, e.g. `ma-dataset-v2`, `receiver-logs`, `recording`.
    pub id: String,
    /// Its options.
    #[serde(default)]
    pub opts: serde_json::Value,
}

/// The V2X applications every equipped vehicle runs over what it hears, and how drivers
/// respond to the one that advises (`v2xw_node::apps`, whose table cites each threshold).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Apps {
    /// Which applications run: any of `fcw`, `eebl`, `ima`, `lta`, `bsw` (blind spot and
    /// lane change), `pcw`, `rlvw`, `glosa`. Empty runs none.
    #[serde(default = "Apps::default_enabled")]
    pub enabled: Vec<String>,
    /// The fraction of equipped drivers who follow GLOSA's speed advice, `0..=1`.
    #[serde(default)]
    pub glosa_compliance: f64,
    /// FCW's time-to-collision threshold, seconds.
    #[serde(default = "Apps::default_fcw_ttc_s")]
    pub fcw_ttc_s: f64,
    /// IMA's look-ahead to the conflict point, seconds.
    #[serde(default = "Apps::default_ima_tti_s")]
    pub ima_tti_s: f64,
    /// LTA's critical gap, seconds.
    #[serde(default = "Apps::default_lta_gap_s")]
    pub lta_gap_s: f64,
    /// PCW's time threshold, seconds.
    #[serde(default = "Apps::default_pcw_ttc_s")]
    pub pcw_ttc_s: f64,
    /// RLVW's comfortable-stop deceleration, m/s².
    #[serde(default = "Apps::default_rlvw_decel_mps2")]
    pub rlvw_decel_mps2: f64,
}

impl Apps {
    /// Every application.
    pub const ALL: [&'static str; 8] = ["fcw", "eebl", "ima", "lta", "bsw", "pcw", "rlvw", "glosa"];

    fn default_enabled() -> Vec<String> {
        Self::ALL.iter().map(|s| (*s).to_string()).collect()
    }
    fn default_fcw_ttc_s() -> f64 {
        2.4
    }
    fn default_ima_tti_s() -> f64 {
        4.0
    }
    fn default_lta_gap_s() -> f64 {
        4.1
    }
    fn default_pcw_ttc_s() -> f64 {
        3.0
    }
    fn default_rlvw_decel_mps2() -> f64 {
        3.4
    }

    /// Whether this is the default.
    pub fn is_default(&self) -> bool {
        *self == Apps::default()
    }

    /// Whether `app` runs.
    pub fn runs(&self, app: &str) -> bool {
        self.enabled.iter().any(|a| a == app)
    }
}

impl Default for Apps {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            glosa_compliance: 0.0,
            fcw_ttc_s: Self::default_fcw_ttc_s(),
            ima_tti_s: Self::default_ima_tti_s(),
            lta_gap_s: Self::default_lta_gap_s(),
            pcw_ttc_s: Self::default_pcw_ttc_s(),
            rlvw_decel_mps2: Self::default_rlvw_decel_mps2(),
        }
    }
}

/// One item on the scenario timeline.
///
/// §13 writes these as `{t, until?, type, params}`; `type` is a Rust keyword, so the field
/// is `kind` in Rust and `type` on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineItem {
    /// When it takes effect, simulated seconds.
    pub t: f64,
    /// When it stops taking effect, simulated seconds. A one-shot item has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<f64>,
    /// What happens.
    #[serde(rename = "type")]
    pub kind: TimelineKind,
    /// WP-21: the condition that fires it. Unset, the item fires at `t` as always; set,
    /// it is armed at `t` and fires at the first mobility step at which the condition
    /// holds on the run's ground truth (the scenario director's hand, not a node's view).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
    /// WP-21: how many times a `when` item may fire. Unset is once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fires: Option<u32>,
    /// WP-21: the least time between two firings of a `when` item, seconds. Unset is 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_s: Option<f64>,
    /// Its parameters, in the shape the kind takes.
    #[serde(default, flatten)]
    pub params: BTreeMap<String, serde_json::Value>,
}

/// A timeline item's firing condition (WP-21), evaluated once per mobility step on the
/// run's ground truth, actors visited in actor-id order.
///
/// Closed and tagged by `kind`. One level of `all`/`any` over the [`Clause`]s; a clause
/// that names a pair binds `when.leader` and `when.follower`, one that names an area binds
/// `when.actor`, and an effect's `target` may name the binding. The design reference is
/// Artery's storyboard (behaviour only); the use is Euro NCAP-style scripted tests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    /// A vehicle's time to collision with the vehicle ahead on its lane is below `ttc_s`
    /// (gap over closing speed; binds the pair).
    TtcBelow {
        /// The threshold, seconds.
        ttc_s: f64,
    },
    /// A vehicle is inside the polygon (binds it).
    InPolygon {
        /// The polygon's corners in the world's local frame, metres `[x, y]`; it closes
        /// itself.
        points_m: Vec<[f64; 2]>,
    },
    /// A vehicle closes on the vehicle ahead on its lane, within 100 m, faster than
    /// `speed_diff_mps` (binds the pair).
    SpeedDiffAbove {
        /// The closing speed, m/s.
        speed_diff_mps: f64,
    },
    /// A draw per step, keyed by the seed, the item and the step, holds with
    /// probability `p`.
    Probability {
        /// The probability per mobility step.
        p: f64,
    },
    /// Every clause holds for the same vehicle.
    All {
        /// The clauses.
        of: Vec<Clause>,
    },
    /// At least one clause holds.
    Any {
        /// The clauses.
        of: Vec<Clause>,
    },
}

/// One clause of an `all` or `any` [`Condition`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Clause {
    /// See [`Condition::TtcBelow`].
    TtcBelow {
        /// The threshold, seconds.
        ttc_s: f64,
    },
    /// See [`Condition::InPolygon`].
    InPolygon {
        /// The polygon's corners, metres `[x, y]`.
        points_m: Vec<[f64; 2]>,
    },
    /// See [`Condition::SpeedDiffAbove`].
    SpeedDiffAbove {
        /// The closing speed, m/s.
        speed_diff_mps: f64,
    },
    /// See [`Condition::Probability`].
    Probability {
        /// The probability per mobility step.
        p: f64,
    },
}

/// The kinds of timeline item (03-interfaces.md §13).
///
/// Closed: a scenario cannot invent a control event, because the engine would have nothing
/// to do with it and an unvalidated `type` string is a typo that runs silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum TimelineKind {
    /// Scale the offered demand by `value`.
    #[serde(rename = "demand.multiplier")]
    DemandMultiplier,
    /// A weather front arrives: `value` is a [`WeatherKind`].
    #[serde(rename = "weather.front")]
    WeatherFront,
    /// An attack wave starts: `ids` names the attacker populations.
    #[serde(rename = "attack.wave")]
    AttackWave,
    /// An entity goes down: `target` names it.
    #[serde(rename = "outage")]
    Outage,
    /// A parameter changes: `path` is a dotted path into this scenario, `value` the new
    /// value.
    #[serde(rename = "param.change")]
    ParamChange,
    /// A road closes: `target` names what, in one of three spellings
    /// ([`crate::timeline::ClosureTarget::parse`]): one lane by id (`123` or `"lane:123"`),
    /// every lane of one edge (`"edge:45"`), or every edge of a named street, ignoring case
    /// (`"street:West 42nd Street"`). Only vehicle lanes close; footways and crossings stay
    /// open. With `until`, the road reopens then.
    #[serde(rename = "closure")]
    Closure,
    /// A scripted emergency stop: the vehicle `target` names (a node id, or `"auto"` for
    /// the first moving vehicle with a vehicle close behind it) brakes at `decel_mps2`
    /// (default 0.5 g) to a standstill, stands for `hold_s` (default 2 s) and drives on.
    /// The traffic model's own drivers almost never brake past the 0.4 g event threshold
    /// (none did in 18,052 samples of a dense run), so a hard-braking warning needs one.
    #[serde(rename = "safety.hard-brake")]
    HardBrake,
    /// A vehicle breaks down: `target` (as for a hard brake, `"auto"` picking the first
    /// moving vehicle) brakes to a stop at `decel_mps2` (default 3 m/s²), switches its
    /// hazard warning lights on, and stands until `until` (or the end of the run).
    #[serde(rename = "safety.breakdown")]
    Breakdown,
    /// A cut-in: `target` (a node id, or `"auto"` for the first vehicle with a car on the
    /// `side` lane just behind it) changes lane to `side` (`left` or `right`, default
    /// `left`) at once, into the gap ahead of that lane's follower, whatever its
    /// lane-change model would have chosen.
    #[serde(rename = "safety.cut-in")]
    CutIn,
}

impl TimelineKind {
    /// The parameter keys this kind requires, which `validate` checks are present.
    pub const fn required_params(self) -> &'static [&'static str] {
        match self {
            TimelineKind::DemandMultiplier => &["value"],
            TimelineKind::WeatherFront => &["value"],
            TimelineKind::AttackWave => &["ids"],
            TimelineKind::Outage => &["target"],
            TimelineKind::ParamChange => &["path", "value"],
            TimelineKind::Closure => &["target"],
            TimelineKind::HardBrake | TimelineKind::Breakdown | TimelineKind::CutIn => &[],
        }
    }

    /// Whether this kind is meaningful with an `until`.
    ///
    /// A weather front does not end — the next front replaces it — and neither does a
    /// parameter change, so an `until` on one is an author error rather than a no-op.
    pub const fn takes_until(self) -> bool {
        matches!(
            self,
            TimelineKind::DemandMultiplier
                | TimelineKind::Outage
                | TimelineKind::AttackWave
                | TimelineKind::Closure
                | TimelineKind::Breakdown
        )
    }
}

/// A parameter sweep (08-measurement-and-data.md §4).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Experiment {
    /// Dotted paths into the scenario, each with the values to sweep over.
    #[serde(default)]
    pub sweep: BTreeMap<String, Vec<serde_json::Value>>,
    /// The seeds to repeat each point with.
    #[serde(default)]
    pub seeds: Vec<u64>,
    /// How many replications per (point, seed).
    #[serde(default)]
    pub replications: u32,
}

/// For `skip_serializing_if`: a default-`true` flag that is still `true`.
#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn is_true(b: &bool) -> bool {
    *b
}

/// `true`, as a function, for `#[serde(default = …)]`.
pub(crate) fn truth() -> bool {
    true
}

/// The sensors of the vehicles and roadside units that send CPMs, and how a road user is
/// hidden from them ([`Scenario::perception`], `crate::perception`).
///
/// Absent, every CPM-sending vehicle carries the default front suite (a 250 m ±9°
/// long-range radar, a 70 m ±45° short-range radar and an 80 m ±26° camera) and occlusion
/// is the centre-line rule, as before 2026-10-26.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerceptionSpec {
    /// How a road user is hidden from a sensor: `centre-line` (the line to its reference
    /// point; other vehicles are discs half a body width across) or `outline` (seen when
    /// any corner of its outline is in view with a clear line; other vehicles are their
    /// oriented body rectangles; the visible-corner rule of Günther et al., IEEE VNC
    /// 2016).
    #[serde(default)]
    pub occlusion: OcclusionSpec,
    /// The suite every CPM-sending vehicle carries unless its class has its own. Absent:
    /// the default front suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<Vec<SensorConf>>,
    /// Suites by vehicle class (`passenger`, `truck`, `bus`, …), over `vehicle`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub classes: BTreeMap<String, Vec<SensorConf>>,
    /// The suite of every roadside unit with the `cpm` role; its azimuths are
    /// counter-clockwise from east and its offsets east, north and up from the unit's
    /// antenna. Absent: four 80 m cameras with a 90° opening facing east, north, west and
    /// south.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rsu: Option<Vec<SensorConf>>,
}

/// How a road user is hidden from a sensor ([`PerceptionSpec::occlusion`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OcclusionSpec {
    /// The line to the reference point; other vehicles are discs.
    #[default]
    CentreLine,
    /// Any visible outline corner; other vehicles are oriented rectangles.
    Outline,
}

/// What a sensor is ([`SensorConf::kind`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SensorKindSpec {
    /// A radar (CPM `SensorType` 1).
    #[default]
    Radar,
    /// A mono camera (CPM `SensorType` 3).
    Camera,
    /// A lidar (CPM `SensorType` 2).
    Lidar,
}

/// Where on the body a vehicle sensor's offset is measured from ([`SensorConf::mount`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MountSpec {
    /// The front bumper.
    #[default]
    Front,
    /// The rear bumper.
    Rear,
    /// The body's centre.
    Centre,
}

/// One sensor of a suite ([`PerceptionSpec`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorConf {
    /// Its id within the station, 1 to 7: the CPM's `sensorId`.
    pub id: u8,
    /// What it is: `radar`, `camera` or `lidar`.
    #[serde(default)]
    pub kind: SensorKindSpec,
    /// How far it sees, metres (at most 409.5, the CPM's `StandardLength12b`).
    pub range_m: f64,
    /// Its full horizontal opening, degrees (360 sees all round).
    pub fov_deg: f64,
    /// Its boresight, degrees counter-clockwise from the vehicle's heading (180 looks
    /// back; from east at a roadside unit).
    #[serde(default)]
    pub azimuth_deg: f64,
    /// What the offset is measured from on a vehicle: `front`, `rear` or `centre`.
    #[serde(default)]
    pub mount: MountSpec,
    /// The offset ahead of the mount, metres (negative is behind it; east at a roadside
    /// unit).
    #[serde(default)]
    pub ahead_m: f64,
    /// The offset to the left, metres (north at a roadside unit).
    #[serde(default)]
    pub left_m: f64,
    /// The height above the reference point, metres.
    #[serde(default = "SensorConf::default_height")]
    pub height_m: f64,
    /// Detection probability at the sensor (default 0.99).
    #[serde(default = "SensorConf::default_pd_near")]
    pub pd_near: f64,
    /// Detection probability at the range limit (default 0.90).
    #[serde(default = "SensorConf::default_pd_far")]
    pub pd_far: f64,
    /// Position error σ at the sensor, metres. Absent: 0.10 for radar, 0.20 for a camera,
    /// 0.03 for lidar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sigma_base_m: Option<f64>,
    /// Position error σ growth as a fraction of range. Absent: 0.005 for radar, 0.02 for
    /// a camera, 0.001 for lidar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sigma_range_fraction: Option<f64>,
}

impl SensorConf {
    fn default_height() -> f64 {
        0.6
    }
    fn default_pd_near() -> f64 {
        0.99
    }
    fn default_pd_far() -> f64 {
        0.90
    }
}

#[cfg(test)]
mod gnss_settings_tests {
    use super::GnssSettings;

    /// E4-BODY: `gnss.velocity_noise_mps` reaches the receiver model, and unset keeps the
    /// SPS default (so no scenario that does not set it moves).
    #[test]
    fn velocity_noise_reaches_the_receiver_and_unset_keeps_the_default() {
        let default = v2xw_mobility::gnss::GaussMarkovParams::default().sigma_velocity_mps;
        assert!((GnssSettings::default().params().sigma_velocity_mps - default).abs() < 1e-12);
        let quiet = GnssSettings {
            velocity_noise_mps: Some(0.0),
            ..GnssSettings::default()
        };
        assert_eq!(quiet.params().sigma_velocity_mps, 0.0);
        assert!(
            default > 0.1,
            "the SPS default is 0.2 m/s at 95 %: {default}"
        );
    }
}

/// WP-17 P17.2: the `control: nema` choice and the `traffic.signals.nema` block load,
/// check and refuse as the build plan says, with no behaviour behind them yet.
#[cfg(test)]
mod nema_settings_tests {
    use super::{
        ForceOffMode, LeftTurnMode, NemaSettings, PermissiveDisplay, SignalControl,
        SignalControlKind, Traffic,
    };

    fn errors(g: &SignalControl) -> Vec<String> {
        let mut s = crate::Scenario::minimal();
        s.traffic = Some(Traffic {
            signals: g.clone(),
            ..Traffic::default()
        });
        crate::scenario::validate(&s)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn nema(n: NemaSettings) -> SignalControl {
        SignalControl {
            control: SignalControlKind::Nema,
            nema: Some(n),
            ..SignalControl::default()
        }
    }

    /// The block's own errors: not the refusals of the parts later WP-17 pieces build,
    /// which `nema_loads_and_its_unbuilt_parts_are_refused` checks.
    fn block_errors(n: NemaSettings) -> Vec<String> {
        errors(&nema(n))
            .into_iter()
            .filter(|e| !e.contains("is not run by this build"))
            .collect()
    }

    /// The defaults are the STM 2nd ed. layout (Exhibit 5-3; SUMO's "usually 4,8" and
    /// "2,6" barriers), the P17.1 left-turn timing and Exhibit 6-22's recalls; an empty
    /// block takes them all, and the default signal settings write no `nema` key (so no
    /// scenario digest moves).
    #[test]
    fn nema_defaults_are_the_stm_layout() {
        let d = NemaSettings::default();
        assert_eq!(d.ring1, [1, 2, 3, 4]);
        assert_eq!(d.ring2, [5, 6, 7, 8]);
        assert_eq!(d.barrier_phases, [4, 8]);
        assert_eq!(d.barrier2_phases, [2, 6]);
        assert_eq!(
            (d.left_min_green_s, d.left_max_green_s, d.left_passage_s),
            (4.0, 25.0, 2.5)
        );
        assert!(d.min_recall.is_empty() && d.max_recall.is_empty() && d.ped_recall.is_empty());
        assert_eq!(d.soft_recall, [2, 6]);
        assert_eq!(d.coordinate_phases, [2, 6]);
        assert!(!d.coordination);
        assert_eq!(d.left_turn_mode, LeftTurnMode::ProtectedPermissive);
        assert_eq!(d.permissive_display, PermissiveDisplay::Fya);
        assert_eq!(d.force_off, ForceOffMode::Floating);
        let empty: NemaSettings = serde_json::from_str("{}").expect("an empty block loads");
        assert_eq!(empty, d);
        let wire = serde_json::to_value(SignalControl::default()).expect("serialises");
        assert!(wire.get("nema").is_none(), "{wire}");
        // The default block is valid in itself.
        assert!(block_errors(d).is_empty());
    }

    /// P17.5 wired the controller: `control: nema` loads with its defaults (the P17.2
    /// contract replaced). P17.6 built the protected-only and permissive-only lefts, P17.7
    /// pedestrian recall and P17.8 coordination, which now load. What stays refused under
    /// nema is what the rings would ignore or undo: a green wave without coordination,
    /// and the short-link rewrite of the plans. A block beside any other control is a
    /// setting that would do nothing.
    #[test]
    fn nema_loads_and_its_unbuilt_parts_are_refused() {
        assert!(errors(&nema(NemaSettings::default())).is_empty());
        let d = NemaSettings::default;
        for mode in [LeftTurnMode::Protected, LeftTurnMode::Permissive] {
            let n = NemaSettings {
                left_turn_mode: mode,
                ..d()
            };
            assert!(errors(&nema(n)).is_empty(), "{mode:?} loads");
        }
        // P17.7 built pedestrian recall.
        let recall = NemaSettings {
            ped_recall: vec![2],
            ..d()
        };
        assert!(errors(&nema(recall)).is_empty(), "pedestrian recall loads");
        // P17.8 built coordination; a green wave then sets the offsets it keeps.
        let coordinated = NemaSettings {
            coordination: true,
            ..d()
        };
        assert!(
            errors(&nema(coordinated.clone())).is_empty(),
            "coordination loads"
        );
        let wave = SignalControl {
            green_wave_speed_mps: Some(12.0),
            ..nema(d())
        };
        assert!(
            errors(&wave)
                .iter()
                .any(|x| x.contains("traffic.signals.green_wave_speed_mps")),
            "a green wave under free-running rings"
        );
        let coordinated_wave = SignalControl {
            green_wave_speed_mps: Some(12.0),
            ..nema(coordinated)
        };
        assert!(
            errors(&coordinated_wave).is_empty(),
            "a coordinated green wave loads"
        );
        let short = SignalControl {
            short_link_clearance_m: Some(60.0),
            ..nema(d())
        };
        assert!(
            errors(&short)
                .iter()
                .any(|x| x.contains("traffic.signals.short_link_clearance_m")),
            "the short-link rewrite under the dual ring"
        );
        let stray = SignalControl {
            control: SignalControlKind::Actuated,
            nema: Some(NemaSettings::default()),
            ..SignalControl::default()
        };
        let e = errors(&stray);
        assert!(
            e.iter().any(|x| x.contains("traffic.signals.nema")),
            "{e:?}"
        );
        // Neither fires on the default settings.
        assert!(errors(&SignalControl::default()).is_empty());
    }

    /// A ring, barrier, recall or coordination list that does not describe a dual-ring
    /// layout is refused, naming the field.
    #[test]
    fn a_malformed_ring_layout_is_refused() {
        let refused = |n: NemaSettings, field: &str| {
            let e = block_errors(n);
            assert!(e.iter().any(|x| x.contains(field)), "{field}: {e:?}");
        };
        let d = NemaSettings::default;
        refused(
            NemaSettings {
                ring1: vec![1, 2, 2, 4],
                ..d()
            },
            "nema.ring1",
        );
        refused(
            NemaSettings {
                ring1: vec![2, 1, 3, 4],
                ..d()
            },
            "nema.ring1",
        );
        refused(
            NemaSettings {
                ring2: vec![1, 6, 7, 8],
                ..d()
            },
            "nema.ring2",
        );
        refused(
            NemaSettings {
                ring2: vec![],
                ..d()
            },
            "nema.ring2",
        );
        refused(
            NemaSettings {
                barrier_phases: vec![2, 6],
                ..d()
            },
            "nema.barrier_phases",
        );
        refused(
            NemaSettings {
                barrier2_phases: vec![2],
                ..d()
            },
            "nema.barrier2_phases",
        );
        refused(
            NemaSettings {
                max_recall: vec![9],
                ..d()
            },
            "nema.max_recall",
        );
        refused(
            NemaSettings {
                ped_recall: vec![1],
                ..d()
            },
            "nema.ped_recall",
        );
        refused(
            NemaSettings {
                coordination: true,
                coordinate_phases: vec![2, 8],
                ..d()
            },
            "nema.coordinate_phases",
        );
        refused(
            NemaSettings {
                left_min_green_s: 20.0,
                left_max_green_s: 10.0,
                ..d()
            },
            "nema.left_max_green_s",
        );
        refused(
            NemaSettings {
                yellow_s: Some(2.0),
                ..d()
            },
            "nema.yellow_s",
        );
        // A layout with no side-street lefts (no phases 3 and 7) still loads.
        let t = NemaSettings {
            ring1: vec![1, 2, 4],
            ring2: vec![5, 6, 8],
            ..d()
        };
        assert!(block_errors(t).is_empty());
    }

    /// Coordination-only and left-phase-only settings are refused where nothing reads them.
    #[test]
    fn nema_settings_that_would_do_nothing_are_refused() {
        let d = NemaSettings::default;
        let fixed = NemaSettings {
            force_off: ForceOffMode::Fixed,
            ..d()
        };
        assert!(
            block_errors(fixed.clone())
                .iter()
                .any(|x| x.contains("nema.force_off"))
        );
        assert!(
            block_errors(NemaSettings {
                coordination: true,
                ..fixed
            })
            .is_empty()
        );
        let permissive_with_timing = NemaSettings {
            left_turn_mode: LeftTurnMode::Permissive,
            left_max_green_s: 20.0,
            ..d()
        };
        assert!(
            block_errors(permissive_with_timing)
                .iter()
                .any(|x| x.contains("nema.left_max_green_s"))
        );
        let protected_green_ball = NemaSettings {
            left_turn_mode: LeftTurnMode::Protected,
            permissive_display: PermissiveDisplay::GreenBall,
            ..d()
        };
        assert!(
            block_errors(protected_green_ball)
                .iter()
                .any(|x| x.contains("nema.permissive_display"))
        );
    }

    /// WP-17 P17.9, for WP-23: a SUMO NEMA programme's documented parameters become these
    /// settings. Ring order states lead or lag, `0` marks an absent phase, coordinate
    /// mode coordinates `barrier2Phases`, an unset `minRecall` calls every phase (SUMO's
    /// default), and the result loads.
    #[test]
    fn nema_settings_from_sumo_params() {
        use super::{NemaControllerType, NemaLeftSequence};
        use std::collections::BTreeMap;
        let p = |kv: &[(&str, &str)]| -> BTreeMap<String, String> {
            kv.iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect()
        };
        let free = NemaSettings::from_sumo_params(&p(&[
            ("ring1", "1,2,3,4"),
            ("ring2", "6,5,0,8"),
            ("barrierPhases", "4,8"),
            ("barrier2Phases", "2,6"),
        ]))
        .expect("parses");
        assert_eq!(free.lefts, NemaLeftSequence::LeadLag);
        assert_eq!(free.ring2, [5, 6, 8]);
        assert_eq!(free.min_recall, [1, 2, 3, 4, 5, 6, 8]);
        assert!(!free.coordination);
        let coordinated = NemaSettings::from_sumo_params(&p(&[
            ("ring1", "2,1,4,3"),
            ("ring2", "6,5,8,7"),
            ("barrierPhases", "4,8"),
            ("barrier2Phases", "2,6"),
            ("coordinate-mode", "true"),
            ("minRecall", "2,6"),
            ("maxRecall", "4"),
            ("fixForceOff", "true"),
            ("controllerType", "Type 170"),
        ]))
        .expect("parses");
        assert_eq!(coordinated.lefts, NemaLeftSequence::LagLag);
        assert!(coordinated.coordination);
        assert_eq!(coordinated.coordinate_phases, [2, 6]);
        assert_eq!(coordinated.min_recall, [2, 6]);
        assert_eq!(coordinated.max_recall, [4]);
        assert_eq!(coordinated.force_off, ForceOffMode::Fixed);
        assert_eq!(coordinated.controller_type, NemaControllerType::Type170);
        assert!(block_errors(coordinated).is_empty());
        assert!(block_errors(free).is_empty());
        assert!(
            NemaSettings::from_sumo_params(&p(&[("ring1", "2,1,3,4"), ("ring2", "5,6,7,8")]))
                .is_err()
        );
        assert!(NemaSettings::from_sumo_params(&p(&[("ring1", "1,x")])).is_err());
    }
}
