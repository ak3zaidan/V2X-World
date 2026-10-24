//! The per-device BSM timing model (`v2xw_node::bsm_timing`, Paper 1 item B) against the
//! SM-MBR RandomPosition variants (`v2xw_threat::us_mbr`, item C), with a perfect
//! receiver: every BSM of one device is heard, in order, and the vehicle's belief is its
//! true straight-line motion (no GNSS error), so the only thing that can fire the check is
//! the timing of the message.
//!
//! Each null is paired with the fault that must turn it red, in the same test: a check that
//! cannot fail proves nothing.

use std::collections::BTreeMap;

use v2xw_core::belief::{FixQuality, PositionEstimate};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_core::rng::{EntityRef, RngStream};
use v2xw_core::time::SimTime;
use v2xw_node::bsm_timing::{
    BsmExtras, BsmTimingCalibration, BsmTimingDevice, Jitter, PhaseDist, rng_domain,
};
use v2xw_threat::ctx::CollectingCtx;
use v2xw_threat::detect::{Detector, DetectorId};
use v2xw_threat::obs::{
    NoMap, ObservedKind, ObservedMessage, SelfBelief, StationType, VerificationState,
};
use v2xw_threat::us_mbr::{RandomPositionReading, TimeBase, UsMbrParams, UsSmMbr};

const MS: SimTime = 1_000_000;
/// One simulated minute of 10 Hz BSMs.
const MESSAGES: u64 = 600;

fn me() -> SelfBelief {
    SelfBelief {
        node: NodeId::new(0),
        believed_time: 0,
        x_m: 0.0,
        y_m: 0.0,
        radio_range_m: 500.0,
    }
}

fn params(reading: RandomPositionReading, time_base: TimeBase) -> UsMbrParams {
    UsMbrParams {
        reading,
        time_base,
        ..UsMbrParams::default()
    }
}

const COMBOS: [(RandomPositionReading, TimeBase); 4] = [
    (RandomPositionReading::V1_0, TimeBase::GenerationTime),
    (RandomPositionReading::V1_0, TimeBase::SecMark),
    (RandomPositionReading::Asn1V1_1, TimeBase::GenerationTime),
    (RandomPositionReading::Asn1V1_1, TimeBase::SecMark),
];

/// What one device's minute produced.
struct Minute {
    triggers: usize,
    slips: usize,
}

/// One device at `speed` for a minute, its BSMs `phase_ms` after each 100 ms fix epoch,
/// judged by the US profile with `p`.
fn minute(cal: &BsmTimingCalibration, device: u32, speed: f64, p: &UsMbrParams) -> Minute {
    let mut rng = RngStream::derive(11, rng_domain(), EntityRef::Node(NodeId::new(device)));
    let mut dev = BsmTimingDevice::assign(cal, &mut rng);
    let mut det = UsSmMbr::new(p.clone());
    let mut ctx = CollectingCtx::new(1);
    // The device's transmit phase against the fix grid: its own, steady.
    let phase_ns = SimTime::from(device % 97 + 1) * MS;
    let (mut triggers, mut slips) = (0, 0);
    for k in 0..MESSAGES {
        let t = 10_000 * MS + k * 100 * MS + phase_ns;
        let mut b = PositionEstimate::no_fix(t);
        b.fix = FixQuality::Rtk;
        b.pos = Vec3::new(speed * t as f64 * 1e-9, 0.0, 0.0);
        b.vel = Vec3::new(speed, 0.0, 0.0);
        let rep = dev.report(cal, &mut rng, &b).expect("a fix");
        slips += usize::from(rep.slip != 0);
        let generated = if rep.header_jitter_ns >= 0 {
            t + rep.header_jitter_ns.unsigned_abs()
        } else {
            t - rep.header_jitter_ns.unsigned_abs()
        };
        let m = ObservedMessage {
            bsm: BsmExtras {
                sec_mark_ms: Some(((rep.sec_mark_at / MS) % 60_000) as u16),
                accel_long_mps2: Some(0.0),
            },
            signer: [9; 8],
            kind: ObservedKind::Beacon,
            received_at: t + 2 * MS,
            claimed_generation_time: generated,
            claimed_x_m: rep.pos.x,
            claimed_y_m: rep.pos.y,
            claimed_speed_mps: speed,
            claimed_heading_rad: 0.0,
            claimed_pos_confidence_m: 1.5,
            repetitions: 1,
            cert_valid_from: 0,
            cert_valid_to: SimTime::MAX,
            station_type: StationType::Vehicle,
            verification: VerificationState::Valid,
        };
        let v = det.on_message(&mut ctx, &me(), &m, &NoMap);
        triggers += v
            .fired
            .iter()
            .filter(|o| o.detector == DetectorId::PositionJump)
            .count();
    }
    Minute { triggers, slips }
}

/// RandomPosition triggers per ID-minute over `n` devices.
fn rate(cal: &BsmTimingCalibration, n: u32, speed: f64, p: &UsMbrParams) -> f64 {
    let total: usize = (0..n).map(|d| minute(cal, d, speed, p).triggers).sum();
    total as f64 / f64::from(n)
}

/// A device locked to the fix epoch with nothing wrong with it: no phase spread, no slip,
/// no repeat, no header jitter.
fn perfectly_locked() -> BsmTimingCalibration {
    let mut cal = BsmTimingCalibration::provisional()
        .with_locked_share(1.0)
        .expect("valid");
    let c = &mut cal.classes[0];
    c.secmark_phase_ms = PhaseDist::Normal { mean: 0.0, sd: 0.0 };
    c.epoch_slip_prob = 0.0;
    c.frozen_repeat_prob = 0.0;
    c.header_jitter_ms = Jitter { sd: 0.0 };
    cal
}

#[test]
fn a_locked_device_with_a_perfect_receiver_never_fires_and_an_injected_slip_does() {
    let clean = perfectly_locked();
    let mut slipping = clean.clone();
    slipping.classes[0].epoch_slip_prob = 0.05;
    slipping.classes[0].slip_epochs = BTreeMap::from([(-1, 1.0)]);
    for (reading, base) in COMBOS {
        let p = params(reading, base);
        for speed in [5.0, 15.0, 25.0, 35.0] {
            for device in 0..4 {
                let m = minute(&clean, device, speed, &p);
                assert_eq!(
                    m.triggers, 0,
                    "{reading:?}/{base:?} at {speed} m/s: a perfectly locked device fired"
                );
            }
        }
        // The fault: the same device with injected epoch slips (position one fix older
        // than the timestamp says) must fire, about twice per slip (into and out of it).
        let m = minute(&slipping, 1, 20.0, &p);
        assert!(m.slips > 10, "the fault was injected ({} slips)", m.slips);
        assert!(
            m.triggers >= m.slips,
            "{reading:?}/{base:?}: {} slips gave only {} triggers",
            m.slips,
            m.triggers
        );
    }
}

#[test]
fn the_free_class_fires_at_a_rate_that_rises_with_speed() {
    let free = BsmTimingCalibration::provisional()
        .with_locked_share(0.0)
        .expect("valid");
    let locked = BsmTimingCalibration::provisional()
        .with_locked_share(1.0)
        .expect("valid");
    // On the BSM's own clock (secMark), as the real-data figures were computed.
    let sm = params(RandomPositionReading::V1_0, TimeBase::SecMark);
    let r: Vec<f64> = [5.0, 15.0, 30.0]
        .iter()
        .map(|v| rate(&free, 30, *v, &sm))
        .collect();
    println!("free, secMark, v1.0: {r:?} triggers per ID-minute at 5, 15, 30 m/s");
    assert!(r[0] < r[1] && r[0] < r[2], "{r:?}");
    // At speed the free class sits in the CV-pilot band (Tampa 20.3 ... Orem 195 per
    // ID-minute at v >= 10 m/s, check-realdata-and-vesnos.md §2.3).
    let at_speed = rate(&free, 30, 20.0, &sm);
    assert!((20.0..=195.0).contains(&at_speed), "{at_speed} per ID-minute");
    // The class contrast is the fault: the locked class at the same speed is near 0.
    let l = rate(&locked, 30, 20.0, &sm);
    println!("locked, secMark, v1.0: {l} per ID-minute at 20 m/s; free {at_speed}");
    assert!(l < 1.0 && l < at_speed / 10.0, "locked {l} vs free {at_speed}");
    // On the header clock the 27 ms generationTime jitter adds a residual that grows
    // with speed, so the rate rises strictly (check-realdata-and-vesnos.md §2.4).
    let gt = params(RandomPositionReading::V1_0, TimeBase::GenerationTime);
    let g: Vec<f64> = [5.0, 15.0, 30.0]
        .iter()
        .map(|v| rate(&free, 30, *v, &gt))
        .collect();
    println!("free, generationTime, v1.0: {g:?}");
    assert!(g[0] < g[1] && g[1] < g[2], "{g:?}");
}

#[test]
fn the_two_readings_and_the_two_clocks_disagree_where_the_texts_do() {
    let bsm = |t_ms: u64, sm: u16, x: f64, speed: f64, accel: f64| ObservedMessage {
        bsm: BsmExtras {
            sec_mark_ms: Some(sm),
            accel_long_mps2: Some(accel),
        },
        signer: [3; 8],
        kind: ObservedKind::Beacon,
        received_at: t_ms * MS,
        claimed_generation_time: t_ms * MS,
        claimed_x_m: x,
        claimed_y_m: 0.0,
        claimed_speed_mps: speed,
        claimed_heading_rad: 0.0,
        claimed_pos_confidence_m: 1.5,
        repetitions: 1,
        cert_valid_from: 0,
        cert_valid_to: SimTime::MAX,
        station_type: StationType::Vehicle,
        verification: VerificationState::Valid,
    };
    let fires = |p: UsMbrParams, a: &ObservedMessage, b: &ObservedMessage| {
        let mut d = UsSmMbr::new(p);
        let mut ctx = CollectingCtx::new(1);
        d.on_message(&mut ctx, &me(), a, &NoMap);
        d.on_message(&mut ctx, &me(), b, &NoMap)
            .fired
            .iter()
            .any(|o| o.detector == DetectorId::PositionJump)
    };
    // Acceleration: 20 m/s² over 0.1 s is 0.1 m the velocity-only text does not predict.
    let a = bsm(1_000, 1_000, 0.0, 10.0, 20.0);
    let b = bsm(1_100, 1_100, 1.0 + 0.1 + 0.98, 10.0, 20.0);
    let v10 = |base| params(RandomPositionReading::V1_0, base);
    let v11 = |base| params(RandomPositionReading::Asn1V1_1, base);
    assert!(fires(v10(TimeBase::GenerationTime), &a, &b));
    assert!(!fires(v11(TimeBase::GenerationTime), &a, &b));
    // Spacing: ASN.1 v1.1 judges only BSMs sent 0.1 s apart; v1.0 any pair within the
    // gap limit, which is itself a parameter.
    let c = bsm(1_300, 1_300, 3.0 + 5.0, 10.0, 0.0);
    let a0 = bsm(1_000, 1_000, 0.0, 10.0, 0.0);
    assert!(fires(v10(TimeBase::GenerationTime), &a0, &c));
    assert!(!fires(v11(TimeBase::GenerationTime), &a0, &c));
    let short_gap = UsMbrParams {
        max_gap_s: 0.2,
        ..v10(TimeBase::GenerationTime)
    };
    assert!(!fires(short_gap, &a0, &c));
    // Clock: the header says 160 ms, the secMark 100 ms, and the position moved 100 ms'
    // worth at 20 m/s: the header clock fires, the BSM's own does not. The secMark wraps
    // at the minute (59,950 -> 50).
    let h1 = bsm(60_000 - 50, 59_950, 0.0, 20.0, 0.0);
    let mut h2 = bsm(60_000 - 50 + 160, 50, 2.0, 20.0, 0.0);
    h2.received_at = h2.claimed_generation_time;
    assert!(fires(v10(TimeBase::GenerationTime), &h1, &h2));
    assert!(!fires(v10(TimeBase::SecMark), &h1, &h2));
    // The choice keys by name and by number.
    let mut p = UsMbrParams::default();
    assert!(p.apply_str("reading", "asn1_v1_1") && p.apply_str("time_base", "secmark"));
    assert_eq!(p, v11(TimeBase::SecMark));
    assert!(!p.apply_str("reading", "v2"));
    assert!(p.apply("reading", 0.0) && p.apply("time_base", 0.0));
    assert_eq!(p, UsMbrParams::default());
}
