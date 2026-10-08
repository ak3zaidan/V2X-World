//! `net.geonet`: a breakdown's warning is forwarded hop by hop, and the setting changes
//! the outcome (so the check can go red: with flooding the transmission count differs
//! from CBF's, and with no `net.geonet` there is no multi-hop result at all).

use std::path::Path;

use v2xw_engine::scenario::schema::Geonet;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn scenario(geonet: Option<Geonet>) -> Scenario {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/connected-intersections.yaml");
    let mut s = Scenario::load(path).expect("the shipped scenario loads");
    s.time.duration_s = 14.0;
    s.metrics = vec![];
    s.actors.vehicles.demand.rate_veh_per_h = Some(12_000.0);
    s.net.layer = "gn-btp".to_string();
    s.security.envelope = "etsi103097".to_string();
    s.messages.sets = vec!["cam".into(), "denm".into()];
    // The roadside units' SPaT and MAP roles would broadcast nothing without those sets, and
    // the validator refuses that; the units stay as relays only.
    for rsu in &mut s.actors.rsus {
        rsu.roles.clear();
    }
    s.messages.codec_tier = "uper".to_string();
    s.events = serde_json::from_value(serde_json::json!([
        {"t": 10.0, "until": 13.0, "type": "safety.breakdown"},
    ]))
    .expect("timeline items");
    s.net.geonet = geonet;
    s
}

fn run(s: Scenario) -> RunReport {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs")
}

/// The share of BSM transmissions that start inside a 1609.4 control-channel interval
/// (4-50 ms of each 100 ms sync interval, with 1 ms of slack for the MAC).
fn bsm_share_in_cch(access: Option<&str>) -> f64 {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/connected-intersections.yaml");
    let mut s = Scenario::load(path).expect("loads");
    s.time.duration_s = 8.0;
    s.metrics = vec![];
    s.net.channel_access = access.map(str::to_string);
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    let ts: Vec<u64> = rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "node.tx")
        .filter_map(|(_, r)| {
            v2xw_metrics::channels::decode::<v2xw_metrics::channels::NodeTxView>(r).ok()
        })
        .filter(|t| t.msg_type.as_deref() == Some("bsm"))
        .map(|t| t.t)
        .collect();
    assert!(ts.len() > 50, "{} BSMs", ts.len());
    let inside = ts
        .iter()
        .filter(|&&t| (4_000_000..51_000_000).contains(&(t % 100_000_000)))
        .count();
    inside as f64 / ts.len() as f64
}

#[test]
fn alternating_channel_access_holds_bsms_to_the_control_channel_interval() {
    let cont = bsm_share_in_cch(None);
    let alt = bsm_share_in_cch(Some("alternating"));
    eprintln!("continuous {cont:.3} alternating {alt:.3}");
    assert!(alt > 0.97, "{alt}");
    assert!(
        cont < 0.8,
        "without switching BSMs go out across the whole interval: {cont}"
    );
}

#[test]
fn a_breakdown_warning_spreads_multi_hop_and_the_forwarding_setting_matters() {
    assert!(run(scenario(None)).geonet.is_empty());
    let cbf = run(scenario(Some(Geonet {
        forwarding: "overlay".into(),
        range_m: 200.0,
        ..Geonet::default()
    })));
    let flood = run(scenario(Some(Geonet {
        forwarding: "overlay".into(),
        range_m: 200.0,
        area_forwarding: "simple".into(),
        ..Geonet::default()
    })));
    eprintln!("cbf {:?}\nflood {:?}", cbf.geonet, flood.geonet);
    let (c, f) = (&cbf.geonet[0], &flood.geonet[0]);
    assert!(
        c.in_area > 5,
        "a dense grid has vehicles within 1 km: {c:?}"
    );
    assert!(c.reached >= 2, "{c:?}");
    assert!(
        c.max_hops >= 1,
        "some vehicle is reached through a relay: {c:?}"
    );
    // Flooding rebroadcasts at once; CBF waits out its contention timers (up to 100 ms),
    // so the slowest vehicles hear it later under CBF.
    assert!(f.transmissions >= c.transmissions, "{f:?} vs {c:?}");
    assert!(f.latency_p95_us < c.latency_p95_us, "{f:?} vs {c:?}");
    // The setting is validated: geonet on WSMP is refused.
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/connected-intersections.yaml");
    let mut bad = Scenario::load(path).expect("loads");
    assert_eq!(bad.net.layer, "wsmp");
    bad.net.geonet = Some(Geonet::default());
    let err = bad.validate();
    let msg = format!("{err:?}");
    assert!(err.is_err() && msg.contains("net.geonet"), "{msg}");
}

/// The DENM transmissions on `node.tx`, by transmitter.
fn denm_senders(rec: &MemoryRecorder) -> std::collections::BTreeMap<u64, u64> {
    let mut by = std::collections::BTreeMap::new();
    for (_, r) in rec.records() {
        if r.channel != "node.tx" {
            continue;
        }
        let Ok(t) = v2xw_metrics::channels::decode::<v2xw_metrics::channels::NodeTxView>(r) else {
            continue;
        };
        if t.msg_type.as_deref() == Some("denm") {
            *by.entry(u64::from(t.node.index())).or_insert(0) += 1;
        }
    }
    by
}

/// On the air, every forward is a real frame: it goes through a relay's MAC and shows on
/// `node.tx` under the relay, which the overlay's forwards never do. CBF candidates stand
/// down when they hear a copy, and the warning is still followed to its reach.
#[test]
fn on_air_forwards_are_frames_in_the_relays_own_queue() {
    // A breakdown raises the stationary-vehicle DENM once its 10 s triggering timer has
    // run out (C2C-CC RS 2006), repeated every second, so the run lasts past it.
    let hard_brake = |s: &mut Scenario| {
        s.time.duration_s = 24.0;
        s.events = serde_json::from_value(serde_json::json!([
            {"t": 8.0, "until": 23.0, "type": "safety.breakdown"},
        ]))
        .expect("timeline items");
    };
    let mut air = scenario(Some(Geonet::default()));
    hard_brake(&mut air);
    assert_eq!(air.net.geonet.as_ref().unwrap().forwarding, "on-air");
    let mut engine = Engine::build(air, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let rep = engine.run(&mut rec).expect("runs");
    let stats = rep.gn_air.clone().expect("on-air totals");
    let senders = denm_senders(&rec);
    eprintln!("on-air {:?}\n{stats:?}\nsenders {senders:?}", rep.geonet);
    let w = &rep.geonet[0];
    assert!(stats.gbc_originated >= 1, "{stats:?}");
    assert!(stats.forwards_sent >= 1, "{stats:?}");
    assert!(
        stats.cbf_armed > stats.forwards_sent,
        "CBF contenders stand down: {stats:?}"
    );
    assert!(
        senders.len() >= 2,
        "DENMs go on the air from relays, not only the origin: {senders:?}"
    );
    assert!(w.in_area > 5 && w.reached >= 2, "{w:?}");
    assert!(w.forwarded >= 1, "{w:?}");

    let mut over = scenario(Some(Geonet {
        forwarding: "overlay".into(),
        ..Geonet::default()
    }));
    hard_brake(&mut over);
    let mut overlay = Engine::build(over, "").expect("builds");
    let mut rec2 = MemoryRecorder::new();
    let rep2 = overlay.run(&mut rec2).expect("runs");
    assert!(rep2.gn_air.is_none());
    let senders2 = denm_senders(&rec2);
    assert_eq!(
        senders2.len(),
        1,
        "the overlay puts only the origin's DENMs on the air: {senders2:?}"
    );
    assert!(
        senders.values().sum::<u64>() > senders2.values().sum::<u64>(),
        "forwards load the channel on the air: {senders:?} vs {senders2:?}"
    );
}

/// The access-layer settings change the run: the ETSI DCC profiles put CAMs in the
/// best-effort queue (a longer AIFS and contention window, so a longer access delay), and
/// the global CBR raises the congestion control's input above what a station hears.
#[test]
fn queue_profiles_and_the_global_cbr_change_the_run() {
    let base = || {
        let mut s = scenario(None);
        s.time.duration_s = 8.0;
        s.events.clear();
        s
    };
    let single = run(base());
    let mut etsi = base();
    etsi.net.access_categories = Some("etsi-profiles".into());
    etsi.net.dcc_cbr = Some("global".into());
    etsi.net.mac_queue_depth = Some(4);
    let etsi = run(etsi);
    let per = |r: &RunReport| r.mac_access_delay_ns as f64 / r.mac_grants.max(1) as f64;
    eprintln!(
        "single {:.0} ns/grant ({} grants), etsi {:.0} ns/grant ({} grants), gcbr {:?}",
        per(&single),
        single.mac_grants,
        per(&etsi),
        etsi.mac_grants,
        etsi.gcbr
    );
    assert!(single.mac_grants > 100 && etsi.mac_grants > 100);
    assert!(
        per(&etsi) > per(&single),
        "AC_BE waits longer than AC_VO: {} vs {}",
        per(&etsi),
        per(&single)
    );
    assert!(single.gcbr.is_none());
    let g = etsi.gcbr.expect("global CBR tally");
    assert!(g.updates > 0 && g.raised > 0, "{g:?}");
    // Validated: a global CBR needs GeoNetworking headers.
    let mut bad = base();
    bad.net.layer = "wsmp".into();
    bad.security.envelope = "ieee1609.2".into();
    bad.messages.sets = vec!["bsm".into()];
    bad.net.dcc_cbr = Some("global".into());
    let msg = format!("{:?}", bad.validate());
    assert!(msg.contains("net.dcc_cbr"), "{msg}");
}
