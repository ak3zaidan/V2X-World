//! `.rou.xml` demand: import as a [`Demand`], and export.
//!
//! Read (SUMO documentation, "Definition of Vehicles, Vehicle Types, and Routes", and the
//! `routes_file.xsd` attribute names; SUMO was read for behaviour only and no code was
//! copied):
//!
//! * `<vType>`: `vClass`, `maxSpeed`, `accel`, `decel`, `tau`, `minGap`, `sigma`,
//!   `speedFactor` (a number, or `norm(mean,dev)` / `normc(mean,dev[,min,max])`),
//!   `speedDev`, `desiredMaxSpeed` and `probability` reach the vehicle
//!   ([`v2xw_mobility::views::TripDriver`]); every other attribute and every `<param>` is
//!   counted in the [`ImportLedger`] by name, never dropped silently.
//! * `<vTypeDistribution>` (nested `<vType probability>` or `vTypes`/`probabilities`) and
//!   `<routeDistribution>`: a vehicle naming one draws its member from its own keyed
//!   stream, as SUMO draws per vehicle.
//! * `<route>`, `<vehicle>`, `<trip>` and `<flow>`. Flows: `vehsPerHour`, `period` and
//!   `number` are equally spaced (SUMO's own spacing); `period="exp(λ)"` draws
//!   exponential headways at rate λ per second (a Poisson stream); `probability` p is a
//!   Bernoulli trial every second from `begin` (SUMO's definition), so a flow's count
//!   over n seconds has mean n·p and variance n·p·(1−p). The old equal spacing at 1/p,
//!   which kept the mean and dropped that variance, stays available as
//!   [`ProbabilityFlows::EqualSpacing`].
//! * `departLane` (a number, `first`, `random`, `best`, `free`, `allowed`), `departPos`
//!   (metres, negative from the lane's end, `base`, `random`, `random_free`, `free`,
//!   `last`, `stop`) and `departSpeed` (m/s, `max`, `desired`, `speedLimit`, `random`,
//!   `avg`, `last`). The values that need the occupancy at the instant of insertion
//!   (`free`, `allowed`, `last`, `avg`, `stop`) fall back to the nearest supported meaning
//!   and are counted as approximations under their value.
//!
//! A vehicle with a route keeps it: [`Demand::planned_route`] returns the world-lane route
//! through exactly the file's edges (found by Dijkstra restricted to those edges and the
//! connectors between them). A trip is routed by the engine.
//!
//! Every draw (a distribution member, a speedFactor, a random lane, position or speed, a
//! flow's arrivals) comes from a stream keyed by the import seed and the vehicle's or the
//! flow's id, so the expansion is the same for the same seed whatever else is in the file.

use std::collections::BTreeMap;

use v2xw_core::card::{Family, ModelCard};
use v2xw_core::ids::LaneId;
use v2xw_core::model::Model;
use v2xw_core::rng::{EntityRef, RngDomain, RngRegistry, RngStream};
use v2xw_core::time::SimTime;
use v2xw_mobility::ctx::MobCtx;
use v2xw_mobility::routing::dijkstra::DijkstraParams;
use v2xw_mobility::traits::Demand;
use v2xw_mobility::views::{DepartSpeed, EdgeCost, TripDriver, TripRequest};
use v2xw_mobility::{Dijkstra, FreeFlowCost, VehicleClass};
use v2xw_world::World;

use crate::SumoIds;

/// The demand model's card id.
pub const MODEL_ID: &str = "mobility/demand/sumo-rou";

/// The key of a flow's arrival stream.
const FLOW_STREAM_ID: &str = "mobility/demand/sumo-rou/flow";

/// A SUMO `speedFactor` law: a normal with mean and deviation, redrawn until inside
/// `[min, max]` (SUMO documentation, "Vehicle speed": `normc(mean, dev, min, max)`; the
/// vType default is `normc(1, 0.1, 0.2, 2)` for a passenger car).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedFactorLaw {
    /// Mean.
    pub mean: f64,
    /// Standard deviation; 0 makes every vehicle drive at `mean`.
    pub dev: f64,
    /// Lower bound.
    pub min: f64,
    /// Upper bound.
    pub max: f64,
}

impl SpeedFactorLaw {
    /// SUMO's bounds for a plain number or a `speedDev`.
    const MIN: f64 = 0.2;
    const MAX: f64 = 2.0;

    /// One draw from standard normals `z` (redrawn while out of bounds; after the last the
    /// value is clamped).
    fn draw(&self, z: &[f64]) -> f64 {
        if self.dev <= 0.0 {
            return self.mean;
        }
        for zi in z {
            let v = self.mean + self.dev * zi;
            if (self.min..=self.max).contains(&v) {
                return v;
            }
        }
        (self.mean + self.dev * z[z.len() - 1]).clamp(self.min, self.max)
    }

    /// Parses `speedFactor` with the type's `speedDev` (or the class's deviation).
    fn parse(s: &str, dev: f64) -> Option<Self> {
        let s = s.trim();
        if let Ok(mean) = s.parse::<f64>() {
            return Some(Self {
                mean,
                dev,
                min: Self::MIN,
                max: Self::MAX,
            });
        }
        let (name, rest) = s.split_once('(')?;
        let args: Vec<f64> = rest
            .trim_end_matches(')')
            .split(',')
            .map(|a| a.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .ok()?;
        match (name.trim(), args.as_slice()) {
            ("norm", [m, d]) => Some(Self {
                mean: *m,
                dev: *d,
                min: 0.0,
                max: f64::MAX,
            }),
            ("normc", [m, d]) => Some(Self {
                mean: *m,
                dev: *d,
                min: Self::MIN,
                max: Self::MAX,
            }),
            ("normc", [m, d, lo, hi]) => Some(Self {
                mean: *m,
                dev: *d,
                min: *lo,
                max: *hi,
            }),
            _ => None,
        }
    }
}

/// One `<vType>`.
#[derive(Debug, Clone, PartialEq)]
pub struct VType {
    /// `id`.
    pub id: String,
    /// `vClass`, when it names a class this engine models.
    pub class: VehicleClass,
    /// `maxSpeed`, m/s (with `desiredMaxSpeed` below it, that).
    pub max_speed_mps: Option<f64>,
    /// `accel`, m/s².
    pub accel_mps2: Option<f64>,
    /// `decel`, m/s².
    pub decel_mps2: Option<f64>,
    /// `tau`, seconds.
    pub tau_s: Option<f64>,
    /// `minGap`, metres.
    pub min_gap_m: Option<f64>,
    /// `sigma`.
    pub sigma: Option<f64>,
    /// `speedFactor` / `speedDev`, when the type states either.
    pub speed_factor: Option<SpeedFactorLaw>,
    /// `carFollowModel` (SUMO's name), and the library model it maps to
    /// ([`library_model`]).
    pub car_follow_model: Option<String>,
    /// `probability` inside a `<vTypeDistribution>`.
    pub probability: Option<f64>,
}

/// The driving-library model (`v2xw_mobility::carfollowing::library`) a SUMO
/// `carFollowModel` names, when the library has it.
pub fn library_model(sumo: &str) -> Option<&'static str> {
    Some(match sumo {
        "Krauss" | "KraussOrig1" | "KraussPS" => "krauss",
        "IDM" => "idm",
        "EIDM" => "eidm",
        "W99" => "w99",
        "ACC" => "acc",
        "CACC" => "cacc-milanes",
        _ => return None,
    })
}

/// The library model a vType's `carFollowModel` is driven by per vehicle: the
/// [`library_model`] it maps to, when that model is not cooperative (a cooperative model
/// needs the run's own cooperative link and drives the engine's model instead).
pub fn applied_model(sumo: &str) -> Option<&'static str> {
    library_model(sumo).filter(|m| {
        v2xw_mobility::carfollowing::library::factory(m).is_some_and(|f| !f.cooperative)
    })
}

/// A `departLane`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DepartLane {
    /// A lane index, `0` = rightmost.
    Index(u32),
    /// `first` (SUMO's default): the rightmost lane the vehicle may use.
    First,
    /// `random`: uniform over the lanes the vehicle may use, at the drawn `u`.
    Random(f64),
    /// `best`: the lane the stated route is followed on with the fewest lane changes.
    Best,
}

/// A `departPos`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DepartPos {
    /// Metres from the lane's start; negative counts from its end.
    Metres(f64),
    /// `base` (SUMO's default): the vehicle's rear at the lane's start.
    Base,
    /// `random`: uniform along the lane, at the drawn `u`.
    Random(f64),
}

/// One vehicle of the file, flows already expanded and distributions drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteVehicle {
    /// `id` (a flow's vehicles are `flowId.n`, as SUMO names them).
    pub id: String,
    /// `depart`, seconds.
    pub depart_s: f64,
    /// The vehicle type: `type`, or the member drawn from the distribution it names.
    pub vtype: Option<String>,
    /// The route's edges; a trip has `[from, to]` (plus `via`) and `routed == false`.
    pub edges: Vec<String>,
    /// Whether `edges` is a full route (true) or trip endpoints the engine must route.
    pub routed: bool,
    /// `departLane`.
    pub depart_lane: DepartLane,
    /// `departPos`.
    pub depart_pos: DepartPos,
    /// `departSpeed`; `None` is SUMO's default, 0.
    pub depart_speed: Option<DepartSpeed>,
    /// This vehicle's speedFactor: the vehicle's own attribute, or the draw from its
    /// type's law; `None` leaves the engine's own draw for the class.
    pub speed_factor: Option<(f64, f64, f64)>,
}

impl RouteVehicle {
    /// The numeric `departLane`, when the file gave one.
    pub fn lane_index(&self) -> Option<u32> {
        match self.depart_lane {
            DepartLane::Index(i) => Some(i),
            _ => None,
        }
    }
}

/// How every attribute of the file was used: each occurrence is counted exactly once,
/// under `element@attribute` (with `=value` for a keyword approximated), so
/// [`ImportLedger::total`] equals the number of attributes in the file and nothing is
/// dropped without a count.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportLedger {
    /// Read and applied as SUMO defines it.
    pub applied: BTreeMap<String, u64>,
    /// Read and applied with a nearer supported meaning (named keyword values).
    pub approximated: BTreeMap<String, u64>,
    /// Read and not modelled (per-type body size, lane-change and junction parameters,
    /// stops, persons, …).
    pub not_modelled: BTreeMap<String, u64>,
    /// Display-only (colours, shapes, the schema reference): no effect on the traffic.
    pub cosmetic: BTreeMap<String, u64>,
    /// Elements the importer does not model, by name (their attributes are in
    /// `not_modelled`).
    pub elements_not_modelled: BTreeMap<String, u64>,
}

impl ImportLedger {
    fn bump(map: &mut BTreeMap<String, u64>, k: String) {
        *map.entry(k).or_default() += 1;
    }

    /// Every attribute occurrence counted.
    pub fn total(&self) -> u64 {
        [
            &self.applied,
            &self.approximated,
            &self.not_modelled,
            &self.cosmetic,
        ]
        .iter()
        .flat_map(|m| m.values())
        .sum()
    }

    /// The count under `key` in any of the four maps.
    pub fn count(&self, key: &str) -> u64 {
        [
            &self.applied,
            &self.approximated,
            &self.not_modelled,
            &self.cosmetic,
        ]
        .iter()
        .filter_map(|m| m.get(key))
        .sum()
    }
}

/// How `probability` flows are expanded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProbabilityFlows {
    /// A Bernoulli trial every second (SUMO's definition).
    #[default]
    Bernoulli,
    /// Equal spacing at 1/p: the mean rate without the variance (the importer's behaviour
    /// before WP-15).
    EqualSpacing,
}

/// What the import draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportOptions {
    /// The seed of every draw.
    pub seed: u64,
    /// How `probability` flows expand.
    pub probability_flows: ProbabilityFlows,
}

/// A parsed `.rou.xml`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RouteFile {
    /// Vehicle types by id.
    pub vtypes: BTreeMap<String, VType>,
    /// Vehicles in departure order (then id).
    pub vehicles: Vec<RouteVehicle>,
    /// How every attribute was used.
    pub ledger: ImportLedger,
    /// The import seed: also the seed of the per-type car-following models' keyed noise.
    pub seed: u64,
}

fn class_of(name: &str) -> Option<VehicleClass> {
    serde_json::from_value(serde_json::Value::String(name.to_string())).ok()
}

/// The SUMO vClass name of `class`.
pub fn sumo_class(class: VehicleClass) -> String {
    serde_json::to_value(class)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "passenger".to_string())
}

/// FNV-1a over `s`: the stable entity key of a vehicle or flow id.
fn key_of(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The attributes of an element whose use is how SUMO defines it.
fn applied_attrs(element: &str) -> &'static [&'static str] {
    match element {
        "vType" => &[
            "id",
            "vClass",
            "maxSpeed",
            "accel",
            "decel",
            "tau",
            "minGap",
            "sigma",
            "speedFactor",
            "speedDev",
            "probability",
            "carFollowModel",
        ],
        "vTypeDistribution" => &["id", "vTypes", "probabilities"],
        "routeDistribution" => &["id", "routes", "probabilities"],
        "route" => &["id", "edges", "probability"],
        "vehicle" | "trip" => &[
            "id",
            "type",
            "depart",
            "route",
            "from",
            "to",
            "via",
            "departLane",
            "departPos",
            "departSpeed",
            "speedFactor",
        ],
        "flow" => &[
            "id",
            "type",
            "begin",
            "end",
            "route",
            "from",
            "to",
            "via",
            "departLane",
            "departPos",
            "departSpeed",
            "speedFactor",
            "vehsPerHour",
            "period",
            "number",
            "probability",
        ],
        _ => &[],
    }
}

/// Attributes that change nothing on the road.
fn is_cosmetic(attr: &str) -> bool {
    matches!(
        attr,
        "color" | "guiShape" | "imgFile" | "osgFile" | "width_gui" | "name"
    ) || attr.starts_with("xmlns")
        || attr.starts_with("xsi:")
}

/// The elements whose attributes the importer reads.
const READ_ELEMENTS: [&str; 8] = [
    "routes",
    "vType",
    "vTypeDistribution",
    "routeDistribution",
    "route",
    "vehicle",
    "trip",
    "flow",
];

/// A distribution of vehicle types or routes: members and weights.
#[derive(Debug, Clone, Default)]
struct Dist {
    members: Vec<(String, f64)>,
}

impl Dist {
    fn pick(&self, u: f64) -> Option<&str> {
        let total: f64 = self.members.iter().map(|(_, w)| w.max(0.0)).sum();
        if total <= 0.0 {
            return self.members.first().map(|(m, _)| m.as_str());
        }
        let mut acc = 0.0;
        let target = u * total;
        for (m, w) in &self.members {
            acc += w.max(0.0);
            if target < acc {
                return Some(m);
            }
        }
        self.members.last().map(|(m, _)| m.as_str())
    }
}

/// An element's attributes before expansion.
type Attrs = BTreeMap<String, String>;

/// The draws one vehicle takes, in a fixed order so a value never shifts another.
struct VehicleDraws {
    type_u: f64,
    route_u: f64,
    lane_u: f64,
    pos_u: f64,
    speed_u: f64,
    speed_z: [f64; 8],
}

impl VehicleDraws {
    fn of(rng: &RngRegistry, id: &str) -> Self {
        let mut s = rng.ephemeral(RngDomain::Spawn, EntityRef::custom(MODEL_ID, key_of(id)));
        let type_u = s.f64();
        let route_u = s.f64();
        let lane_u = s.f64();
        let pos_u = s.f64();
        let speed_u = s.f64();
        let mut speed_z = [0.0; 8];
        for z in &mut speed_z {
            *z = s.normal(0.0, 1.0);
        }
        Self {
            type_u,
            route_u,
            lane_u,
            pos_u,
            speed_u,
            speed_z,
        }
    }
}

struct Parser {
    out: RouteFile,
    opts: ImportOptions,
    rng: RngRegistry,
    routes: BTreeMap<String, Vec<String>>,
    type_dists: BTreeMap<String, Dist>,
    route_dists: BTreeMap<String, Dist>,
    /// While set, keyword uses are not counted (a flow's vehicles after the flow).
    quiet: bool,
}

impl Parser {
    /// Counts every attribute of `element` (applied, cosmetic or not modelled); the
    /// keyword approximations are moved across by [`Parser::approx`].
    fn count(&mut self, element: &str, all: &Attrs) {
        let applied = applied_attrs(element);
        for k in all.keys() {
            let key = format!("{element}@{k}");
            let l = &mut self.out.ledger;
            if is_cosmetic(k) {
                ImportLedger::bump(&mut l.cosmetic, key);
            } else if applied.contains(&k.as_str()) {
                ImportLedger::bump(&mut l.applied, key);
            } else {
                ImportLedger::bump(&mut l.not_modelled, key);
            }
        }
    }

    /// Moves one count of `element@attr` from applied to approximated, under its value.
    fn approx(&mut self, element: &str, attr: &str, value: &str) {
        if self.quiet {
            return;
        }
        let l = &mut self.out.ledger;
        let key = format!("{element}@{attr}");
        if let Some(n) = l.applied.get_mut(&key) {
            *n -= 1;
            if *n == 0 {
                l.applied.remove(&key);
            }
        }
        ImportLedger::bump(&mut l.approximated, format!("{key}={value}"));
    }

    /// Moves one count of `element@attr` from applied to not modelled, under its value.
    fn unsupported(&mut self, element: &str, attr: &str, value: &str) {
        if self.quiet {
            return;
        }
        let l = &mut self.out.ledger;
        let key = format!("{element}@{attr}");
        if let Some(n) = l.applied.get_mut(&key) {
            *n -= 1;
            if *n == 0 {
                l.applied.remove(&key);
            }
        }
        ImportLedger::bump(&mut l.not_modelled, format!("{key}={value}"));
    }

    fn vtype(&mut self, all: &Attrs, dist: Option<&str>) {
        let id = all.get("id").cloned().unwrap_or_default();
        let class = all
            .get("vClass")
            .and_then(|c| class_of(c))
            .unwrap_or_default();
        if let Some(c) = all.get("vClass")
            && class_of(c).is_none()
        {
            self.approx("vType", "vClass", c);
        }
        let num = |k: &str| all.get(k).and_then(|s| s.trim().parse::<f64>().ok());
        let max_speed = match (num("maxSpeed"), num("desiredMaxSpeed")) {
            (Some(m), Some(d)) => Some(m.min(d)),
            (m, d) => m.or(d),
        };
        if let Some(d) = all.get("desiredMaxSpeed") {
            // SUMO scales desiredMaxSpeed by the speedFactor; here it caps the desired
            // speed the way maxSpeed does.
            let key = "vType@desiredMaxSpeed".to_string();
            let l = &mut self.out.ledger;
            if let Some(n) = l.not_modelled.get_mut(&key) {
                *n -= 1;
                if *n == 0 {
                    l.not_modelled.remove(&key);
                }
            }
            ImportLedger::bump(&mut l.approximated, format!("{key}={d}"));
        }
        let dev = num("speedDev").unwrap_or_else(|| class.spec().speed_dev);
        let speed_factor = match all.get("speedFactor") {
            Some(s) => {
                let law = SpeedFactorLaw::parse(s, dev);
                if law.is_none() {
                    self.unsupported("vType", "speedFactor", s);
                }
                law
            }
            None if all.contains_key("speedDev") => Some(SpeedFactorLaw {
                mean: 1.0,
                dev,
                min: SpeedFactorLaw::MIN,
                max: SpeedFactorLaw::MAX,
            }),
            None => None,
        };
        let car_follow_model = all.get("carFollowModel").cloned();
        // Applied when the driving library has the model and it needs no cooperative link
        // (`v2xw_mobility::carfollowing::library::build_one`); otherwise not modelled.
        if let Some(m) = &car_follow_model
            && applied_model(m).is_none()
        {
            self.unsupported("vType", "carFollowModel", m);
        }
        let probability = num("probability");
        if let Some(d) = dist {
            self.type_dists
                .entry(d.to_string())
                .or_default()
                .members
                .push((id.clone(), probability.unwrap_or(1.0)));
        }
        self.out.vtypes.insert(
            id.clone(),
            VType {
                id,
                class,
                max_speed_mps: max_speed,
                accel_mps2: num("accel"),
                decel_mps2: num("decel"),
                tau_s: num("tau"),
                min_gap_m: num("minGap"),
                sigma: num("sigma"),
                speed_factor,
                car_follow_model,
                probability,
            },
        );
    }

    fn depart_lane(&mut self, el: &str, all: &Attrs, u: f64) -> DepartLane {
        let Some(s) = all.get("departLane") else {
            return DepartLane::First;
        };
        if let Ok(i) = s.trim().parse::<u32>() {
            return DepartLane::Index(i);
        }
        match s.as_str() {
            "first" => DepartLane::First,
            "random" => DepartLane::Random(u),
            "best" => DepartLane::Best,
            // `free` and `allowed` pick the emptiest lane at the instant of insertion,
            // which a demand model does not see.
            other => {
                self.approx(el, "departLane", other);
                DepartLane::First
            }
        }
    }

    fn depart_pos(&mut self, el: &str, all: &Attrs, u: f64) -> DepartPos {
        let Some(s) = all.get("departPos") else {
            return DepartPos::Base;
        };
        if let Ok(x) = s.trim().parse::<f64>() {
            return DepartPos::Metres(x);
        }
        match s.as_str() {
            "base" => DepartPos::Base,
            "random" => DepartPos::Random(u),
            // `random_free` is random with up to 100 retries for free room: the engine's
            // insertion check refuses an occupied position instead.
            "random_free" => {
                self.approx(el, "departPos", "random_free");
                DepartPos::Random(u)
            }
            other => {
                self.approx(el, "departPos", other);
                DepartPos::Base
            }
        }
    }

    fn depart_speed(&mut self, el: &str, all: &Attrs, u: f64) -> Option<DepartSpeed> {
        let s = all.get("departSpeed")?;
        if let Ok(v) = s.trim().parse::<f64>() {
            return Some(DepartSpeed::Mps(v));
        }
        match s.as_str() {
            "max" => Some(DepartSpeed::Max),
            "desired" => Some(DepartSpeed::Desired),
            "speedLimit" => Some(DepartSpeed::SpeedLimit),
            "random" => Some(DepartSpeed::Random(u)),
            // `avg` and `last` read the speeds of the vehicles already on the lane.
            other => {
                self.approx(el, "departSpeed", other);
                Some(DepartSpeed::Max)
            }
        }
    }

    /// The vehicle `id` of element `el` (a `vehicle`, `trip` or one of a flow's), with its
    /// draws taken. `explicit_route` is a nested `<route>`.
    #[allow(clippy::too_many_arguments)]
    fn vehicle(
        &mut self,
        el: &str,
        all: &Attrs,
        id: String,
        depart_s: f64,
        explicit_route: &[String],
        count_keywords: bool,
    ) -> RouteVehicle {
        let d = VehicleDraws::of(&self.rng, &id);
        let vtype = all.get("type").map(|t| {
            self.type_dists
                .get(t)
                .and_then(|dist| dist.pick(d.type_u))
                .map_or_else(|| t.clone(), str::to_string)
        });
        let (edges, routed) = if !explicit_route.is_empty() {
            (explicit_route.to_vec(), true)
        } else if let Some(r) = all.get("route") {
            let name = self
                .route_dists
                .get(r)
                .and_then(|dist| dist.pick(d.route_u))
                .map_or_else(|| r.clone(), str::to_string);
            (self.routes.get(&name).cloned().unwrap_or_default(), true)
        } else {
            let mut e = Vec::new();
            if let Some(x) = all.get("from") {
                e.push(x.clone());
            }
            if let Some(v) = all.get("via") {
                e.extend(v.split_whitespace().map(str::to_string));
            }
            if let Some(x) = all.get("to") {
                e.push(x.clone());
            }
            (e, false)
        };
        // A flow's keywords are counted once for the flow, not once per vehicle.
        self.quiet = !count_keywords;
        let depart_lane = self.depart_lane(el, all, d.lane_u);
        let depart_pos = self.depart_pos(el, all, d.pos_u);
        let depart_speed = self.depart_speed(el, all, d.speed_u);
        let own = all
            .get("speedFactor")
            .and_then(|s| s.trim().parse::<f64>().ok());
        if let Some(s) = all.get("speedFactor")
            && own.is_none()
        {
            self.unsupported(el, "speedFactor", s);
        }
        self.quiet = false;
        let law = vtype
            .as_ref()
            .and_then(|t| self.out.vtypes.get(t))
            .and_then(|t| t.speed_factor);
        let speed_factor = match (own, law) {
            (Some(f), _) => Some((f, f, 0.0)),
            (None, Some(law)) => Some((law.draw(&d.speed_z), law.mean, law.dev)),
            (None, None) => None,
        };
        RouteVehicle {
            id,
            depart_s,
            vtype,
            edges,
            routed,
            depart_lane,
            depart_pos,
            depart_speed,
            speed_factor,
        }
    }

    fn expand_flow(&mut self, f: &Attrs, nested: &[String]) {
        let id = f.get("id").cloned().unwrap_or_default();
        let num = |k: &str| f.get(k).and_then(|s| s.trim().parse::<f64>().ok());
        let begin = num("begin").unwrap_or(0.0);
        let end = num("end").unwrap_or(3600.0);
        let cap = f
            .get("number")
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(u64::MAX);
        // Count the flow's own keyword uses once.
        let probe = VehicleDraws::of(&self.rng, &id);
        let _ = self.depart_lane("flow", f, probe.lane_u);
        let _ = self.depart_pos("flow", f, probe.pos_u);
        let _ = self.depart_speed("flow", f, probe.speed_u);
        if let Some(s) = f.get("speedFactor")
            && s.trim().parse::<f64>().is_err()
        {
            self.unsupported("flow", "speedFactor", s);
        }
        let mut arrivals = self.rng.ephemeral(
            RngDomain::Spawn,
            EntityRef::custom(FLOW_STREAM_ID, key_of(&id)),
        );
        let times: Vec<f64> = if let Some(v) = num("vehsPerHour") {
            spaced(begin, end, 3600.0 / v.max(1e-9), cap)
        } else if let Some(p) = f.get("period") {
            let p = p.trim();
            if let Some(rate) = p
                .strip_prefix("exp(")
                .and_then(|r| r.strip_suffix(')'))
                .and_then(|r| r.trim().parse::<f64>().ok())
            {
                poisson(begin, end, rate, cap, &mut arrivals)
            } else if let Ok(period) = p.parse::<f64>() {
                spaced(begin, end, period, cap)
            } else {
                self.unsupported("flow", "period", p);
                Vec::new()
            }
        } else if let Some(p) = num("probability") {
            match self.opts.probability_flows {
                ProbabilityFlows::Bernoulli => bernoulli(begin, end, p, cap, &mut arrivals),
                ProbabilityFlows::EqualSpacing => {
                    self.approx("flow", "probability", "equal-spacing");
                    spaced(begin, end, 1.0 / p.max(1e-9), cap)
                }
            }
        } else if let Some(n) = num("number") {
            spaced(begin, end, (end - begin) / n.max(1.0), cap)
        } else {
            Vec::new()
        };
        for (n, t) in times.into_iter().enumerate() {
            let v = self.vehicle("flow", f, format!("{id}.{n}"), t, nested, false);
            self.out.vehicles.push(v);
        }
    }
}

/// Departures every `period` seconds from `begin`, before `end`, at most `cap`.
fn spaced(begin: f64, end: f64, period: f64, cap: u64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut n = 0u64;
    let mut t = begin;
    while t < end && n < cap && period > 0.0 {
        out.push(t);
        n += 1;
        t = begin + n as f64 * period;
    }
    out
}

/// One Bernoulli trial of probability `p` every second from `begin` (SUMO's
/// `probability`), before `end`, at most `cap` departures.
fn bernoulli(begin: f64, end: f64, p: f64, cap: u64, rng: &mut RngStream) -> Vec<f64> {
    let mut out = Vec::new();
    let mut k = 0u64;
    loop {
        let t = begin + k as f64;
        if t >= end || out.len() as u64 >= cap {
            break;
        }
        if rng.f64() < p {
            out.push(t);
        }
        k += 1;
    }
    out
}

/// Exponential headways of rate `rate` per second from `begin` (`period="exp(rate)"`).
fn poisson(begin: f64, end: f64, rate: f64, cap: u64, rng: &mut RngStream) -> Vec<f64> {
    let mut out = Vec::new();
    if rate <= 0.0 {
        return out;
    }
    let mut t = begin + rng.exponential(rate);
    while t < end && (out.len() as u64) < cap {
        out.push(t);
        t += rng.exponential(rate);
    }
    out
}

impl RouteFile {
    /// Parses a `.rou.xml` document with the default options (seed 0, Bernoulli
    /// `probability` flows).
    ///
    /// # Errors
    /// Malformed XML.
    pub fn parse(xml: &str) -> Result<Self, String> {
        Self::parse_with(xml, &ImportOptions::default())
    }

    /// Parses a `.rou.xml` document.
    ///
    /// # Errors
    /// Malformed XML.
    pub fn parse_with(xml: &str, opts: &ImportOptions) -> Result<Self, String> {
        let mut reader = quick_xml::Reader::from_str(xml);
        let mut p = Parser {
            out: RouteFile {
                seed: opts.seed,
                ..RouteFile::default()
            },
            opts: *opts,
            rng: RngRegistry::new(opts.seed),
            routes: BTreeMap::new(),
            type_dists: BTreeMap::new(),
            route_dists: BTreeMap::new(),
            quiet: false,
        };
        // Elements in document order, expanded once the whole file is read (a vehicle may
        // name a type or route defined after it).
        enum Item {
            Vehicle(String, Attrs, Vec<String>),
            Flow(Attrs, Vec<String>),
        }
        let mut items: Vec<Item> = Vec::new();
        // The open element stack, and the open vehicle/flow waiting for a nested route.
        let mut stack: Vec<String> = Vec::new();
        let mut open: Option<(String, Attrs, Vec<String>)> = None;
        let mut type_dist: Option<String> = None;
        let mut route_dist: Option<String> = None;
        loop {
            let ev = reader.read_event().map_err(|e| format!("rou.xml: {e}"))?;
            match ev {
                quick_xml::events::Event::Eof => break,
                // `<?xml version encoding standalone?>`: the document's own encoding,
                // counted with the display-only attributes.
                quick_xml::events::Event::Decl(ref d) => {
                    let l = &mut p.out.ledger;
                    if d.version().is_ok() {
                        ImportLedger::bump(&mut l.cosmetic, "xml@version".to_string());
                    }
                    if d.encoding().is_some() {
                        ImportLedger::bump(&mut l.cosmetic, "xml@encoding".to_string());
                    }
                    if d.standalone().is_some() {
                        ImportLedger::bump(&mut l.cosmetic, "xml@standalone".to_string());
                    }
                }
                quick_xml::events::Event::Start(ref e) | quick_xml::events::Event::Empty(ref e) => {
                    let empty = matches!(ev, quick_xml::events::Event::Empty(_));
                    let name = e.name().into_inner().to_string();
                    let all: Attrs = e
                        .attributes()
                        .flatten()
                        .map(|a| (a.key.into_inner().to_string(), a.value.to_string()))
                        .collect();
                    let parent = stack.last().cloned().unwrap_or_default();
                    if !empty {
                        stack.push(name.clone());
                    }
                    if !READ_ELEMENTS.contains(&name.as_str()) {
                        // `<param>`, `<stop>`, `<person>`, `<carFollowing-*>`, …: counted
                        // under the element (and a param under its key).
                        ImportLedger::bump(&mut p.out.ledger.elements_not_modelled, name.clone());
                        for k in all.keys() {
                            let key = if name == "param" {
                                format!(
                                    "{parent}/param[{}]@{k}",
                                    all.get("key").cloned().unwrap_or_default()
                                )
                            } else {
                                format!("{name}@{k}")
                            };
                            ImportLedger::bump(&mut p.out.ledger.not_modelled, key);
                        }
                        continue;
                    }
                    p.count(&name, &all);
                    match name.as_str() {
                        "vTypeDistribution" => {
                            let id = all.get("id").cloned().unwrap_or_default();
                            if let Some(list) = all.get("vTypes") {
                                let probs: Vec<f64> = all
                                    .get("probabilities")
                                    .map(|s| {
                                        s.split_whitespace()
                                            .filter_map(|x| x.parse().ok())
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let d = p.type_dists.entry(id.clone()).or_default();
                                for (i, m) in list.split_whitespace().enumerate() {
                                    d.members.push((
                                        m.to_string(),
                                        probs.get(i).copied().unwrap_or(1.0),
                                    ));
                                }
                            }
                            if !empty {
                                type_dist = Some(id);
                            }
                        }
                        "routeDistribution" => {
                            let id = all.get("id").cloned().unwrap_or_default();
                            if let Some(list) = all.get("routes") {
                                let probs: Vec<f64> = all
                                    .get("probabilities")
                                    .map(|s| {
                                        s.split_whitespace()
                                            .filter_map(|x| x.parse().ok())
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let d = p.route_dists.entry(id.clone()).or_default();
                                for (i, m) in list.split_whitespace().enumerate() {
                                    d.members.push((
                                        m.to_string(),
                                        probs.get(i).copied().unwrap_or(1.0),
                                    ));
                                }
                            }
                            if !empty {
                                route_dist = Some(id);
                            }
                        }
                        "vType" => {
                            let dist = type_dist.clone();
                            p.vtype(&all, dist.as_deref());
                        }
                        "route" => {
                            let edges: Vec<String> = all
                                .get("edges")
                                .map(|s| s.split_whitespace().map(str::to_string).collect())
                                .unwrap_or_default();
                            if let Some((_, _, nested)) = open.as_mut() {
                                *nested = edges;
                            } else {
                                let id = all.get("id").cloned().unwrap_or_else(|| {
                                    format!(
                                        "{}#{}",
                                        route_dist.clone().unwrap_or_default(),
                                        p.routes.len()
                                    )
                                });
                                if let Some(d) = route_dist.as_ref() {
                                    let w = all
                                        .get("probability")
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(1.0);
                                    p.route_dists
                                        .entry(d.clone())
                                        .or_default()
                                        .members
                                        .push((id.clone(), w));
                                }
                                p.routes.insert(id, edges);
                            }
                        }
                        "vehicle" | "trip" | "flow" => {
                            if empty {
                                items.push(if name == "flow" {
                                    Item::Flow(all, Vec::new())
                                } else {
                                    Item::Vehicle(name, all, Vec::new())
                                });
                            } else {
                                open = Some((name, all, Vec::new()));
                            }
                        }
                        _ => {}
                    }
                }
                quick_xml::events::Event::End(ref e) => {
                    let name = e.name().into_inner();
                    stack.pop();
                    match name {
                        "vehicle" | "trip" | "flow" => {
                            if let Some((n, all, nested)) = open.take() {
                                items.push(if n == "flow" {
                                    Item::Flow(all, nested)
                                } else {
                                    Item::Vehicle(n, all, nested)
                                });
                            }
                        }
                        "vTypeDistribution" => type_dist = None,
                        "routeDistribution" => route_dist = None,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        for item in items {
            match item {
                Item::Vehicle(el, all, nested) => {
                    let id = all.get("id").cloned().unwrap_or_default();
                    let depart_s = match all.get("depart") {
                        None => 0.0,
                        Some(s) => s.trim().parse().unwrap_or_else(|_| {
                            // `triggered`, `containerTriggered`, `now`, `begin`.
                            p.approx(&el, "depart", s);
                            0.0
                        }),
                    };
                    let v = p.vehicle(&el, &all, id, depart_s, &nested, true);
                    p.out.vehicles.push(v);
                }
                Item::Flow(all, nested) => p.expand_flow(&all, &nested),
            }
        }
        let mut out = p.out;
        out.vehicles.sort_by(|a, b| {
            a.depart_s
                .total_cmp(&b.depart_s)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(out)
    }

    /// Reads and parses a file with the default options.
    ///
    /// # Errors
    /// I/O or malformed XML.
    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        Self::read_with(path, &ImportOptions::default())
    }

    /// Reads and parses a file.
    ///
    /// # Errors
    /// I/O or malformed XML.
    pub fn read_with(path: &std::path::Path, opts: &ImportOptions) -> Result<Self, String> {
        let xml = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse_with(&xml, opts)
    }

    /// The vehicle type `v` drives, as the engine applies it.
    pub fn trip_driver(&self, v: &RouteVehicle) -> TripDriver {
        let t = v.vtype.as_ref().and_then(|id| self.vtypes.get(id));
        TripDriver {
            max_accel_mps2: t.and_then(|t| t.accel_mps2),
            comfort_decel_mps2: t.and_then(|t| t.decel_mps2),
            time_headway_s: t.and_then(|t| t.tau_s),
            min_gap_m: t.and_then(|t| t.min_gap_m),
            sigma: t.and_then(|t| t.sigma),
            speed_factor: v.speed_factor,
            depart_speed: v.depart_speed,
            car_following: t
                .and_then(|t| t.car_follow_model.as_deref())
                .and_then(applied_model),
            model_seed: self.seed,
        }
    }
}

/// Restricts routing to a set of SUMO edges plus every junction connector.
struct EdgeSetCost<'a> {
    inner: FreeFlowCost<'a>,
    world: &'a World,
    allowed: std::collections::BTreeSet<v2xw_core::ids::EdgeId>,
}

impl EdgeCost for EdgeSetCost<'_> {
    fn lane_cost_s(&self, lane: LaneId, at: SimTime) -> Option<f64> {
        let l = self.world.lane(lane);
        if l.junction.is_none() && !self.allowed.contains(&l.edge) {
            return None;
        }
        self.inner.lane_cost_s(lane, at)
    }
}

/// The world-lane route from `origin` along exactly `edges` (SUMO ids), or `None`.
pub fn lanes_along_edges(
    world: &World,
    ids: &SumoIds,
    class: VehicleClass,
    origin: LaneId,
    edges: &[String],
) -> Option<Vec<LaneId>> {
    let last = edges.last()?;
    let mask = class.class_mask();
    let dest = *ids
        .lanes_of
        .get(last)?
        .iter()
        .find(|l| world.lane(**l).admits(mask))?;
    let allowed = edges
        .iter()
        .filter_map(|e| ids.lanes_of.get(e).and_then(|ls| ls.first()))
        .map(|l| world.lane(*l).edge)
        .collect();
    let cost = EdgeSetCost {
        inner: FreeFlowCost::new(world),
        world,
        allowed,
    };
    Dijkstra::new(DijkstraParams::for_classes(mask))
        .search(world, origin, dest, 0, &cost)
        .map(|r| r.lanes)
}

/// The number of lane changes along `lanes`: consecutive lanes not joined by a connection.
fn lane_changes(world: &World, lanes: &[LaneId]) -> usize {
    lanes
        .windows(2)
        .filter(|w| {
            !world
                .successors(w[0])
                .iter()
                .any(|c| c.to_lane == w[1] || c.via == Some(w[1]))
        })
        .count()
}

/// The demand a `.rou.xml` states, as this engine's [`Demand`].
pub struct RouteFileDemand {
    card: ModelCard,
    file: RouteFile,
    cursor: usize,
    ids: Option<SumoIds>,
    planned: BTreeMap<u64, Vec<String>>,
    drivers: BTreeMap<u64, TripDriver>,
    /// Vehicles whose edges this world does not have.
    pub unresolved: u64,
}

impl RouteFileDemand {
    /// Demand from a parsed file.
    pub fn new(file: RouteFile) -> Self {
        let mut card = ModelCard::new(
            MODEL_ID,
            Family::Mobility,
            "2.0.0",
            "The vehicles of a SUMO .rou.xml (vehicles, trips, flows, vType and route \
             distributions) as trips of this engine, each driven by its own vType; a \
             vehicle's own route is kept through Demand::planned_route. Re-implemented from \
             the SUMO documentation (Definition of Vehicles, Vehicle Types, and Routes; \
             Vehicle speed; Randomness). SUMO was read for behaviour only and no code was \
             copied.",
        );
        card.limitations.push(
            "A vehicle type's body size (length, width, height), emergencyDecel, \
             apparentDecel, laneChangeModel and lc*/jm* parameters and actionStepLength are \
             counted in the import ledger and not applied: the vClass's body and the run's \
             own lane-change model are used. A type's carFollowModel is driven per vehicle \
             at the library model's cited defaults under the type's own accel, decel, tau, \
             minGap and sigma; CACC (cooperative) and unknown models are not modelled and \
             drive the run's model."
                .to_string(),
        );
        card.limitations.push(
            "departLane free/allowed, departPos free/last/stop and departSpeed avg/last \
             need the occupancy at the instant of insertion: they fall back to first, base \
             and max and are counted as approximations. A stated departSpeed (a number, desired \
             or speedLimit) above the safe insertion speed delays the insertion, as SUMO \
             does; max and random are lowered to it."
                .to_string(),
        );
        card.limitations.push(
            "Stops, persons, containers and arrivalLane/Pos/Speed are counted and not \
             modelled."
                .to_string(),
        );
        Self {
            card,
            file,
            cursor: 0,
            ids: None,
            planned: BTreeMap::new(),
            drivers: BTreeMap::new(),
            unresolved: 0,
        }
    }

    /// The parsed file.
    pub fn file(&self) -> &RouteFile {
        &self.file
    }
}

impl Model for RouteFileDemand {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Demand for RouteFileDemand {
    fn spawns_in(&mut self, ctx: &mut dyn MobCtx, from: SimTime, to: SimTime) -> Vec<TripRequest> {
        let world = ctx.world();
        let ids = self.ids.get_or_insert_with(|| SumoIds::new(world));
        let mut out = Vec::new();
        while let Some(v) = self.file.vehicles.get(self.cursor) {
            let t = (v.depart_s * 1e9).round() as SimTime;
            if t >= to {
                break;
            }
            self.cursor += 1;
            if t < from && from > 0 {
                continue;
            }
            let vt = v.vtype.as_ref().and_then(|id| self.file.vtypes.get(id));
            let class = vt.map(|t| t.class).unwrap_or_default();
            let mask = class.class_mask();
            let lanes_of = |e: &String| ids.lanes_of.get(e);
            let (Some(first), Some(last)) = (
                v.edges.first().and_then(lanes_of),
                v.edges.last().and_then(lanes_of),
            ) else {
                self.unresolved += 1;
                continue;
            };
            let allowed: Vec<LaneId> = first
                .iter()
                .copied()
                .filter(|l| world.lane(*l).admits(mask))
                .collect();
            let origin = match v.depart_lane {
                DepartLane::Index(i) => first
                    .get(i as usize)
                    .copied()
                    .filter(|l| world.lane(*l).admits(mask))
                    .or_else(|| allowed.first().copied()),
                DepartLane::First => allowed.first().copied(),
                DepartLane::Random(u) => {
                    let n = allowed.len();
                    (n > 0).then(|| allowed[((u * n as f64) as usize).min(n - 1)])
                }
                DepartLane::Best if v.routed => allowed
                    .iter()
                    .filter_map(|l| {
                        lanes_along_edges(world, ids, class, *l, &v.edges)
                            .map(|r| (lane_changes(world, &r), *l))
                    })
                    .min_by_key(|(n, _)| *n)
                    .map(|(_, l)| l)
                    .or_else(|| allowed.first().copied()),
                DepartLane::Best => allowed.first().copied(),
            };
            let destination = last.iter().copied().find(|l| world.lane(*l).admits(mask));
            let (Some(origin), Some(destination)) = (origin, destination) else {
                self.unresolved += 1;
                continue;
            };
            let seq = self.cursor as u64 - 1;
            if v.routed {
                self.planned.insert(seq, v.edges.clone());
            }
            self.drivers.insert(seq, self.file.trip_driver(v));
            let spec = class.spec();
            let len = world.lane(origin).length_m;
            let pos = match v.depart_pos {
                DepartPos::Metres(x) if x < 0.0 => len + x,
                DepartPos::Metres(x) => x,
                DepartPos::Base => 0.0,
                DepartPos::Random(u) => u * len,
            };
            out.push(TripRequest {
                seq,
                t: t.max(from),
                origin,
                origin_s_m: pos.clamp(0.0, len),
                destination,
                class,
                desired_speed_mps: vt
                    .and_then(|t| t.max_speed_mps)
                    .unwrap_or(spec.desired_speed_mps()),
            });
        }
        out
    }

    fn planned_route(&self, world: &World, trip: &TripRequest) -> Option<Vec<LaneId>> {
        let edges = self.planned.get(&trip.seq)?;
        lanes_along_edges(world, self.ids.as_ref()?, trip.class, trip.origin, edges)
    }

    fn trip_driver(&self, trip: &TripRequest) -> Option<TripDriver> {
        self.drivers.get(&trip.seq).copied()
    }
}

/// Writes trips as a `.rou.xml`: one `<vehicle>` with its full `<route>` when `edges` has
/// more than the endpoints' worth, else a `<trip>`. Times in seconds with millisecond
/// precision; one `<vType>` per class used, with SUMO's defaults for that vClass.
pub fn write_rou_xml(trips: &[(TripRequest, Vec<String>)], ids: &SumoIds, world: &World) -> String {
    use std::fmt::Write as _;
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- written by v2xw-interop -->\n<routes>\n",
    );
    let mut classes: Vec<VehicleClass> = trips.iter().map(|(t, _)| t.class).collect();
    classes.sort();
    classes.dedup();
    for c in &classes {
        let n = sumo_class(*c);
        let _ = writeln!(s, "  <vType id=\"v2xw_{n}\" vClass=\"{n}\"/>");
    }
    for (t, edges) in trips {
        let lane_index = world.lane(t.origin).index;
        let depart = t.t as f64 / 1e9;
        let ty = format!("v2xw_{}", sumo_class(t.class));
        let common = format!(
            "id=\"t{}\" type=\"{ty}\" depart=\"{depart:.3}\" departLane=\"{lane_index}\" departPos=\"{:.2}\" departSpeed=\"max\"",
            t.seq, t.origin_s_m
        );
        if !edges.is_empty() {
            let _ = writeln!(
                s,
                "  <vehicle {common}>\n    <route edges=\"{}\"/>\n  </vehicle>",
                edges.join(" ")
            );
        } else {
            match (
                ids.edge_of_lane(world, t.origin),
                ids.edge_of_lane(world, t.destination),
            ) {
                (Some(from), Some(to)) => {
                    let _ = writeln!(s, "  <trip {common} from=\"{from}\" to=\"{to}\"/>");
                }
                // An endpoint inside a junction has no SUMO edge: said, not written.
                _ => {
                    let _ = writeln!(
                        s,
                        "  <!-- trip {} left out: an endpoint is not on a SUMO edge -->",
                        t.seq
                    );
                }
            }
        }
    }
    s.push_str("</routes>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vehicles_trips_and_flows_parse() {
        let xml = r#"<routes>
  <vType id="car" vClass="passenger" maxSpeed="13.9"/>
  <route id="r0" edges="a b c"/>
  <vehicle id="v0" type="car" depart="5" route="r0" departLane="1"/>
  <vehicle id="v1" depart="1"><route edges="a b"/></vehicle>
  <trip id="t0" depart="2.5" from="a" to="c" departPos="random_free"/>
  <flow id="f" begin="0" end="10" vehsPerHour="1800" from="a" to="b"/>
</routes>"#;
        let f = RouteFile::parse(xml).unwrap();
        assert_eq!(f.vtypes["car"].max_speed_mps, Some(13.9));
        // 1800 veh/h over 10 s = one every 2 s: 0, 2, 4, 6, 8.
        assert_eq!(
            f.vehicles.iter().filter(|v| v.id.starts_with("f.")).count(),
            5
        );
        let v0 = f.vehicles.iter().find(|v| v.id == "v0").unwrap();
        assert_eq!(v0.edges, vec!["a", "b", "c"]);
        assert!(v0.routed);
        assert_eq!(v0.lane_index(), Some(1));
        let v1 = f.vehicles.iter().find(|v| v.id == "v1").unwrap();
        assert_eq!(v1.edges, vec!["a", "b"]);
        let t0 = f.vehicles.iter().find(|v| v.id == "t0").unwrap();
        assert!(!t0.routed);
        assert!(matches!(t0.depart_pos, DepartPos::Random(_)));
        assert_eq!(
            f.ledger.approximated.get("trip@departPos=random_free"),
            Some(&1)
        );
        // Sorted by departure.
        assert!(
            f.vehicles
                .windows(2)
                .all(|w| w[0].depart_s <= w[1].depart_s)
        );
    }

    /// Every driver attribute of a vType reaches the trip's [`TripDriver`].
    #[test]
    fn vtype_attributes_reach_the_trip_driver() {
        let xml = r#"<routes>
  <vType id="t" vClass="passenger" accel="1.1" decel="3.3" tau="1.7" sigma="0.2" minGap="3.1" maxSpeed="20" speedFactor="normc(1.2,0.05,1.0,1.4)"/>
  <vehicle id="v" type="t" depart="0" from="a" to="b" departSpeed="desired" departLane="random" departPos="-10"/>
</routes>"#;
        let f = RouteFile::parse(xml).unwrap();
        let v = &f.vehicles[0];
        let d = f.trip_driver(v);
        assert_eq!(d.max_accel_mps2, Some(1.1));
        assert_eq!(d.comfort_decel_mps2, Some(3.3));
        assert_eq!(d.time_headway_s, Some(1.7));
        assert_eq!(d.sigma, Some(0.2));
        assert_eq!(d.min_gap_m, Some(3.1));
        assert_eq!(d.depart_speed, Some(DepartSpeed::Desired));
        let (sf, mean, dev) = d.speed_factor.unwrap();
        assert!(
            (1.0..=1.4).contains(&sf) && mean == 1.2 && dev == 0.05,
            "{sf}"
        );
        assert!(matches!(v.depart_lane, DepartLane::Random(_)));
        assert_eq!(v.depart_pos, DepartPos::Metres(-10.0));
        assert_eq!(f.vtypes["t"].max_speed_mps, Some(20.0));
        assert!(f.ledger.not_modelled.is_empty(), "{:?}", f.ledger);
        assert!(f.ledger.approximated.is_empty(), "{:?}", f.ledger);
    }

    /// The count of a `probability` flow is binomial: over n one-second trials its mean
    /// is n·p and its variance n·p·(1−p) (SUMO's Bernoulli definition).
    #[test]
    fn a_probability_flow_keeps_the_bernoulli_variance() {
        let (n, p) = (200.0, 0.3);
        let xml = format!(
            r#"<routes><flow id="f" begin="0" end="{n}" probability="{p}" from="a" to="b"/></routes>"#
        );
        let counts: Vec<f64> = (0..400u64)
            .map(|seed| {
                let o = ImportOptions {
                    seed,
                    ..ImportOptions::default()
                };
                RouteFile::parse_with(&xml, &o).unwrap().vehicles.len() as f64
            })
            .collect();
        let m = counts.iter().sum::<f64>() / counts.len() as f64;
        let var = counts.iter().map(|c| (c - m) * (c - m)).sum::<f64>() / (counts.len() - 1) as f64;
        let want = n * p * (1.0 - p);
        // 400 samples: the variance estimate's standard error is about 7 %.
        assert!((m - n * p).abs() < 1.0, "mean {m}");
        assert!(
            (var / want - 1.0).abs() < 0.2,
            "variance {var}, n·p·(1−p) = {want}"
        );
        // Every departure is on a whole second from `begin`.
        let one = RouteFile::parse(&xml).unwrap();
        assert!(one.vehicles.iter().all(|v| v.depart_s.fract() == 0.0));
        // The legacy spacing keeps the mean and has no variance at all.
        let eq = ImportOptions {
            probability_flows: ProbabilityFlows::EqualSpacing,
            ..ImportOptions::default()
        };
        let a = RouteFile::parse_with(&xml, &eq).unwrap().vehicles.len();
        let b = RouteFile::parse_with(&xml, &ImportOptions { seed: 9, ..eq })
            .unwrap()
            .vehicles
            .len();
        assert_eq!(a, b);
    }

    /// `period="exp(λ)"` is a Poisson stream of rate λ per second, not a period of λ s.
    #[test]
    fn an_exponential_period_is_a_poisson_stream() {
        let xml = r#"<routes><flow id="f" begin="0" end="20000" period="exp(0.1)" from="a" to="b"/></routes>"#;
        let f = RouteFile::parse(xml).unwrap();
        let n = f.vehicles.len() as f64;
        // 2,000 expected; Poisson standard deviation about 45.
        assert!((n - 2000.0).abs() < 200.0, "{n}");
        let gaps: Vec<f64> = f
            .vehicles
            .windows(2)
            .map(|w| w[1].depart_s - w[0].depart_s)
            .collect();
        let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
        let var = gaps.iter().map(|g| (g - mean) * (g - mean)).sum::<f64>() / gaps.len() as f64;
        // An exponential headway has its standard deviation equal to its mean.
        assert!(
            (var.sqrt() / mean - 1.0).abs() < 0.1,
            "cv {}",
            var.sqrt() / mean
        );
    }

    /// A vehicle naming a `vTypeDistribution` draws its member by the stated
    /// probabilities; a `routeDistribution` likewise.
    #[test]
    fn distributions_are_drawn_per_vehicle() {
        let xml = r#"<routes>
  <vTypeDistribution id="mix">
    <vType id="a" accel="2.0" probability="0.75"/>
    <vType id="b" accel="3.0" probability="0.25"/>
  </vTypeDistribution>
  <vTypeDistribution id="mix2" vTypes="a b" probabilities="1 3"/>
  <routeDistribution id="rd">
    <route id="r1" edges="x y" probability="1"/>
    <route id="r2" edges="x z" probability="1"/>
  </routeDistribution>
  <flow id="f" type="mix" route="rd" begin="0" end="4000" period="1"/>
  <flow id="g" type="mix2" route="rd" begin="0" end="4000" period="1"/>
</routes>"#;
        let f = RouteFile::parse(xml).unwrap();
        let share = |flow: &str, ty: &str| {
            let all: Vec<_> = f
                .vehicles
                .iter()
                .filter(|v| v.id.starts_with(flow))
                .collect();
            all.iter()
                .filter(|v| v.vtype.as_deref() == Some(ty))
                .count() as f64
                / all.len() as f64
        };
        assert!(
            (share("f.", "a") - 0.75).abs() < 0.03,
            "{}",
            share("f.", "a")
        );
        assert!(
            (share("g.", "a") - 0.25).abs() < 0.03,
            "{}",
            share("g.", "a")
        );
        let r1 = f.vehicles.iter().filter(|v| v.edges == ["x", "y"]).count() as f64;
        assert!((r1 / f.vehicles.len() as f64 - 0.5).abs() < 0.03);
        assert!(f.vehicles.iter().all(|v| v.routed && v.edges.len() == 2));
    }

    /// The same seed expands the same file to the same vehicles; another seed does not.
    #[test]
    fn the_expansion_is_deterministic() {
        let xml = r#"<routes>
  <vType id="t" speedDev="0.2"/>
  <flow id="f" type="t" begin="0" end="300" probability="0.2" from="a" to="b" departLane="random" departPos="random" departSpeed="random"/>
</routes>"#;
        let o = |seed| ImportOptions {
            seed,
            ..ImportOptions::default()
        };
        let a = RouteFile::parse_with(xml, &o(7)).unwrap();
        assert_eq!(a, RouteFile::parse_with(xml, &o(7)).unwrap());
        assert_ne!(
            a.vehicles,
            RouteFile::parse_with(xml, &o(8)).unwrap().vehicles
        );
        // A flow's keywords are counted once for the flow, not per vehicle.
        assert_eq!(a.ledger.count("flow@departLane"), 1);
    }

    /// Every attribute of a file is in the ledger: the ledger's total equals an independent
    /// count of the attributes in the text.
    #[test]
    fn no_attribute_is_dropped_silently() {
        let xml = r#"<routes xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <vType id="t" vClass="passenger" length="7" lcStrategic="0.5" color="1,0,0" emergencyDecel="9" carFollowModel="IDM">
    <param key="has.rerouting.device" value="false"/>
  </vType>
  <vehicle id="v" type="t" depart="0" departLane="free" departPos="last" departSpeed="avg" arrivalPos="-1">
    <route edges="a b"/>
    <stop busStop="s" duration="20"/>
  </vehicle>
</routes>"#;
        let f = RouteFile::parse(xml).unwrap();
        let in_text = xml.matches("=\"").count() as u64;
        assert_eq!(f.ledger.total(), in_text, "{:#?}", f.ledger);
        let l = &f.ledger;
        assert_eq!(l.not_modelled.get("vType@length"), Some(&1));
        assert_eq!(l.not_modelled.get("vType@lcStrategic"), Some(&1));
        // WP-15 follow-up: an IDM type is driven by the library's IDM per vehicle.
        assert_eq!(l.applied.get("vType@carFollowModel"), Some(&1));
        assert_eq!(l.not_modelled.get("vType@carFollowModel"), None);
        assert_eq!(
            l.not_modelled
                .get("vType/param[has.rerouting.device]@value"),
            Some(&1)
        );
        assert_eq!(l.not_modelled.get("stop@busStop"), Some(&1));
        assert_eq!(l.approximated.get("vehicle@departLane=free"), Some(&1));
        assert_eq!(l.approximated.get("vehicle@departPos=last"), Some(&1));
        assert_eq!(l.approximated.get("vehicle@departSpeed=avg"), Some(&1));
        assert_eq!(l.cosmetic.get("vType@color"), Some(&1));
        assert_eq!(l.elements_not_modelled.get("stop"), Some(&1));
        assert_eq!(library_model("IDM"), Some("idm"));
        assert_eq!(f.vtypes["t"].car_follow_model.as_deref(), Some("IDM"));
    }

    /// A vType's carFollowModel reaches the vehicle as the library model it maps to; a
    /// cooperative or unknown model is counted not modelled and reaches no vehicle.
    #[test]
    fn a_types_car_following_model_reaches_its_vehicles() {
        let xml = r#"<routes>
  <vType id="i" carFollowModel="IDM"/>
  <vType id="k" carFollowModel="Krauss"/>
  <vType id="c" carFollowModel="CACC"/>
  <vType id="x" carFollowModel="Wiedemann"/>
  <vType id="d"/>
  <vehicle id="vi" type="i" depart="0"><route edges="a b"/></vehicle>
  <vehicle id="vk" type="k" depart="0"><route edges="a b"/></vehicle>
  <vehicle id="vc" type="c" depart="0"><route edges="a b"/></vehicle>
  <vehicle id="vx" type="x" depart="0"><route edges="a b"/></vehicle>
  <vehicle id="vd" type="d" depart="0"><route edges="a b"/></vehicle>
</routes>"#;
        let f = RouteFile::parse(xml).unwrap();
        let model = |id: &str| {
            let v = f.vehicles.iter().find(|v| v.id == id).unwrap();
            f.trip_driver(v).car_following
        };
        assert_eq!(model("vi"), Some("idm"));
        assert_eq!(model("vk"), Some("krauss"));
        assert_eq!(
            model("vc"),
            None,
            "cooperative: the run's own link is needed"
        );
        assert_eq!(model("vx"), None);
        assert_eq!(model("vd"), None);
        let l = &f.ledger;
        assert_eq!(l.applied.get("vType@carFollowModel"), Some(&2));
        assert_eq!(l.not_modelled.get("vType@carFollowModel=CACC"), Some(&1));
        assert_eq!(
            l.not_modelled.get("vType@carFollowModel=Wiedemann"),
            Some(&1)
        );
        assert!(
            v2xw_mobility::carfollowing::library::build_one("idm", 1).is_some()
                && v2xw_mobility::carfollowing::library::build_one("cacc-milanes", 1).is_none()
        );
    }
}
