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
            lane_changes: true,
            strategic_lane_choice: true,
            keep_junctions_clear: true,
            gridlock_timeout_s: Traffic::DEFAULT_GRIDLOCK_TIMEOUT_S,
            signals: SignalControl::default(),
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
}

/// Signal control ([`Traffic::signals`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalControl {
    /// Fixed-time or vehicle-actuated.
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
    /// A green wave: every signal's offset set so that a vehicle at this speed meets green
    /// after green travelling in green_wave_heading_deg. Absent keeps the plans' own
    /// offsets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub green_wave_speed_mps: Option<f64>,
    /// The direction the green wave runs, degrees clockwise from north (90 is eastbound).
    #[serde(default = "SignalControl::default_wave_heading")]
    pub green_wave_heading_deg: f64,
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
            green_wave_speed_mps: None,
            green_wave_heading_deg: Self::default_wave_heading(),
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

    /// The mobility period as a [`Duration`].
    pub fn mobility_step(&self) -> Duration {
        Duration::from_millis(self.mobility_step_ms)
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

impl Default for Time {
    fn default() -> Self {
        Time {
            t0: Time::default_t0(),
            duration_s: Time::default_duration_s(),
            mobility_step_ms: Time::default_mobility_step_ms(),
            des_resolution: Time::default_des_resolution(),
            time_dilation: Vec::new(),
            metric_period_s: None,
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
}

impl Default for WorldSignalOptions {
    fn default() -> Self {
        WorldSignalOptions {
            coordinate: true,
            progression_speed_mps: None,
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
    /// What it does: `crl`, `provisioning-proxy`, `report-forward`, `spat`, `map`, `wsa`.
    #[serde(default)]
    pub roles: Vec<String>,
    /// Its hardware profile id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Its backhaul link model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backhaul: Option<String>,
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
}

/// Multi-hop GeoNetworking forwarding of hazard warnings ([`Net::geonet`]), EN 302 636-4-1
/// with the DEN basic service of EN 302 637-3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geonet {
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
    /// The single-hop radio range the forwarding sees, metres; 500 m by default.
    #[serde(default = "Geonet::default_range_m")]
    pub range_m: f64,
    /// How often the originator repeats the warning, ms; 0 sends it once.
    #[serde(default = "Geonet::default_repetition_interval_ms")]
    pub repetition_interval_ms: u64,
    /// How long it keeps repeating, ms.
    #[serde(default = "Geonet::default_repetition_duration_ms")]
    pub repetition_duration_ms: u64,
    /// Keep-alive forwarding: vehicles in the area re-send a warning they stop hearing,
    /// so it reaches vehicles that arrive later.
    #[serde(default)]
    pub keep_alive_forwarding: bool,
    /// How long each warning is followed, seconds.
    #[serde(default = "Geonet::default_window_s")]
    pub window_s: f64,
}

impl Geonet {
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
    fn default_repetition_duration_ms() -> u64 {
        5_000
    }
    fn default_window_s() -> f64 {
        10.0
    }
}

impl Default for Geonet {
    fn default() -> Self {
        Geonet {
            area_forwarding: Geonet::default_area_forwarding(),
            non_area_forwarding: Geonet::default_non_area_forwarding(),
            cbf_variant: Geonet::default_cbf_variant(),
            hop_limit: Geonet::default_hop_limit(),
            area_radius_m: Geonet::default_area_radius_m(),
            range_m: Geonet::default_range_m(),
            repetition_interval_ms: Geonet::default_repetition_interval_ms(),
            repetition_duration_ms: Geonet::default_repetition_duration_ms(),
            keep_alive_forwarding: false,
            window_s: Geonet::default_window_s(),
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
    /// member, `<role>-member-<k>` (`pca-member-0`, `linkage-member-2`, `crl-member-1`).
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
    /// Its parameters, in the shape the kind takes.
    #[serde(default, flatten)]
    pub params: BTreeMap<String, serde_json::Value>,
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
