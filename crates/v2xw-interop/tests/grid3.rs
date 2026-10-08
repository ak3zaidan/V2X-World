//! The shipped SUMO grid: id mapping, `.rou.xml` demand and plain-XML export, without
//! needing SUMO installed. The TraCI path is exercised by `cross_validate`.

use std::path::PathBuf;

use v2xw_core::rng::RngRegistry;
use v2xw_interop::SumoIds;
use v2xw_interop::routes::{RouteFile, RouteFileDemand};
use v2xw_mobility::MobilityCtx;
use v2xw_mobility::traits::Demand;
use v2xw_world::sumo::{SumoOptions, import_sumo_net};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn route_file_demand_and_export_on_the_shipped_grid() {
    let opts = SumoOptions::default().imported_at("1970-01-01T00:00:00Z");
    let (world, _) = import_sumo_net(root().join("worlds/sumo/grid3.net.xml"), &opts).unwrap();
    let ids = SumoIds::new(&world);
    // netgenerate's 3 x 3 grid has 24 normal edges of 2 lanes.
    assert_eq!(ids.edge_id.len(), 24);
    assert_eq!(ids.lane_of.len(), 48);
    assert!(ids.lane_of.contains_key("A1B1_1"));

    let file = RouteFile::read(&root().join("worlds/sumo/grid3.rou.xml")).unwrap();
    let mut demand = RouteFileDemand::new(file);
    let rng = RngRegistry::new(1);
    let mut ctx = MobilityCtx::new(0, &world, &rng);
    let trips = demand.spawns_in(&mut ctx, 0, 60_000_000_000);
    // 1,200 veh/h for 60 s, equally spaced per flow: 9 + 9 + 4 departures in [0, 60).
    assert_eq!(trips.len(), 22, "{trips:?}");
    assert_eq!(demand.unresolved, 0);
    for t in &trips {
        assert_eq!(ids.edge_of_lane(&world, t.origin).is_some(), true);
    }

    let px = v2xw_interop::netexport::to_plain_xml(&world, &ids);
    assert_eq!(px.edges.matches("<edge ").count(), 24);
    assert!(px.connections.matches("<connection ").count() >= 24);
    assert!(px.nodes.contains("type=\"traffic_light\""));
}

/// SUMO drives the shipped grid's route file through the plug-in. Skipped (passes) when no
/// SUMO is installed; set `V2XW_SUMO_VENV` or `SUMO_HOME` to run it.
#[test]
fn sumo_drives_the_route_file_over_traci() {
    use v2xw_mobility::traits::Mobility;
    let Some(home) = v2xw_interop::sumo_home() else {
        eprintln!("SUMO not found: skipped");
        return;
    };
    let opts = SumoOptions::default().imported_at("1970-01-01T00:00:00Z");
    let net = root().join("worlds/sumo/grid3.net.xml");
    let (world, report) = import_sumo_net(&net, &opts).unwrap();
    let mut cfg = v2xw_interop::sumo::SumoConfig::new(&home, &net);
    cfg.route_files = vec![root().join("worlds/sumo/grid3.rou.xml")];
    cfg.shift_m = report.shift_m;
    let mut sumo = v2xw_interop::sumo::SumoMobility::new(cfg, SumoIds::new(&world));
    let rng = RngRegistry::new(1);
    let mut ctx = MobilityCtx::new(0, &world, &rng);
    sumo.init(&mut ctx, Box::new(v2xw_mobility::NoDemand::new()))
        .unwrap();
    let mut spawned = 0;
    let mut t = 0;
    let mut last = None;
    for _ in 0..300 {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let u = sumo.step(&mut ctx, v2xw_core::time::Duration::from_millis(100));
        t = u.t;
        spawned += u.spawned.len();
        last = Some(u);
    }
    assert_eq!(sumo.error, None);
    // 30 s of 1,200 veh/h in three flows: departures at 0, 7.2, ... s.
    assert!(spawned >= 9, "spawned {spawned}");
    let u = last.unwrap();
    // Every published vehicle is inside the world's box.
    for (_, k) in &u.states {
        assert!(
            k.pos.x > -50.0 && k.pos.y > -50.0 && k.pos.x < 500.0 && k.pos.y < 500.0,
            "{k:?}"
        );
    }
    assert!(u.states.iter().any(|(_, k)| k.lane.is_some()));
    assert!(u.states.iter().any(|(_, k)| k.vel.x.hypot(k.vel.y) > 3.0));
}
