//! WP-12 in the engine: with every new knob off the run is the default run, record for
//! record; with them on it is deterministic and changes what goes on the air; and the
//! loader refuses each new knob where the run would not read it. The rules themselves are
//! tested in `v2xw-node` (`cpm::tests`, `events::wp12_tests`, and
//! `tests/runtime_behaviour.rs` for the DCC gap reaching the CP service through the
//! runtime).

use std::path::{Path, PathBuf};

use v2xw_engine::scenario::schema::ModelChoice;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn junctions(secs: f64) -> Scenario {
    let mut s =
        Scenario::load(root().join("scenarios/connected-intersections.yaml")).expect("loads");
    s.time.duration_s = secs;
    s.metrics = vec![];
    s
}

/// The run's report and the JSON of every `node.tx` record.
fn run(s: Scenario) -> (RunReport, Vec<serde_json::Value>) {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "the scenario should load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    let tx = rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "node.tx")
        .filter_map(|(_, r)| serde_json::from_slice(&r.json).ok())
        .collect();
    (report, tx)
}

fn refused(s: &Scenario, field: &str) -> bool {
    v2xw_engine::scenario::validate::validate(s)
        .iter()
        .any(|e| e.to_string().contains(field))
}

/// An ETSI CAM + DENM + CPM run in Europe.
fn etsi_cpm() -> Scenario {
    let mut base = junctions(8.0);
    base.events.clear();
    base.radio.region = Some(serde_json::from_value(serde_json::json!("eu")).expect("region"));
    base.radio.models.insert(
        "dcc".to_string(),
        ModelChoice {
            id: "dcc/etsi/adaptive-ts102687".to_string(),
            params: serde_json::Value::Null,
        },
    );
    base.net.layer = "gn-btp".to_string();
    base.security.envelope = "etsi103097".to_string();
    base.messages.sets = vec!["cam".into(), "denm".into(), "cpm".into()];
    base.messages.codec_tier = "uper".to_string();
    for rsu in &mut base.actors.rsus {
        rsu.roles.clear();
    }
    base.actors.vehicles.equipped_fraction = 0.4;
    base.actors.vru.pedestrians = 40;
    base.actors.vru.device_fraction = 0.0;
    base
}

fn knobs(s: &mut Scenario, on: bool) {
    s.messages.cpm_dcc_limited = Some(on);
    s.messages.denm_traffic_condition = Some(on);
    s.messages.denm_traction_loss = Some(on);
    if on {
        s.messages.cpm_quality_threshold = Some(3);
        s.messages.cpm_max_objects = Some(2);
    }
}

/// Every WP-12 knob set to off is the default run, `node.tx` record for record (the
/// default path does not move); with every knob on the run is deterministic and the
/// quality threshold and the 2-object budget cut what each CPM carries (the cut objects
/// wait for the next event, so more, smaller CPMs go). Fault: wiring that drops
/// `cpm_quality_threshold` and `cpm_max_objects` leaves the CPMs at the default run's.
#[test]
fn the_wp12_knobs_off_are_the_default_run_and_on_is_deterministic() {
    let base = etsi_cpm();
    let (b, btx) = run(base.clone());
    let mut off = base.clone();
    knobs(&mut off, false);
    let (o, otx) = run(off);
    let mut on = base;
    knobs(&mut on, true);
    let (l1, l1tx) = run(on.clone());
    let (l2, l2tx) = run(on);
    let sent = |r: &RunReport| r.cpm.as_ref().map_or(0, |c| c.cpms_sent);
    let objects = |r: &RunReport| r.cpm.as_ref().map_or(0, |c| c.objects_sent);
    let bytes = |tx: &[serde_json::Value]| -> u64 {
        tx.iter()
            .filter_map(|v| v.get("bytes_on_wire").and_then(serde_json::Value::as_u64))
            .sum()
    };
    eprintln!(
        "cpms default {} off {} on {} / {}; objects {} {} {} / {}; tx records {} {} {}; bytes {} {} {}",
        sent(&b),
        sent(&o),
        sent(&l1),
        sent(&l2),
        objects(&b),
        objects(&o),
        objects(&l1),
        objects(&l2),
        btx.len(),
        otx.len(),
        l1tx.len(),
        bytes(&btx),
        bytes(&otx),
        bytes(&l1tx)
    );
    assert!(sent(&b) > 10, "too few CPMs to compare");
    assert_eq!(sent(&o), sent(&b), "off must be the default path");
    assert!(
        otx == btx,
        "off must put the default run's frames on the air"
    );
    assert_eq!(sent(&l1), sent(&l2), "same seed, same CPMs");
    assert!(l1tx == l2tx, "same seed, same frames");
    assert_eq!(objects(&l1), objects(&l2));
    // The budget holds: no CPM carries more than two objects, so the objects the default
    // run packs into one CPM go out over more of them (the cut ones stay due).
    assert!(
        objects(&l1) <= 2 * sent(&l1),
        "{} objects in {} CPMs breaks the 2-object budget",
        objects(&l1),
        sent(&l1)
    );
    assert!(
        objects(&b) > sent(&b) && (sent(&l1), objects(&l1)) != (sent(&b), objects(&b)),
        "the threshold and the 2-object budget changed nothing: {} CPMs, {} objects",
        sent(&l1),
        objects(&l1)
    );
}

/// The loader refuses each WP-12 knob without its service, and out-of-range numbers.
#[test]
fn the_wp12_knobs_are_refused_where_the_run_would_not_read_them() {
    let s = junctions(1.0);
    for field in [
        "messages.cpm_dcc_limited",
        "messages.cpm_quality_threshold",
        "messages.cpm_max_objects",
    ] {
        let mut x = s.clone();
        match field {
            "messages.cpm_dcc_limited" => x.messages.cpm_dcc_limited = Some(true),
            "messages.cpm_quality_threshold" => x.messages.cpm_quality_threshold = Some(3),
            _ => x.messages.cpm_max_objects = Some(20),
        }
        assert!(refused(&x, field), "{field} without cpm in messages.sets");
    }
    let mut x = s.clone();
    x.messages.denm_traffic_condition = Some(true);
    assert!(refused(&x, "messages.denm_traffic_condition"));
    let mut x = s.clone();
    x.messages.denm_traction_loss = Some(true);
    assert!(refused(&x, "messages.denm_traction_loss"));

    let mut c = s.clone();
    c.messages.sets.push("cpm".into());
    c.messages.cpm_quality_threshold = Some(16);
    assert!(refused(&c, "messages.cpm_quality_threshold"), "above 15");
    c.messages.cpm_quality_threshold = Some(3);
    c.messages.cpm_max_objects = Some(0);
    assert!(refused(&c, "messages.cpm_max_objects"), "zero objects");
    c.messages.cpm_max_objects = Some(256);
    assert!(
        refused(&c, "messages.cpm_max_objects"),
        "above the ASN.1 bound"
    );
    c.messages.cpm_max_objects = Some(255);
    c.messages.cpm_dcc_limited = Some(true);
    assert!(
        !refused(&c, "messages.cpm_"),
        "{:?}",
        v2xw_engine::scenario::validate::validate(&c)
    );
}
