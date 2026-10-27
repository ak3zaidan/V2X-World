//! WP-13 multi-channel, part 1: the run's own channel map (`radio.channels`), the second
//! radio (`radio.devices.*.radios`) and the leakage between the run's own channels.
//!
//! Each property is checked against its theory: a map that names only the safety channel
//! is the single-channel run byte for byte; moving SPaT and MAP off the safety channel
//! lowers its busy ratio by the busy share those messages had there; and a frame on the
//! adjacent channel raises the noise a safety frame is decoded against by exactly the
//! ACIR `v2xw_radio::regulation` computes. Every check was shown red under an injected
//! fault before it was kept (adoption plan WP-13 status note).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_metrics::channels::{NodeTxView, PhyRxView};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// Four roadside units broadcasting SPaT and MAP on the procedural grid, `secs` long.
fn intersections(secs: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("connected-intersections.yaml"))
        .expect("the shipped scenario loads");
    s.time.duration_s = secs;
    s
}

fn with_map(mut s: Scenario, map: serde_json::Value) -> Scenario {
    s.radio.channels = serde_json::from_value(map).expect("radio.channels");
    s
}

fn run(scenario: Scenario) -> (RunReport, MemoryRecorder) {
    let errors = v2xw_engine::scenario::validate(&scenario);
    assert!(errors.is_empty(), "the scenario does not load: {errors:?}");
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    (report, recorder)
}

fn views<V: v2xw_metrics::channels::ChannelView + serde::de::DeserializeOwned>(
    recorder: &MemoryRecorder,
) -> Vec<V> {
    recorder
        .records()
        .iter()
        .filter(|(_, r)| r.channel == V::CHANNEL)
        .map(|(_, r)| v2xw_metrics::channels::decode(r).expect("decodes"))
        .collect()
}

fn digest(r: &MemoryRecorder) -> Vec<u8> {
    let mut out = Vec::new();
    for (t, rec) in r.records() {
        out.extend_from_slice(&t.to_le_bytes());
        out.extend_from_slice(rec.channel.as_bytes());
        out.extend_from_slice(&rec.json);
    }
    out
}

fn refusals(s: &Scenario) -> String {
    v2xw_engine::scenario::validate(s)
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

/// The mean busy ratio of one channel over every unit tuned to it.
fn cbr(report: &RunReport, ch: u16) -> f64 {
    let busy = report.channel_busy_ns.get(&ch).copied().unwrap_or(0) as f64;
    let window = report.channel_window_ns.get(&ch).copied().unwrap_or(0) as f64;
    assert!(window > 0.0, "no unit listened on channel {ch}: {report:?}");
    busy / window
}

/// Each transmitted message's type and channel, by message id.
fn tx_index(recorder: &MemoryRecorder) -> BTreeMap<u64, (String, u16)> {
    views::<NodeTxView>(recorder)
        .into_iter()
        .filter_map(|v| Some((v.msg?, (v.msg_type?, v.channel?))))
        .collect()
}

/// A map that names only the safety channel builds a plan and takes every multi-channel
/// code path, and is the single-channel run byte for byte: every record on every
/// channel, and the report. The fault that proved it can fail: applying the ACIR to a
/// co-channel interferer (`onto` attenuating whenever a plan exists).
#[test]
fn a_map_of_the_safety_channel_alone_is_the_single_channel_run() {
    let base = intersections(6.0);
    let (r0, rec0) = run(base.clone());
    let mapped = with_map(base, serde_json::json!([{"channel": 172, "messages": ["spat", "map"]}]));
    let (r1, rec1) = run(mapped);
    assert!(rec0.records().len() > 1_000, "{} records", rec0.records().len());
    assert!(r0.channel_busy_ns.is_empty());
    let mut r1_cmp = r1.clone();
    r1_cmp.channel_busy_ns.clear();
    r1_cmp.channel_window_ns.clear();
    assert_eq!(r0, r1_cmp, "the report moved");
    assert!(digest(&rec0) == digest(&rec1), "the records moved");
    // Same seed, same multi-channel run: byte-identical.
    let multi = || {
        let mut s = with_map(
            intersections(4.0),
            serde_json::json!([{"channel": 174, "messages": ["spat", "map"]}]),
        );
        s.radio.devices.obu.radios = Some(2);
        run(s)
    };
    let (a, ra) = multi();
    let (b, rb) = multi();
    assert_eq!(a, b);
    assert!(digest(&ra) == digest(&rb), "a multi-channel run is not deterministic");
}

/// Moving SPaT and MAP off the safety channel lowers its channel busy ratio by the share
/// of its busy time those messages took: `CBR'₁₇₂ = CBR₁₇₂ · (1 − s)` with `s` the
/// airtime-weighted share of the moved messages among the safety channel's above-threshold
/// receptions in the single-channel run, and the service channel then carries
/// `CBR₁₇₂ · s`. With ideal filters (`radio.channel_leakage: false`) nothing else moves
/// the busy time. The fault that proved it can fail: the busy interval noted on the run's
/// safety channel whatever the frame's channel (the engine before WP-13).
#[test]
fn moving_spat_and_map_off_the_safety_channel_lowers_its_cbr_by_their_share() {
    let secs = 12.0;
    let mut single = with_map(
        intersections(secs),
        serde_json::json!([{"channel": 172, "messages": ["spat", "map"]}]),
    );
    single.radio.devices.obu.radios = Some(2);
    let (r_single, rec_single) = run(single);
    let cbr_single = cbr(&r_single, 172);
    // The moved share, from the single-channel run's own receptions.
    let tx = tx_index(&rec_single);
    let mut moved_ns = 0u64;
    let mut all_ns = 0u64;
    for v in views::<PhyRxView>(&rec_single) {
        let (Some(msg), Some(rssi)) = (v.msg, v.rssi_dbm) else {
            continue;
        };
        if rssi < -85.0 {
            continue;
        }
        let air = v.t_end.saturating_sub(v.t_start);
        all_ns += air;
        if tx
            .get(&msg)
            .is_some_and(|(m, _)| m == "spat" || m == "map")
        {
            moved_ns += air;
        }
    }
    let share = moved_ns as f64 / all_ns as f64;
    assert!(share > 0.05, "SPaT and MAP are {share} of the busy time");

    let mut multi = with_map(
        intersections(secs),
        serde_json::json!([{"channel": 180, "messages": ["spat", "map"]}]),
    );
    multi.radio.devices.obu.radios = Some(2);
    multi.radio.channel_leakage = Some(false);
    let (r_multi, rec_multi) = run(multi.clone());
    // The SPaT and MAP really left: every one on 180, every BSM on 172.
    let channels: std::collections::BTreeSet<(String, u16)> =
        tx_index(&rec_multi).into_values().collect();
    assert!(channels.contains(&("spat".to_string(), 180)), "{channels:?}");
    assert!(
        channels
            .iter()
            .all(|(m, c)| (m == "spat" || m == "map") == (*c == 180)),
        "{channels:?}"
    );
    let cbr_172 = cbr(&r_multi, 172);
    let cbr_180 = cbr(&r_multi, 180);
    let want_172 = cbr_single * (1.0 - share);
    let want_180 = cbr_single * share;
    let drop = cbr_single - want_172;
    eprintln!(
        "CBR172 single {cbr_single:.5}, share {share:.4}; multi 172 {cbr_172:.5} (want \
         {want_172:.5}), 180 {cbr_180:.5} (want {want_180:.5})"
    );
    assert!(
        (cbr_172 - want_172).abs() <= 0.15 * drop,
        "the safety channel's CBR is {cbr_172:.5}, theory {want_172:.5} (drop {drop:.5})"
    );
    assert!(
        (cbr_180 - want_180).abs() <= 0.15 * want_180,
        "the service channel's CBR is {cbr_180:.5}, theory {want_180:.5}"
    );
    // With the leakage on, the RSU frames on 180 still put energy on 172 (ACIR about
    // 32 dB), so 172 is busier than with ideal filters but well under the single run.
    multi.radio.channel_leakage = None;
    let (r_leak, _) = run(multi);
    let cbr_leak = cbr(&r_leak, 172);
    assert!(
        cbr_leak >= cbr_172 && cbr_leak < cbr_single,
        "with leakage 172 is {cbr_leak:.5}; ideal {cbr_172:.5}, single {cbr_single:.5}"
    );
}

/// A frame on the adjacent channel raises the noise a safety-channel frame is decoded
/// against by exactly the ACIR `v2xw_radio::regulation` computes for the pair: for a BSM
/// on 172 whose air time lies inside one SPaT or MAP frame on 174 at the same receiver
/// (a unit with both radios hears both), the SINR the PHY reports is `S − (N ⊕ (P −
/// ACIR))`, `P` that frame's received power. The noise `N` is read from the receptions
/// nothing overlapped. The fault that proved it can fail: leakage disabled.
#[test]
fn the_adjacent_channel_noise_rise_is_the_regulation_acir() {
    use v2xw_radio::regulation::{Region, Technology, adjacent_acir_db};
    let g5 = Technology::Ieee80211p;
    let acir = adjacent_acir_db(Region::Us2016, g5, 174, g5, 172, 2).expect("adjacent pair");
    let mut s = with_map(
        intersections(15.0),
        serde_json::json!([{"channel": 174, "messages": ["spat", "map"]}]),
    );
    s.radio.devices.obu.radios = Some(2);
    let (_, rec) = run(s);
    let tx = tx_index(&rec);
    let rx = views::<PhyRxView>(&rec);
    let channel_of = |v: &PhyRxView| v.msg.and_then(|m| tx.get(&m)).map(|(_, c)| *c);
    // Every reception per receiver, for the overlap search.
    let mut at: BTreeMap<u32, Vec<&PhyRxView>> = BTreeMap::new();
    for v in &rx {
        at.entry(v.rx.index()).or_default().push(v);
    }
    let overlaps = |a: &PhyRxView, b: &PhyRxView| a.t_start < b.t_end && b.t_start < a.t_end;
    // The noise floor as recorded: S − SINR of the safety frames nothing overlapped.
    let mut quiet: Vec<f64> = Vec::new();
    let mut covered: Vec<(f64, f64, f64)> = Vec::new();
    for v in &rx {
        if channel_of(v) != Some(172) {
            continue;
        }
        let (Some(s_dbm), Some(sinr)) = (v.rssi_dbm, v.sinr_db) else {
            continue;
        };
        let others: Vec<&&PhyRxView> = at[&v.rx.index()]
            .iter()
            .filter(|o| o.msg != v.msg && overlaps(o, v))
            .collect();
        if others.is_empty() {
            quiet.push(s_dbm - sinr);
        } else if let [o] = others.as_slice()
            && channel_of(o) == Some(174)
            && o.t_start <= v.t_start
            && o.t_end >= v.t_end
            && let Some(p) = o.rssi_dbm
        {
            covered.push((s_dbm, sinr, p));
        }
    }
    assert!(quiet.len() > 100, "{} quiet receptions", quiet.len());
    quiet.sort_by(f64::total_cmp);
    let n = quiet[quiet.len() / 2];
    let rises: Vec<(f64, f64)> = covered
        .iter()
        .map(|&(s_dbm, sinr, p)| {
            let measured = (s_dbm - sinr) - n;
            let want = 10.0 * (1.0 + 10f64.powf((p - acir - n) / 10.0)).log10();
            (measured, want)
        })
        .filter(|&(_, want)| want >= 1.0)
        .collect();
    eprintln!(
        "ACIR {acir:.3} dB, N {n:.2} dBm, {} covered, {} with a rise of 1 dB or more",
        covered.len(),
        rises.len()
    );
    assert!(rises.len() >= 5, "{} covered safety frames with a rise", rises.len());
    let close = rises
        .iter()
        .filter(|(m, w)| (m - w).abs() <= 0.3)
        .count();
    assert!(
        close * 10 >= rises.len() * 9,
        "{close} of {} noise rises within 0.3 dB of the ACIR's: {rises:?}",
        rises.len()
    );
}

/// The second radio decides who hears and who sends the service channel. With one radio
/// a vehicle hears no SPaT or MAP on 180 (the energy reaches it, not the frame), and a
/// roadside unit with one radio sends none; with two, both do. The counterexample is a
/// unit that sends and hears every channel whatever its radios (every unit tuned to all).
#[test]
fn the_second_radio_decides_who_hears_and_sends_the_service_channel() {
    let map = serde_json::json!([{"channel": 180, "messages": ["spat", "map"]}]);
    let vehicles_hear = |rec: &MemoryRecorder| -> usize {
        let tx = tx_index(rec);
        let rsus: std::collections::BTreeSet<u32> = views::<NodeTxView>(rec)
            .iter()
            .filter(|v| v.msg_type.as_deref() == Some("spat"))
            .map(|v| v.node.index())
            .collect();
        views::<PhyRxView>(rec)
            .iter()
            .filter(|v| !rsus.contains(&v.rx.index()))
            .filter(|v| {
                v.msg
                    .and_then(|m| tx.get(&m))
                    .is_some_and(|(m, _)| m == "spat")
            })
            .count()
    };
    let one = with_map(intersections(5.0), map.clone());
    let (r1, rec1) = run(one);
    assert!(r1.untuned_arrivals > 0, "{r1:?}");
    assert_eq!(r1.untuned_tx_drops, 0);
    assert_eq!(vehicles_hear(&rec1), 0, "a one-radio vehicle heard a SPaT on 180");
    let mut two = with_map(intersections(5.0), map.clone());
    two.radio.devices.obu.radios = Some(2);
    let (_, rec2) = run(two);
    assert!(vehicles_hear(&rec2) > 0, "a two-radio vehicle heard no SPaT");
    let mut mute = with_map(intersections(5.0), map);
    mute.radio.devices.rsu.radios = Some(1);
    let (r3, rec3) = run(mute);
    assert!(r3.untuned_tx_drops > 0, "{r3:?}");
    assert!(
        !tx_index(&rec3).values().any(|(m, _)| m == "spat"),
        "a one-radio unit sent a SPaT on the service channel"
    );
}

/// The loader refuses a map the band plan or the model cannot carry, and says why.
#[test]
fn the_loader_refuses_a_map_the_band_plan_cannot_carry() {
    let base = intersections(1.0);
    let cases = [
        (serde_json::json!([{"channel": 180, "messages": ["bsm"]}]), "radio.channel"),
        (serde_json::json!([{"channel": 183, "messages": ["spat"]}]), "opens 802.11p"),
        (serde_json::json!([{"channel": 180, "messages": ["tim"]}]), "not a message set"),
        (
            serde_json::json!([
                {"channel": 180, "messages": ["spat"]},
                {"channel": 182, "messages": ["map"]}
            ]),
            "second service channel",
        ),
        (
            serde_json::json!([
                {"channel": 180, "messages": ["spat"]},
                {"channel": 180, "messages": ["spat"]}
            ]),
            "second time",
        ),
    ];
    for (map, why) in cases {
        let s = with_map(base.clone(), map.clone());
        let r = refusals(&s);
        assert!(r.contains(why), "{map} -> {r}");
    }
    let mut three = with_map(base.clone(), serde_json::json!([{"channel": 180, "messages": ["spat"]}]));
    three.radio.devices.obu.radios = Some(3);
    assert!(refusals(&three).contains("radio.devices.obu.radios"), "{}", refusals(&three));
    // An empty map is the default and loads.
    assert_eq!(refusals(&base), "");
}

/// Each frame's wait between the MAC hand-off and the air, ms, by message type.
fn gate_waits_ms(recorder: &MemoryRecorder) -> BTreeMap<String, Vec<f64>> {
    let mut out: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for (_, r) in recorder.records() {
        if r.channel != "node.tx" {
            continue;
        }
        let v: serde_json::Value = serde_json::from_slice(&r.json).expect("json");
        let (Some(t), Some(h), Some(m)) =
            (v["t"].as_u64(), v["t_handoff"].as_u64(), v["msg_type"].as_str())
        else {
            continue;
        };
        out.entry(m.to_string())
            .or_default()
            .push(t.saturating_sub(h) as f64 / 1e6);
    }
    out
}

/// radio-fix3 R3.2.DCC: an ETSI gatekeeper runs per channel. The roadside units send SPaT
/// at 10 Hz on the safety channel and, with the map, MAP at 1 Hz on the service channel
/// through their second radio. TS 102 687's adaptive gate at low load spaces a unit's
/// transmissions by `T_on / delta_max`, held to its 25 ms floor, and the CBR that sets it
/// is the channel's own: on its own channel neither a SPaT (100 ms apart) nor a MAP (1 s
/// apart) finds its gate shut, so no frame waits more than an access delay (AIFS plus
/// backoff, well under 1 ms). One gate shared by the two radios holds every MAP sent
/// within 25 ms of the unit's last SPaT, or the SPaT after it.
/// The fault that proved it: `dcc_service` never built (the shared gate).
#[test]
fn the_etsi_gate_spaces_each_channel_on_its_own() {
    let mut s = with_map(
        intersections(10.0),
        serde_json::json!([{"channel": 174, "messages": ["map"]}]),
    );
    s.radio.models.insert(
        "dcc".to_string(),
        v2xw_engine::scenario::ModelChoice {
            id: "dcc/etsi/adaptive-ts102687".to_string(),
            params: serde_json::Value::Null,
        },
    );
    let (_, rec) = run(s);
    let waits = gate_waits_ms(&rec);
    let held = |m: &str| {
        let v = waits.get(m).map(Vec::as_slice).unwrap_or_default();
        (v.iter().filter(|w| **w > 1.0).count(), v.len())
    };
    let (map_held, maps) = held("map");
    let (spat_held, spats) = held("spat");
    println!("map held {map_held}/{maps}, spat held {spat_held}/{spats}");
    assert!(maps >= 20 && spats >= 200, "too few frames: {maps} MAP, {spats} SPaT");
    assert_eq!(map_held, 0, "a MAP waited at a gate its own channel never shut");
    assert_eq!(spat_held, 0, "a SPaT waited at a gate its own channel never shut");
}

/// Frames by (sender, message type, channel).
fn sent_by(recorder: &MemoryRecorder) -> BTreeMap<(u64, String, u16), usize> {
    let mut out = BTreeMap::new();
    for v in views::<NodeTxView>(recorder) {
        if let (Some(m), Some(c)) = (v.msg_type, v.channel) {
            *out.entry((u64::from(v.node.index()), m, c)).or_insert(0) += 1;
        }
    }
    out
}

/// radio-fix3 R3.2.JAM: a jammer acts on its own channel. A constant jammer riding one
/// roadside unit on the safety channel (172) holds that unit's 172 radio busy, so its SPaT
/// waits (the denial of channel access a constant jammer causes); its second radio on the
/// service channel (174) senses through its own channel filter, where the jammer is not,
/// so its MAP goes out as often as with no jammer. Before the fix the energy detector of
/// every radio counted every jammer, whatever its channel, and the MAP was held with the
/// SPaT. The fault that proved it: the `a.channel == channel` filter removed from the
/// engine's carrier sense.
#[test]
fn a_safety_channel_jammer_leaves_the_service_radio_free() {
    let mapped = || {
        with_map(
            intersections(8.0),
            serde_json::json!([{"channel": 174, "messages": ["map"]}]),
        )
    };
    let (_, clean) = run(mapped());
    let clean_n = sent_by(&clean);
    let rsu = clean_n
        .keys()
        .find(|(_, m, _)| m == "spat")
        .map(|(n, _, _)| *n)
        .expect("a SPaT sender");
    let mut s = mapped();
    s.threats.jammers = vec![v2xw_engine::scenario::ModelChoice {
        id: "attacker/jammer/constant".to_string(),
        params: serde_json::json!({ "follow_node": rsu, "power_dbm": 23.0, "from_s": 2.0 }),
    }];
    let (_, jammed) = run(s);
    let jammed_n = sent_by(&jammed);
    let n = |m: &BTreeMap<(u64, String, u16), usize>, t: &str, c: u16| {
        m.get(&(rsu, t.to_string(), c)).copied().unwrap_or(0)
    };
    let (spat0, spat1) = (n(&clean_n, "spat", 172), n(&jammed_n, "spat", 172));
    let (map0, map1) = (n(&clean_n, "map", 174), n(&jammed_n, "map", 174));
    println!("unit {rsu}: SPaT {spat0} -> {spat1}, MAP {map0} -> {map1}");
    assert!(spat0 >= 60 && map0 >= 6, "too few frames: {spat0} SPaT, {map0} MAP");
    // The jammer denies the safety channel from 2 s: at most the first 2 s of SPaT.
    assert!(spat1 * 2 < spat0, "the jammer did not deny the safety channel: {spat0} -> {spat1}");
    assert!(map1 + 1 >= map0, "the service radio was held by a 172 jammer: MAP {map0} -> {map1}");
}
