//! The channel a run's radios share, measured end to end: what the channel busy ratio
//! reports on every radio access technology, which band plan and power limits a region
//! imposes, and how the channel behaves as density rises.
//!
//! Every property is shown against its counterexample in the same test where one exists.

use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_metrics::channels::MacCbrView;

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The procedural Midtown grid (no external input), `secs` long, at `rate` vehicles an
/// hour, with building obstruction off so the medium is shared.
fn grid(rate: f64, secs: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("the shipped scenario loads");
    s.time.duration_s = secs;
    s.actors.vehicles.demand.rate_veh_per_h = Some(rate);
    s.world.buildings.enabled = false;
    s
}

fn with_rat(mut s: Scenario, rat: &str) -> Scenario {
    s.radio.rat = serde_json::from_value(serde_json::json!(rat)).expect("a rat");
    s
}

fn run_recorded(scenario: Scenario) -> (RunReport, MemoryRecorder) {
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

/// A sidelink run reports its channel busy ratio on `mac.cbr` as an 802.11p run does:
/// one record per UE per metric window, on the sidelink's own channel, with the load the
/// UE offered its MAC in that window. The busy ratio is the sidelink CBR the SPS engine
/// measures, so with a loaded pool some UEs see a busy channel.
///
/// The counterexample is the engine before this change, which wrote `mac.cbr` only when
/// an EDCA MAC existed: an LTE-V2X or NR-V2X run had no `mac.cbr` record at all and the
/// `cbr` metric was empty, although the chase view showed a CBR on every sidelink frame.
#[test]
fn a_sidelink_run_reports_its_channel_busy_ratio() {
    for rat in ["lte-v2x-pc5", "nr-v2x-pc5"] {
        let (report, recorder) = run_recorded(with_rat(grid(6_000.0, 8.0), rat));
        assert!(report.frames_transmitted > 0, "{rat}: nothing was sent");
        let cbr = views::<MacCbrView>(&recorder);
        assert!(!cbr.is_empty(), "{rat}: no mac.cbr record on a sidelink run");
        assert!(
            cbr.iter().all(|c| c.channel == Some(183)),
            "{rat}: mac.cbr must be on the sidelink's channel 183"
        );
        let offered: u64 = cbr.iter().filter_map(|c| c.offered_frames).sum();
        assert!(offered > 0, "{rat}: the windows carry no offered load");
        let busy = cbr.iter().filter(|c| c.cbr > 0.0).count();
        assert!(busy > 0, "{rat}: no UE ever measured a busy sub-channel: {cbr:?}");
        assert!(cbr.iter().all(|c| (0.0..=1.0).contains(&c.cbr)));
    }
}

/// An NR-V2X run defaults to the ETSI EN 303 798 pool the Lusvarghi 2024 link-level
/// curves were generated in, reads its block errors from those curves (the manifest pins
/// their card, not the spectral-efficiency fit's), and delivers at short range.
///
/// The counterexample is the engine before this change: Todisco's 10 MHz study pool at
/// TS 38.214 Table 5.1.3.1-1 MCS 9, whose curve was the fit `phy/cv2x/bler-se-gap-fit`.
#[test]
fn an_nr_run_reads_its_block_errors_from_the_link_level_curves() {
    let scenario = with_rat(grid(6_000.0, 10.0), "nr-v2x-pc5");
    let errors = v2xw_engine::scenario::validate(&scenario);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(scenario, "").expect("builds");
    assert!(
        engine
            .registry()
            .contains(v2xw_radio::bler::SidelinkErrorModel::ID_LUSVARGHI),
        "the NR run does not pin the link-level curves' card"
    );
    assert!(
        !engine
            .registry()
            .contains(v2xw_radio::bler::SidelinkErrorModel::ID_SE_FIT),
        "the NR run still pins the fitted curve"
    );
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    let sl = report.sidelink.as_ref().expect("a sidelink report");
    assert_eq!(sl.profile, "etsi-en303798");
    assert_eq!(sl.mcs, "nr-t2-mcs7");
    assert_eq!(sl.subchannels, 4);
    assert_eq!(sl.slot_ns, 500_000);
    let near = views::<v2xw_metrics::channels::PhyRxView>(&recorder)
        .into_iter()
        .filter(|v| v.dist_m.is_some_and(|d| d < 100.0))
        .collect::<Vec<_>>();
    assert!(!near.is_empty(), "no reception within 100 m: {report:?}");
    let ok = near
        .iter()
        .filter(|v| v.outcome == v2xw_metrics::channels::RxOutcome::Ok)
        .count();
    assert!(
        ok as f64 / near.len() as f64 > 0.8,
        "NR delivers {ok} of {} within 100 m",
        near.len()
    );
}

// -----------------------------------------------------------------------------------------
// radio.region and radio.channel
// -----------------------------------------------------------------------------------------

fn with_region(mut s: Scenario, region: &str) -> Scenario {
    s.radio.region = Some(serde_json::from_value(serde_json::json!(region)).expect("a region"));
    s
}

fn refusals(s: &Scenario) -> String {
    v2xw_engine::scenario::validate(s)
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

fn tx_channels(recorder: &MemoryRecorder) -> std::collections::BTreeSet<u16> {
    views::<v2xw_metrics::channels::NodeTxView>(recorder)
        .iter()
        .filter_map(|v| v.channel)
        .collect()
}

/// Which band a run transmits in follows its region's rules, read from the rules
/// themselves (`v2xw_radio::regulation`):
///
/// * unset, 802.11p takes the 2016 US band plan and the J2945/1 channel 172 — every
///   existing DSRC run, unchanged;
/// * `us` refuses 802.11p, because FCC 20-164 gave channel 172 to Wi-Fi and FCC 24-123
///   ends DSRC, and says so;
/// * `eu` puts ITS-G5 on 5.895-5.905 GHz (channel 180), LTE-V2X on the 10 MHz channel
///   182 under ETSI EN 303 613's pool (five 10-PRB sub-channels) and NR-V2X on 178 in
///   24 PRB (two 12-PRB sub-channels);
/// * a channel the region does not open, or a 20 MHz US profile in Europe, is refused.
///
/// The counterexample is the engine before `radio.region`: every run was on 172 or 183
/// whatever it named, and neither key existed.
#[test]
fn the_region_decides_the_channel_and_refuses_what_its_rules_forbid() {
    let base = grid(3_000.0, 3.0);

    let (_, dsrc) = run_recorded(base.clone());
    assert_eq!(tx_channels(&dsrc), [172].into_iter().collect());

    let us_dsrc = with_region(base.clone(), "us");
    let why = refusals(&us_dsrc);
    assert!(
        why.contains("radio.region") && why.contains("FCC 24-123") && why.contains("us-2016"),
        "{why}"
    );

    let (_, g5) = run_recorded(with_region(base.clone(), "eu"));
    assert_eq!(tx_channels(&g5), [180].into_iter().collect());

    let (lte, lte_rec) = run_recorded(with_region(with_rat(base.clone(), "lte-v2x-pc5"), "eu"));
    let sl = lte.sidelink.as_ref().expect("a sidelink report");
    assert_eq!(
        (sl.profile.as_str(), sl.region.as_str(), sl.channel, sl.channel_mhz, sl.subchannels),
        ("etsi-en303613", "eu", 182, 10, 5)
    );
    assert_eq!(tx_channels(&lte_rec), [182].into_iter().collect());

    let (nr, _) = run_recorded(with_region(with_rat(base.clone(), "nr-v2x-pc5"), "eu"));
    let sl = nr.sidelink.as_ref().expect("a sidelink report");
    assert_eq!(
        (sl.profile.as_str(), sl.channel, sl.channel_mhz, sl.subchannels),
        ("etsi-en303798", 178, 10, 2)
    );

    // The US C-V2X default is SAE J3161/1's 20 MHz channel 183.
    let (us_lte, _) = run_recorded(with_rat(base.clone(), "lte-v2x-pc5"));
    let sl = us_lte.sidelink.as_ref().expect("a sidelink report");
    assert_eq!(
        (sl.profile.as_str(), sl.region.as_str(), sl.channel, sl.channel_mhz, sl.subchannels),
        ("sae-j3161", "us", 183, 20, 10)
    );

    // J3161/1's 20 MHz pool has no channel in Europe's 10 MHz plan.
    let mut j3161_eu = with_region(with_rat(base.clone(), "lte-v2x-pc5"), "eu");
    j3161_eu.radio.models.insert(
        "sidelink".to_string(),
        v2xw_engine::scenario::schema::ModelChoice {
            id: "access/sidelink/engine-coupling".to_string(),
            params: serde_json::json!({ "profile": "sae-j3161" }),
        },
    );
    let why = refusals(&j3161_eu);
    assert!(why.contains("sae-j3161") && why.contains("20 MHz"), "{why}");

    // A channel the region does not open to the technology.
    let mut odd = with_region(base.clone(), "eu");
    odd.radio.channel = Some(184);
    let why = refusals(&odd);
    assert!(why.contains("radio.channel") && why.contains("184"), "{why}");
    let mut odd = base;
    odd.radio.channel = Some(169);
    assert!(refusals(&odd).contains("radio.channel"));
}

/// A unit configured above its region's EIRP limit transmits at the limit: a US C-V2X
/// OBU with no geofence may radiate 27 dBm toward the horizon (47 CFR §95.3204(a)(5)),
/// so a 30 dBm radio behind a 3 dBi antenna is turned down to 24 dBm conducted, while in
/// Europe (EN 302 571, 33 dBm) the same radio keeps its 30 dBm.
///
/// The counterexample is the engine before this change, which transmitted whatever
/// `radio.devices` said.
#[test]
fn a_unit_above_its_regions_eirp_limit_transmits_at_the_limit() {
    let mut s = with_rat(grid(3_000.0, 3.0), "lte-v2x-pc5");
    s.radio.devices.obu.tx_power_dbm = 30.0;
    let powers = |rec: &MemoryRecorder| -> std::collections::BTreeSet<i64> {
        views::<v2xw_metrics::channels::NodeTxView>(rec)
            .iter()
            .filter_map(|v| v.power_dbm)
            .map(|p| (p * 10.0).round() as i64)
            .collect()
    };
    let (_, us) = run_recorded(s.clone());
    assert_eq!(powers(&us), [240].into_iter().collect(), "US: 27 dBm EIRP less 3 dBi");
    let (_, eu) = run_recorded(with_region(s, "eu"));
    assert_eq!(powers(&eu), [300].into_iter().collect(), "EU: 33 dBm allows 30 + 3");
}

// -----------------------------------------------------------------------------------------
// Congestion control per region
// -----------------------------------------------------------------------------------------

fn dcc_labels(recorder: &MemoryRecorder) -> std::collections::BTreeSet<String> {
    views::<v2xw_metrics::channels::NodeTxView>(recorder)
        .iter()
        .filter_map(|v| v.dcc_state.as_deref())
        .map(|s| s.split(' ').next().unwrap_or("").to_string())
        .collect()
}

/// An 802.11p unit runs its region's congestion control, and every frame it sends says
/// which: SAE J2945/1 under the US DSRC plan, ETSI TS 102 687's adaptive gatekeeper in
/// Europe (EN 302 571 requires a DCC; TS 102 687 V1.2.1 makes the adaptive approach the
/// normative one), and whichever `radio.models.dcc` names. A DCC on a sidelink is refused.
///
/// The counterexample is the engine before this change: J2945/1 everywhere, including
/// Europe, `radio.models.dcc` refused as an unknown family, and no DCC state on `node.tx`.
#[test]
fn each_region_runs_its_own_congestion_control() {
    let base = grid(3_000.0, 3.0);
    let (_, us) = run_recorded(base.clone());
    assert_eq!(dcc_labels(&us), ["sae-j2945-1".to_string()].into_iter().collect());
    let (_, eu) = run_recorded(with_region(base.clone(), "eu"));
    assert_eq!(dcc_labels(&eu), ["etsi-adaptive".to_string()].into_iter().collect());
    let mut reactive = with_region(base.clone(), "eu");
    reactive.radio.models.insert(
        "dcc".to_string(),
        v2xw_engine::scenario::schema::ModelChoice {
            id: "dcc/etsi/reactive-ts102687".to_string(),
            params: serde_json::json!({}),
        },
    );
    let (_, re) = run_recorded(reactive);
    assert_eq!(dcc_labels(&re), ["etsi-reactive".to_string()].into_iter().collect());
    let mut sl = with_rat(base, "lte-v2x-pc5");
    sl.radio.models.insert(
        "dcc".to_string(),
        v2xw_engine::scenario::schema::ModelChoice {
            id: "dcc/etsi/adaptive-ts102687".to_string(),
            params: serde_json::json!({}),
        },
    );
    let why = refusals(&sl);
    assert!(why.contains("radio.models.dcc"), "{why}");
}
