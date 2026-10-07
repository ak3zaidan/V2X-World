//! The city library: where a scenario runs, chosen separately from what it runs.
//!
//! A scenario states the traffic, the radio, the network, the security and the attacks; a
//! city states the streets. The registry (`worlds/cities.yaml`, compiled into the binary
//! so every interface lists the same cities) names each city's OpenStreetMap extract, its
//! downtown box and its jurisdiction's rules ([`v2xw_world::jurisdiction`]). Three steps
//! make any scenario run sensibly on any city:
//!
//! 1. **The world.** [`Scenario::with_city`] (and, for a file that names a city and no
//!    world, [`fill_world_from_city`]) puts the city's extract, box, highway preset and
//!    fetch date in `world`, keeping the scenario's building and signal options.
//! 2. **What belonged to the old city.** Roadside units at explicit positions become an
//!    [`RsuPlacement`] of the same count and roles; a fixed arrival rate becomes a rate per
//!    kilometre of driving lane (from the registry's measured network size); the clock
//!    keeps its local time of day; a closure of a named street closes a street of the new
//!    city (`auto`); a focus box is dropped.
//! 3. **The built world.** [`adapt_to_world`], run once the world is imported, turns the
//!    per-lane-kilometre rate into vehicles per hour and stands the placed roadside units
//!    at the city's busiest signalised intersections.
//!
//! The extracts live in `worlds/cache/<id>.osm.xml` (git-ignored, ODbL); `worlds/fetch.sh
//! <id>` or `--all` downloads them. `V2XW_WORLDS_DIR` points elsewhere.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use v2xw_world::model::{JunctionControl, LaneKind};
use v2xw_world::{GeoBbox, World, WorldSourceSpec};

use crate::error::{EngineError, ScenarioError};
use crate::scenario::schema::{Rsu, RsuPlacement, Scenario, WorldSpec};

/// The registry, as committed.
pub const REGISTRY_YAML: &str = include_str!("../../../worlds/cities.yaml");

/// The driving-lane length of the network a fixed rate is assumed to have been written
/// for when the scenario's own city is not in the library (Midtown, the scenarios' first
/// city: 119 km, `world_report` on the D7 extract).
pub const REFERENCE_LANE_KM: f64 = 119.0;

/// One city of the library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct City {
    /// Its name in every interface; the extract is `worlds/cache/<id>.osm.xml`.
    pub id: String,
    /// What a person reads.
    pub name: String,
    /// The state's USPS code (or the region's, outside the US).
    pub state: String,
    /// The state's name.
    pub state_name: String,
    /// northeast, southeast, south, midwest, southwest, west, europe.
    pub region: String,
    /// Why it is in the library: largest, studied, metro, international.
    pub role: String,
    /// `[south, west, north, east]`, degrees.
    pub bbox: [f64; 4],
    /// The box's area, km².
    pub area_km2: f64,
    /// The standard-time offset from UTC, hours.
    pub utc_offset_h: f64,
    /// The jurisdiction's highway preset label.
    pub highway_preset: String,
    /// The date the extract was fetched (`YYYY-MM-DD`).
    pub fetched: String,
    /// The area the box covers.
    pub description: String,
    /// Kilometres of driving lane in the imported network (measured by the city audit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive_lane_km: Option<f64>,
    /// Signalised intersections in the imported network (measured by the city audit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signals: Option<u32>,
    /// What the city audit found beyond a clean pass: the residual safety-class counts of
    /// the kinds the reference cities (Manhattan, Portland) show at the same load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<String>,
}

/// A city taken out of the default list, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dropped {
    /// Its id.
    pub id: String,
    /// Its name.
    #[serde(default)]
    pub name: String,
    /// Why it is not offered.
    pub reason: String,
}

/// The whole registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Registry {
    /// `v2xw/cities/1`.
    pub schema: String,
    /// Where the extracts come from.
    pub source: String,
    /// Their licence.
    pub licence: String,
    /// The attribution a world built from one carries.
    pub attribution: String,
    /// The cities offered.
    pub cities: Vec<City>,
    /// The cities withheld, with reasons.
    #[serde(default)]
    pub dropped: Vec<Dropped>,
}

/// The registry compiled into this binary.
///
/// # Panics
/// If the committed `worlds/cities.yaml` does not parse, which a unit test rules out.
pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| serde_yml::from_str(REGISTRY_YAML).expect("worlds/cities.yaml parses"))
}

/// The city with this id.
pub fn find(id: &str) -> Option<&'static City> {
    registry().cities.iter().find(|c| c.id == id)
}

/// The directory the extracts are read from: `V2XW_WORLDS_DIR`, else `worlds/cache`
/// relative to the working directory (the repository root, where the CLI and the server
/// run).
pub fn worlds_dir() -> PathBuf {
    std::env::var_os("V2XW_WORLDS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("worlds/cache"))
}

impl City {
    /// Where its extract is expected.
    pub fn extract_path(&self) -> String {
        worlds_dir()
            .join(format!("{}.osm.xml", self.id))
            .to_string_lossy()
            .into_owned()
    }

    /// Whether the extract has been fetched.
    pub fn extract_present(&self) -> bool {
        std::fs::metadata(self.extract_path()).is_ok_and(|m| m.len() > 0)
    }

    /// Its box as the importer takes it.
    pub fn geo_bbox(&self) -> GeoBbox {
        let [s, w, n, e] = self.bbox;
        GeoBbox {
            min_lat_deg: s,
            min_lon_deg: w,
            max_lat_deg: n,
            max_lon_deg: e,
        }
    }

    /// The world section for this city, keeping the building, signal and cache options
    /// of `keep` (a scenario's previous world) when given.
    pub fn world_spec(&self, keep: Option<&WorldSpec>) -> Result<WorldSpec, ScenarioError> {
        let preset = v2xw_world::osm::HighwayPreset::parse(&self.highway_preset).ok_or_else(|| {
            ScenarioError::conflict(
                "city",
                format!(
                    "city '{}' names the highway preset '{}', which this build does not know",
                    self.id, self.highway_preset
                ),
            )
        })?;
        let mut signals = keep.map(|w| w.signals.clone()).unwrap_or_default();
        // A green-wave speed is a city's own timing (Portland's 12 mph); another city's
        // plans progress at each link's speed limit.
        signals.progression_speed_mps = None;
        Ok(WorldSpec {
            source: WorldSourceSpec::OsmXml {
                path: self.extract_path(),
                bbox: Some(self.geo_bbox()),
            },
            imported_at: format!("{}T00:00:00Z", self.fetched),
            buildings: keep.map(|w| w.buildings.clone()).unwrap_or_default(),
            // A terrain raster covers one place; another city is flat until it has its own.
            terrain: Default::default(),
            cache: keep.and_then(|w| w.cache.clone()),
            highway_preset: Some(preset),
            signals,
            // The street network, not the alleys and garage ramps behind it
            // (`v2xw_world::osm::OsmLayers::service`).
            service_roads: false,
        })
    }

    /// What the Studio and `cities.list` show for it.
    pub fn summary(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Some(map) = v.as_object_mut() {
            map.insert("present".into(), Value::Bool(self.extract_present()));
            map.insert(
                "fetch".into(),
                Value::String(format!("worlds/fetch.sh {}", self.id)),
            );
        }
        v
    }
}

/// Every city, for an interface: the registry's fields plus whether the extract is here.
pub fn list_json() -> Value {
    let r = registry();
    serde_json::json!({
        "schema": r.schema,
        "source": r.source,
        "licence": r.licence,
        "attribution": r.attribution,
        "cities": r.cities.iter().map(City::summary).collect::<Vec<_>>(),
        "dropped": r.dropped,
    })
}

fn unknown_city(id: &str) -> ScenarioError {
    let ids: Vec<&str> = registry().cities.iter().map(|c| c.id.as_str()).collect();
    let why = registry()
        .dropped
        .iter()
        .find(|d| d.id == id)
        .map(|d| format!(" It was taken out of the library: {}.", d.reason))
        .unwrap_or_default();
    ScenarioError::conflict(
        "city",
        format!(
            "'{id}' is not a city in worlds/cities.yaml.{why} Known cities: {}",
            ids.join(", ")
        ),
    )
}

/// For a document that names a `city` and no `world`: the world becomes the city's.
///
/// A document that names a world keeps it (a saved scenario carries both, the world
/// already resolved). A document that names neither is refused with the fix.
///
/// # Errors
/// [`ScenarioError::Conflict`] on `city` for an unknown id, and on `world` when there is
/// neither.
pub fn fill_world_from_city(doc: &mut Value) -> crate::error::Result<()> {
    let Some(map) = doc.as_object_mut() else {
        return Ok(());
    };
    if map.get("world").is_some_and(|w| !w.is_null()) {
        return Ok(());
    }
    let Some(id) = map.get("city").and_then(Value::as_str).map(str::to_string) else {
        return Err(EngineError::Scenario(ScenarioError::conflict(
            "world",
            "the scenario names neither a world nor a city; add `city: <id>` (`v2xw cities` \
             lists them, e.g. manhattan, houston, seattle) or a `world` block",
        )));
    };
    let city = find(&id).ok_or_else(|| EngineError::Scenario(unknown_city(&id)))?;
    let world = city.world_spec(None).map_err(EngineError::Scenario)?;
    let value = serde_json::to_value(world)
        .map_err(|e| EngineError::Scenario(ScenarioError::conflict("world", e.to_string())))?;
    map.insert("world".into(), value);
    Ok(())
}

/// The library city a world section is, when it is one: by the extract's file name and
/// the box.
pub fn city_of_world(world: &WorldSpec) -> Option<&'static City> {
    let WorldSourceSpec::OsmXml { path, bbox } = &world.source else {
        return None;
    };
    let stem = std::path::Path::new(path).file_name()?.to_str()?.strip_suffix(".osm.xml")?;
    let city = find(stem)?;
    let same_box = bbox.is_none_or(|b| {
        let c = city.geo_bbox();
        (b.min_lat_deg - c.min_lat_deg).abs() < 1e-6
            && (b.min_lon_deg - c.min_lon_deg).abs() < 1e-6
            && (b.max_lat_deg - c.max_lat_deg).abs() < 1e-6
            && (b.max_lon_deg - c.max_lon_deg).abs() < 1e-6
    });
    same_box.then_some(city)
}

/// `t0` (`YYYY-MM-DDTHH:MM:SS[.fff]Z`) moved by `hours`; `None` when it does not parse.
pub fn shift_utc(t0: &str, hours: f64) -> Option<String> {
    let body = t0.trim().strip_suffix('Z')?;
    let (date, time) = body.split_once('T')?;
    let mut d = date.split('-');
    let (y, m, day): (i64, i64, i64) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    let mut t = time.split(':');
    let (hh, mm): (i64, i64) = (t.next()?.parse().ok()?, t.next()?.parse().ok()?);
    let sec = t.next().unwrap_or("00");
    let shift_min = (hours * 60.0).round() as i64;
    let total_min = days_from_civil(y, m, day) * 1440 + hh * 60 + mm + shift_min;
    let days = total_min.div_euclid(1440);
    let minutes = total_min.rem_euclid(1440);
    let (y, m, day) = civil_from_days(days);
    Some(format!("{y:04}-{m:02}-{day:02}T{:02}:{:02}:{sec}Z", minutes / 60, minutes % 60))
}

// Howard Hinnant's days-from-civil and its inverse (public domain).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl Scenario {
    /// The library city this scenario runs on: [`Scenario::city`], else the city its world
    /// section is.
    pub fn current_city(&self) -> Option<&'static City> {
        self.city
            .as_deref()
            .and_then(find)
            .filter(|c| city_of_world(&self.world).is_none_or(|w| w.id == c.id))
            .or_else(|| city_of_world(&self.world))
    }

    /// This scenario on another city: the city's world, and everything that named a place
    /// in the old one restated so it fits the new one (see the module documentation).
    ///
    /// # Errors
    /// [`ScenarioError::Conflict`] on `city` when the id is not in the library.
    pub fn with_city(mut self, id: &str) -> Result<Scenario, ScenarioError> {
        let city = find(id).ok_or_else(|| unknown_city(id))?;
        let old = self.current_city();
        if old.is_some_and(|o| o.id == city.id) {
            self.city = Some(city.id.clone());
            connect_demand(&mut self);
            return Ok(self);
        }
        self.world = city.world_spec(Some(&self.world))?;
        self.city = Some(city.id.clone());

        // The clock keeps its local time of day: noon in Manhattan is noon in Houston.
        if let Some(old) = old
            && let Some(t0) = shift_utc(&self.time.t0, old.utc_offset_h - city.utc_offset_h)
        {
            self.time.t0 = t0;
        }

        // A fixed arrival rate was written for the old network's size; restate it per
        // kilometre of driving lane so the new city gets the same density, not the same
        // count. Crowds scale the same way.
        // A world outside the library (a procedural grid) has no measured size here, so its
        // fixed rate is kept as written; a library city without a measurement yet is taken
        // to be the size of Midtown.
        let old_km = old.map(|o| o.drive_lane_km.unwrap_or(REFERENCE_LANE_KM)).unwrap_or(0.0);
        let demand = &mut self.actors.vehicles.demand;
        if demand.rate_veh_per_h_per_lane_km.is_none()
            && let Some(rate) = demand.rate_veh_per_h
            && old_km > 0.0
        {
            demand.rate_veh_per_h_per_lane_km = Some(rate / old_km);
        }
        if let Some(new_km) = city.drive_lane_km
            && old_km > 0.0
        {
            let scale = |n: u32| -> u32 {
                if n == 0 { 0 } else { ((f64::from(n) * new_km / old_km).round() as u32).max(1) }
            };
            self.actors.vru.pedestrians = scale(self.actors.vru.pedestrians);
            self.actors.vru.cyclists = scale(self.actors.vru.cyclists);
        }

        // Roadside units standing at the old city's coordinates move to the new city's
        // signalised intersections, as many and doing the same.
        let (placed, kept): (Vec<Rsu>, Vec<Rsu>) = std::mem::take(&mut self.actors.rsus)
            .into_iter()
            .partition(|r| r.position_m.is_some() || r.site.is_some());
        self.actors.rsus = kept;
        if !placed.is_empty() {
            let first = &placed[0];
            let mut roles: Vec<String> = Vec::new();
            for r in &placed {
                for role in &r.roles {
                    if !roles.contains(role) {
                        roles.push(role.clone());
                    }
                }
            }
            let extra = u32::try_from(placed.len()).unwrap_or(u32::MAX);
            match &mut self.actors.rsu_placement {
                Some(p) => p.count = p.count.saturating_add(extra),
                None => {
                    self.actors.rsu_placement = Some(RsuPlacement {
                        count: extra,
                        roles,
                        profile: first.profile.clone(),
                        backhaul: first.backhaul.clone(),
                        min_spacing_m: 150.0,
                    });
                }
            }
        }

        // A closure of a named street names the old city's street.
        for item in &mut self.events {
            if item.kind != crate::scenario::schema::TimelineKind::Closure {
                continue;
            }
            if let Some(target) = item.params.get_mut("target")
                && target.as_str().is_some_and(|t| t.trim_start().starts_with("street:"))
            {
                *target = Value::String("auto".into());
            }
        }

        // A jammer standing at the old city's coordinates moves to the new city's busiest
        // signalised intersection; one following a node or a path keeps doing so (a path in
        // the old city's metres is dropped to the same intersection).
        for jammer in &mut self.threats.jammers {
            if let Some(params) = jammer.params.as_object_mut()
                && (params.contains_key("position_m") || params.contains_key("path_m"))
            {
                params.remove("position_m");
                params.remove("path_m");
                params.remove("speed_mps");
                params.remove("loop_path");
                params.insert("at".into(), Value::String("busiest-junction".into()));
            }
        }

        connect_demand(&mut self);

        // A focus box is a place in the old city.
        if let Some(focus) = &self.radio.tiers.focus
            && matches!(focus.region, crate::scenario::schema::FocusRegion::Bbox { .. })
        {
            self.radio.tiers.focus = None;
        }
        Ok(self)
    }
}

/// On a city, trips are drawn from the part of the network a car can drive round
/// (`v2xw_mobility::demand::OdParams::connected_only`): a box cut out of a city leaves bus-only
/// lanes and one-way streets that only leave it, and a trip from or to one has no route.
/// Only the Poisson model has an OD law; the others are left alone.
pub fn connect_demand(s: &mut Scenario) {
    let d = &mut s.actors.vehicles.demand;
    if !d.kind.starts_with("mobility/demand/poisson") {
        return;
    }
    if !d.params.is_object() {
        d.params = Value::Object(serde_json::Map::new());
    }
    if let Some(params) = d.params.as_object_mut() {
        let od = params
            .entry("od")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(od) = od.as_object_mut() {
            od.entry("connected_only").or_insert(Value::Bool(true));
        }
    }
}

/// Kilometres of driving lane in a world: every driving lane outside a junction.
pub fn drive_lane_km(world: &World) -> f64 {
    world
        .roads
        .lanes()
        .iter()
        .filter(|l| l.kind == LaneKind::Driving && l.junction.is_none())
        .map(|l| l.length_m)
        .sum::<f64>()
        / 1000.0
}

/// The world's signalised intersections, busiest first: by the number of driving lanes
/// that enter them, then nearest the centre of the map (a junction at the edge of the box
/// has lost approaches to the cut), then by id.
pub fn signalised_by_rank(world: &World) -> Vec<(v2xw_core::geom::Vec3, usize)> {
    let centre = world.bbox.center();
    let mut out: Vec<(v2xw_core::geom::Vec3, usize, f64, u32)> = world
        .roads
        .junctions()
        .iter()
        .filter(|j| matches!(j.control, JunctionControl::Signalised { .. }))
        .map(|j| {
            let approaches = j
                .incoming
                .iter()
                .filter(|l| world.roads.lane(**l).kind == LaneKind::Driving)
                .count();
            let dx = j.position.x - centre.x;
            let dy = j.position.y - centre.y;
            (j.position, approaches, (dx * dx + dy * dy).sqrt(), j.id.index())
        })
        .collect();
    out.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(a.2.total_cmp(&b.2))
            .then(a.3.cmp(&b.3))
    });
    out.into_iter().map(|(p, n, _, _)| (p, n)).collect()
}

/// Stands the [`RsuPlacement`]'s units at the world's busiest signalised intersections,
/// no two closer than its spacing, and appends them to `actors.rsus`; returns how many.
pub fn place_rsus(scenario: &mut Scenario, world: &World) -> usize {
    let Some(placement) = scenario.actors.rsu_placement.take() else {
        return 0;
    };
    let mut chosen: Vec<v2xw_core::geom::Vec3> = Vec::new();
    for (p, _) in signalised_by_rank(world) {
        if chosen.len() >= placement.count as usize {
            break;
        }
        let spaced = chosen.iter().all(|q| {
            let (dx, dy) = (p.x - q.x, p.y - q.y);
            (dx * dx + dy * dy).sqrt() >= placement.min_spacing_m
        });
        if spaced {
            chosen.push(p);
        }
    }
    let n = chosen.len();
    for p in chosen {
        scenario.actors.rsus.push(Rsu {
            site: None,
            position_m: Some([
                (p.x * 10.0).round() / 10.0,
                (p.y * 10.0).round() / 10.0,
                (p.z * 10.0).round() / 10.0,
            ]),
            roles: placement.roles.clone(),
            profile: placement.profile.clone(),
            backhaul: placement.backhaul.clone(),
        });
    }
    n
}

/// Fits a scenario to the world it is about to run on: the per-lane-kilometre arrival rate
/// becomes vehicles per hour, and the placed roadside units are stood at intersections.
/// A scenario with neither is returned unchanged.
pub fn adapt_to_world(mut scenario: Scenario, world: &World) -> Scenario {
    if let Some(per_km) = scenario.actors.vehicles.demand.rate_veh_per_h_per_lane_km {
        let km = drive_lane_km(world);
        scenario.actors.vehicles.demand.rate_veh_per_h = Some((per_km * km).round().max(0.0));
        scenario.actors.vehicles.demand.rate_veh_per_h_per_lane_km = None;
    }
    place_rsus(&mut scenario, world);
    let busiest = signalised_by_rank(world)
        .first()
        .map(|(p, _)| *p)
        .unwrap_or_else(|| world.bbox.center());
    for jammer in &mut scenario.threats.jammers {
        if let Some(params) = jammer.params.as_object_mut()
            && params.get("at").and_then(Value::as_str) == Some("busiest-junction")
        {
            params.remove("at");
            params.insert(
                "position_m".into(),
                serde_json::json!([
                    (busiest.x * 10.0).round() / 10.0,
                    (busiest.y * 10.0).round() / 10.0
                ]),
            );
        }
    }
    scenario
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_committed_registry_parses_and_every_city_names_a_known_preset() {
        let r = registry();
        assert_eq!(r.schema, "v2xw/cities/1");
        assert!(r.cities.len() >= 51, "{} cities", r.cities.len());
        let mut ids: Vec<&str> = r.cities.iter().map(|c| c.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), r.cities.len(), "ids are unique");
        for c in &r.cities {
            assert!(v2xw_world::osm::HighwayPreset::parse(&c.highway_preset).is_some(), "{}", c.id);
            let [s, w, n, e] = c.bbox;
            assert!(s < n && w < e, "{}", c.id);
            assert!((0.5..=5.0).contains(&c.area_km2), "{}: {} km²", c.id, c.area_km2);
        }
        // Every state and DC has a city.
        let mut states: Vec<&str> = r
            .cities
            .iter()
            .filter(|c| c.region != "europe")
            .map(|c| c.state.as_str())
            .collect();
        states.sort_unstable();
        states.dedup();
        assert_eq!(states.len(), 51, "{states:?}");
    }

    #[test]
    fn utc_shift_crosses_midnight_and_month_ends() {
        assert_eq!(shift_utc("2027-03-04T17:00:00Z", 1.0).as_deref(), Some("2027-03-04T18:00:00Z"));
        assert_eq!(shift_utc("2027-03-04T23:30:00Z", 2.0).as_deref(), Some("2027-03-05T01:30:00Z"));
        assert_eq!(shift_utc("2027-03-01T01:00:00Z", -3.0).as_deref(), Some("2027-02-28T22:00:00Z"));
        assert_eq!(shift_utc("not a time", 1.0), None);
    }

    #[test]
    fn a_scenario_moves_city_with_its_clock_its_density_and_its_units() {
        let mut s = Scenario::minimal();
        s = s.with_city("manhattan").expect("manhattan");
        s.time.t0 = "2027-03-04T17:00:00Z".into(); // noon Eastern
        s.actors.vehicles.demand.rate_veh_per_h = Some(3000.0);
        s.actors.rsus.push(Rsu {
            site: None,
            position_m: Some([1.0, 2.0, 0.0]),
            roles: vec!["crl".into()],
            profile: None,
            backhaul: None,
        });
        let h = s.with_city("houston").expect("houston");
        assert_eq!(h.city.as_deref(), Some("houston"));
        assert_eq!(h.time.t0, "2027-03-04T18:00:00Z", "noon Central");
        assert!(h.actors.rsus.is_empty());
        assert_eq!(h.actors.rsu_placement.as_ref().map(|p| p.count), Some(1));
        assert!(h.actors.vehicles.demand.rate_veh_per_h_per_lane_km.is_some());
        assert_eq!(
            h.world.highway_preset.map(|p| p.label()),
            Some("urban-us-tx")
        );
        assert!(h.with_city("atlantis").is_err());
    }
}
