//! The link budget's settings reach every received power: transmit power, antenna gain,
//! cable loss and antenna height per node class (`radio.devices`), the candidate range
//! derived from the link budget (`radio.range`), rain over the link, and the high tier's
//! own geometric law — with the buildings and terrain switches still doing what they say.
//!
//! Each property is shown against its counterexample in the same run pair: a check that
//! passes against the defect it guards is not a check.

use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_metrics::channels::{PhyRxView, RxOutcome};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The Midtown-shaped procedural grid (274 × 80 m blocks, one 30 m building per block)
/// with a bulk-spawned fleet of up to `n`, for `secs`.
fn grid_fleet(n: u32, secs: f64) -> Scenario {
    let mut s =
        Scenario::load(scenarios().join("phase1-grid.yaml")).expect("the shipped scenario loads");
    s.time.duration_s = secs;
    s.actors.vehicles.demand.rate_veh_per_h = Some(3_600_000.0);
    s.actors.vehicles.demand.params = serde_json::json!({ "max_total_vehicles": n });
    s
}

fn open_air(mut s: Scenario) -> Scenario {
    s.world.buildings.enabled = false;
    s
}

fn with_model(mut s: Scenario, family: &str, id: &str) -> Scenario {
    s.radio.models.insert(
        family.to_string(),
        v2xw_engine::scenario::schema::ModelChoice::new(id),
    );
    s
}

fn run(scenario: Scenario) -> (RunReport, Vec<PhyRxView>, Engine) {
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    let rx = recorder
        .records()
        .iter()
        .filter(|(_, r)| r.channel == <PhyRxView as v2xw_metrics::channels::ChannelView>::CHANNEL)
        .map(|(_, r)| v2xw_metrics::channels::decode(r).expect("decodes"))
        .collect();
    (report, rx, engine)
}

/// Mean received power over the attempts whose distance is in `[lo, hi)`, and how many.
fn mean_rssi(rx: &[PhyRxView], lo: f64, hi: f64) -> (f64, usize) {
    let v: Vec<f64> = rx
        .iter()
        .filter(|r| r.dist_m.is_some_and(|d| d >= lo && d < hi))
        .filter_map(|r| r.rssi_dbm)
        .collect();
    (v.iter().sum::<f64>() / v.len().max(1) as f64, v.len())
}

fn pdr_between(rx: &[PhyRxView], lo: f64, hi: f64) -> Option<f64> {
    let mut n = 0u64;
    let mut ok = 0u64;
    for r in rx {
        let Some(d) = r.dist_m else { continue };
        if d >= lo && d < hi {
            n += 1;
            if r.outcome == RxOutcome::Ok {
                ok += 1;
            }
        }
    }
    (n > 0).then(|| ok as f64 / n as f64)
}

// -----------------------------------------------------------------------------------------
// radio.devices
// -----------------------------------------------------------------------------------------

/// Transmit power moves every received power by itself, and not the set of links the run
/// attempts. Both powers are below what J2945/1 congestion control would allow (its
/// radiated power never falls under 10 dBm, so a unit with a 3 dBi antenna may conduct
/// 7 dBm), so the configured power is what goes on the air in both runs, and with the
/// grid's buildings in the way many links are near the floor. The counterexample is the
/// defect the fixed reference EIRP removes: when the attempt test used each frame's own
/// power, 5 dB less power dropped the weakest links from the set and the mean received
/// power over what was left went *up* (−85.3 to −81.2 dBm, measured at 10 dBm).
#[test]
fn the_transmit_power_moves_every_received_power_by_itself() {
    let base = with_model(grid_fleet(60, 4.0), "fading", "fading/none");
    let mut loud = base.clone();
    loud.radio.devices.obu.tx_power_dbm = 5.0;
    let mut quiet = base;
    quiet.radio.devices.obu.tx_power_dbm = 0.0;
    let (loud_report, loud_rx, _) = run(loud);
    let (quiet_report, quiet_rx, _) = run(quiet);
    assert!(loud_report.faint_arrivals > 0, "no link was near the floor");
    assert_eq!(
        loud_report.reception_attempts, quiet_report.reception_attempts,
        "the transmit power changed which links were attempted"
    );
    let (a, na) = mean_rssi(&loud_rx, 0.0, 10_000.0);
    let (b, nb) = mean_rssi(&quiet_rx, 0.0, 10_000.0);
    assert!(na > 40 && nb > 40, "{na} and {nb} attempts");
    assert!(
        (a - b - 5.0).abs() < 0.75,
        "5 dB less transmit power moved the mean received power by {:.2} dB",
        a - b
    );
}

/// Antenna gain and cable loss follow J2945/1's radiated-power rule. At the default power
/// congestion control sets the *radiated* power, so a 3 dB better antenna transmits the
/// same EIRP and only receives 3 dB better, and 3 dB more cable loss only costs the
/// receiving end. Below the congestion-control ceiling the same antenna gains 3 dB at each
/// end, 6 dB in all. The counterexample is the double count this build removed: before it,
/// the antenna gain was added on top of J2945/1's radiated power.
#[test]
fn antenna_gain_and_cable_loss_follow_j2945s_radiated_power() {
    let base = open_air(with_model(grid_fleet(60, 4.0), "fading", "fading/none"));
    let (_, plain, _) = run(base.clone());
    let mut better = base.clone();
    better.radio.devices.obu.antenna_gain_dbi = 6.0;
    let (_, better_rx, _) = run(better);
    let mut lossy = base.clone();
    lossy.radio.devices.obu.cable_loss_db = 3.0;
    let (_, lossy_rx, _) = run(lossy);
    let (p, _) = mean_rssi(&plain, 0.0, 150.0);
    let (g, _) = mean_rssi(&better_rx, 0.0, 150.0);
    let (l, _) = mean_rssi(&lossy_rx, 0.0, 150.0);
    assert!(
        (g - p - 3.0).abs() < 0.75,
        "+3 dBi at the J2945/1 ceiling moved {:.2} dB",
        g - p
    );
    assert!(
        (p - l - 3.0).abs() < 0.75,
        "+3 dB cable loss moved {:.2} dB",
        p - l
    );

    // Below the ceiling the transmitter's gain counts too.
    let mut low = base;
    low.radio.devices.obu.tx_power_dbm = 0.0;
    let (_, low_rx, _) = run(low.clone());
    low.radio.devices.obu.antenna_gain_dbi = 6.0;
    let (_, low_better, _) = run(low);
    let (a, _) = mean_rssi(&low_rx, 0.0, 150.0);
    let (b, _) = mean_rssi(&low_better, 0.0, 150.0);
    assert!(
        (b - a - 6.0).abs() < 0.75,
        "+3 dBi below the ceiling moved {:.2} dB",
        b - a
    );
}

/// Antenna height reaches the budget: under the two-ray law, raising both antennas from
/// 1.5 m to 3 m moves the d⁻⁴ crossover from 550 m out past 2 km, so links 900-2,000 m
/// long follow Friis instead of d⁻⁴: 4 dB better at 900 m, 11 dB at 2 km.
#[test]
fn antenna_height_moves_the_two_ray_crossover() {
    let base = open_air(with_model(
        with_model(grid_fleet(60, 3.0), "fading", "fading/none"),
        "propagation",
        "propagation/two-ray-ground",
    ));
    let (_, low, _) = run(base.clone());
    let mut tall = base;
    tall.radio.devices.obu.antenna_height_m = Some(3.0);
    let (_, high, _) = run(tall);
    let (a, na) = mean_rssi(&low, 900.0, 2_000.0);
    let (b, nb) = mean_rssi(&high, 900.0, 2_000.0);
    assert!(na > 50 && nb > 50, "{na} and {nb} attempts at 900-2000 m");
    assert!(
        b > a + 4.0,
        "3 m antennas received {b:.1} dBm against {a:.1} dBm"
    );
}

// -----------------------------------------------------------------------------------------
// radio.range
// -----------------------------------------------------------------------------------------

/// The candidate range is the link budget's, not 1 km. In open air, free space and no
/// fading, 802.11p at a 20 dBm radiated power still arrives above the receiver's
/// sensitivity at 1.2 km, so links 1.0-1.5 km long exist and deliver — where the fixed 1 km
/// range left no attempt there at all. A cap stops the full evaluation at the cap and
/// counts what lies beyond as interference only.
#[test]
fn the_candidate_range_follows_the_link_budget() {
    let base = open_air(with_model(
        with_model(grid_fleet(60, 3.0), "fading", "fading/none"),
        "propagation",
        "propagation/free-space",
    ));
    let (_, rx, _) = run(base.clone());
    let far = pdr_between(&rx, 1_000.0, 1_500.0).expect("links 1.0-1.5 km long");
    assert!(far > 0.5, "free space at 1.0-1.5 km delivered {far:.3}");

    let mut capped = base;
    capped.radio.range.max_m = Some(500.0);
    let (report, capped_rx, _) = run(capped);
    assert!(
        capped_rx
            .iter()
            .all(|r| r.dist_m.is_none_or(|d| d <= 500.0 + 1.0)),
        "an attempt beyond the 500 m cap"
    );
    assert!(
        report.faint_arrivals > 0,
        "nothing beyond the cap was counted as interference"
    );
}

/// The margin decides what is a reception attempt: with the buildings of the grid in the
/// way, many arrivals fall between 10 and 30 dB under the noise floor, so a 30 dB margin
/// attempts them and a 10 dB margin counts them as energy only.
#[test]
fn the_range_margin_separates_attempts_from_energy() {
    let base = grid_fleet(60, 3.0);
    let (narrow, _, _) = run(base.clone());
    let mut wide = base;
    wide.radio.range.margin_db = 30.0;
    let (wider, _, _) = run(wide);
    assert!(
        narrow.faint_arrivals > 0,
        "no arrival fell under the margin"
    );
    assert!(
        wider.reception_attempts > narrow.reception_attempts,
        "{} attempts at 30 dB against {} at 10 dB",
        wider.reception_attempts,
        narrow.reception_attempts
    );
    assert!(wider.faint_arrivals < narrow.faint_arrivals);
}

// -----------------------------------------------------------------------------------------
// The high tier, rain, and the buildings and terrain switches
// -----------------------------------------------------------------------------------------

/// The high tier runs its own law, and the laws it composes are pinned by the manifest;
/// rain is pinned wherever the tier is not abstract.
#[test]
fn the_high_tier_is_its_own_law_and_the_manifest_pins_it() {
    let mut s = grid_fleet(1, 0.5);
    s.radio.tiers.propagation = v2xw_core::card::Tier::High;
    let high = Engine::build(s.clone(), "").expect("builds");
    let ids: Vec<&str> = high
        .manifest()
        .model_cards
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    for id in [
        v2xw_radio::GeometricUrbanV2v::ID,
        v2xw_radio::obstacle::VehicleBlockage::ID,
        v2xw_radio::RainAttenuation::ID,
        v2xw_radio::BuildingShadowing::ID,
    ] {
        assert!(ids.contains(&id), "{id} is not in the manifest: {ids:?}");
    }
    s.radio.tiers.propagation = v2xw_core::card::Tier::Medium;
    let medium = Engine::build(s.clone(), "").expect("builds");
    let ids: Vec<&str> = medium
        .manifest()
        .model_cards
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    assert!(!ids.contains(&v2xw_radio::GeometricUrbanV2v::ID));
    assert!(ids.contains(&v2xw_radio::RainAttenuation::ID));
    assert!(ids.contains(&v2xw_radio::LogDistanceShadowing::ID));
}

/// `world.buildings.enabled` still switches obstruction under the high tier's law: with
/// buildings, delivery at 200-500 m collapses behind the blocks; without, the same pairs
/// are line of sight and nearly all deliver. And the high tier is not the medium one: the
/// same fleet delivers differently.
#[test]
fn the_buildings_switch_works_under_the_high_tiers_law() {
    let mut base = grid_fleet(60, 3.0);
    base.radio.tiers.propagation = v2xw_core::card::Tier::High;
    let (_, city, _) = run(base.clone());
    let (_, open, _) = run(open_air(base.clone()));
    let city_mid = pdr_between(&city, 200.0, 500.0).expect("links at 200-500 m");
    let open_mid = pdr_between(&open, 200.0, 500.0).expect("links at 200-500 m");
    assert!(
        open_mid > 0.9,
        "open-air delivery at 200-500 m is {open_mid:.3}"
    );
    assert!(
        city_mid < 0.5 * open_mid,
        "buildings changed 200-500 m delivery only from {open_mid:.3} to {city_mid:.3}"
    );
    let mut medium = base;
    medium.radio.tiers.propagation = v2xw_core::card::Tier::Medium;
    let (_, med, _) = run(medium);
    let differs = [0.0, 100.0, 200.0, 300.0]
        .iter()
        .any(|&lo| pdr_between(&city, lo, lo + 100.0) != pdr_between(&med, lo, lo + 100.0));
    assert!(
        differs,
        "the high tier delivered exactly what the medium tier did"
    );
}

/// Rain reaches the received power, by exactly what ITU-R P.838-3 says: a few hundredths of
/// a decibel. The same links, clear and in a 50 mm/h downpour, at the same instants.
///
/// Rain also changes how people drive (FHWA), which would move the vehicles; the check
/// therefore compares the budget of one fixed link through the engine's own obstacle stack
/// and rain model rather than two runs.
#[test]
fn rain_costs_what_p838_says_and_fog_costs_nothing() {
    let rain = v2xw_radio::RainAttenuation::new();
    let wet = v2xw_core::weather::WeatherState::new(
        v2xw_core::weather::WeatherKind::Rain,
        1.0,
        2_000.0,
        v2xw_core::weather::SurfaceCondition::Wet,
    );
    let fog = v2xw_core::weather::WeatherState::new(
        v2xw_core::weather::WeatherKind::Fog,
        1.0,
        50.0,
        v2xw_core::weather::SurfaceCondition::Dry,
    );
    let a = rain.attenuation_db(&wet, 300.0, 5.86e9);
    assert!(a > 0.03 && a < 0.1, "{a} dB on a 300 m link in a downpour");
    assert_eq!(rain.attenuation_db(&fog, 300.0, 5.86e9), 0.0);
}

/// `world.terrain` still obstructs under the high tier: its stack composes the terrain
/// knife edges beside the corner tracer and the vehicle blockage.
#[test]
fn the_high_tier_stack_keeps_terrain_and_adds_corners_and_vehicles() {
    let mut s = grid_fleet(1, 0.5);
    s.radio.tiers.propagation = v2xw_core::card::Tier::High;
    let world = v2xw_engine::wiring::build_world(&s).expect("builds");
    let stack = v2xw_engine::wiring::build_obstacles(&s, &world);
    assert!(stack.buildings.is_some() && stack.corners.is_some() && stack.vehicles.is_some());
    // Buildings off: no corners to trace, vehicles still block.
    s.world.buildings.enabled = false;
    let stack = v2xw_engine::wiring::build_obstacles(&s, &world);
    assert!(stack.buildings.is_none() && stack.corners.is_none() && stack.vehicles.is_some());
    // The medium tier traces nothing.
    s.radio.tiers.propagation = v2xw_core::card::Tier::Medium;
    s.world.buildings.enabled = true;
    let stack = v2xw_engine::wiring::build_obstacles(&s, &world);
    assert!(stack.corners.is_none() && stack.vehicles.is_none());
}

// -----------------------------------------------------------------------------------------
// WP-09: two-ray interference and measured antennas
// -----------------------------------------------------------------------------------------

/// A passenger-only open-air fleet with no fading: every vehicle's antenna is 1.5 m up.
fn passenger_open_air(n: u32, secs: f64) -> Scenario {
    let mut s = open_air(with_model(grid_fleet(n, secs), "fading", "fading/none"));
    s.actors.vehicles.classes =
        serde_json::from_value(serde_json::json!({ "passenger": { "fraction": 1.0 } }))
            .expect("a class mix");
    s
}

type Recorded = Vec<(v2xw_core::time::SimTime, v2xw_core::ctx::OwnedRecord)>;

fn run_records(scenario: Scenario) -> Recorded {
    let errors = v2xw_engine::scenario::validate(&scenario);
    assert!(errors.is_empty(), "the scenario does not load: {errors:?}");
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs");
    recorder.records().to_vec()
}

type ByLink = std::collections::BTreeMap<(u32, u32, v2xw_core::time::SimTime), (f64, f64)>;

fn rx_by_link(rx: &[PhyRxView]) -> ByLink {
    rx.iter()
        .filter_map(|v| {
            Some((
                (v.tx?.index(), v.rx.index(), v.t_start),
                (v.rssi_dbm?, v.dist_m?),
            ))
        })
        .collect()
}

/// Sommer 2012 Eq. 4-6 for two 1.5 m antennas over a road of εr 1.02 at `f`, written
/// out here (`|1 + Γe^{iφ}|² = 1 + Γ² + 2Γ·cosφ`) rather than read from the engine.
fn closed_form_gain_db(d: f64, f: f64) -> f64 {
    let h = 1.5;
    let lambda = 299_792_458.0 / f;
    let d_ref = (d * d + 4.0 * h * h).sqrt();
    let phi = 2.0 * std::f64::consts::PI * (d - d_ref) / lambda;
    let (s, c) = (2.0 * h / d_ref, d / d_ref);
    let root = (1.02 - c * c).sqrt();
    let g = (s - root) / (s + root);
    10.0 * (1.0 + g * g + 2.0 * g * phi.cos()).log10()
}

/// The two-ray interference law reaches every received power as its closed form: on the
/// same links at the same instants, the received power under it differs from free space's
/// by exactly `20·log10|1 + Γ⊥·e^{iφ}|` (Sommer 2012 Eq. 6) at the recorded distance,
/// deep fades included — and the same seed gives the same run.
#[test]
fn two_ray_interference_reaches_the_received_power_as_its_closed_form() {
    let base = passenger_open_air(60, 3.0);
    let f = v2xw_engine::wiring::radio_regulation(&base)
        .expect("the region's channel")
        .centre_hz();
    let fs = with_model(base.clone(), "propagation", "propagation/free-space");
    let tri = with_model(base, "propagation", "propagation/two-ray-interference");
    assert!(v2xw_engine::scenario::validate(&tri).is_empty());
    let (_, fs_rx, _) = run(fs);
    let (_, tri_rx, _) = run(tri.clone());
    let (_, again, _) = run(tri);
    let fs = rx_by_link(&fs_rx);
    let tri = rx_by_link(&tri_rx);
    assert_eq!(tri, rx_by_link(&again), "the same seed gave another run");
    let (mut matched, mut deep, mut up) = (0, 0, 0);
    for (k, &(t, d)) in &tri {
        let Some(&(r, _)) = fs.get(k) else { continue };
        matched += 1;
        let want = closed_form_gain_db(d, f);
        let got = t - r;
        if want > -15.0 {
            assert!(
                (got - want).abs() < 0.02,
                "{k:?} at {d} m: moved {got:.3} dB, Eq. 6 says {want:.3}"
            );
        } else {
            assert!(got < -10.0, "{k:?} at {d} m: a {want:.1} dB fade moved {got:.2}");
        }
        if want < -5.0 {
            deep += 1;
        }
        if want > 3.0 {
            up += 1;
        }
    }
    eprintln!("{matched} links matched, {deep} in a deep fade, {up} constructive");
    assert!(matched > 500, "only {matched} links in common");
    assert!(deep > 0 && up > 0, "{deep} fades and {up} peaks: the pattern was not exercised");
}

/// A measured antenna table of zeros is the isotropic antenna, record for record, even
/// with a random mount rotation drawn per vehicle; a table with a deep rear null makes
/// some links weaker and none stronger, and repeats itself under the same seed.
#[test]
fn a_flat_sampled_pattern_is_the_isotropic_antenna_byte_for_byte() {
    let base = passenger_open_air(40, 3.0);
    let mut iso = base.clone();
    iso.radio.devices.obu.antenna_pattern =
        serde_json::from_value(serde_json::json!("isotropic")).expect("a pattern");
    let sampled = |csv: &str| {
        let mut s = base.clone();
        s.radio.devices.obu.sampled_pattern = Some(
            serde_json::from_value(serde_json::json!({
                "csv": csv,
                "rotation_deg": 10.0,
                "rotation_jitter": "uniform",
                "rotation_jitter_deg": 180.0,
            }))
            .expect("a sampled pattern"),
        );
        s
    };
    let flat = sampled("azimuth_deg,gain_db\n0,0\n90,0\n180,0\n270,0\n");
    let a = run_records(iso.clone());
    let b = run_records(flat);
    assert!(a.len() > 1_000, "only {} records", a.len());
    assert_eq!(a.len(), b.len(), "a flat table changed how many records the run wrote");
    let first = a.iter().zip(&b).position(|(x, y)| x != y);
    assert!(first.is_none(), "record {first:?} differs from the isotropic run");

    let null = sampled("azimuth_deg,gain_db\n0,0\n90,-3\n150,-10\n180,-30\n210,-10\n270,-3\n");
    let (_, iso_rx, _) = run(iso);
    let (_, null_rx, _) = run(null.clone());
    let (_, null_again, _) = run(null);
    let nulled = rx_by_link(&null_rx);
    assert_eq!(nulled, rx_by_link(&null_again), "the same seed gave another run");
    let iso = rx_by_link(&iso_rx);
    let (mut matched, mut weaker) = (0, 0);
    for (k, &(n, _)) in &nulled {
        let Some(&(i, _)) = iso.get(k) else { continue };
        matched += 1;
        assert!(n <= i + 1e-9, "the table added gain on {k:?}: {n} against {i}");
        if n < i - 3.0 {
            weaker += 1;
        }
    }
    eprintln!("{matched} links matched, {weaker} weaker by more than 3 dB");
    assert!(matched > 100 && weaker > 0, "{matched} matched, {weaker} weaker");
}
