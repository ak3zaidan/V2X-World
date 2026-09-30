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

fn sidelink_params(mut s: Scenario, params: serde_json::Value) -> Scenario {
    s.radio.models.insert(
        "sidelink".to_string(),
        v2xw_engine::scenario::schema::ModelChoice {
            id: "access/sidelink/engine-coupling".to_string(),
            params,
        },
    );
    s
}

/// An LTE-V2X unit under the US J3161/1 profile runs SAE J3161/1's rate control: every BSM
/// it sends names `sae-j3161-1` and the interval it was generated at, and none carries a
/// J2945/1 power (J3161/1 has no power control), so every BSM goes out at the unit's
/// configured power. `rate_control: off` removes it, NR-V2X runs none by default, and
/// asking for it on NR-V2X, which has no published rate control, is refused.
///
/// The counterexample is the engine before this change: a sidelink run had no
/// application-layer congestion control at all and `node.tx` carried no DCC state.
#[test]
fn an_lte_v2x_unit_runs_j3161_rate_control_without_power_control() {
    let base = with_rat(grid(3_000.0, 3.0), "lte-v2x-pc5");
    let (_, on) = run_recorded(base.clone());
    let tx = views::<v2xw_metrics::channels::NodeTxView>(&on);
    let bsm: Vec<_> = tx
        .iter()
        .filter(|v| v.msg_type.as_deref() == Some("bsm"))
        .collect();
    assert!(!bsm.is_empty(), "the run sent BSMs");
    for v in &bsm {
        let label = v.dcc_state.as_deref().unwrap_or("");
        assert!(label.starts_with("sae-j3161-1 itt="), "{label}");
        assert!(!label.contains("rp="), "J3161/1 controls no power: {label}");
    }
    let powers: std::collections::BTreeSet<i64> = bsm
        .iter()
        .filter_map(|v| v.power_dbm)
        .map(|p| (p * 10.0).round() as i64)
        .collect();
    assert_eq!(powers.len(), 1, "every BSM at the configured power: {powers:?}");

    let (_, off) = run_recorded(sidelink_params(
        base,
        serde_json::json!({ "rate_control": "off" }),
    ));
    assert!(dcc_labels(&off).is_empty(), "{:?}", dcc_labels(&off));

    let nr = sidelink_params(
        with_rat(grid(3_000.0, 3.0), "nr-v2x-pc5"),
        serde_json::json!({ "rate_control": "sae-j3161" }),
    );
    let why = refusals(&nr);
    assert!(why.contains("rate_control"), "{why}");
    let (_, nr_default) = run_recorded(with_rat(grid(3_000.0, 1.0), "nr-v2x-pc5"));
    assert!(dcc_labels(&nr_default).is_empty(), "{:?}", dcc_labels(&nr_default));
}

// -----------------------------------------------------------------------------------------
// Antenna patterns
// -----------------------------------------------------------------------------------------

/// Every vehicle's antenna has TR 37.885's pattern: a car's rooftop antenna is the same
/// all round, a truck's front and rear panels are 6.75 dB down to its side. On a fleet
/// of half cars and half trucks, the same links with every vehicle given a rooftop
/// antenna are never weaker, and some — a truck heard off its axis — are more than 1 dB
/// stronger. With no pattern at all (`isotropic`) no link is weaker than rooftop either.
///
/// The runs have no medium access (`radio.tiers.mac: abstract`), so no transmission's
/// timing depends on what was received and the three runs put the same frames on the air:
/// every link can be compared with itself.
///
/// The counterexample is the engine before this change, which gave every antenna its
/// scalar gain in every direction, so the two runs could not differ.
#[test]
fn a_trucks_antenna_panels_are_weaker_to_its_side() {
    let mut base = grid(20_000.0, 5.0);
    base.radio.tiers.mac = v2xw_core::card::Tier::Abstract;
    base.actors.vehicles.classes = serde_json::from_value(serde_json::json!({
        "passenger": { "fraction": 0.5 },
        "truck": { "fraction": 0.5 },
    }))
    .expect("a class mix");
    let run = |pattern: &str| {
        let mut s = base.clone();
        s.radio.devices.obu.antenna_pattern =
            serde_json::from_value(serde_json::json!(pattern)).expect("a pattern");
        let (_, rec) = run_recorded(s);
        views::<v2xw_metrics::channels::PhyRxView>(&rec)
            .into_iter()
            .filter_map(|v| Some(((v.tx?.index(), v.rx.index(), v.t_start), v.rssi_dbm?)))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let tr = run("tr37885");
    let roof = run("rooftop");
    let iso = run("isotropic");
    let mut matched = 0;
    let mut weaker = 0;
    for (k, &r) in &roof {
        if let Some(&t) = tr.get(k) {
            matched += 1;
            assert!(t <= r + 1e-9, "a pattern added gain on {k:?}: {t} against {r}");
            if t < r - 1.0 {
                weaker += 1;
            }
        }
        if let Some(&i) = iso.get(k) {
            assert!(i >= r - 1e-9, "no pattern is weaker than a rooftop one on {k:?}");
        }
    }
    eprintln!("{matched} links matched, {weaker} weaker by more than 1 dB");
    assert!(matched > 100, "only {matched} links in common");
    assert!(weaker > 0, "no truck was heard off its axis");
}

// -----------------------------------------------------------------------------------------
// Adjacent-channel interference
// -----------------------------------------------------------------------------------------

fn adjacent(
    rat: &str,
    channel: Option<u16>,
    params: serde_json::Value,
) -> v2xw_engine::scenario::schema::AdjacentEmitter {
    serde_json::from_value(serde_json::json!({
        "rat": rat,
        "channel": channel,
        "params": params,
    }))
    .expect("an adjacent emitter")
}

fn centre(s: &Scenario) -> [f64; 2] {
    let world = v2xw_engine::wiring::build_world(s).expect("the world builds");
    [
        (world.bbox.min.x + world.bbox.max.x) * 0.5,
        (world.bbox.min.y + world.bbox.max.y) * 0.5,
    ]
}

/// Europe lets ITS-G5 and LTE-V2X operate side by side (ITS-G5 on 180, LTE-V2X on 182).
/// An LTE-V2X transmitter at the centre of an ITS-G5 run, on all the time at 23 dBm,
/// leaks into the ITS-G5 channel by the ACIR the rules allow (about 13 dB: the ITS-G5
/// receiver's 6 Mbit/s adjacent-channel rejection), and frames near it are lost to
/// `adjacent-channel` — not to `jammed`, since there is no jammer. The same transmitter
/// with a measured-hardware ACIR of 80 dB costs nothing. One asked for on the run's own
/// channel is co-channel and refused, and in the United States, which gives 802.11p no
/// channel, an 802.11p emitter beside a C-V2X run is refused.
///
/// The counterexample is the engine before this change: `radio.adjacent_channel` did not
/// exist and a mixed deployment could not be expressed.
#[test]
fn an_adjacent_channel_transmitter_leaks_in_by_its_acir() {
    let base = with_region(grid(3_000.0, 3.0), "eu");
    let c = centre(&base);
    let mut leaky = base.clone();
    leaky.radio.adjacent_channel = vec![adjacent(
        "lte-v2x-pc5",
        None,
        serde_json::json!({ "position_m": c, "power_dbm": 23.0 }),
    )];
    let (r, _) = run_recorded(leaky.clone());
    eprintln!("losses {:?}", r.rx_losses);
    let aci = r.rx_losses.get("adjacent-channel").copied().unwrap_or(0);
    assert!(aci > 0, "no frame lost to the adjacent channel: {:?}", r.rx_losses);
    assert_eq!(r.rx_losses.get("jammed"), None, "{:?}", r.rx_losses);

    let mut good = leaky.clone();
    good.radio.adjacent_channel[0].acir_db = Some(80.0);
    let (g, _) = run_recorded(good);
    assert_eq!(g.rx_losses.get("adjacent-channel"), None, "{:?}", g.rx_losses);

    let mut co = leaky.clone();
    co.radio.adjacent_channel[0].channel = Some(180);
    let why = refusals(&co);
    assert!(why.contains("co-channel"), "{why}");

    let mut us = with_rat(grid(3_000.0, 3.0), "lte-v2x-pc5");
    us.radio.adjacent_channel = vec![adjacent(
        "dsrc-80211p",
        Some(172),
        serde_json::json!({ "position_m": c }),
    )];
    let why = refusals(&us);
    assert!(why.contains("radio.adjacent_channel"), "{why}");
}
