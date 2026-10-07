//! Network export to SUMO plain XML (`.nod.xml`, `.edg.xml`, `.con.xml`) and, through
//! `netconvert`, to a `.net.xml` (SUMO documentation, "Networks/PlainXML").
//!
//! | world-1 | plain XML |
//! |---|---|
//! | [`Junction`](v2xw_world::model::Junction) | `<node id="j<n>" x y type>`: `traffic_light` when signalised, `allway_stop` for stop, `priority` for yield and priority, `right_before_left` when uncontrolled |
//! | normal [`Edge`](v2xw_world::model::Edge) | `<edge id from to numLanes speed shape spreadType="right">` with `<lane index speed width allow>` |
//! | permitted [`Connection`](v2xw_world::model::Connection) between normal lanes | `<connection from to fromLane toLane>` |
//!
//! The edge shape is the leftmost lane's centreline moved half a lane width to the left,
//! which is what `spreadType="right"` places lanes against. Junction internals are not
//! written: `netconvert` computes its own junction shapes and internal lanes from the
//! connections. **Signal programmes are not exported**: netconvert generates its default
//! programme for each `traffic_light` node (SUMO documentation, "Simulation/Traffic
//! Lights": a fixed-time cycle with 4 s yellow… the default green split), so a SUMO run on an
//! exported world has the same signalised junctions and different timings. A world that
//! came *from* a `net.xml` keeps SUMO's own programmes and should be run on that file.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use v2xw_world::World;
use v2xw_world::model::JunctionControl;

use crate::SumoIds;

/// The three plain-XML documents.
#[derive(Debug, Clone, Default)]
pub struct PlainXml {
    /// `.nod.xml`.
    pub nodes: String,
    /// `.edg.xml`.
    pub edges: String,
    /// `.con.xml`.
    pub connections: String,
    /// What was not exported, counted.
    pub skipped: std::collections::BTreeMap<&'static str, u64>,
}

fn allow_list(world: &World, lane: &v2xw_world::model::Lane) -> Option<String> {
    let _ = world;
    let names = lane.allowed.names();
    if names.is_empty() {
        return None;
    }
    // Map this crate's class names onto SUMO vClasses where they differ.
    let mut out: Vec<&str> = names
        .iter()
        .flat_map(|n| match *n {
            "car" | "passenger" => vec!["passenger", "taxi", "private", "evehicle"],
            "truck" => vec!["truck", "trailer", "delivery"],
            "bus" => vec!["bus", "coach"],
            "emergency" => vec!["emergency", "authority"],
            "moto" => vec!["motorcycle", "moped"],
            "bicycle" => vec!["bicycle"],
            "pedestrian" => vec!["pedestrian"],
            "tram" => vec!["tram"],
            "rail" => vec!["rail"],
            other => vec![other],
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    Some(out.join(" "))
}

fn left_offset(points: &[v2xw_core::geom::Vec3], off: f64) -> Vec<(f64, f64)> {
    let n = points.len();
    (0..n)
        .map(|i| {
            let a = points[i.saturating_sub(1)];
            let b = points[(i + 1).min(n - 1)];
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len = dx.hypot(dy).max(1e-9);
            // Left normal of the travel direction.
            (points[i].x - dy / len * off, points[i].y + dx / len * off)
        })
        .collect()
}

/// Writes `world` as plain XML.
pub fn to_plain_xml(world: &World, ids: &SumoIds) -> PlainXml {
    let mut px = PlainXml::default();
    let mut n = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<nodes>\n");
    for j in world.roads.junctions() {
        let ty = match j.control {
            JunctionControl::Signalised { .. } => "traffic_light",
            JunctionControl::Stop => "allway_stop",
            JunctionControl::Uncontrolled => "right_before_left",
            _ => "priority",
        };
        let _ = writeln!(
            n,
            "  <node id=\"j{}\" x=\"{:.3}\" y=\"{:.3}\" type=\"{ty}\"/>",
            j.id.index(),
            j.position.x,
            j.position.y
        );
    }
    n.push_str("</nodes>\n");
    let mut e = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<edges>\n");
    for edge in world.roads.edges() {
        let Some(id) = ids.edge_id.get(&edge.id) else {
            continue;
        };
        let lanes = &ids.lanes_of[id];
        let Some(left) = lanes.last().map(|l| world.lane(*l)) else {
            *px.skipped.entry("edge without lanes").or_default() += 1;
            continue;
        };
        if left.centreline.len() < 2 {
            *px.skipped.entry("edge with a degenerate centreline").or_default() += 1;
            continue;
        }
        let speed = lanes
            .iter()
            .map(|l| world.lane(*l).speed_limit_mps)
            .fold(0.0_f64, f64::max);
        let shape: Vec<String> = left_offset(&left.centreline, left.width_m / 2.0)
            .into_iter()
            .map(|(x, y)| format!("{x:.3},{y:.3}"))
            .collect();
        let _ = writeln!(
            e,
            "  <edge id=\"{id}\" from=\"j{}\" to=\"j{}\" numLanes=\"{}\" speed=\"{speed:.2}\" spreadType=\"right\" shape=\"{}\">",
            edge.from.index(),
            edge.to.index(),
            lanes.len(),
            shape.join(" ")
        );
        for (i, l) in lanes.iter().enumerate() {
            let lane = world.lane(*l);
            let allow = allow_list(world, lane)
                .map(|a| format!(" allow=\"{a}\""))
                .unwrap_or_default();
            let _ = writeln!(
                e,
                "    <lane index=\"{i}\" speed=\"{:.2}\" width=\"{:.2}\"{allow}/>",
                lane.speed_limit_mps, lane.width_m
            );
        }
        e.push_str("  </edge>\n");
    }
    e.push_str("</edges>\n");
    let mut c = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<connections>\n");
    for con in world.roads.connections() {
        if !con.permitted {
            continue;
        }
        let (Some(from), Some(to)) = (ids.sumo_lane.get(&con.from_lane), ids.sumo_lane.get(&con.to_lane))
        else {
            continue;
        };
        let (fe, fl) = from.rsplit_once('_').unwrap_or((from, "0"));
        let (te, tl) = to.rsplit_once('_').unwrap_or((to, "0"));
        let _ = writeln!(
            c,
            "  <connection from=\"{fe}\" to=\"{te}\" fromLane=\"{fl}\" toLane=\"{tl}\"/>"
        );
    }
    c.push_str("</connections>\n");
    px.nodes = n;
    px.edges = e;
    px.connections = c;
    px
}

/// Writes the plain XML under `dir` with prefix `stem` and runs `netconvert` on it, giving
/// `<dir>/<stem>.net.xml`. Returns the network's path and netconvert's log.
///
/// # Errors
/// I/O, or netconvert failing (its log in the message).
pub fn export_net(
    world: &World,
    ids: &SumoIds,
    netconvert: &Path,
    dir: &Path,
    stem: &str,
) -> Result<(PathBuf, String), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let px = to_plain_xml(world, ids);
    let p = |ext: &str| dir.join(format!("{stem}.{ext}"));
    for (ext, body) in [("nod.xml", &px.nodes), ("edg.xml", &px.edges), ("con.xml", &px.connections)] {
        std::fs::write(p(ext), body).map_err(|e| e.to_string())?;
    }
    let out = p("net.xml");
    let mut cmd = Command::new(netconvert);
    if let Some(home) = netconvert.parent().and_then(Path::parent) {
        cmd.env("SUMO_HOME", home);
    }
    let o = cmd
        .arg("--node-files")
        .arg(p("nod.xml"))
        .arg("--edge-files")
        .arg(p("edg.xml"))
        .arg("--connection-files")
        .arg(p("con.xml"))
        .arg("--output-file")
        .arg(&out)
        .arg("--no-turnarounds")
        .arg("true")
        .output()
        .map_err(|e| format!("{}: {e}", netconvert.display()))?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    if !o.status.success() {
        return Err(format!("netconvert failed: {log}"));
    }
    Ok((out, log))
}
