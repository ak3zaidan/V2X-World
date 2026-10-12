//! The network parameters the 2026-10-11 audit exposed (docs/guides/config-inventory-network.md):
//! each group changes a measured number of the run, and the loader refuses the values and
//! combinations that would leave one with nothing to act on.
//!
//! Every test runs a baseline and the setting side by side on the same seed, so a setting
//! that changed nothing fails its assertion.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use v2xw_engine::scenario::schema::{Geonet, ModelChoice, Rat};
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The scaling ladder's Manhattan base with a bulk-spawned fleet of `n`, in open air.
fn fleet(n: u32, secs: f64) -> Scenario {
    let mut s = Scenario::load(root().join("scenarios/scale/base.yaml")).expect("loads");
    s.time.duration_s = secs;
    s.actors.vehicles.demand.params = serde_json::json!({ "max_total_vehicles": n });
    s.world.buildings.enabled = false;
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root().join(&*path).to_string_lossy().into_owned();
    }
    s
}

/// The procedural four-junction scenario: roadside units running SPaT and MAP, prioritized
/// verification.
fn junctions(secs: f64) -> Scenario {
    let mut s =
        Scenario::load(root().join("scenarios/connected-intersections.yaml")).expect("loads");
    s.time.duration_s = secs;
    s.metrics = vec![];
    s
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder) {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "the scenario should load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    (report, rec)
}

fn refused(s: &Scenario, field: &str) -> bool {
    v2xw_engine::scenario::validate::validate(s)
        .iter()
        .any(|e| e.to_string().contains(field))
}

/// The JSON of every record on `channel`.
fn records(rec: &MemoryRecorder, channel: &str) -> Vec<serde_json::Value> {
    rec.records()
        .iter()
        .filter(|(_, r)| r.channel == channel)
        .filter_map(|(_, r)| serde_json::from_slice(&r.json).ok())
        .collect()
}

/// How many `node.tx` records carry `msg_type`.
fn sent(rec: &MemoryRecorder, msg_type: &str) -> usize {
    records(rec, "node.tx")
        .iter()
        .filter(|v| v.get("msg_type").and_then(|m| m.as_str()) == Some(msg_type))
        .count()
}

/// Sidelink: a 15 dB noise figure loses receptions against TR 36.885's 9 dB, and a
/// -112 dBm busy threshold counts more of the pool busy than the -94 dBm default.
#[test]
fn the_sidelink_receiver_numbers_change_the_run() {
    let mut base = fleet(20, 2.0);
    base.radio.rat = Rat::LteV2xPc5;
    let (b, _) = run(base.clone());
    let mut noisy = base.clone();
    noisy.radio.sidelink.noise_figure_db = Some(15.0);
    let (n, _) = run(noisy);
    let mut low = base;
    low.radio.sidelink.cbr_threshold_dbm = Some(-112.0);
    let (l, _) = run(low);
    assert!(b.reception_attempts > 200, "too few attempts to compare");
    assert!(
        n.receptions_ok < b.receptions_ok,
        "NF 15 dB decoded {} against {} at 9 dB",
        n.receptions_ok,
        b.receptions_ok
    );
    let cbr = |r: &RunReport| r.sidelink.as_ref().map_or(0, |s| s.mean_cbr_at_grant_pm);
    assert!(
        cbr(&l) > cbr(&b),
        "a -112 dBm threshold measured CBR {} ‰ against {} ‰ at -94 dBm",
        cbr(&l),
        cbr(&b)
    );
}

/// GeoNetworking: a 400 ms CBF maximum makes contenders wait longer before forwarding, so
/// the warning reaches the area later than with Annex H's 100 ms.
#[test]
fn the_cbf_timers_change_the_forwarding_latency() {
    let warn = |g: Geonet| {
        let mut s = junctions(14.0);
        s.actors.vehicles.demand.rate_veh_per_h = Some(12_000.0);
        s.net.layer = "gn-btp".to_string();
        s.security.envelope = "etsi103097".to_string();
        s.messages.sets = vec!["cam".into(), "denm".into()];
        for rsu in &mut s.actors.rsus {
            rsu.roles.clear();
        }
        s.security.verification_policy = "verify-all".to_string();
        s.events = serde_json::from_value(serde_json::json!([
            {"t": 10.0, "until": 13.0, "type": "safety.breakdown"},
        ]))
        .expect("timeline items");
        s.net.geonet = Some(g);
        run(s).0.geonet
    };
    let overlay = || Geonet {
        forwarding: "overlay".into(),
        range_m: 200.0,
        ..Geonet::default()
    };
    let b = warn(overlay());
    let slow = warn(Geonet {
        cbf_max_time_ms: Some(400),
        ..overlay()
    });
    eprintln!("base {b:?}\nslow {slow:?}");
    let p95 = |w: &[v2xw_engine::run::GeonetWarning]| w.iter().map(|x| x.latency_p95_us).max();
    assert!(b.iter().any(|w| w.forwarded > 0), "nothing was forwarded");
    assert!(
        p95(&slow) > p95(&b),
        "CBF max 400 ms: p95 {:?} us against {:?} us at 100 ms",
        p95(&slow),
        p95(&b)
    );
}

/// Infrastructure messages: SPaT every 200 ms sends about half the SPaTs of CTI 4501's
/// 100 ms, and MAP every 500 ms about twice the MAPs of its 1 s.
#[test]
fn the_spat_and_map_intervals_change_the_infrastructure_load() {
    let base = junctions(6.0);
    let (_, brec) = run(base.clone());
    let mut tuned = base;
    tuned.messages.spat_interval_ms = Some(200);
    tuned.messages.map_interval_ms = Some(500);
    let (_, trec) = run(tuned);
    let (bs, ts) = (sent(&brec, "spat"), sent(&trec, "spat"));
    let (bm, tm) = (sent(&brec, "map"), sent(&trec, "map"));
    eprintln!("spat {bs} -> {ts}, map {bm} -> {tm}");
    assert!(bs > 40, "too few SPaTs to compare: {bs}");
    assert!((ts as f64) < 0.65 * bs as f64, "SPaT {ts} against {bs}");
    assert!((tm as f64) > 1.5 * bm as f64, "MAP {tm} against {bm}");
}

/// Collective perception: a 500 ms `T_GenCpm` checks for a due CPM a fifth as often as
/// TS 103 324's 100 ms, so fewer CPMs go out (objects that change between checks are
/// batched; the 1 s refresh still sends the rest).
#[test]
fn the_cpm_interval_changes_the_cpm_rate() {
    let mut base = junctions(8.0);
    base.events.clear();
    base.net.layer = "gn-btp".to_string();
    base.security.envelope = "etsi103097".to_string();
    base.messages.sets = vec!["cam".into(), "cpm".into()];
    base.messages.codec_tier = "uper".to_string();
    for rsu in &mut base.actors.rsus {
        rsu.roles.clear();
    }
    base.actors.vehicles.equipped_fraction = 0.4;
    base.actors.vru.pedestrians = 40;
    base.actors.vru.device_fraction = 0.0;
    let (b, _) = run(base.clone());
    let mut slow = base;
    slow.messages.cpm_interval_ms = Some(500);
    let (l, _) = run(slow);
    let sent = |r: &RunReport| r.cpm.as_ref().map_or(0, |c| c.cpms_sent);
    eprintln!("cpms {} -> {}", sent(&b), sent(&l));
    assert!(sent(&b) > 10, "too few CPMs to compare");
    assert!(
        sent(&l) < sent(&b),
        "T_GenCpm 500 ms sent {} CPMs against {} at 100 ms",
        sent(&l),
        sent(&b)
    );
}

/// A per-outcome tally of the `node.verify` records, and their total queueing wait.
fn verify_tally(rec: &MemoryRecorder) -> (BTreeMap<String, u64>, u64) {
    let mut tally = BTreeMap::new();
    let mut wait = 0;
    for v in records(rec, "node.verify") {
        let key = format!(
            "{}/{}",
            v.get("outcome")
                .and_then(|o| o.as_str())
                .unwrap_or_default(),
            v.get("reason").and_then(|o| o.as_str()).unwrap_or_default()
        );
        *tally.entry(key).or_insert(0) += 1;
        let at = |k: &str| v.get(k).and_then(|t| t.as_u64());
        if let (Some(q), Some(s)) = (at("t_enqueue"), at("t_start")) {
            wait += s.saturating_sub(q);
        }
    }
    (tally, wait)
}

/// Verification: on a software-crypto unit in heavy traffic the verification queue backs
/// up, so a one-entry queue sheds messages the 64-entry one holds and the survivors wait
/// less.
#[test]
fn the_verification_numbers_change_what_is_verified() {
    let mut base = fleet(80, 2.0);
    base.security.verification_policy = "prioritized".to_string();
    base.nodes.default_obu = "obu/unex-obu-301-craton2-software".to_string();
    let (_, brec) = run(base.clone());
    let (bt, bw) = verify_tally(&brec);
    let mut shallow = base;
    shallow.nodes.verify_queue_depth = Some(1);
    let (_, srec) = run(shallow);
    let (st, sw) = verify_tally(&srec);
    eprintln!("base {bt:?} wait {bw}\nshallow {st:?} wait {sw}");
    assert!(
        bt.values().sum::<u64>() > 100,
        "too few verifications: {bt:?}"
    );
    assert!(sw < bw, "a one-entry queue waited {sw} ns against {bw} ns");
    assert_ne!(st, bt, "a one-entry verification queue changed nothing");
}

/// Reactive DCC: Table A.2's 50 ms relaxed idle time lets frames out that Table A.1's
/// 100 ms holds back, so the transmit instants differ.
#[test]
fn the_reactive_table_changes_the_gate() {
    let mut base = fleet(20, 3.0);
    base.radio.region = Some(serde_json::from_value(serde_json::json!("eu")).expect("region"));
    base.radio.models.insert(
        "dcc".to_string(),
        ModelChoice {
            id: "dcc/etsi/reactive-ts102687".to_string(),
            params: serde_json::Value::Null,
        },
    );
    let (b, brec) = run(base.clone());
    let mut a2 = base;
    a2.radio.dcc.reactive_table = Some("a2".to_string());
    let (t, trec) = run(a2);
    let instants = |rec: &MemoryRecorder| -> Vec<u64> {
        records(rec, "node.tx")
            .iter()
            .filter_map(|v| v.get("t").and_then(|t| t.as_u64()))
            .collect()
    };
    eprintln!(
        "frames {} -> {}",
        b.frames_transmitted, t.frames_transmitted
    );
    assert!(b.frames_transmitted > 100, "too few frames to compare");
    assert_ne!(
        instants(&brec),
        instants(&trec),
        "table A.2 changed nothing"
    );
}

/// The loader refuses each new number where the run would not read it.
#[test]
fn the_loader_refuses_network_settings_that_would_do_nothing() {
    let mut dsrc = fleet(2, 1.0);
    dsrc.radio.sidelink.noise_figure_db = Some(6.0);
    assert!(refused(&dsrc, "radio.sidelink"));

    let mut lte = fleet(2, 1.0);
    lte.radio.rat = Rat::LteV2xPc5;
    lte.radio.sidelink.cbr_threshold_dbm = Some(-93.0);
    assert!(
        refused(&lte, "radio.sidelink.cbr_threshold_dbm"),
        "not -112 + 2n"
    );
    lte.radio.sidelink.cbr_threshold_dbm = Some(-120.0);
    assert!(
        refused(&lte, "radio.sidelink.cbr_threshold_dbm"),
        "out of range"
    );

    let mut us = fleet(2, 1.0);
    us.radio.dcc.reactive_table = Some("a2".to_string());
    assert!(
        refused(&us, "radio.dcc.reactive_table"),
        "J2945/1 runs, not reactive"
    );

    let mut gn = junctions(1.0);
    gn.net.geonet = Some(Geonet {
        area_forwarding: "simple".into(),
        cbf_max_time_ms: Some(50),
        ..Geonet::default()
    });
    assert!(refused(&gn, "net.geonet"), "no CBF timer runs");
    gn.net.geonet = Some(Geonet {
        cbf_min_time_ms: Some(200),
        ..Geonet::default()
    });
    assert!(refused(&gn, "net.geonet.cbf_min_time_ms"), "min above max");

    let mut no_spat = fleet(2, 1.0);
    no_spat.messages.spat_interval_ms = Some(200);
    assert!(refused(&no_spat, "messages.spat_interval_ms"));
    let mut fast = junctions(1.0);
    fast.messages.map_interval_ms = Some(50);
    assert!(
        refused(&fast, "messages.map_interval_ms"),
        "below the 100 ms check"
    );

    let mut cpm = junctions(1.0);
    cpm.messages.cpm_interval_ms = Some(200);
    assert!(
        refused(&cpm, "messages.cpm_interval_ms"),
        "no cpm in messages.sets"
    );
    let mut depth = junctions(1.0);
    depth.nodes.verify_queue_depth = Some(0);
    assert!(refused(&depth, "nodes.verify_queue_depth"));

    // And the defaults still load.
    assert!(v2xw_engine::scenario::validate::validate(&junctions(1.0)).is_empty());
}
