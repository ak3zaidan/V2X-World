//! `v2xw export-sumo SCENARIO --out DIR`: the scenario's world and demand in SUMO formats.
//!
//! Writes `<stem>.nod.xml`, `<stem>.edg.xml`, `<stem>.con.xml` and, when `netconvert` is
//! found (`$SUMO_HOME`, `$V2XW_SUMO_VENV`), `<stem>.net.xml`; and `<stem>.rou.xml` with every
//! trip the scenario's demand model draws over the run, each with the route this engine's
//! router plans for it (`v2xw_interop::netexport`, `v2xw_interop::routes`). A world that was
//! itself imported from a `net.xml` keeps its SUMO edge ids, so its `.rou.xml` runs on the
//! original network unchanged.

use std::path::Path;

use serde::Serialize;
use v2xw_core::rng::RngRegistry;
use v2xw_mobility::MobilityCtx;

use crate::error::{CliError, Result};

/// What was written.
#[derive(Debug, Clone, Serialize)]
pub struct ExportOutcome {
    /// Files written.
    pub files: Vec<String>,
    /// Edges exported.
    pub edges: usize,
    /// Trips written.
    pub trips: usize,
    /// Trips the router could not route (left out).
    pub unrouted: usize,
    /// netconvert's verdict, when it ran.
    pub netconvert: Option<String>,
}

/// Runs the export.
///
/// # Errors
/// The scenario failing to load or its world to build; a file failing to write.
pub fn export(scenario: &Path, out: &Path) -> Result<ExportOutcome> {
    let s = v2xw_engine::scenario::Scenario::load(scenario)?;
    let world = v2xw_engine::wiring::build_world(&s)?;
    let ids = v2xw_interop::SumoIds::new(&world);
    let stem = scenario
        .file_stem()
        .map_or_else(|| "scenario".to_string(), |s| s.to_string_lossy().into_owned());
    let io = |what: &'static str, p: &Path, e: std::io::Error| CliError::Io {
        what,
        path: p.display().to_string(),
        source: e,
    };
    std::fs::create_dir_all(out).map_err(|e| io("cannot create", out, e))?;
    let px = v2xw_interop::netexport::to_plain_xml(&world, &ids);
    let mut files = Vec::new();
    for (ext, body) in [("nod.xml", &px.nodes), ("edg.xml", &px.edges), ("con.xml", &px.connections)] {
        let p = out.join(format!("{stem}.{ext}"));
        std::fs::write(&p, body).map_err(|e| io("cannot write", &p, e))?;
        files.push(p.display().to_string());
    }
    let netconvert = v2xw_interop::sumo_home().map(|home| {
        match v2xw_interop::netexport::export_net(
            &world,
            &ids,
            &v2xw_interop::sumo_tool(&home, "netconvert"),
            out,
            &stem,
        ) {
            Ok((p, _)) => {
                files.push(p.display().to_string());
                "ok".to_string()
            }
            Err(e) => e,
        }
    });

    let mut demand = v2xw_engine::wiring::build_demand(&s, &world)?;
    let rng = RngRegistry::new(s.seed);
    let horizon = (s.time.duration_s * 1e9) as u64;
    let mut t = 0u64;
    let mut trips = Vec::new();
    let mut unrouted = 0;
    while t < horizon {
        let t1 = (t + 1_000_000_000).min(horizon);
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        for trip in demand.spawns_in(&mut ctx, t, t1) {
            let lanes = demand
                .planned_route(&world, &trip)
                .or_else(|| v2xw_interop::route_trip(&world, &trip));
            match lanes {
                Some(l) => {
                    let edges = ids.edges_of_route(&world, &l);
                    trips.push((trip, edges));
                }
                None => unrouted += 1,
            }
        }
        t = t1;
    }
    let rou = out.join(format!("{stem}.rou.xml"));
    std::fs::write(&rou, v2xw_interop::routes::write_rou_xml(&trips, &ids, &world))
        .map_err(|e| io("cannot write", &rou, e))?;
    files.push(rou.display().to_string());
    Ok(ExportOutcome {
        files,
        edges: ids.edge_id.len(),
        trips: trips.len(),
        unrouted,
        netconvert,
    })
}

/// The outcome as text.
pub fn format(o: &ExportOutcome) -> String {
    let mut s = format!(
        "exported {} edges and {} trips ({} unrouted, left out)\n",
        o.edges, o.trips, o.unrouted
    );
    match &o.netconvert {
        Some(v) if v == "ok" => s.push_str("netconvert   built the .net.xml\n"),
        Some(e) => s.push_str(&format!("netconvert   failed: {e}\n")),
        None => s.push_str(
            "netconvert   not found (set SUMO_HOME or V2XW_SUMO_VENV); run netconvert on the plain XML yourself\n",
        ),
    }
    for f in &o.files {
        s.push_str(&format!("  {f}\n"));
    }
    s
}
