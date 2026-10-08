//! The researcher-grade settings (`radio.phy`, `radio.mac`, `radio.channel_model`,
//! `radio.dcc`, `time.warmup_s`): each one changes a measured number of the run, and the
//! loader refuses the values and combinations that would leave one with nothing to act on.
//!
//! Every test runs a baseline and the setting side by side on the same seed, so a setting
//! that changed nothing fails its assertion (a check that cannot fail is not a check).

use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The scaling ladder's Manhattan base with a bulk-spawned fleet of `n`, in open air.
fn fleet(n: u32, secs: f64) -> Scenario {
    let mut s =
        Scenario::load(scenarios().join("scale/base.yaml")).expect("the shipped scenario loads");
    s.time.duration_s = secs;
    s.actors.vehicles.demand.params = serde_json::json!({ "max_total_vehicles": n });
    s.world.buildings.enabled = false;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
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

fn pdr(r: &RunReport) -> f64 {
    r.receptions_ok as f64 / r.reception_attempts.max(1) as f64
}

fn access_delay_us(r: &RunReport) -> f64 {
    r.mac_access_delay_ns as f64 / r.mac_grants.max(1) as f64 / 1e3
}

fn refused(s: &Scenario, field: &str) -> bool {
    v2xw_engine::scenario::validate::validate(s)
        .iter()
        .any(|e| e.to_string().contains(field))
}

/// PHY: 27 Mbit/s needs about 15 dB more signal than 6 Mbit/s, so fewer attempts decode;
/// a 15 dB noise figure costs reception the same way.
#[test]
fn the_data_rate_and_noise_figure_change_delivery() {
    let base = fleet(20, 2.0);
    let (b, _) = run(base.clone());
    let mut fast = base.clone();
    fast.radio.phy.data_rate_mbps = Some(27.0);
    let (f, _) = run(fast);
    let mut noisy = base.clone();
    noisy.radio.phy.noise_figure_db = Some(15.0);
    let (n, _) = run(noisy);
    assert!(b.reception_attempts > 200, "too few attempts to compare");
    assert!(
        pdr(&f) < pdr(&b) - 0.02,
        "27 Mbit/s delivered {:.3} against 6 Mbit/s's {:.3}",
        pdr(&f),
        pdr(&b)
    );
    assert!(
        n.receptions_ok < b.receptions_ok,
        "a 15 dB noise figure decoded {} against {}",
        n.receptions_ok,
        b.receptions_ok
    );
}

/// MAC: a 1023-slot voice window makes every frame wait thousands of microseconds where
/// the standard window of 3 slots waits tens.
#[test]
fn the_contention_window_changes_the_access_delay() {
    let base = fleet(20, 2.0);
    let (b, _) = run(base.clone());
    let mut wide = base;
    wide.radio.mac.cw_min_vo = Some(1023);
    wide.radio.mac.aifsn_vo = Some(9);
    let (w, _) = run(wide);
    assert!(
        access_delay_us(&w) > 5.0 * access_delay_us(&b).max(1.0),
        "CWmin 1023 waited {:.1} µs against {:.1} µs",
        access_delay_us(&w),
        access_delay_us(&b)
    );
}

/// Channel: an exponent of 3.8 loses far more than free space's 2, and Rayleigh fading
/// loses frames that no fading keeps.
#[test]
fn the_channel_model_numbers_change_delivery() {
    let base = fleet(20, 2.0);
    let mut fs = base.clone();
    fs.radio.channel_model.path_loss_exponent = Some(2.0);
    fs.radio.channel_model.shadowing_sigma_db = Some(0.0);
    fs.radio.channel_model.fading = Some("none".into());
    let (a, _) = run(fs.clone());
    let mut steep = fs.clone();
    steep.radio.channel_model.path_loss_exponent = Some(3.8);
    let (st, _) = run(steep);
    assert!(
        st.receptions_ok < a.receptions_ok,
        "exponent 3.8 decoded {} against free space's {}",
        st.receptions_ok,
        a.receptions_ok
    );
    let mut ray = fs;
    ray.radio.channel_model.fading = Some("rayleigh".into());
    let (r, _) = run(ray);
    assert!(
        r.receptions_ok < a.receptions_ok,
        "Rayleigh decoded {} against no fading's {}",
        r.receptions_ok,
        a.receptions_ok
    );
}

fn dcc_labels(rec: &MemoryRecorder) -> Vec<String> {
    rec.records()
        .iter()
        .filter(|(_, r)| r.channel == "node.tx")
        .filter_map(|(_, r)| {
            let v: serde_json::Value = serde_json::from_slice(&r.json).ok()?;
            v.get("dcc_state")?.as_str().map(str::to_string)
        })
        .collect()
}

/// DCC: in Europe the ETSI adaptive gatekeeper runs. A smoothing factor of 0.5 drives the
/// allowed duty cycle δ from 3 % to about 0.1 % (δ ≈ 2·G_max+ when α = 0.5), so the
/// gate holds frames back and fewer go out; with DCC off no frame carries a DCC state.
#[test]
fn the_congestion_control_numbers_change_the_message_rate() {
    let mut base = fleet(20, 3.0);
    base.radio.region = Some(serde_json::from_value(serde_json::json!("eu")).expect("a region"));
    let (d, drec) = run(base.clone());
    let mut fast = base.clone();
    fast.radio.dcc.adaptive_alpha = Some(0.5);
    let (f, _) = run(fast);
    let mut off = base;
    off.radio.dcc.enabled = Some(false);
    let (_, orec) = run(off);
    assert!(
        (f.frames_transmitted as f64) < 0.8 * d.frames_transmitted as f64,
        "alpha 0.5 sent {} frames against {} at the standard 0.016",
        f.frames_transmitted,
        d.frames_transmitted
    );
    assert!(
        dcc_labels(&drec)
            .iter()
            .any(|l| l.starts_with("etsi-adaptive"))
    );
    assert!(
        dcc_labels(&orec).is_empty(),
        "a frame carried a DCC state with congestion control off"
    );
}

/// Run: a warm-up keeps every metric sample of its windows out of the recording.
#[test]
fn the_warm_up_drops_the_early_metric_samples() {
    let base = fleet(10, 3.0);
    let count = |rec: &MemoryRecorder| {
        rec.records()
            .iter()
            .filter(|(_, r)| r.channel == "metric.sample")
            .map(|(t, _)| *t)
            .collect::<Vec<_>>()
    };
    let (_, b) = run(base.clone());
    let mut warm = base;
    warm.time.warmup_s = Some(2.0);
    let (_, w) = run(warm);
    let (bt, wt) = (count(&b), count(&w));
    assert!(!bt.is_empty() && !wt.is_empty());
    assert!(bt.iter().any(|t| *t <= 2_000_000_000));
    assert!(
        wt.iter().all(|t| *t > 2_000_000_000),
        "a sample inside the warm-up was recorded"
    );
}

/// The loader refuses what would do nothing or is not a value the standard has.
#[test]
fn the_loader_refuses_meaningless_settings() {
    let base = fleet(5, 1.0);
    let mut s = base.clone();
    s.radio.phy.data_rate_mbps = Some(7.0);
    assert!(refused(&s, "radio.phy.data_rate_mbps"));
    let mut s = base.clone();
    s.radio.mac.cw_min_vo = Some(10);
    assert!(refused(&s, "radio.mac.cw_min_vo"));
    let mut s = base.clone();
    s.radio.channel_model.nakagami_shape = Some(2.0);
    assert!(refused(&s, "radio.channel_model.nakagami_shape"));
    let mut s = base.clone();
    s.radio.dcc.cbr_target = Some(0.6);
    // The US 802.11p default runs J2945/1, which has no target load.
    assert!(refused(&s, "radio.dcc.cbr_target"));
    let mut s = base.clone();
    s.time.warmup_s = Some(5.0);
    assert!(refused(&s, "time.warmup_s"));
    let mut s = base;
    s.radio.rat = v2xw_engine::scenario::schema::Rat::LteV2xPc5;
    s.radio.phy.sensitivity_dbm = Some(-90.0);
    assert!(refused(&s, "radio.phy"));
}

/// C-V2X: random selection instead of sensing, and never keeping a resource, change how
/// many transport blocks collide, so the decoded count moves.
#[test]
fn the_sidelink_scheduling_numbers_change_the_run() {
    let mut base = fleet(20, 2.0);
    base.radio.rat = v2xw_engine::scenario::schema::Rat::LteV2xPc5;
    let (b, _) = run(base.clone());
    let mut blind = base;
    blind.radio.sidelink.sensing = Some(false);
    blind.radio.sidelink.resource_keep_probability = Some(0.0);
    let (r, _) = run(blind);
    assert!(b.reception_attempts > 200, "too few attempts to compare");
    assert_ne!(
        (b.receptions_ok, b.rx_losses.clone()),
        (r.receptions_ok, r.rx_losses.clone()),
        "turning sensing off changed nothing"
    );
}

fn mean_speed(rec: &MemoryRecorder) -> f64 {
    let speeds: Vec<f64> = rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "gt.kinematics")
        .filter_map(|(_, r)| {
            let v: serde_json::Value = serde_json::from_slice(&r.json).ok()?;
            v.get("speed_mps")?.as_f64()
        })
        .collect();
    assert!(
        !speeds.is_empty(),
        "no ground-truth kinematics were recorded"
    );
    speeds.iter().sum::<f64>() / speeds.len() as f64
}

/// Traffic: cars that accelerate at 0.5 m/s² and keep a 2.5 s gap are slower over the
/// first seconds than cars that accelerate at 2.5 m/s² with a 0.8 s gap.
#[test]
fn the_driver_numbers_change_the_traffic() {
    let base = fleet(20, 5.0);
    let mut timid = base.clone();
    timid.actors.vehicles.driver.max_accel_mps2 = Some(0.5);
    timid.actors.vehicles.driver.time_headway_s = Some(2.5);
    let (_, t) = run(timid);
    let mut brisk = base;
    brisk.actors.vehicles.driver.max_accel_mps2 = Some(2.5);
    brisk.actors.vehicles.driver.time_headway_s = Some(0.8);
    brisk.actors.vehicles.driver.politeness = Some(0.0);
    let (_, b) = run(brisk);
    assert!(
        mean_speed(&t) < 0.9 * mean_speed(&b),
        "timid drivers averaged {:.2} m/s against brisk drivers' {:.2} m/s",
        mean_speed(&t),
        mean_speed(&b)
    );
}

/// Messages: a fixed 5 Hz rate sends about half the frames of J2945/1's 10 Hz.
#[test]
fn the_message_rate_changes_the_frame_count() {
    let mut base = fleet(15, 3.0);
    base.radio.dcc.enabled = Some(false);
    let (b, _) = run(base.clone());
    let mut slow = base;
    slow.messages.rate_hz = Some(5.0);
    let (s, _) = run(slow);
    let ratio = s.frames_transmitted as f64 / b.frames_transmitted.max(1) as f64;
    assert!(
        (0.35..0.65).contains(&ratio),
        "5 Hz sent {} frames against 10 Hz's {} (ratio {ratio:.2})",
        s.frames_transmitted,
        b.frames_transmitted
    );
}
