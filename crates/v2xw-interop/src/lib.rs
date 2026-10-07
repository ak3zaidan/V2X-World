//! SUMO interoperability (04-models.md §1.1, §2.8): the co-simulation tier and the
//! cross-validation against SUMO.
//!
//! | What | Module |
//! |---|---|
//! | TraCI wire protocol | [`traci`] |
//! | `SumoMobility`: SUMO drives traffic over TraCI while this engine runs radio, security, backend and UI | [`sumo`] |
//! | `.rou.xml` import as a [`v2xw_mobility::traits::Demand`], and export | [`routes`] |
//! | network export to SUMO plain XML (`.nod/.edg/.con/.tll.xml`) and `netconvert` | [`netexport`] |
//! | the cross-validation study and report (`docs/validation/cross-validation.md`) | [`xval`], [`radio`] |
//!
//! # Identity between the two simulators
//!
//! A world imported from a `net.xml` keeps each SUMO edge id as the edge's name
//! (`v2xw_world::sumo`), and each lane's `index`; a world built here and exported gets edge
//! ids `e<EdgeId>`. [`SumoIds`] holds the mapping both ways, so a SUMO lane id is always a
//! world lane and the reverse, without geometric matching.

pub mod netexport;
pub mod radio;
pub mod routes;
pub mod sumo;
pub mod traci;
pub mod xval;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use v2xw_core::ids::{EdgeId, LaneId};
use v2xw_world::World;

/// The SUMO edge id this crate uses for a world edge: its name when the name is unique in
/// the world (a SUMO import), `e<index>` otherwise.
#[derive(Debug, Clone, Default)]
pub struct SumoIds {
    /// World edge → SUMO edge id.
    pub edge_id: BTreeMap<EdgeId, String>,
    /// SUMO edge id → its world lanes, by SUMO lane index.
    pub lanes_of: BTreeMap<String, Vec<LaneId>>,
    /// SUMO lane id (`edge_index`) → world lane.
    pub lane_of: BTreeMap<String, LaneId>,
    /// World lane → SUMO lane id. Connector lanes inside junctions have none.
    pub sumo_lane: BTreeMap<LaneId, String>,
}

impl SumoIds {
    /// The mapping for `world`. Internal (junction) edges are left out: SUMO names its own.
    pub fn new(world: &World) -> Self {
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for e in world.roads.edges() {
            if let Some(n) = e.name {
                *counts.entry(world.symbols.resolve(n).to_string()).or_default() += 1;
            }
        }
        let mut ids = Self::default();
        let mut used = BTreeSet::new();
        for e in world.roads.edges() {
            if e.is_internal() {
                continue;
            }
            let name = e.name.map(|n| world.symbols.resolve(n).to_string());
            let id = match name {
                Some(n)
                    if counts.get(&n) == Some(&1)
                        && !n.is_empty()
                        && !n.starts_with(':')
                        && !n.contains(char::is_whitespace) =>
                {
                    n
                }
                _ => format!("e{}", e.id.index()),
            };
            if !used.insert(id.clone()) {
                continue;
            }
            let mut lanes: Vec<(u8, LaneId)> =
                e.lanes.iter().map(|l| (world.lane(*l).index, *l)).collect();
            lanes.sort();
            for (i, l) in &lanes {
                let lid = format!("{id}_{i}");
                ids.lane_of.insert(lid.clone(), *l);
                ids.sumo_lane.insert(*l, lid);
            }
            ids.lanes_of
                .insert(id.clone(), lanes.into_iter().map(|(_, l)| l).collect());
            ids.edge_id.insert(e.id, id);
        }
        ids
    }

    /// The SUMO edge id of the edge `lane` is on, when it is a normal edge.
    pub fn edge_of_lane(&self, world: &World, lane: LaneId) -> Option<&str> {
        self.edge_id.get(&world.lane(lane).edge).map(String::as_str)
    }

    /// The SUMO edges a route of world lanes passes, connectors left out, repeats merged.
    pub fn edges_of_route(&self, world: &World, lanes: &[LaneId]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for l in lanes {
            if let Some(e) = self.edge_of_lane(world, *l)
                && out.last().map(String::as_str) != Some(e)
            {
                out.push(e.to_string());
            }
        }
        out
    }
}

/// Where SUMO is: `$SUMO_HOME`, else the `sumo` directory of the `eclipse-sumo` wheel in
/// `$V2XW_SUMO_VENV` (`<venv>/lib/python3.*/site-packages/sumo`).
pub fn sumo_home() -> Option<PathBuf> {
    if let Ok(h) = std::env::var("SUMO_HOME") {
        let p = PathBuf::from(h);
        if p.join("bin").join("sumo").exists() {
            return Some(p);
        }
    }
    let venv = PathBuf::from(std::env::var("V2XW_SUMO_VENV").ok()?);
    let lib = std::fs::read_dir(venv.join("lib")).ok()?;
    for entry in lib.flatten() {
        let p = entry.path().join("site-packages").join("sumo");
        if p.join("bin").join("sumo").exists() {
            return Some(p);
        }
    }
    None
}

/// A SUMO tool's path (`sumo`, `netconvert`, `netgenerate`, …).
pub fn sumo_tool(home: &std::path::Path, tool: &str) -> PathBuf {
    home.join("bin").join(tool)
}
