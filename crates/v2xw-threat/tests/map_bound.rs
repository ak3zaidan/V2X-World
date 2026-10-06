//! The map check with the stated confidence disc (`DetectorParams::offroad_confidence_bound`)
//! against the legacy check on the point.
//!
//! A sender driving east along the one road (`y = 0`) claims positions a fixed distance
//! to the side, with the stated 95 % confidence a BSM from a mid-range GNSS receiver
//! carries (4.4 m). One claim 18 m off the road is a GNSS outlier the confidence disc
//! nearly reaches; one 45 m off is a falsified position (the legacy `ConstPosOffset`
//! is 25 m on both axes, 35 m off a road aligned with either).

mod common;

use common::{OneRoad, one_message, rx_at_origin};
use v2xw_core::time::{NS_PER_S, SimTime};
use v2xw_threat::ctx::CollectingCtx;
use v2xw_threat::detect::{Detector, DetectorId, DetectorParams, Legacy12};
use v2xw_threat::obs::ObservedMessage;

/// The stated 95 % confidence, metres.
const CONF_M: f64 = 4.4;

/// Twelve claims a second apart at 11 m/s, `off_m` metres beside the road.
fn trace(off_m: f64) -> Vec<ObservedMessage> {
    (0..12u64)
        .map(|i| {
            let t: SimTime = i * NS_PER_S;
            let mut m = one_message(11.0 * i as f64, off_m, 11.0, t);
            m.claimed_heading_rad = 0.0;
            m.claimed_pos_confidence_m = CONF_M;
            m
        })
        .collect()
}

/// Peak `mapOffRoad` score and whether it fired.
fn off_road(bounded: bool, trace: &[ObservedMessage]) -> (f64, bool) {
    let mut det = Legacy12::new(DetectorParams {
        offroad_confidence_bound: bounded,
        ..DetectorParams::default()
    });
    let mut ctx = CollectingCtx::new(7);
    let mut peak = 0.0_f64;
    let mut fired = false;
    for m in trace {
        ctx.set_now(m.received_at);
        let v = det.on_message(&mut ctx, &rx_at_origin(m.received_at), m, &OneRoad);
        peak = peak.max(v.fingerprint.get(DetectorId::MapOffRoad));
        fired |= v.fired.iter().any(|o| o.detector == DetectorId::MapOffRoad);
    }
    (peak, fired)
}

#[test]
fn a_claim_whose_confidence_disc_reaches_the_road_is_not_off_it() {
    let t = trace(18.0);
    let (legacy_peak, legacy_fired) = off_road(false, &t);
    let (bounded_peak, bounded_fired) = off_road(true, &t);
    println!("18 m: legacy {legacy_peak:.2}, bounded {bounded_peak:.2}");
    assert!(legacy_fired, "the legacy check was expected to fire at 18 m / 15 m");
    assert!(
        !bounded_fired,
        "an 18 m claim stating 4.4 m confidence was scored off the road ({bounded_peak:.2})"
    );
    assert!((bounded_peak - (18.0 - CONF_M) / 15.0).abs() < 1e-9);
}

#[test]
fn a_falsified_position_well_off_the_road_is_caught_either_way() {
    let t = trace(45.0);
    let (_, legacy_fired) = off_road(false, &t);
    let (bounded_peak, bounded_fired) = off_road(true, &t);
    println!("45 m: bounded {bounded_peak:.2}");
    assert!(legacy_fired);
    assert!(bounded_fired, "a claim 45 m off the road was not caught");
}
