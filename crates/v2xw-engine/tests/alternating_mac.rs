//! WP-16: IEEE 1609.4 alternating access in the EDCA MAC, end to end.
//!
//! Under `net.channel_access: alternating` a single-radio unit is on the safety channel
//! only in the CCH interval (4-50 ms of each 100 ms sync interval) and on the service
//! channel only in the SCH interval (54-100 ms) [IEEE 1609.4-2016 §6.2.5; the interval
//! lengths are UNVERIFIED at the primary level]. The MAC holds every grant that would
//! start in a guard or cross its interval's end (Veins `Mac1609_4` `timeLeftInSlot`,
//! behaviour only), and the receiver side hears a channel only while its radio is on it.
//!
//! Each check was shown red under an injected fault before it was kept (adoption plan
//! WP-16 status note).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::{Engine, RunRecorder, RunReport, Scenario};
use v2xw_metrics::channels::{NodeTxView, PhyRxView, RxOutcome, decode};
use v2xw_radio::wave_schedule::{ChannelAccess, ChannelSchedule, Interval};

const ALT: ChannelSchedule = ChannelSchedule {
    access: ChannelAccess::Alternating,
};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// Keeps the transmissions, and the receptions when asked; nothing else, so a two-minute
/// dense run stays small.
#[derive(Default)]
struct Kept {
    rx: bool,
    tx: Vec<NodeTxView>,
    rxs: Vec<PhyRxView>,
}

impl RunRecorder for Kept {
    fn write(&mut self, _at: SimTime, record: &OwnedRecord) {
        match record.channel {
            "node.tx" => self.tx.push(decode(record).expect("node.tx decodes")),
            "phy.rx" if self.rx => self.rxs.push(decode(record).expect("phy.rx decodes")),
            _ => {}
        }
    }
}

fn run(s: Scenario, rx: bool) -> (RunReport, Kept) {
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "the scenario does not load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut kept = Kept {
        rx,
        ..Kept::default()
    };
    let report = engine.run(&mut kept).expect("runs");
    (report, kept)
}

/// Whether a frame on the air from `t` for `air_us` sits wholly inside the interval
/// `want`, recomputed from the record alone.
fn inside(t: SimTime, air_us: u64, want: Interval) -> bool {
    ALT.interval(t) == want && t + air_us * 1_000 <= ALT.interval_end_ns(t)
}

/// 80 single-radio vehicles on the 1.2 km open grid for 120 s (the plan's ten minutes,
/// scaled to stay inside one piece): every BSM starts inside a CCH interval and ends by
/// its end, recomputed from the `node.tx` records, and the report's own invariant
/// counter agrees. Fault: removing the end-of-interval check (b) from the gate makes
/// frames straddle the interval end.
#[test]
fn dense_run_has_no_straddling_frame() {
    let mut s = Scenario::load(scenarios().join("sensitivity/dsrc-grid.yaml")).expect("loads");
    s.time.duration_s = 120.0;
    s.metrics = vec![];
    s.net.channel_access = Some("alternating".to_string());
    s.actors.vehicles.demand.params = serde_json::json!({ "max_total_vehicles": 80 });
    let (report, kept) = run(s, false);
    let bsms: Vec<&NodeTxView> = kept
        .tx
        .iter()
        .filter(|t| t.msg_type.as_deref() == Some("bsm"))
        .collect();
    let senders: BTreeSet<u32> = bsms.iter().map(|t| t.node.index()).collect();
    assert!(senders.len() >= 70, "{} senders", senders.len());
    assert!(bsms.len() > 60_000, "{} BSMs", bsms.len());
    let straddling: Vec<(SimTime, u64)> = bsms
        .iter()
        .filter(|t| !inside(t.t, t.airtime_us.expect("air time"), Interval::Cch))
        .map(|t| (t.t, t.airtime_us.unwrap_or(0)))
        .collect();
    assert!(
        straddling.is_empty(),
        "{} of {} BSMs straddle a guard, first {:?}",
        straddling.len(),
        bsms.len(),
        straddling.first()
    );
    assert_eq!(report.straddling_frames, 0);
    // The gate was busy holding frames, so the check above had something to check.
    assert!(report.mac_guard_deferrals > 100, "{report:?}");
    assert!(report.mac_gate_deferrals > 1_000, "{report:?}");
}

/// The roadside units (two radios, the default) send SPaT and MAP continuously on the
/// service channel 180. A single-radio vehicle is on 180 only in the SCH interval, so it
/// decodes a SPaT only when the frame falls wholly inside one: none outside, and of the
/// SPaTs two-radio vehicles decode in the same run, the share that fell inside an SCH
/// interval. For a uniformly timed stream that share is 46 % (`usable_fraction`); here
/// each roadside unit sends at its own fixed 100 ms phase, so the share is whatever the
/// four phases make it, and the test weighs it by the receptions it predicts. Fault:
/// `tuned_at` ignoring the time makes the ratio about 1.
#[test]
fn single_radio_hears_spat_only_in_sch() {
    let base = || {
        let mut s =
            Scenario::load(scenarios().join("connected-intersections.yaml")).expect("loads");
        s.time.duration_s = 20.0;
        s.metrics = vec![];
        s.radio.channels = serde_json::from_value(
            serde_json::json!([{"channel": 180, "messages": ["spat", "map"]}]),
        )
        .expect("radio.channels");
        s.net.channel_access = Some("alternating".to_string());
        s
    };
    // Vehicle SPaT decodes: (all, of frames wholly inside an SCH interval, of the others).
    let heard = |kept: &Kept| -> (usize, usize, usize) {
        let spat: BTreeMap<u64, (u32, SimTime, u64)> = kept
            .tx
            .iter()
            .filter(|t| t.msg_type.as_deref() == Some("spat"))
            .filter_map(|t| Some((t.msg?, (t.node.index(), t.t, t.airtime_us?))))
            .collect();
        let rsus: BTreeSet<u32> = spat.values().map(|v| v.0).collect();
        let (mut inn, mut out) = (0, 0);
        for r in kept
            .rxs
            .iter()
            .filter(|r| !rsus.contains(&r.rx.index()) && r.outcome == RxOutcome::Ok)
        {
            if let Some((_, t, air)) = r.msg.and_then(|m| spat.get(&m)) {
                if inside(*t, *air, Interval::Sch) {
                    inn += 1;
                } else {
                    out += 1;
                }
            }
        }
        (inn + out, inn, out)
    };
    let (r1, one) = run(base(), true);
    let mut two = base();
    two.radio.devices.obu.radios = Some(2);
    let (_, both) = run(two, true);
    let (n1, _, outside1) = heard(&one);
    let (n2, inside2, _) = heard(&both);
    assert!(n2 > 200, "two-radio vehicles heard {n2} SPaTs");
    let ratio = n1 as f64 / n2 as f64;
    let expected = inside2 as f64 / n2 as f64;
    eprintln!(
        "one-radio {n1} (outside SCH {outside1}) two-radio {n2} ratio {ratio:.3} expected {expected:.3} uniform {:.3}",
        ALT.usable_fraction()
    );
    assert_eq!(
        outside1, 0,
        "a one-radio vehicle decoded a SPaT outside the SCH interval"
    );
    assert!(
        n1 > 0,
        "a one-radio vehicle never heard the service channel"
    );
    assert!(
        (ratio - expected).abs() <= 0.05,
        "a one-radio vehicle heard {ratio:.3} of the SPaTs, the SCH share predicts {expected:.3}"
    );
    // And a one-radio vehicle could send on the service channel: nothing was refused.
    assert_eq!(r1.untuned_tx_drops, 0);
}

/// Every expectation row on `net.channel_access` (`tests/expectations/*.yaml`) holds on
/// the family the catalogue routes it to (the DSRC grid, 2 s: `gn` refuses alternating
/// on GeoNetworking): the latency, the gate's guard holds, the reception ratio and the
/// collision rate all move as the rows' sources say. Fault: the engine building the MAC
/// without the gate leaves `report.mac_guard_deferrals` at zero.
#[test]
fn the_channel_access_rows_hold_on_the_dsrc_family() {
    use v2xw_engine::sensitivity::{self as sens, Row, sweep};
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expectations");
    let rows: Vec<sweep::Expectation> = sweep::load_expectations(&[dir])
        .expect("rows parse")
        .into_iter()
        .filter(|e| e.setting == "net.channel_access")
        .collect();
    assert!(rows.len() >= 6, "{} rows", rows.len());
    let base = Scenario::load(scenarios().join("sensitivity/dsrc-grid.yaml")).expect("loads");
    let set =
        serde_json::from_value(serde_json::json!({"time.duration_s": 2.0, "metrics": ["all"]}))
            .expect("set");
    let base = sens::apply(&base, &set).expect("applies");
    let mut alt = base.clone();
    alt.net.channel_access = Some("alternating".to_string());
    let b = sens::measure(base).expect("runs");
    let a = sens::measure(alt).expect("runs");
    let mut failed = Vec::new();
    for e in &rows {
        assert_eq!(e.change, serde_json::json!("alternating"));
        let row = Row {
            name: e.setting.clone(),
            set: BTreeMap::new(),
            base_set: BTreeMap::new(),
            metric: e.metric.clone(),
            expect: e.expect,
            band: e.band,
            min_change: e.min_change,
            source: Some(e.source.clone()),
        };
        let r = sens::judge(&row, b.get(&e.metric), a.get(&e.metric));
        eprintln!(
            "row {} {:?} -> {:?} pass {}",
            e.metric, r.base, r.value, r.pass
        );
        if !r.pass {
            failed.push(format!("{}: {}", e.metric, r.note));
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
