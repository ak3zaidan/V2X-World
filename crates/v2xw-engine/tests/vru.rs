//! `actors.vru` reaches the road: pedestrians walk the sidewalks and cyclists ride, on an
//! OpenStreetMap world, through the kernel's own wiring.
//!
//! Until this build the loader refused any VRU count, because nothing spawned one. The
//! test drives the Midtown fixture extract (tests/fixtures/midtown-6block.osm.xml, which
//! carries 185 footways) for a simulated minute and checks the population is there, stays
//! at its size, keeps to walkable lanes, and moves.

use std::collections::BTreeMap;
use std::path::PathBuf;

use v2xw_core::rng::RngRegistry;
use v2xw_engine::Scenario;
use v2xw_mobility::vru::social_force::SocialForce;
use v2xw_mobility::{Mobility, MobilityCtx, VehicleClass};
use v2xw_world::WorldSourceSpec;

fn fixture_scenario(pedestrians: u32, cyclists: u32) -> Scenario {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::minimal();
    s.world.source = WorldSourceSpec::OsmXml {
        path: root
            .join("tests/fixtures/midtown-6block.osm.xml")
            .to_string_lossy()
            .into_owned(),
        bbox: None,
    };
    s.world.highway_preset = Some(v2xw_world::osm::HighwayPreset::UrbanUsNyc);
    s.world.imported_at = "2026-09-18T00:00:00Z".to_string();
    s.actors.vru.pedestrians = pedestrians;
    s.actors.vru.cyclists = cyclists;
    s
}

#[test]
fn pedestrians_and_cyclists_walk_and_ride_on_an_osm_world() {
    let scenario = fixture_scenario(25, 4);
    scenario.validate().expect("a VRU population validates");
    let world = v2xw_engine::wiring::build_world(&scenario).expect("the fixture imports");
    let rng = RngRegistry::new(scenario.seed);
    let mut mobility = v2xw_engine::wiring::native_mobility(&scenario);
    {
        let demand = v2xw_engine::wiring::build_demand(&scenario, &world).expect("demand");
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        mobility.init(&mut ctx, demand).expect("init");
    }
    let step = scenario.time.mobility_step();
    let mut first_seen: BTreeMap<u32, v2xw_core::geom::Vec3> = BTreeMap::new();
    let mut last: BTreeMap<u32, v2xw_core::geom::Vec3> = BTreeMap::new();
    let mut classes: BTreeMap<u32, VehicleClass> = BTreeMap::new();
    let mut t = 0u64;
    let mut pedestrians_now = 0usize;
    let mut cyclists_now = 0usize;
    while t < 60_000_000_000 {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = mobility.step(&mut ctx, step);
        for s in &update.spawned {
            classes.insert(s.actor.index(), s.class);
        }
        pedestrians_now = 0;
        cyclists_now = 0;
        for (actor, k) in &update.states {
            let id = actor.index();
            match classes.get(&id) {
                Some(VehicleClass::Pedestrian) => {
                    pedestrians_now += 1;
                    // On a walkable lane — or, off every lane, crossing the street
                    // mid-block, which the model must say it is doing.
                    match k.lane {
                        Some(l) => assert!(
                            SocialForce::is_walkable(&world, l.lane),
                            "pedestrian {id} on lane {:?}, which is not walkable",
                            l.lane
                        ),
                        None => assert!(
                            mobility
                                .vru()
                                .and_then(|v| v.get(*actor))
                                .is_some_and(|p| p.crossing_midblock()),
                            "pedestrian {id} is off every lane and not crossing mid-block"
                        ),
                    }
                }
                Some(VehicleClass::Bicycle) => {
                    cyclists_now += 1;
                    // Riders draw their own speeds (conventional 15.3 km/h, pedelec
                    // 17.4 km/h on average, Schleinitz et al. 2017), capped at the 25 km/h
                    // a pedelec's assistance stops at; a little over it for the speed
                    // controller's overshoot.
                    assert!(
                        k.ground_speed_mps() <= 25.0 / 3.6 + 0.1,
                        "cyclist {id} at {} m/s",
                        k.ground_speed_mps()
                    );
                }
                _ => {}
            }
            first_seen.entry(id).or_insert(k.pos);
            last.insert(id, k.pos);
        }
        t = update.t;
    }
    // The population holds at its size.
    assert_eq!(pedestrians_now, 25, "pedestrians on the world at the end");
    assert_eq!(cyclists_now, 4, "cyclists on the world at the end");
    // And they move: most walkers and every rider cover ground in a minute.
    let moved = |class: VehicleClass| {
        classes
            .iter()
            .filter(|(_, c)| **c == class)
            .filter(|(id, _)| {
                first_seen
                    .get(id)
                    .zip(last.get(id))
                    .is_some_and(|(a, b)| a.distance_2d(*b) > 5.0)
            })
            .count()
    };
    assert!(
        moved(VehicleClass::Pedestrian) >= 15,
        "{} walkers moved",
        moved(VehicleClass::Pedestrian)
    );
    assert!(
        moved(VehicleClass::Bicycle) >= 3,
        "{} riders moved",
        moved(VehicleClass::Bicycle)
    );
}

/// Runs the fixture with `pedestrians` equipped pedestrians (and a little vehicle
/// traffic) for `seconds`, on the message sets given; returns the records and the report.
fn run_equipped(
    sets: &[&str],
    seconds: f64,
) -> (v2xw_engine::MemoryRecorder, v2xw_engine::RunReport) {
    // A small signalised grid with sidewalks, crosswalks and cyclists on the carriageway.
    let mut scenario = Scenario::minimal();
    scenario.world.source = WorldSourceSpec::procedural(
        "world/source/procedural-grid",
        serde_json::json!({
            "cols": 4, "rows": 4, "block_x_m": 120.0, "block_y_m": 90.0,
            "lanes_per_direction": 1, "sidewalk_m": 2.0, "crossings": true,
            "signalised": true, "corner_radius_m": 4.5, "bicycles_on_roads": true
        }),
    );
    scenario.actors.vru.pedestrians = 12;
    scenario.actors.vru.device_fraction = 1.0;
    scenario.actors.vehicles.equipped_fraction = 1.0;
    scenario.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
    scenario.actors.vehicles.demand.rate_veh_per_h = Some(3600.0);
    scenario.messages.sets = sets.iter().map(|s| (*s).to_string()).collect();
    scenario.time.duration_s = seconds;
    scenario
        .validate()
        .expect("an equipped VRU population validates");
    let mut engine = v2xw_engine::Engine::build(scenario, "").expect("builds");
    let mut recorder = v2xw_engine::MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    (recorder, report)
}

/// The `msg_type` of every record on `channel`, with its node fields.
fn by_type(recorder: &v2xw_engine::MemoryRecorder, channel: &str) -> Vec<serde_json::Value> {
    recorder
        .records()
        .iter()
        .filter(|(_, r)| r.channel == channel)
        .map(|(_, r)| serde_json::from_slice(&r.json).expect("json"))
        .collect()
}

/// `actors.vru.device_fraction` equips pedestrians with a VRU device, hosted in the node
/// phase: on the SAE stack each sends signed SAE J2735 PSMs, which appear on `node.tx`
/// beside the vehicles' BSMs, and other nodes' receptions of them appear on `node.rx`.
#[test]
fn equipped_pedestrians_send_psms_that_other_nodes_hear() {
    let (recorder, report) = run_equipped(&["bsm"], 30.0);
    assert!(report.vru_devices_created >= 12, "{report:?}");
    let tx = by_type(&recorder, "node.tx");
    let psm: Vec<&serde_json::Value> = tx.iter().filter(|r| r["msg_type"] == "psm").collect();
    assert!(!psm.is_empty(), "no PSM on node.tx");
    // A pedestrian sends PSMs and nothing else; a vehicle sends BSMs.
    let psm_nodes: std::collections::BTreeSet<u64> =
        psm.iter().filter_map(|r| r["node"].as_u64()).collect();
    for r in &tx {
        if psm_nodes.contains(&r["node"].as_u64().unwrap_or(u64::MAX)) {
            assert_eq!(r["msg_type"], "psm", "{r}");
        }
    }
    assert!(
        tx.iter().any(|r| r["msg_type"] == "bsm"),
        "no BSM either: {report:?}"
    );
    // Signed like a vehicle's: an envelope around the payload.
    assert!(
        psm.iter()
            .all(|r| r["envelope_bytes"].as_u64().unwrap_or(0) > 0)
    );
    // A real J2735 PSM: its decoded content is on the record, with the pseudonym's first
    // four octets as its temporary id, a position on the globe and the device's speed.
    for r in &psm {
        let p = r["pseudonym"].as_str().expect("a PSM names its pseudonym");
        let c = &r["content"];
        assert_eq!(c["temp_id"].as_str(), Some(&p[..8]), "{r}");
        assert!(
            c["lat_deg"].as_f64().is_some_and(|l| l.abs() <= 90.0),
            "{r}"
        );
        assert!(
            c["speed_mps"]
                .as_f64()
                .is_some_and(|v| (0.0..3.0).contains(&v)),
            "{r}"
        );
        assert!(c["msg_count"].as_u64().is_some(), "{r}");
        // 31 octets: the mandatory PSM in its MessageFrame.
        assert_eq!(r["payload_bytes"].as_u64(), Some(31), "{r}");
    }
    // Heard: a reception attempt of a PSM decoded somewhere.
    let rx = by_type(&recorder, "node.rx");
    let heard = rx
        .iter()
        .filter(|r| r["msg_type"] == "psm" && r["outcome"] == "delivered")
        .count();
    assert!(
        heard > 0,
        "no PSM delivered: {} psm attempts",
        rx.iter().filter(|r| r["msg_type"] == "psm").count()
    );
    // And a pedestrian's own device hears the vehicles.
    assert!(
        rx.iter().any(
            |r| psm_nodes.contains(&r["rx"].as_u64().unwrap_or(u64::MAX)) && r["msg_type"] == "bsm"
        ),
        "no pedestrian received a BSM"
    );
    // Its queues and load are published like a vehicle's.
    let telemetry = by_type(&recorder, "node.telemetry");
    assert!(
        telemetry
            .iter()
            .any(|r| psm_nodes.contains(&r["node"].as_u64().unwrap_or(u64::MAX))),
        "no telemetry window from a pedestrian's device"
    );
}

/// On the ETSI stack the same pedestrians send VAMs.
#[test]
fn on_the_etsi_stack_equipped_pedestrians_send_vams() {
    let (recorder, _) = run_equipped(&["cam"], 12.0);
    let tx = by_type(&recorder, "node.tx");
    assert!(
        tx.iter().any(|r| r["msg_type"] == "vam"),
        "no VAM on node.tx"
    );
    assert!(!tx.iter().any(|r| r["msg_type"] == "psm"));
    // A real ETSI VAM, decoded on the record like a PSM.
    for r in tx.iter().filter(|r| r["msg_type"] == "vam") {
        let p = r["pseudonym"].as_str().expect("a VAM names its pseudonym");
        let c = &r["content"];
        assert_eq!(c["temp_id"].as_str(), Some(&p[..8]), "{r}");
        assert!(
            c["lon_deg"].as_f64().is_some_and(|l| l.abs() <= 180.0),
            "{r}"
        );
        assert!(c["speed_mps"].as_f64().is_some(), "{r}");
    }
}

/// Pedestrians, cyclists and their devices keep the determinism contract: the same scenario
/// twice gives byte-identical records, every channel included.
#[test]
fn a_run_with_vru_devices_is_deterministic() {
    let digest = |r: &v2xw_engine::MemoryRecorder| -> Vec<u8> {
        let mut out = Vec::new();
        for (t, rec) in r.records() {
            out.extend_from_slice(&t.to_le_bytes());
            out.extend_from_slice(rec.channel.as_bytes());
            out.extend_from_slice(&rec.json);
        }
        out
    };
    let (a, ra) = run_equipped(&["bsm"], 6.0);
    let (b, rb) = run_equipped(&["bsm"], 6.0);
    assert!(ra.vru_devices_created > 0);
    assert_eq!(ra.records, rb.records);
    assert!(digest(&a) == digest(&b), "two runs of one scenario differ");
}

/// `security.verification_policy` reaches a pedestrian's device. A handset used to run
/// `verify-all` whatever the scenario said, so every check it made carried priority 0 and
/// was served first in, first out; under `prioritized` it now serves the near first, by the
/// same rule as a vehicle.
#[test]
fn a_vru_device_runs_the_scenarios_verification_policy() {
    let run = |policy: &str| {
        let mut scenario = Scenario::minimal();
        scenario.world.source = WorldSourceSpec::procedural(
            "world/source/procedural-grid",
            serde_json::json!({
                "cols": 4, "rows": 4, "block_x_m": 120.0, "block_y_m": 90.0,
                "lanes_per_direction": 1, "sidewalk_m": 2.0, "crossings": true,
                "signalised": true, "corner_radius_m": 4.5, "bicycles_on_roads": true
            }),
        );
        scenario.actors.vru.pedestrians = 12;
        scenario.actors.vru.device_fraction = 1.0;
        scenario.actors.vehicles.equipped_fraction = 1.0;
        scenario.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
        scenario.actors.vehicles.demand.rate_veh_per_h = Some(3600.0);
        scenario.messages.sets = vec!["bsm".to_string()];
        scenario.time.duration_s = 20.0;
        scenario.security.verification_policy = policy.to_string();
        scenario.validate().expect("validates");
        let mut engine = v2xw_engine::Engine::build(scenario, "").expect("builds");
        let mut recorder = v2xw_engine::MemoryRecorder::new();
        engine.run(&mut recorder).expect("runs");
        let tx = by_type(&recorder, "node.tx");
        let vru: std::collections::BTreeSet<u64> = tx
            .iter()
            .filter(|r| r["msg_type"] == "psm")
            .filter_map(|r| r["node"].as_u64())
            .collect();
        let ns = |v: &serde_json::Value| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64));
        let served: Vec<(u64, f64)> = by_type(&recorder, "node.verify")
            .iter()
            .filter(|r| vru.contains(&r["node"].as_u64().unwrap_or(u64::MAX)))
            .filter(|r| r["outcome"] == "valid")
            .filter_map(|r| {
                Some((
                    r["priority"].as_u64()?,
                    ns(&r["t_start"])?.saturating_sub(ns(&r["t_enqueue"])?) as f64 / 1e6,
                ))
            })
            .collect();
        served
    };
    let all = run("verify-all");
    let pri = run("prioritized");
    let near: Vec<f64> = pri.iter().filter(|p| p.0 >= 500).map(|p| p.1).collect();
    let far: Vec<f64> = pri.iter().filter(|p| p.0 < 500).map(|p| p.1).collect();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    println!(
        "MEAS vru verify: verify-all {} checks (max priority {}); prioritized {} checks, near {} \
         wait {:.3} ms, far {} wait {:.3} ms",
        all.len(),
        all.iter().map(|p| p.0).max().unwrap_or(0),
        pri.len(),
        near.len(),
        mean(&near),
        far.len(),
        mean(&far)
    );
    assert!(!all.is_empty() && all.iter().all(|p| p.0 == 0));
    assert!(
        pri.iter().any(|p| p.0 > 0),
        "no VRU check carried a priority: the policy did not reach the device"
    );
}

/// Under the US SCMS a pedestrian's handset holds pseudonym certificates that permit the
/// PSM it signs (PSID 0x27, SAE J2945/9) and the misbehaviour report (0x26), and a
/// vehicle's do not permit the PSM. Every receiver checks the SPDU's PSID against the
/// signer's `appPermissions` (1609.2 §5.2), so an honest run refuses no PSM, no detector
/// accuses a pedestrian of a bad signature, and nobody honest is revoked.
///
/// Fault injected: `Phase2::provision` without its `set_device_psids` call for a VRU
/// device (the defect of docs/benchmark/scms-real-benchmark.md §9.1: handsets requested
/// the vehicles' profile, 0x20 and 0x26) — every PSM is refused `psid-not-permitted` and
/// this test fails.
#[test]
fn under_the_scms_a_handset_signs_psms_that_every_receiver_accepts() {
    let mut s = Scenario::minimal();
    s.world.source = WorldSourceSpec::procedural(
        "world/source/procedural-grid",
        serde_json::json!({
            "cols": 4, "rows": 4, "block_x_m": 120.0, "block_y_m": 90.0,
            "lanes_per_direction": 1, "sidewalk_m": 2.0, "crossings": true,
            "signalised": true, "corner_radius_m": 4.5, "bicycles_on_roads": true
        }),
    );
    s.actors.vru.pedestrians = 10;
    s.actors.vru.device_fraction = 1.0;
    s.actors.vehicles.equipped_fraction = 1.0;
    s.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
    s.actors.vehicles.demand.rate_veh_per_h = Some(2400.0);
    s.messages.sets = vec!["bsm".to_string()];
    s.time.duration_s = 20.0;
    s.security.verification_policy = "verify-all".to_string();
    s.security.protocol = Some(v2xw_engine::scenario::ModelChoice::new(
        v2xw_engine::phase2::CAMP_SCMS,
    ));
    s.actors.backend.protocol = Some(v2xw_engine::phase2::CAMP_SCMS.to_string());
    s.detection.local = vec![v2xw_engine::scenario::ModelChoice::new(
        v2xw_engine::phase2::LEGACY_12,
    )];
    s.net.uu = Some(v2xw_engine::scenario::ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: serde_json::json!({"preset": "4g-east-coast"}),
    });
    assert!(
        v2xw_engine::scenario::validate::validate(&s).is_empty(),
        "{:?}",
        v2xw_engine::scenario::validate::validate(&s)
    );
    let mut engine = v2xw_engine::Engine::build(s, "").expect("builds");
    let mut recorder = v2xw_engine::MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    let rv = &report.receiver_validation;
    let p = &report.phase2;
    let psm_valid = by_type(&recorder, "node.verify")
        .iter()
        .filter(|r| r["msg_type"] == "psm" && r["outcome"] == "valid")
        .count();
    println!(
        "MEAS scms vru: {} handsets, refused {:?}, {} PSM verifications valid, verdicts {:?}, \
         {} reports, {} honest revoked",
        report.vru_devices_created,
        rv.refused,
        psm_valid,
        p.verdicts_by_detector,
        p.reports_sent,
        p.revoked_honest
    );
    assert!(report.vru_devices_created >= 10, "{report:?}");
    assert!(rv.checked > 0, "no SPDU was validated to a root");
    assert_eq!(
        rv.refused.get("psid-not-permitted").copied().unwrap_or(0),
        0,
        "an honest message was signed under a PSID its certificate does not permit"
    );
    assert!(psm_valid > 0, "no receiver verified a PSM");
    assert_eq!(
        p.verdicts_by_detector
            .get("signatureVerification")
            .copied()
            .unwrap_or(0),
        0,
        "an honest sender was accused of a bad signature"
    );
    assert_eq!(p.revoked_honest, 0);
    // What the backend issued: the handsets' certificates permit the PSM and the report,
    // the vehicles' never the PSM.
    let psid_vru = v2xw_msg::registry::PSID_VRU.to_string();
    let (mut handsets, mut vehicles) = (0, 0);
    for run in engine.phase2().expect("phase 2").scms_runs() {
        for d in run.state.devices.values() {
            for c in d.credentials.values() {
                let Some(imp) = c.implicit.as_ref() else {
                    continue;
                };
                let perms: Vec<String> = imp
                    .certificate
                    .0
                    .to_be_signed
                    .app_permissions
                    .as_ref()
                    .map(|p| p.0.iter().map(|x| x.psid.0.to_string()).collect())
                    .unwrap_or_default();
                if d.psids.is_some() {
                    handsets += 1;
                    assert_eq!(perms, ["38", "39"], "a handset's certificate");
                } else {
                    vehicles += 1;
                    assert!(
                        !perms.contains(&psid_vru),
                        "a vehicle may sign PSMs: {perms:?}"
                    );
                }
            }
        }
    }
    assert!(handsets > 0 && vehicles > 0, "{handsets} {vehicles}");
}
