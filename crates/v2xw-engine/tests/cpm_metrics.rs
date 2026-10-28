//! Collective perception's figures: with CPMs, an equipped vehicle knows about road users
//! that send nothing themselves (unequipped cars, pedestrians without a device), so the
//! awareness with CPMs exceeds the awareness from awareness messages alone; and every
//! object is reported to a receiver at least once per second it is reported at all.

use std::path::Path;

use v2xw_engine::{Engine, MemoryRecorder, Scenario};

fn scenario(sets: &[&str]) -> Scenario {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/connected-intersections.yaml");
    let mut s = Scenario::load(path).expect("the shipped scenario loads");
    s.time.duration_s = 12.0;
    s.metrics = vec![];
    s.events.clear();
    s.net.layer = "gn-btp".to_string();
    s.security.envelope = "etsi103097".to_string();
    s.messages.sets = sets.iter().map(|m| (*m).to_string()).collect();
    s.messages.codec_tier = "uper".to_string();
    for rsu in &mut s.actors.rsus {
        rsu.roles.clear();
    }
    s.actors.vehicles.equipped_fraction = 0.4;
    s.actors.vru.pedestrians = 40;
    s.actors.vru.device_fraction = 0.0;
    s
}

#[test]
fn cpms_raise_object_awareness_beyond_what_awareness_messages_give() {
    let mut e = Engine::build(scenario(&["cam"]), "").expect("builds");
    let rep = e.run(&mut MemoryRecorder::new()).expect("runs");
    assert!(rep.cpm.is_none(), "no CPM, no CPM figures");

    let mut e = Engine::build(scenario(&["cam", "cpm"]), "").expect("builds");
    let rep = e.run(&mut MemoryRecorder::new()).expect("runs");
    let c = rep.cpm.expect("CPM figures");
    eprintln!("{c:?}");
    assert!(c.cpms_sent > 10 && c.senders > 1, "{c:?}");
    assert!(c.objects_sent > 0, "CPMs carry perceived objects: {c:?}");
    assert!(
        c.size_p95_bytes >= c.size_p50_bytes && c.size_p50_bytes > 0,
        "{c:?}"
    );
    // At most 10 Hz per station (T_GenCpmMin = 100 ms), and at least once a second.
    assert!(
        c.generation_mhz <= 10_000 && c.generation_mhz >= 500,
        "{c:?}"
    );
    assert!(c.awareness_pairs > 100, "{c:?}");
    assert!(
        c.awareness_with_cpm_permille > c.awareness_direct_permille,
        "CPMs report road users that send nothing: {c:?}"
    );
    assert!(c.redundancy_milli >= 1_000, "{c:?}");
}

/// WP-14: a roadside unit at a junction given the `cpm` role watches it with its own
/// cameras (`perception.rsu`, the default four) and sends infrastructure CPMs — the
/// `originatingRsuContainer` — that the vehicles around it verify and decode, read from
/// the payload each vehicle's own receiver parsed.
#[test]
fn a_roadside_camera_at_a_junction_sends_cpms_that_vehicles_decode() {
    let mut s = scenario(&["cam", "cpm"]);
    for rsu in &mut s.actors.rsus {
        rsu.roles = vec!["cpm".to_string()];
    }
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut e = Engine::build(s, "").expect("builds");
    let rep = e.run(&mut MemoryRecorder::new()).expect("runs");
    let c = rep.cpm.expect("CPM figures");
    eprintln!("{c:?}");
    assert!(c.rsu_cpms_sent > 10, "the units send CPMs: {c:?}");
    assert!(
        c.rsu_cpms_verified > 10 && c.rsu_objects_decoded > 10,
        "vehicles verify and decode them, with objects in them: {c:?}"
    );
}

/// A unit without the role sends none, and the vehicles count none.
#[test]
fn without_the_role_no_roadside_cpm_is_sent() {
    let mut e = Engine::build(scenario(&["cam", "cpm"]), "").expect("builds");
    let c = e
        .run(&mut MemoryRecorder::new())
        .expect("runs")
        .cpm
        .expect("CPM figures");
    assert_eq!((c.rsu_cpms_sent, c.rsu_cpms_verified), (0, 0), "{c:?}");
    // The vehicles' own CPMs verify and decode at their neighbours.
    assert!(c.cpms_verified > 10, "{c:?}");
}

/// The suites are checked against what a CPM can carry, and a block nothing reads is
/// refused.
#[test]
fn the_perception_block_refuses_what_a_cpm_cannot_carry() {
    use v2xw_engine::scenario::schema::{PerceptionSpec, SensorConf};
    let sensor = |id: u8, range_m: f64, fov_deg: f64| SensorConf {
        id,
        kind: Default::default(),
        range_m,
        fov_deg,
        azimuth_deg: 0.0,
        mount: Default::default(),
        ahead_m: 0.0,
        left_m: 0.0,
        height_m: 0.6,
        pd_near: 0.99,
        pd_far: 0.9,
        sigma_base_m: None,
        sigma_range_fraction: None,
    };
    let mut s = scenario(&["cam", "cpm"]);
    s.perception = Some(PerceptionSpec {
        vehicle: Some(vec![sensor(1, 250.0, 18.0), sensor(1, 70.0, 90.0)]),
        classes: [
            ("truck".to_string(), vec![sensor(8, 500.0, 0.0)]),
            ("pedestrian".to_string(), vec![sensor(1, 50.0, 90.0)]),
            ("hovercraft".to_string(), vec![sensor(1, 50.0, 90.0)]),
        ]
        .into_iter()
        .collect(),
        rsu: Some(vec![sensor(1, 80.0, 90.0)]),
        ..Default::default()
    });
    let errors: Vec<String> = v2xw_engine::scenario::validate::validate(&s)
        .iter()
        .map(ToString::to_string)
        .collect();
    for field in [
        "perception.vehicle[1].id",
        "perception.classes.truck[0].id",
        "perception.classes.truck[0].range_m",
        "perception.classes.truck[0].fov_deg",
        "perception.classes.pedestrian",
        "perception.classes.hovercraft",
        "perception.rsu",
    ] {
        assert!(
            errors.iter().any(|e| e.contains(field)),
            "{field} is not refused: {errors:#?}"
        );
    }
    assert!(
        !errors.iter().any(|e| e.contains("perception.vehicle[0]")),
        "{errors:#?}"
    );
    // A well-formed block loads.
    s.perception = Some(PerceptionSpec {
        vehicle: Some(vec![sensor(1, 250.0, 18.0), sensor(4, 80.0, 150.0)]),
        ..Default::default()
    });
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    // And one in a run with no CPM changes nothing.
    s.messages.sets = vec!["cam".to_string()];
    let errors: Vec<String> = v2xw_engine::scenario::validate::validate(&s)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        errors.iter().any(|e| e.contains("perception")),
        "{errors:?}"
    );
}

/// An outline-occlusion run with a rear radar on every car is deterministic and still
/// raises awareness; its CPMs state each sensor's mount and azimuth.
#[test]
fn a_configured_suite_runs_deterministically() {
    let yaml = r"
occlusion: outline
vehicle:
  - {id: 1, kind: radar, range_m: 250, fov_deg: 18, ahead_m: -1.0}
  - {id: 2, kind: radar, range_m: 70, fov_deg: 90, ahead_m: -1.0}
  - {id: 3, kind: camera, range_m: 80, fov_deg: 52, ahead_m: -1.0}
  - {id: 4, kind: radar, range_m: 80, fov_deg: 150, azimuth_deg: 180, mount: rear, height_m: 0.5}
";
    let spec: v2xw_engine::scenario::schema::PerceptionSpec =
        serde_yml::from_str(yaml).expect("the block parses");
    let mut s = scenario(&["cam", "cpm"]);
    s.perception = Some(spec);
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let run = |s: &Scenario| {
        let mut e = Engine::build(s.clone(), "").expect("builds");
        e.run(&mut MemoryRecorder::new())
            .expect("runs")
            .cpm
            .expect("CPM figures")
    };
    let a = run(&s);
    let b = run(&s);
    assert_eq!(a, b, "same seed, same figures");
    assert!(a.objects_sent > 0, "{a:?}");
    assert!(
        a.awareness_with_cpm_permille > a.awareness_direct_permille,
        "{a:?}"
    );
}
