//! `.rou.xml` demand: import as a [`Demand`], and export.
//!
//! Read (SUMO documentation, "Definition of Vehicles, Vehicle Types, and Routes"):
//! `<vType id vClass maxSpeed>`, `<route id edges>`, `<vehicle id type depart route
//! departLane departPos departSpeed>` with an optional nested `<route edges>`, `<trip id
//! type depart from to via>`, and `<flow id type begin end from to route vehsPerHour |
//! period | number | probability>`. Flows are expanded deterministically: `vehsPerHour`
//! and `number` become equally spaced departures (SUMO's own default spacing for those
//! attributes); `probability` p per second becomes equally spaced departures at 1/p s,
//! which keeps the mean and drops SUMO's Bernoulli variance (stated, not hidden: the card
//! lists it). Departure attributes other than numbers (`random`, `best`, `free`, …) fall
//! back to lane 0 / position 0, and are counted.
//!
//! A vehicle with a route keeps it: [`Demand::planned_route`] returns the world-lane route
//! through exactly the file's edges (found by Dijkstra restricted to those edges and the
//! connectors between them). A trip is routed by the engine.

use std::collections::BTreeMap;

use v2xw_core::card::{Family, ModelCard};
use v2xw_core::ids::LaneId;
use v2xw_core::model::Model;
use v2xw_core::time::SimTime;
use v2xw_mobility::ctx::MobCtx;
use v2xw_mobility::routing::dijkstra::DijkstraParams;
use v2xw_mobility::traits::Demand;
use v2xw_mobility::views::{EdgeCost, TripRequest};
use v2xw_mobility::{Dijkstra, FreeFlowCost, VehicleClass};
use v2xw_world::World;

use crate::SumoIds;

/// One `<vType>`.
#[derive(Debug, Clone, PartialEq)]
pub struct VType {
    /// `id`.
    pub id: String,
    /// `vClass`, when it names a class this engine models.
    pub class: VehicleClass,
    /// `maxSpeed`, m/s.
    pub max_speed_mps: Option<f64>,
}

/// One vehicle of the file, flows already expanded.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteVehicle {
    /// `id` (a flow's vehicles are `flowId.n`, as SUMO names them).
    pub id: String,
    /// `depart`, seconds.
    pub depart_s: f64,
    /// `type`.
    pub vtype: Option<String>,
    /// The route's edges; a trip has `[from, to]` (plus `via`) and `routed == false`.
    pub edges: Vec<String>,
    /// Whether `edges` is a full route (true) or trip endpoints the engine must route.
    pub routed: bool,
    /// `departLane` when numeric.
    pub depart_lane: Option<u32>,
    /// `departPos` when numeric, metres.
    pub depart_pos_m: Option<f64>,
}

/// A parsed `.rou.xml`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RouteFile {
    /// Vehicle types by id.
    pub vtypes: BTreeMap<String, VType>,
    /// Vehicles in departure order (then id).
    pub vehicles: Vec<RouteVehicle>,
    /// Attributes read but not representable (non-numeric `departLane`, …), by name.
    pub approximations: BTreeMap<String, u64>,
}

fn attr(e: &quick_xml::events::BytesStart<'_>, key: &str) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.into_inner() == key).map(|a| {
        a.value.to_string()
    })
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

impl RouteFile {
    /// Parses a `.rou.xml` document.
    ///
    /// # Errors
    /// Malformed XML.
    pub fn parse(xml: &str) -> Result<Self, String> {
        let mut reader = quick_xml::Reader::from_str(xml);
        let mut out = RouteFile::default();
        let mut routes: BTreeMap<String, Vec<String>> = BTreeMap::new();
        // An open <vehicle> waiting for its nested <route>.
        let mut open: Option<RouteVehicle> = None;
        let mut flows: Vec<(BTreeMap<String, String>, Vec<String>)> = Vec::new();
        let mut open_flow: Option<BTreeMap<String, String>> = None;
        let mut pending_route_ref: Vec<(usize, String)> = Vec::new();
        loop {
            let ev = reader.read_event().map_err(|e| format!("rou.xml: {e}"))?;
            match ev {
                quick_xml::events::Event::Eof => break,
                quick_xml::events::Event::Start(ref e) | quick_xml::events::Event::Empty(ref e) => {
                    let empty = matches!(ev, quick_xml::events::Event::Empty(_));
                    let all: BTreeMap<String, String> = e
                        .attributes()
                        .flatten()
                        .map(|a| {
                            (
                                a.key.into_inner().to_string(),
                                a.value.to_string(),
                            )
                        })
                        .collect();
                    match e.name().into_inner() {
                        "vType" => {
                            let id = attr(e, "id").unwrap_or_default();
                            let class = attr(e, "vClass")
                                .and_then(|c| class_of(&c))
                                .unwrap_or_default();
                            out.vtypes.insert(
                                id.clone(),
                                VType {
                                    id,
                                    class,
                                    max_speed_mps: attr(e, "maxSpeed").and_then(|s| s.parse().ok()),
                                },
                            );
                        }
                        "route" => {
                            let edges: Vec<String> = attr(e, "edges")
                                .unwrap_or_default()
                                .split_whitespace()
                                .map(str::to_string)
                                .collect();
                            if let Some(v) = open.as_mut() {
                                v.edges = edges;
                                v.routed = true;
                            } else if let Some(f) = open_flow.as_mut() {
                                f.insert("__edges".to_string(), edges.join(" "));
                            } else if let Some(id) = attr(e, "id") {
                                routes.insert(id, edges);
                            }
                        }
                        "vehicle" | "trip" => {
                            let mut edges = Vec::new();
                            let mut routed = false;
                            if let Some(from) = all.get("from") {
                                edges.push(from.clone());
                                if let Some(via) = all.get("via") {
                                    edges.extend(via.split_whitespace().map(str::to_string));
                                }
                                if let Some(to) = all.get("to") {
                                    edges.push(to.clone());
                                }
                            }
                            let v = RouteVehicle {
                                id: all.get("id").cloned().unwrap_or_default(),
                                depart_s: depart_seconds(all.get("depart"), &mut out.approximations),
                                vtype: all.get("type").cloned(),
                                edges,
                                routed,
                                depart_lane: numeric_u32(all.get("departLane"), "departLane", &mut out.approximations),
                                depart_pos_m: numeric_f64(all.get("departPos"), "departPos", &mut out.approximations),
                            };
                            if let Some(r) = all.get("route") {
                                routed = true;
                                pending_route_ref.push((out.vehicles.len(), r.clone()));
                                let mut v = v;
                                v.routed = routed;
                                if empty {
                                    out.vehicles.push(v);
                                } else {
                                    open = Some(v);
                                }
                            } else if empty {
                                out.vehicles.push(v);
                            } else {
                                open = Some(v);
                            }
                        }
                        "flow" => {
                            if empty {
                                flows.push((all, Vec::new()));
                            } else {
                                open_flow = Some(all);
                            }
                        }
                        _ => {}
                    }
                }
                quick_xml::events::Event::End(ref e) => match e.name().into_inner() {
                    "vehicle" | "trip" => {
                        if let Some(v) = open.take() {
                            out.vehicles.push(v);
                        }
                    }
                    "flow" => {
                        if let Some(mut f) = open_flow.take() {
                            let edges = f
                                .remove("__edges")
                                .map(|s| s.split_whitespace().map(str::to_string).collect())
                                .unwrap_or_default();
                            flows.push((f, edges));
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        for (i, r) in pending_route_ref {
            if let Some(edges) = routes.get(&r) {
                out.vehicles[i].edges = edges.clone();
                out.vehicles[i].routed = true;
            }
        }
        for (f, nested) in flows {
            out.expand_flow(&f, nested, &routes);
        }
        out.vehicles
            .sort_by(|a, b| a.depart_s.total_cmp(&b.depart_s).then_with(|| a.id.cmp(&b.id)));
        Ok(out)
    }

    fn expand_flow(
        &mut self,
        f: &BTreeMap<String, String>,
        nested: Vec<String>,
        routes: &BTreeMap<String, Vec<String>>,
    ) {
        let id = f.get("id").cloned().unwrap_or_default();
        let begin = f.get("begin").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
        let end = f.get("end").and_then(|s| s.parse::<f64>().ok()).unwrap_or(3600.0);
        let (edges, routed) = if !nested.is_empty() {
            (nested, true)
        } else if let Some(r) = f.get("route").and_then(|r| routes.get(r)) {
            (r.clone(), true)
        } else {
            let mut e = Vec::new();
            if let Some(x) = f.get("from") {
                e.push(x.clone());
            }
            if let Some(v) = f.get("via") {
                e.extend(v.split_whitespace().map(str::to_string));
            }
            if let Some(x) = f.get("to") {
                e.push(x.clone());
            }
            (e, false)
        };
        let period = if let Some(v) = f.get("vehsPerHour").and_then(|s| s.parse::<f64>().ok()) {
            3600.0 / v.max(1e-9)
        } else if let Some(p) = f.get("period").and_then(|s| s.trim_start_matches("exp(").trim_end_matches(')').parse::<f64>().ok()) {
            p
        } else if let Some(n) = f.get("number").and_then(|s| s.parse::<f64>().ok()) {
            (end - begin) / n.max(1.0)
        } else if let Some(p) = f.get("probability").and_then(|s| s.parse::<f64>().ok()) {
            *self.approximations.entry("flow@probability".to_string()).or_default() += 1;
            1.0 / p.max(1e-9)
        } else {
            return;
        };
        let cap = f.get("number").and_then(|s| s.parse::<u64>().ok()).unwrap_or(u64::MAX);
        let mut n = 0u64;
        let mut t = begin;
        while t < end && n < cap {
            self.vehicles.push(RouteVehicle {
                id: format!("{id}.{n}"),
                depart_s: t,
                vtype: f.get("type").cloned(),
                edges: edges.clone(),
                routed,
                depart_lane: numeric_u32(f.get("departLane"), "departLane", &mut self.approximations),
                depart_pos_m: numeric_f64(f.get("departPos"), "departPos", &mut self.approximations),
            });
            n += 1;
            t = begin + n as f64 * period;
        }
    }

    /// Reads and parses a file.
    ///
    /// # Errors
    /// I/O or malformed XML.
    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        let xml = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&xml)
    }
}

fn depart_seconds(v: Option<&String>, approx: &mut BTreeMap<String, u64>) -> f64 {
    match v.map(String::as_str) {
        None => 0.0,
        Some(s) => s.parse().unwrap_or_else(|_| {
            *approx.entry(format!("depart={s}")).or_default() += 1;
            0.0
        }),
    }
}

fn numeric_u32(v: Option<&String>, name: &str, approx: &mut BTreeMap<String, u64>) -> Option<u32> {
    let s = v?;
    s.parse().ok().or_else(|| {
        *approx.entry(format!("{name}={s}")).or_default() += 1;
        None
    })
}

fn numeric_f64(v: Option<&String>, name: &str, approx: &mut BTreeMap<String, u64>) -> Option<f64> {
    let s = v?;
    s.parse().ok().or_else(|| {
        *approx.entry(format!("{name}={s}")).or_default() += 1;
        None
    })
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

/// The demand a `.rou.xml` states, as this engine's [`Demand`].
pub struct RouteFileDemand {
    card: ModelCard,
    file: RouteFile,
    cursor: usize,
    ids: Option<SumoIds>,
    planned: BTreeMap<u64, Vec<String>>,
    /// Vehicles whose edges this world does not have.
    pub unresolved: u64,
}

impl RouteFileDemand {
    /// Demand from a parsed file.
    pub fn new(file: RouteFile) -> Self {
        let mut card = ModelCard::new(
            "mobility/demand/sumo-rou",
            Family::Mobility,
            "1.0.0",
            "The vehicles of a SUMO .rou.xml (vehicles, trips, flows) as trips of this \
             engine; a vehicle's own route is kept through Demand::planned_route.",
        );
        card.limitations.push(
            "Flows with `probability` are expanded at equal spacing 1/p: the mean rate is \
             SUMO's, its Bernoulli variance is not."
                .to_string(),
        );
        card.limitations.push(
            "Non-numeric departLane/departPos (random, best, free, …) depart on lane 0 at \
             position 0."
                .to_string(),
        );
        Self {
            card,
            file,
            cursor: 0,
            ids: None,
            planned: BTreeMap::new(),
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
            let pick = |lanes: &Vec<LaneId>, idx: Option<u32>| {
                idx.and_then(|i| lanes.get(i as usize).copied())
                    .filter(|l| world.lane(*l).admits(mask))
                    .or_else(|| lanes.iter().copied().find(|l| world.lane(*l).admits(mask)))
            };
            let (Some(origin), Some(destination)) = (pick(first, v.depart_lane), pick(last, None))
            else {
                self.unresolved += 1;
                continue;
            };
            let seq = self.cursor as u64 - 1;
            if v.routed {
                self.planned.insert(seq, v.edges.clone());
            }
            let spec = class.spec();
            let len = world.lane(origin).length_m;
            out.push(TripRequest {
                seq,
                t: t.max(from),
                origin,
                origin_s_m: v.depart_pos_m.unwrap_or(0.0).clamp(0.0, len),
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
        if edges.len() >= 2 || (edges.len() == 1 && ids.edge_of_lane(world, t.destination) == Some(edges[0].as_str())) {
            let _ = writeln!(s, "  <vehicle {common}>\n    <route edges=\"{}\"/>\n  </vehicle>", edges.join(" "));
        } else {
            let from = ids.edge_of_lane(world, t.origin).unwrap_or("");
            let to = ids.edge_of_lane(world, t.destination).unwrap_or("");
            let _ = writeln!(s, "  <trip {common} from=\"{from}\" to=\"{to}\"/>");
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
  <trip id="t0" depart="2.5" from="a" to="c" departPos="random"/>
  <flow id="f" begin="0" end="10" vehsPerHour="1800" from="a" to="b"/>
</routes>"#;
        let f = RouteFile::parse(xml).unwrap();
        assert_eq!(f.vtypes["car"].max_speed_mps, Some(13.9));
        // 1800 veh/h over 10 s = one every 2 s: 0, 2, 4, 6, 8.
        assert_eq!(f.vehicles.iter().filter(|v| v.id.starts_with("f.")).count(), 5);
        let v0 = f.vehicles.iter().find(|v| v.id == "v0").unwrap();
        assert_eq!(v0.edges, vec!["a", "b", "c"]);
        assert!(v0.routed);
        assert_eq!(v0.depart_lane, Some(1));
        let v1 = f.vehicles.iter().find(|v| v.id == "v1").unwrap();
        assert_eq!(v1.edges, vec!["a", "b"]);
        let t0 = f.vehicles.iter().find(|v| v.id == "t0").unwrap();
        assert!(!t0.routed);
        assert_eq!(f.approximations.get("departPos=random"), Some(&1));
        // Sorted by departure.
        assert!(f.vehicles.windows(2).all(|w| w[0].depart_s <= w[1].depart_s));
    }
}
