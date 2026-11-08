//! The street as NYC builds it: bus, cycle and parking lanes from the tags
//! (`v2xw_world::section`), who turns across whom, and signal timing that shares the green
//! by lanes, keeps pedestrians' crossing time, and runs a green wave along an avenue.
//!
//! Fixtures are hand-written OSM at latitude 0, where 0.001° is 111.3 m (see tests/osm.rs).

use v2xw_world::osm::{HighwayPreset, ImportReport, OsmOptions, import_osm_bytes};
use v2xw_world::{ClassMask, LaneKind, SignalState, TurnDirection, World};

fn opts() -> OsmOptions {
    OsmOptions::default()
        .imported_at("2026-09-18T00:00:00Z")
        .highway_preset(HighwayPreset::UrbanUsNyc)
}

fn import_with(xml: &str, options: &OsmOptions) -> (World, ImportReport) {
    import_osm_bytes(xml.as_bytes(), "fixture", options).expect("the fixture imports")
}

fn document(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<osm version=\"0.6\" generator=\"test\">\n\
         <bounds minlat=\"-0.004\" minlon=\"-0.004\" maxlat=\"0.004\" maxlon=\"0.004\"/>\n{body}\n</osm>\n"
    )
}

fn node(id: i64, lat: f64, lon: f64, tags: &[(&str, &str)]) -> String {
    let inner: String = tags
        .iter()
        .map(|(k, v)| format!("  <tag k=\"{k}\" v=\"{v}\"/>\n"))
        .collect();
    format!("<node id=\"{id}\" lat=\"{lat:.7}\" lon=\"{lon:.7}\">\n{inner}</node>\n")
}

fn way(id: i64, nodes: &[i64], tags: &[(&str, &str)]) -> String {
    let refs: String = nodes
        .iter()
        .map(|n| format!("  <nd ref=\"{n}\"/>\n"))
        .collect();
    let inner: String = tags
        .iter()
        .map(|(k, v)| format!("  <tag k=\"{k}\" v=\"{v}\"/>\n"))
        .collect();
    format!("<way id=\"{id}\">\n{refs}{inner}</way>\n")
}

/// First Avenue's tags as the Midtown extract maps them (way 46469729 and its
/// neighbours): one-way, four lanes of which the rightmost is a bus lane, a
/// parking-protected cycle track on the left, parking on both sides.
const AVENUE: &[(&str, &str)] = &[
    ("highway", "primary"),
    ("oneway", "yes"),
    ("lanes", "4"),
    ("bus:lanes", "|||designated"),
    ("cycleway:left", "track"),
    ("parking:both", "lane"),
    ("name", "1st Avenue"),
];

/// A one-way avenue running north through a crossroads with a two-way street, the
/// crossroads signalised when `signals`.
fn avenue_crossroads(signals: bool) -> String {
    let centre: &[(&str, &str)] = if signals {
        &[("highway", "traffic_signals")]
    } else {
        &[]
    };
    [
        node(1, -0.001, 0.0, &[]),
        node(2, 0.0, 0.0, centre),
        node(3, 0.001, 0.0, &[]),
        node(4, 0.0, -0.001, &[]),
        node(5, 0.0, 0.001, &[]),
        way(10, &[1, 2], AVENUE),
        way(11, &[2, 3], AVENUE),
        way(
            12,
            &[4, 2, 5],
            &[
                ("highway", "residential"),
                ("lanes", "2"),
                ("name", "East 50th Street"),
            ],
        ),
    ]
    .concat()
}

#[test]
fn an_avenue_carries_its_bus_lane_track_and_parking() {
    let (world, report) = import_with(&document(&avenue_crossroads(false)), &opts());
    // Per way: one bus lane, one track, two parking lanes.
    assert_eq!(report.counts.bus_lanes, 2);
    assert_eq!(report.counts.cycle_lanes_on_roads, 2);
    assert_eq!(report.counts.parking_lanes, 4);

    // The northbound approach edge: its lanes right to left.
    // (The world's origin is the extract's south-west corner, so compare, don't threshold.)
    let edge = world
        .roads
        .edges()
        .iter()
        .filter(|e| e.lanes.len() == 7)
        .min_by(|a, b| {
            world
                .lane(a.lanes[0])
                .start()
                .y
                .total_cmp(&world.lane(b.lanes[0]).start().y)
        })
        .expect("the southern block's edge");
    let kinds: Vec<LaneKind> = edge.lanes.iter().map(|l| world.lane(*l).kind).collect();
    assert_eq!(
        kinds,
        vec![
            LaneKind::Parking,
            LaneKind::Bus,
            LaneKind::Driving,
            LaneKind::Driving,
            LaneKind::Driving,
            LaneKind::Parking,
            LaneKind::Cycle
        ]
    );
    let lane = |k: usize| world.lane(edge.lanes[k]);
    // NYC widths: 8 ft parking, 11 ft bus, 10 ft general (NYC DOT's typical moving lane),
    // a 5 ft track 3 ft beyond the left parking lane.
    assert!((lane(0).width_m - 2.438).abs() < 1e-9);
    assert!((lane(1).width_m - 3.353).abs() < 1e-9);
    assert!((lane(2).width_m - 3.048).abs() < 1e-9);
    assert!((lane(6).width_m - 1.524).abs() < 1e-9);
    // Right to left along +x (the avenue runs north, so its left is west, -x).
    for k in 1..7 {
        assert!(
            lane(k).start().x < lane(k - 1).start().x,
            "lane {k} is left of lane {}",
            k - 1
        );
    }
    let gap =
        (lane(5).start().x - 0.5 * lane(5).width_m) - (lane(6).start().x + 0.5 * lane(6).width_m);
    assert!(
        (gap - 0.914).abs() < 1e-6,
        "the track's buffer is 3 ft, got {gap}"
    );
    // Nobody drives in a parking lane; buses only in the bus lane; bicycles on the track.
    assert_eq!(lane(0).allowed, ClassMask::NONE);
    assert!(lane(1).admits(ClassMask::BUS) && !lane(1).admits(ClassMask::CAR));
    assert!(
        !lane(1).admits(ClassMask::BICYCLE),
        "the avenue has a track, so no bikes in the bus lane"
    );
    assert_eq!(lane(6).allowed, ClassMask::BICYCLE);

    // At the crossroads: no connection leaves a parking lane; cars never enter the bus lane
    // or the track; the track runs on into the next block's track through a connector.
    for c in world.roads.connections() {
        let from = world.lane(c.from_lane);
        let to = world.lane(c.to_lane);
        assert_ne!(from.kind, LaneKind::Parking, "a parking lane goes nowhere");
        assert_ne!(
            to.kind,
            LaneKind::Parking,
            "nothing drives into a parking lane"
        );
        if from.kind == LaneKind::Driving {
            assert!(
                to.admits(ClassMask::CAR),
                "a car from lane {} is sent into {:?}",
                from.id.index(),
                to.kind
            );
        }
        if let Some(via) = c.via {
            let v = world.lane(via);
            assert!(!v.allowed.is_empty());
            assert!(from.allowed.contains_any(v.allowed));
        }
    }
    let track_out: Vec<_> = world
        .roads
        .connections()
        .iter()
        .filter(|c| c.from_lane == edge.lanes[6])
        .collect();
    assert!(
        track_out
            .iter()
            .any(|c| c.direction == TurnDirection::Straight
                && world.lane(c.to_lane).kind == LaneKind::Cycle
                && c.via.is_some()),
        "the track continues through the junction: {track_out:?}"
    );
    assert!(world.validate().is_ok());

    // Switched off, the same avenue is four general lanes — the check above can fail.
    let mut plain = opts();
    plain.cross_section.bus_lanes = false;
    plain.cross_section.cycle_lanes = false;
    plain.cross_section.parking_lanes = false;
    let (world, report) = import_with(&document(&avenue_crossroads(false)), &plain);
    assert_eq!(report.counts.bus_lanes + report.counts.parking_lanes, 0);
    assert!(
        world
            .roads
            .lanes()
            .iter()
            .all(|l| matches!(l.kind, LaneKind::Driving | LaneKind::Internal))
    );
}

/// A street with a painted cycle lane on its right, between the parking lane and traffic.
const LANE_STREET: &[(&str, &str)] = &[
    ("highway", "secondary"),
    ("oneway", "yes"),
    ("lanes", "2"),
    ("cycleway:right", "lane"),
    ("parking:right", "lane"),
];

#[test]
fn a_car_turning_across_the_cycle_lane_beside_it_yields_to_it() {
    // A right turn off a street with a painted lane on its right crosses the lane's
    // straight-on path. The two are not ranked by the opposing-traffic rule (neither crosses
    // opposing traffic), so only the same-approach rule makes the car give way.
    let body = [
        node(1, -0.001, 0.0, &[]),
        node(2, 0.0, 0.0, &[]),
        node(3, 0.001, 0.0, &[]),
        node(4, 0.0, -0.001, &[]),
        node(5, 0.0, 0.001, &[]),
        way(10, &[1, 2], LANE_STREET),
        way(11, &[2, 3], LANE_STREET),
        way(
            12,
            &[4, 2, 5],
            &[("highway", "residential"), ("lanes", "2")],
        ),
    ]
    .concat();
    let (world, _) = import_with(&document(&body), &opts());
    let junction = world
        .roads
        .junctions()
        .iter()
        .find(|j| j.internal.len() > 4)
        .expect("the crossroads");
    let movement = |pred: &dyn Fn(LaneKind, TurnDirection) -> bool| {
        junction.internal.iter().position(|via| {
            world
                .roads
                .connections()
                .iter()
                .any(|c| c.via == Some(*via) && pred(world.lane(c.from_lane).kind, c.direction))
        })
    };
    let right = movement(&|k, d| k == LaneKind::Driving && d == TurnDirection::Right)
        .expect("a right turn off the street");
    let bike = movement(&|k, d| k == LaneKind::Cycle && d == TurnDirection::Straight)
        .expect("the cycle lane going straight on");
    assert!(
        junction.conflicts.is_foe(right, bike),
        "the right turn crosses the cycle lane"
    );
    assert!(junction.conflicts.must_yield(right, bike));
    assert!(!junction.conflicts.must_yield(bike, right));
}

#[test]
fn the_green_is_shared_by_lanes_and_leaves_pedestrians_time_to_cross() {
    let (world, _) = import_with(&document(&avenue_crossroads(true)), &opts());
    let plan = &world.signals[0];
    assert!((plan.total_phase_duration_s() - plan.cycle_s).abs() < 1e-6);
    assert!(
        (plan.cycle_s - 90.0).abs() < 1e-6,
        "the 90 s target cycle is met: {}",
        plan.cycle_s
    );
    // Phase 0 is the avenue's green: its four traffic lanes outweigh the street's one.
    let greens: Vec<(usize, f64)> = plan
        .phases
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            p.states
                .iter()
                .any(|s| matches!(s, SignalState::Green | SignalState::GreenYield))
        })
        .map(|(i, p)| (i, p.duration_s))
        .collect();
    assert_eq!(greens.len(), 2);
    let movement_edge_is_avenue = |state_index: usize| {
        let via = plan.controlled[state_index];
        let c = world
            .roads
            .connections()
            .iter()
            .find(|c| c.via == Some(via))
            .unwrap();
        world.lane(c.from_lane).kind != LaneKind::Driving
            || world.edge(world.lane(c.from_lane).edge).lanes.len() > 3
    };
    let first_green_states = &plan.phases[greens[0].0].states;
    assert!(
        first_green_states
            .iter()
            .enumerate()
            .filter(|(_, s)| s.permits_entry())
            .all(|(i, _)| movement_edge_is_avenue(i)),
        "the avenue runs first"
    );
    let (avenue, street) = (greens[0].1, greens[1].1);
    assert!(
        avenue > 1.5 * street,
        "avenue {avenue} s against street {street} s"
    );
    // The street's green lets a pedestrian cross the avenue: walk 7 s plus its width at
    // 3.5 ft/s, less the street phase's yellow and all-red.
    let street_phase = greens[1].0;
    let change: f64 = plan.phases[street_phase + 1..]
        .iter()
        .take_while(|p| {
            !p.states
                .iter()
                .any(|s| matches!(s, SignalState::Green | SignalState::GreenYield))
        })
        .map(|p| p.duration_s)
        .sum();
    // Kerb to kerb: every lane of the avenue's edge (two parking lanes, the bus lane, three
    // general lanes and the track).
    let avenue_width: f64 = world
        .roads
        .edges()
        .iter()
        .filter(|e| e.lanes.len() == 7)
        .map(|e| e.lanes.iter().map(|l| world.lane(*l).width_m).sum::<f64>())
        .fold(0.0, f64::max);
    assert!(avenue_width > 15.0, "{avenue_width}");
    let need = 7.0 + avenue_width / 1.067 - change;
    assert!(
        street + 1e-6 >= need,
        "street green {street} s, pedestrians need {need} s"
    );

    // Equal split, as netconvert: the avenue no longer outweighs the street.
    let mut equal = opts();
    equal.signals.split_by_lanes = false;
    equal.signals.pedestrian_min_green = false;
    let (world, _) = import_with(&document(&avenue_crossroads(true)), &equal);
    let plan = &world.signals[0];
    let g: Vec<f64> = plan
        .phases
        .iter()
        .filter(|p| {
            p.states
                .iter()
                .any(|s| matches!(s, SignalState::Green | SignalState::GreenYield))
        })
        .map(|p| p.duration_s)
        .collect();
    assert!((g[0] - g[1]).abs() < 1.0, "equal split {g:?}");
}

#[test]
fn signals_along_an_avenue_run_a_green_wave() {
    // Three signalised crossings 0.002° (222.6 m) apart on a one-way avenue running north.
    let signal = &[("highway", "traffic_signals")][..];
    let mut body = Vec::new();
    for (i, lat) in [-0.002, 0.0, 0.002].iter().enumerate() {
        let c = 100 + i as i64 * 10;
        body.push(node(c, *lat, 0.0, signal));
        body.push(node(c + 1, *lat, -0.001, &[]));
        body.push(node(c + 2, *lat, 0.001, &[]));
        body.push(way(
            c + 5,
            &[c + 1, c, c + 2],
            &[
                ("highway", "residential"),
                ("oneway", "yes"),
                ("lanes", "1"),
            ],
        ));
    }
    body.push(node(99, -0.003, 0.0, &[]));
    body.push(node(131, 0.003, 0.0, &[]));
    body.push(way(
        50,
        &[99, 100, 110, 120, 131],
        &[
            ("highway", "primary"),
            ("oneway", "yes"),
            ("lanes", "3"),
            ("maxspeed", "25 mph"),
        ],
    ));
    let xml = document(&body.concat());
    let (world, report) = import_with(&xml, &opts());
    assert_eq!(world.signals.len(), 3);
    assert_eq!(report.counts.signals_coordinated, 2);
    let mut plans: Vec<_> = world.signals.iter().collect();
    plans.sort_by(|a, b| {
        world
            .junction(a.junction)
            .position
            .y
            .total_cmp(&world.junction(b.junction).position.y)
    });
    let v = 25.0 * 0.44704;
    for w in plans.windows(2) {
        let (a, b) = (w[0], w[1]);
        let d = world.junction(b.junction).position.y - world.junction(a.junction).position.y;
        let mut lag = (b.offset_s - a.offset_s) % a.cycle_s;
        if lag < 0.0 {
            lag += a.cycle_s;
        }
        // The travel time from one stop line to the next, within the connector's length.
        let t = d / v;
        assert!(
            (lag - t % a.cycle_s).abs() < 3.0,
            "offset lag {lag:.1} s against {t:.1} s of travel over {d:.1} m"
        );
    }
    // Uncoordinated, every plan starts at t0.
    let mut off = opts();
    off.signals.coordinate = false;
    let (world, report) = import_with(&xml, &off);
    assert_eq!(report.counts.signals_coordinated, 0);
    assert!(world.signals.iter().all(|p| p.offset_s == 0.0));
}

/// A newsstand mapped onto the carriageway is dropped; a building the road passes
/// through is a passage and stays.
#[test]
fn a_kiosk_on_the_carriageway_is_dropped_and_a_real_building_kept() {
    use v2xw_world::osm::Anomaly;
    // A two-lane street running east along latitude 0; a 2 x 3 m kiosk straddling its
    // right lane, and a 30 x 30 m block the street runs through further on.
    let d = 0.000_009; // about 1 m
    let body = [
        node(1, 0.0, -0.001, &[]),
        node(2, 0.0, 0.001, &[]),
        way(
            10,
            &[1, 2],
            &[
                ("highway", "residential"),
                ("oneway", "yes"),
                ("lanes", "2"),
            ],
        ),
        node(20, -2.0 * d, -0.0005, &[]),
        node(21, -2.0 * d, -0.0005 + 3.0 * d, &[]),
        node(22, 0.0, -0.0005 + 3.0 * d, &[]),
        node(23, 0.0, -0.0005, &[]),
        way(
            30,
            &[20, 21, 22, 23, 20],
            &[("building", "yes"), ("shop", "newsagent")],
        ),
        node(40, -15.0 * d, 0.0004, &[]),
        node(41, -15.0 * d, 0.0004 + 30.0 * d, &[]),
        node(42, 15.0 * d, 0.0004 + 30.0 * d, &[]),
        node(43, 15.0 * d, 0.0004, &[]),
        way(50, &[40, 41, 42, 43, 40], &[("building", "yes")]),
    ]
    .concat();
    let (world, report) = import_with(&document(&body), &opts());
    assert_eq!(
        report.anomaly(Anomaly::KioskOnCarriageway),
        1,
        "{}",
        report.to_text()
    );
    assert_eq!(world.buildings.len(), 1, "the block stays");
    assert!(world.validate().is_ok());
}

/// The world-validation gate (`v2xw_world::validate`, the `world_report --baseline`
/// regression check) passes the avenue as imported, and fails when the cross-section
/// regresses to general lanes only.
#[test]
fn the_validation_gate_fails_when_the_cross_section_regresses() {
    use v2xw_world::validate::{SourceLink, ValidationParams, validate};
    let xml = document(&avenue_crossroads(true));
    let file = v2xw_world::osm::parse_osm(xml.as_bytes()).expect("parses");
    let (world, report) = import_with(&xml, &opts());
    let good = validate(
        &world,
        Some(SourceLink {
            file: &file,
            edges: &report.edge_sources,
        }),
        &ValidationParams::default(),
    );
    for check in [
        "bus-lanes-mismatch",
        "cycle-lane-missing",
        "parking-lane-missing",
        "lanes-mismatch",
        "oneway-mismatch",
    ] {
        let c = &good.checks[check];
        assert!(c.of > 0, "{check} examined nothing:\n{}", good.to_text());
        assert_eq!(c.count, 0, "{check}:\n{}", good.to_text());
    }
    let baseline = good.as_baseline("the avenue fixture");
    assert!(good.regressions(&baseline).is_empty());

    let mut plain = opts();
    plain.cross_section.bus_lanes = false;
    plain.cross_section.cycle_lanes = false;
    plain.cross_section.parking_lanes = false;
    let (world, report) = import_with(&xml, &plain);
    let bad = validate(
        &world,
        Some(SourceLink {
            file: &file,
            edges: &report.edge_sources,
        }),
        &ValidationParams::default(),
    );
    let regressions = bad.regressions(&baseline);
    for check in [
        "bus-lanes-mismatch",
        "cycle-lane-missing",
        "parking-lane-missing",
    ] {
        assert!(
            regressions.iter().any(|r| r.starts_with(check)),
            "{check} did not regress: {regressions:?}"
        );
    }
}

/// `world.buildings.keep_holes` and `metres_per_level` change the world: the QA of
/// 2026-09-24 saw no effect on a 20 s run's digest, which is the radio's, not the world's.
#[test]
fn building_holes_and_storey_height_change_the_world() {
    let body = [
        node(1, 0.0, 0.0, &[]),
        node(2, 0.0, 0.001, &[]),
        node(3, 0.001, 0.001, &[]),
        node(4, 0.001, 0.0, &[]),
        node(5, 0.0003, 0.0003, &[]),
        node(6, 0.0003, 0.0007, &[]),
        node(7, 0.0007, 0.0007, &[]),
        node(8, 0.0007, 0.0003, &[]),
        way(10, &[1, 2, 3, 4, 1], &[]),
        way(11, &[5, 6, 7, 8, 5], &[]),
        "<relation id=\"20\">\n  <member type=\"way\" ref=\"10\" role=\"outer\"/>\n  \
         <member type=\"way\" ref=\"11\" role=\"inner\"/>\n  <tag k=\"type\" v=\"multipolygon\"/>\n  \
         <tag k=\"building\" v=\"yes\"/>\n  <tag k=\"building:levels\" v=\"7\"/>\n</relation>\n"
            .to_string(),
    ]
    .concat();
    let xml = document(&body);
    let (base, _) = import_with(&xml, &opts());
    let mut no_holes = opts();
    no_holes.import.keep_building_holes = false;
    let (w_holes, _) = import_with(&xml, &no_holes);
    let mut tall = opts();
    tall.import.metres_per_level = 4.0;
    let (w_tall, _) = import_with(&xml, &tall);
    assert_eq!(base.buildings[0].holes.len(), 1);
    assert!(w_holes.buildings[0].holes.is_empty());
    assert!((base.buildings[0].height_m - 21.0).abs() < 1e-9);
    assert!((w_tall.buildings[0].height_m - 28.0).abs() < 1e-9);
    let h = v2xw_world::hash::content_hash_hex;
    assert_ne!(h(&base), h(&w_holes));
    assert_ne!(h(&base), h(&w_tall));
}
