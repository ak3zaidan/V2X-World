//! The heading check with the bearing error its own inputs imply
//! (`DetectorParams::heading_bearing_bound`), against the legacy one-step check.
//!
//! Every case is a crafted claim trace at the legacy one-hertz check rate, with the stated
//! 95 % position confidence a BSM from a mid-range GNSS receiver carries (4.4 m, a 1.8 m
//! one-sigma `semiMajor` scaled by 2.4477). The three cases are the three reasons the check
//! exists: it must catch a sender claiming the opposite heading at city speed, which the
//! legacy gate never looks at; it must not score one GNSS outlier as if the bearing were
//! exact, which is how the legacy check reported honest vehicles; and it must not read a
//! turn as a lie.

mod common;

use common::{one_message, rx_at_origin};
use v2xw_core::time::{NS_PER_S, SimTime};
use v2xw_threat::ctx::CollectingCtx;
use v2xw_threat::detect::{Detector, DetectorId, DetectorParams, Legacy12};
use v2xw_threat::obs::{NoMap, ObservedMessage};

/// The stated 95 % confidence, metres.
const CONF_M: f64 = 4.4;

/// One claim: position, speed and heading at second `i`.
fn claim(i: u64, x: f64, y: f64, speed: f64, heading_rad: f64) -> ObservedMessage {
    let t: SimTime = i * NS_PER_S;
    let mut m = one_message(x, y, speed, t);
    m.claimed_heading_rad = heading_rad;
    m.claimed_pos_confidence_m = CONF_M;
    m
}

/// Runs a trace through the suite; returns the peak heading score and whether it fired.
fn heading(bounded: bool, trace: &[ObservedMessage]) -> (f64, bool) {
    heading_with(
        DetectorParams {
            heading_bearing_bound: bounded,
            ..DetectorParams::default()
        },
        trace,
    )
}

/// [`heading`] at any operating point.
fn heading_with(params: DetectorParams, trace: &[ObservedMessage]) -> (f64, bool) {
    let mut det = Legacy12::new(params);
    let mut ctx = CollectingCtx::new(7);
    let mut peak = 0.0_f64;
    let mut fired = false;
    for m in trace {
        ctx.set_now(m.received_at);
        let v = det.on_message(&mut ctx, &rx_at_origin(m.received_at), m, &NoMap);
        peak = peak.max(v.fingerprint.get(DetectorId::HeadingInconsistency));
        fired |= v
            .fired
            .iter()
            .any(|o| o.detector == DetectorId::HeadingInconsistency);
    }
    (peak, fired)
}

#[test]
fn a_reversed_heading_at_city_speed_is_caught_by_the_bounded_check_and_missed_by_the_legacy_one()
{
    // 10.5 m/s (38 km/h) straight along x, claiming the opposite heading throughout.
    let trace: Vec<ObservedMessage> = (0..12)
        .map(|i| claim(i, 10.5 * i as f64, 0.0, 10.5, core::f64::consts::PI))
        .collect();
    let (legacy_peak, legacy_fired) = heading(false, &trace);
    let (bounded_peak, bounded_fired) = heading(true, &trace);
    println!("reversed: legacy peak {legacy_peak:.2}, bounded peak {bounded_peak:.2}");
    assert!(
        !legacy_fired,
        "the legacy gate (a 1 s step over 2.5 x 4.4 m) was expected never to look"
    );
    assert!(bounded_fired, "the bounded check missed a reversed heading");
}

#[test]
fn one_gnss_outlier_is_not_scored_as_if_the_bearing_were_exact() {
    // An honest vehicle at 11 m/s, heading east, whose fix at second 6 lands 14 m off to
    // the side — three times its stated 95 % radius — and comes back.
    let trace: Vec<ObservedMessage> = (0..12)
        .map(|i| {
            let y = if i == 6 { 14.0 } else { 0.0 };
            claim(i, 11.0 * i as f64, y, 11.0, 0.0)
        })
        .collect();
    let (legacy_peak, legacy_fired) = heading(false, &trace);
    let (bounded_peak, bounded_fired) = heading(true, &trace);
    println!("outlier: legacy peak {legacy_peak:.2}, bounded peak {bounded_peak:.2}");
    assert!(
        legacy_fired,
        "the legacy check was expected to report the outlier (into it and out of it)"
    );
    assert!(
        !bounded_fired,
        "the bounded check reported an honest vehicle for one outlier (peak {bounded_peak:.2})"
    );
}

#[test]
fn a_turn_is_not_read_as_a_heading_lie() {
    // A 90-degree right-hand turn at 8 m/s over three seconds, then straight on; the
    // claims are exact positions along the arc and the true heading at each. The
    // straight-baseline rule (`heading_straight_tol_deg`) is what keeps the chord from
    // spanning the turn: with it switched off (360) this trace scores 1.06.
    let r = 8.0 * 3.0 / core::f64::consts::FRAC_PI_2;
    let mut trace = Vec::new();
    for i in 0..4u64 {
        trace.push(claim(i, 8.0 * i as f64, 0.0, 8.0, 0.0));
    }
    let (x0, y0) = (24.0, -r);
    for k in 1..=3u64 {
        let a = core::f64::consts::FRAC_PI_2 * k as f64 / 3.0;
        trace.push(claim(
            3 + k,
            x0 + r * a.sin(),
            y0 + r * a.cos(),
            8.0,
            -a,
        ));
    }
    let (xe, ye) = (x0 + r, y0);
    for k in 1..=5u64 {
        trace.push(claim(
            6 + k,
            xe,
            ye - 8.0 * k as f64,
            8.0,
            -core::f64::consts::FRAC_PI_2,
        ));
    }
    let (bounded_peak, bounded_fired) = heading(true, &trace);
    println!("turn: bounded peak {bounded_peak:.2}");
    assert!(!bounded_fired, "a turn fired the bounded heading check");
    assert!(bounded_peak < 1.0, "a turn scored {bounded_peak:.2}");
}

#[test]
fn a_45_degree_heading_offset_is_caught_over_a_long_straight_baseline() {
    // The legacy HeadingOffset attacker: the true heading plus 45 degrees, on a straight
    // road at 11 m/s. Over the motion checks' 3.5 s of history the bearing's own error
    // (two 5 m radii over about 33 m) was 17.6 degrees, which put the threshold at 52.6
    // and let the lie through; over a 10 s baseline it is 5.2 degrees.
    let trace: Vec<ObservedMessage> = (0..14)
        .map(|i| claim(i, 11.0 * i as f64, 0.0, 11.0, 45f64.to_radians()))
        .collect();
    let (bounded_peak, bounded_fired) = heading(true, &trace);
    let (short_peak, short_fired) = heading_with(
        DetectorParams {
            heading_bearing_bound: true,
            heading_baseline_max_s: 3.5,
            ..DetectorParams::default()
        },
        &trace,
    );
    println!("45-degree offset: bounded peak {bounded_peak:.2}, over 3.5 s {short_peak:.2}");
    assert!(bounded_fired, "a 45-degree heading lie went unreported");
    assert!(
        !short_fired,
        "over the motion checks' 3.5 s history the bearing error was expected to hide it"
    );
}
