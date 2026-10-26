//! Radio, MAC and congestion parameters move the metrics the way theory says.
//!
//! Each test is one row of a spec in `scenarios/sensitivity/` (`radio-dsrc.yaml`,
//! `radio-dsrc-dense.yaml`, `radio-gn.yaml`, `radio-cv2x.yaml`):
//! the base scenario and the row's change run on the same seed, and the row's metric must
//! move in the expected direction (and inside the band, where one is given). The rows, the
//! theory and the measured numbers are in docs/validation/sensitivity-radio.md; the same
//! rows run from the command line with `v2xw sensitivity <spec>`.
//!
//! A base run is shared by every row with the same base fields, so a test after the first
//! on a base costs one run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use v2xw_engine::Scenario;
use v2xw_engine::sensitivity::{self as sens, Measured, Spec};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/sensitivity")
}

type Specs = BTreeMap<String, (Spec, Scenario)>;
type Bases = BTreeMap<String, Measured>;

fn specs() -> &'static Mutex<Specs> {
    static S: OnceLock<Mutex<Specs>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn bases() -> &'static Mutex<Bases> {
    static B: OnceLock<Mutex<Bases>> = OnceLock::new();
    B.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Runs the row of `file` named `name` that reads `metric`, and fails with its table line
/// unless it holds.
fn row(file: &str, name: &str, metric: &str) {
    let (spec, base) = {
        let mut s = specs().lock().unwrap_or_else(|e| e.into_inner());
        s.entry(file.to_string())
            .or_insert_with(|| Spec::load(dir().join(file)).expect("the spec loads"))
            .clone()
    };
    let row = spec
        .rows
        .iter()
        .find(|r| r.name == name && r.metric == metric)
        .unwrap_or_else(|| panic!("{file} has no row `{name}` on `{metric}`"))
        .clone();
    let key = format!(
        "{file}|{}",
        serde_json::to_string(&row.base_set).expect("serialises")
    );
    let cached = bases()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .cloned();
    let b = match cached {
        Some(b) => b,
        None => {
            let b = sens::measure(sens::apply(&base, &row.base_set).expect("base applies"))
                .expect("the base runs");
            bases()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(key, b.clone());
            b
        }
    };
    let mut both = row.base_set.clone();
    both.extend(row.set.clone());
    let v = sens::measure(sens::apply(&base, &both).expect("change applies")).expect("runs");
    let r = sens::judge(&row, b.get(metric), v.get(metric));
    assert!(r.pass, "\n{}", sens::table(&[r]));
}

fn dsrc(name: &str, metric: &str) {
    row("radio-dsrc.yaml", name, metric);
}

fn dense(name: &str, metric: &str) {
    row("radio-dsrc-dense.yaml", name, metric);
}

fn gn(name: &str, metric: &str) {
    row("radio-gn.yaml", name, metric);
}

fn cv2x(name: &str, metric: &str) {
    row("radio-cv2x.yaml", name, metric);
}

// ---- 802.11p: link budget and channel ----

#[test]
fn dsrc_lower_tx_power_shrinks_the_range() {
    dsrc("tx power 20->5 dBm", "range90_m");
    dsrc("tx power 20->5 dBm", "prr_400_600");
}

#[test]
fn dsrc_higher_noise_figure_shrinks_the_range() {
    dsrc("noise figure 6->12 dB", "range90_m");
    dsrc("noise figure 6->12 dB", "prr_600_1000");
}

#[test]
fn dsrc_steeper_path_loss_lowers_prr() {
    dsrc("PL exponent 2.5->3.5", "prr_200_600");
}

#[test]
fn dsrc_more_shadowing_lowers_prr_inside_the_range() {
    dsrc("shadowing 2->10 dB", "prr_200_600");
}

#[test]
fn dsrc_deeper_fading_lowers_prr() {
    dsrc("nakagami m 3->1", "prr_200_600");
    dsrc("fading none->rayleigh", "prr_200_600");
}

#[test]
fn dsrc_higher_data_rate_shortens_airtime_and_range() {
    dsrc("data rate 6->12 Mbps", "airtime_us");
    dsrc("data rate 6->12 Mbps", "range90_m");
    dsrc("data rate 6->12 Mbps", "cbr_mean");
}

#[test]
fn dsrc_a_certificate_on_every_message_lengthens_the_frame() {
    dsrc("certificate on every message", "psdu_bytes");
    dsrc("certificate on every message", "airtime_us");
    dsrc("certificate on every message", "cbr_mean");
}

// ---- 802.11p: load ----

#[test]
fn dsrc_more_vehicles_load_the_channel() {
    dsrc("vehicles 40->120", "cbr_mean");
    dsrc("vehicles 40->120", "collision_rate");
    dsrc("vehicles 40->120", "prr");
    dsrc("vehicles 40->120", "access_ms");
    dsrc("vehicles 40->120", "e2e_ms_mean");
}

#[test]
fn dsrc_more_vehicles_queue_at_a_software_verifier() {
    dsrc("vehicles 40->80, software verify", "stage_verify_wait_ms");
    dsrc("vehicles 40->80, software verify", "e2e_ms_mean");
}

#[test]
fn dsrc_a_higher_message_rate_loads_the_channel() {
    dsrc("rate 10->20 Hz", "tx_rate_hz");
    dsrc("rate 10->20 Hz", "cbr_mean");
    dsrc("rate 10->20 Hz", "collision_rate");
    dsrc("rate 10->20 Hz", "ipg_ms_mean");
    dsrc("rate 10->20 Hz", "aoi_ms");
}

/// Regression: the mobility step is not a radio parameter. Before the first-step hold a
/// 50 ms step put every BSM in one half of the 100 ms cycle and doubled the
/// hidden-terminal losses.
#[test]
fn dsrc_the_mobility_step_does_not_move_the_channel() {
    dsrc("mobility step 100->50 ms", "collision_rate");
    dsrc("mobility step 100->50 ms", "tx_phase_peak");
}

// ---- 802.11p: EDCA and congestion control (dense base) ----

#[test]
fn dsrc_a_wider_contention_window_lengthens_access() {
    dense("CWmin 3->63", "backoff_us");
    dense("CWmin 3->63", "access_ms");
}

#[test]
fn dsrc_a_longer_aifs_lengthens_access() {
    dense("AIFSN 2->9", "aifs_us");
    dense("AIFSN 2->9", "access_ms");
}

#[test]
fn dsrc_a_one_frame_queue_does_not_bind_below_saturation() {
    dense("queue 64->1, saturated", "mac_drops");
}

#[test]
fn dsrc_reactive_dcc_caps_the_load() {
    dense("reactive DCC off->on", "cbr_mean");
    dense("reactive DCC off->on", "tx_rate_hz");
    dense("reactive DCC off->on", "aoi_ms");
}

#[test]
fn dsrc_j2945_rate_control_stretches_the_interval() {
    dense("J2945/1 B 25->5", "tx_rate_hz");
    dense("J2945/1 B 25->5", "cbr_mean");
    dense("J2945/1 B 25->5", "ipg_ms_mean");
}

#[test]
fn dsrc_adaptive_dcc_holds_the_target() {
    dense("ETSI adaptive target 0.68->0.15", "cbr_mean");
    dense("ETSI adaptive target 0.68->0.15", "tx_rate_hz");
}

// ---- GeoNetworking forwarding ----

#[test]
fn gn_a_higher_hop_limit_reaches_further_and_costs_forwards() {
    gn("GN hop limit 1->10", "gn_reach");
    gn("GN hop limit 1->10", "gn_forwards_sent");
    gn("GN hop limit 1->10", "gn_forwarded_bytes");
}

// ---- C-V2X sidelink ----

#[test]
fn cv2x_lower_tx_power_shrinks_the_range() {
    cv2x("LTE tx power 20->5 dBm", "range90_m");
    cv2x("LTE tx power 20->5 dBm", "prr_600_1000");
    cv2x("NR tx power 20->5 dBm", "range90_m");
}

#[test]
fn cv2x_higher_noise_figure_lowers_prr_at_range() {
    cv2x("LTE noise figure 9->15 dB (5 dBm)", "prr_600_1000");
    cv2x("NR noise figure 9->15 dB (5 dBm)", "prr_600_1000");
}

#[test]
fn cv2x_a_higher_mcs_lowers_prr_at_range() {
    cv2x("LTE MCS 7->11", "prr_600_1000");
}

#[test]
fn cv2x_mcs_and_payload_move_the_frequency_footprint_not_the_airtime() {
    cv2x("LTE MCS 5->7", "cbr_mean");
    cv2x("LTE MCS 7->11", "airtime_us");
    cv2x("LTE certificate on every message", "cbr_mean");
}

#[test]
fn cv2x_more_vehicles_load_the_pool() {
    cv2x("LTE vehicles 40->120", "cbr_mean");
    cv2x("LTE vehicles 40->120", "collision_rate");
    cv2x("LTE vehicles 40->120", "prr");
    cv2x("LTE vehicles 40->120", "e2e_ms_mean");
    cv2x("NR vehicles 40->120", "cbr_mean");
    cv2x("NR vehicles 40->120", "collision_rate");
}

#[test]
fn cv2x_a_higher_message_rate_loads_the_pool() {
    cv2x("LTE rate 10->20 Hz (RRI 50)", "cbr_mean");
}

#[test]
fn cv2x_blind_retransmissions_cost_load_and_buy_prr() {
    cv2x("LTE transmissions 2->1", "cbr_mean");
    cv2x("LTE transmissions 2->1", "prr");
}

#[test]
fn cv2x_sps_parameters_move_access_and_reselection() {
    cv2x("LTE RRI 100->50 ms", "access_ms");
    cv2x("LTE keep probability 0.8->0", "sl_selections");
    cv2x("NR keep probability 0->0.8", "sl_selections");
    cv2x("LTE RSRP threshold -> -60 dBm", "collision_rate");
}

#[test]
fn cv2x_sensing_avoids_collisions() {
    cv2x("LTE sensing off", "collision_rate");
    cv2x("LTE sensing off", "sl_overlapping");
}
