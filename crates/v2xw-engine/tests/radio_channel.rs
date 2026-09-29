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
