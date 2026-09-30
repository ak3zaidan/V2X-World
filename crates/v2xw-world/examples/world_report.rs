//! `world_report` — imports an OpenStreetMap extract the way the engine does and validates
//! the world it produces: geometry a car or a pedestrian cannot use, and every imported
//! attribute against the source tags (`v2xw_world::validate`).
//!
//! ```text
//! cargo run -p v2xw-world --example world_report -- <extract.osm.xml> \
//!     [--bbox MIN_LAT,MIN_LON,MAX_LAT,MAX_LON] [--preset urban-us-nyc] \
//!     [--baseline worlds/validation/<city>.json] [--write-baseline FILE] [--json FILE] \
//!     [--examples N]
//! ```
//!
//! With `--baseline`, it exits with status 1 and names each check whose failure count rose
//! above the baseline's ceiling: the regression gate for a city's import. `--write-baseline`
//! writes the current counts as a new baseline (review the diff before committing one).
//! The options are `OsmOptions::default()` plus the preset and the box, which is exactly
//! what `v2xw_engine::wiring::build_world` imports a scenario's `osm-xml` world with.

use v2xw_world::GeoBbox;
use v2xw_world::osm::{HighwayPreset, OsmOptions, import_osm, parse_osm};
use v2xw_world::validate::{Baseline, SourceLink, ValidationParams, validate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let path = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or(
            "usage: world_report <extract.osm.xml> [--bbox a,b,c,d] [--preset P] [--baseline F]",
        )?
        .clone();
    let preset_name = value("--preset").unwrap_or_else(|| "urban-us-nyc".to_string());
    let preset = HighwayPreset::parse(&preset_name)
        .ok_or_else(|| format!("unknown speed preset {preset_name}"))?;
    let mut options = OsmOptions::default()
        .imported_at("1970-01-01T00:00:00Z")
        .highway_preset(preset);
    if let Some(b) = value("--bbox") {
        let v: Vec<f64> = b
            .split(',')
            .map(|x| x.trim().parse::<f64>())
            .collect::<Result<_, _>>()?;
        if v.len() != 4 {
            return Err("--bbox needs MIN_LAT,MIN_LON,MAX_LAT,MAX_LON".into());
        }
        options = options.bbox(GeoBbox::new(v[0], v[1], v[2], v[3]));
    }
    let mut params = ValidationParams::default();
    if let Some(n) = value("--examples") {
        params.examples = n.parse()?;
    }

    let started = std::time::Instant::now();
    let (world, report) = import_osm(&path, &options)?;
    let imported = started.elapsed();
    let file = parse_osm(&std::fs::read(&path)?)?;
    let result = validate(
        &world,
        Some(SourceLink {
            file: &file,
            edges: &report.edge_sources,
        }),
        &params,
    );
    eprintln!(
        "{path}: {} lanes, {} junctions, {} signal plans, {} buildings; imported in {:.1} s",
        world.roads.lanes().len(),
        world.roads.junctions().len(),
        world.signals.len(),
        world.buildings.len(),
        imported.as_secs_f64()
    );
    // `--describe-lane N[,M...]`: what a lane is, and the tags of the way it came from.
    if let Some(list) = value("--describe-lane") {
        for id in list.split(',').filter_map(|x| x.trim().parse::<u32>().ok()) {
            let Some(lane) = world.try_lane(v2xw_core::ids::LaneId::new(id)) else {
                continue;
            };
            let edge = world.edge(lane.edge);
            let src = report
                .edge_sources
                .get(lane.edge.as_usize())
                .copied()
                .flatten();
            println!(
                "lane {id}: {:?} index {} of edge {} ({} -> {}, {} lanes, {:?}), width {:.2} m, \
                 {:.1} m long, {} -> {} , junction {:?}, allowed {}",
                lane.kind,
                lane.index,
                edge.id.index(),
                edge.from.index(),
                edge.to.index(),
                edge.lanes.len(),
                edge.road_class,
                lane.width_m,
                lane.length_m,
                fmt_point(lane.start()),
                fmt_point(lane.end()),
                lane.junction.map(|j| j.index()),
                lane.allowed
            );
            if args.iter().any(|a| a == "--points") {
                let pts: Vec<String> = lane
                    .centreline
                    .iter()
                    .map(|p| format!("({:.2},{:.2})", p.x, p.y))
                    .collect();
                println!("    points: {}", pts.join(" "));
                let succ: Vec<String> = world
                    .successors(lane.id)
                    .iter()
                    .map(|c| {
                        format!(
                            "{}via{:?}{:?}",
                            c.to_lane.index(),
                            c.via.map(|v| v.index()),
                            c.direction
                        )
                    })
                    .collect();
                println!("    successors: {}", succ.join(" "));
            }
            if let Some(src) = src {
                if let Some(way) = file.way(src.way) {
                    let tags: Vec<String> =
                        way.tags.iter().map(|(k, v)| format!("{k}={v}")).collect();
                    println!("    way {} ({:?}): {}", src.way, src.role, tags.join(" "));
                }
            }
        }
    }
    // `--stub-analysis`: what meets at either end of every stub driving lane.
    if args.iter().any(|a| a == "--stub-analysis") {
        let mut tally: std::collections::BTreeMap<String, u32> = Default::default();
        for lane in world.roads.lanes() {
            if lane.kind != v2xw_world::LaneKind::Driving || lane.length_m >= params.stub_lane_m {
                continue;
            }
            let edge = world.edge(lane.edge);
            let mut kinds: std::collections::BTreeSet<String> = Default::default();
            for j in [edge.from, edge.to] {
                for e in world.roads.edges() {
                    if e.id == edge.id || e.from == e.to || (e.from != j && e.to != j) {
                        continue;
                    }
                    if !world.lane(e.lanes[0]).kind.is_motorised() {
                        continue;
                    }
                    let Some(src) = report.edge_sources.get(e.id.as_usize()).copied().flatten()
                    else {
                        continue;
                    };
                    let Some(way) = file.way(src.way) else {
                        continue;
                    };
                    let h = way.tags.get("highway").unwrap_or("?");
                    let sv = way
                        .tags
                        .get("service")
                        .map(|s| format!(":{s}"))
                        .unwrap_or_default();
                    kinds.insert(format!("{h}{sv}"));
                }
            }
            let own = report
                .edge_sources
                .get(edge.id.as_usize())
                .copied()
                .flatten()
                .and_then(|s| file.way(s.way))
                .map(|w| format!("{}", w.tags.get("highway").unwrap_or("?")))
                .unwrap_or_default();
            let key = format!(
                "{own} | {}",
                kinds.into_iter().collect::<Vec<_>>().join(",")
            );
            *tally.entry(key).or_default() += 1;
        }
        let mut rows: Vec<(u32, String)> = tally.into_iter().map(|(k, v)| (v, k)).collect();
        rows.sort_by(|a, b| b.cmp(a));
        for (n, k) in rows.iter().take(40) {
            println!("stub {n:5}  {k}");
        }
    }
    // `--describe-junction N[,M...]`: its OSM node, the node's tags and its arms.
    if let Some(list) = value("--describe-junction") {
        for id in list.split(',').filter_map(|x| x.trim().parse::<u32>().ok()) {
            let Some(j) = world
                .roads
                .try_junction(v2xw_core::ids::JunctionId::new(id))
            else {
                continue;
            };
            let node = report.junction_nodes.get(id as usize).copied().unwrap_or(0);
            let tags: Vec<String> = file
                .tags_of_node(node)
                .map(|t| t.iter().map(|(k, v)| format!("{k}={v}")).collect())
                .unwrap_or_default();
            println!(
                "junction {id}: node {node} [{}] at {}, control {:?}",
                tags.join(" "),
                fmt_point(j.position),
                j.control
            );
            for e in world.roads.edges() {
                if e.from.index() != id && e.to.index() != id || e.from == e.to {
                    continue;
                }
                let way = report.edge_sources.get(e.id.as_usize()).copied().flatten();
                let lane = world.lane(e.lanes[0]);
                println!(
                    "    edge {} {} -> {} {:?} {} lanes, {:.1} m, way {:?}",
                    e.id.index(),
                    e.from.index(),
                    e.to.index(),
                    lane.kind,
                    e.lanes.len(),
                    lane.length_m,
                    way.map(|w| w.way)
                );
            }
        }
    }
    println!("{}", result.to_text());
    if let Some(out) = value("--json") {
        std::fs::write(out, serde_json::to_string_pretty(&result)?)?;
    }
    if let Some(out) = value("--write-baseline") {
        let description = format!(
            "{path} (preset {preset_name}{}), OsmOptions::default()",
            value("--bbox").map_or(String::new(), |b| format!(", bbox {b}"))
        );
        std::fs::write(
            out,
            serde_json::to_string_pretty(&result.as_baseline(&description))? + "\n",
        )?;
    }
    if let Some(base) = value("--baseline") {
        let baseline: Baseline = serde_json::from_str(&std::fs::read_to_string(&base)?)?;
        let regressions = result.regressions(&baseline);
        if !regressions.is_empty() {
            eprintln!("REGRESSIONS against {base}:");
            for r in &regressions {
                eprintln!("  {r}");
            }
            std::process::exit(1);
        }
        eprintln!("no regressions against {base}");
    }
    Ok(())
}

fn fmt_point(p: v2xw_core::geom::Vec3) -> String {
    format!("({:.1}, {:.1}, {:.1})", p.x, p.y, p.z)
}
