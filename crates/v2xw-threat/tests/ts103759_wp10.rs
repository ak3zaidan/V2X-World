//! WP-10 (docs/design/frameworks/00-adoption-plan.md): the Kalman, oriented-box and
//! reachable-envelope checks of the TS 103 759 suite, and its three fusion modes.
//!
//! Every assertion here was shown to go red under an injected fault; the adoption plan's
//! WP-10 status note lists each fault and the test that caught it.

mod common;

use std::collections::BTreeMap;

use common::sim::{self, SimOptions};
use v2xw_core::ids::NodeId;
use v2xw_core::time::{NS_PER_S, SimTime, secs_to_ns};
use v2xw_mobility::VehicleClass;
use v2xw_threat::consistency::ReferencePoint;
use v2xw_threat::ctx::CollectingCtx;
use v2xw_threat::obs::{
    FrameExtras, NoMap, ObservedKind, ObservedMessage, OwnFootprint, SelfBelief, StationType,
    VerificationState,
};
use v2xw_threat::ts103759::{
    BeaconProfile, CrossCheckInputs, Fusion, Scoring, Ts103759Check, Ts103759Params, Ts103759Suite,
    TsVerdict,
};

const RX: NodeId = NodeId::new(1);
const A: [u8; 8] = [0xAA; 8];
const B: [u8; 8] = [0xBB; 8];
const C: [u8; 8] = [0xCC; 8];
const G: [u8; 8] = [0x66; 8];

/// The engine's passenger car (`v2xw_mobility::classes`).
const CAR_L: f64 = 5.0;
const CAR_W: f64 = 1.8;

fn me_at(t: SimTime, x: f64, y: f64) -> SelfBelief {
    SelfBelief {
        node: RX,
        believed_time: t,
        x_m: x,
        y_m: y,
        radio_range_m: 500.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn msg(signer: [u8; 8], t: SimTime, x: f64, y: f64, v: f64, h: f64, conf: f64) -> ObservedMessage {
    ObservedMessage {
        bsm: Default::default(),
        signer,
        kind: ObservedKind::Beacon,
        received_at: t,
        claimed_generation_time: t,
        claimed_x_m: x,
        claimed_y_m: y,
        claimed_speed_mps: v,
        claimed_heading_rad: h,
        claimed_pos_confidence_m: conf,
        repetitions: 1,
        cert_valid_from: 0,
        cert_valid_to: u64::MAX,
        station_type: StationType::Vehicle,
        verification: VerificationState::Valid,
    }
}

/// A deterministic standard normal (SplitMix64 + Box–Muller).
struct Noise(u64);

impl Noise {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        let (u, w) = (self.unit(), self.unit());
        (-2.0 * u.ln()).sqrt() * (core::f64::consts::TAU * w).cos()
    }
}

/// Runs messages through one suite with the given inputs and returns every verdict.
fn run(
    suite: &mut Ts103759Suite,
    msgs: &[ObservedMessage],
    frame: Option<&FrameExtras>,
) -> Vec<TsVerdict> {
    let mut ctx = CollectingCtx::new(3);
    msgs.iter()
        .map(|m| {
            ctx.set_now(m.received_at);
            let mut x = CrossCheckInputs::none();
            x.frame = frame;
            suite.check(&mut ctx, &me_at(m.received_at, 0.0, 0.0), m, &NoMap, &x)
        })
        .collect()
}

fn fired(vs: &[TsVerdict], check: Ts103759Check) -> usize {
    vs.iter()
        .filter(|v| v.fired.iter().any(|o| o.check == check))
        .count()
}

/// The pairwise class-2 checks (everything but the Kalman filter).
const PAIRWISE: [Ts103759Check; 6] = [
    Ts103759Check::PositionConsistency,
    Ts103759Check::PositionSpeedConsistency,
    Ts103759Check::SpeedConsistency,
    Ts103759Check::PositionHeadingConsistency,
    Ts103759Check::PositionSpeedEnvelope,
    Ts103759Check::PositionSpeedMaxConsistency,
];

/// A 10 Hz BSM track at 16 m/s heading east for `secs`, the claimed position carrying
/// white GNSS noise of σ = 1.6 m per axis (the engine's Gauss–Markov white term) and a
/// lateral drift of `drift_mps` from `drift_from_s`; the claimed speed and heading stay
/// honest.
fn track(secs: f64, drift_mps: f64, drift_from_s: f64, seed: u64) -> Vec<ObservedMessage> {
    let mut n = Noise(seed);
    let steps = (secs * 10.0) as u64;
    (0..steps)
        .map(|i| {
            let t_s = i as f64 * 0.1;
            let drift = drift_mps * (t_s - drift_from_s).max(0.0);
            msg(
                A,
                secs_to_ns(t_s),
                16.0 * t_s + 1.6 * n.normal(),
                drift + 1.6 * n.normal(),
                16.0 + 0.1 * n.normal(),
                0.0,
                4.0,
            )
        })
        .collect()
}

#[test]
fn a_slow_drift_the_pairwise_checks_miss_is_caught_by_the_kalman_check() {
    // Honest, the same noise: the Kalman check never fires.
    let mut s = Ts103759Suite::cited_defaults();
    let honest = run(&mut s, &track(60.0, 0.0, 0.0, 11), None);
    assert_eq!(fired(&honest, Ts103759Check::KalmanConsistency), 0);
    assert!(
        honest
            .iter()
            .filter_map(|v| v.score(Ts103759Check::KalmanConsistency))
            .count()
            > 500,
        "the Kalman check did not run"
    );

    // A 4 m/s lateral drift from t = 10 s: 0.4 m per message, far inside every pairwise
    // gate at 10 Hz, but the claimed track no longer integrates the claimed velocity.
    let mut s = Ts103759Suite::cited_defaults();
    let drift = run(&mut s, &track(60.0, 4.0, 10.0, 11), None);
    for c in PAIRWISE {
        assert_eq!(fired(&drift, c), 0, "{c} fired on the drift");
    }
    let first = drift
        .iter()
        .position(|v| {
            v.fired
                .iter()
                .any(|o| o.check == Ts103759Check::KalmanConsistency)
        })
        .expect("the Kalman check missed a 4 m/s drift");
    // Caught within 10 s of the drift starting (message 100 is t = 10 s).
    assert!(first < 200, "caught only at message {first}");
    // And it keeps firing: most messages after the first catch are flagged.
    let after = fired(&drift[first..], Ts103759Check::KalmanConsistency);
    assert!(
        after * 2 > drift.len() - first,
        "{after} of {}",
        drift.len() - first
    );
}

fn boxes() -> FrameExtras {
    FrameExtras::with_size(CAR_L, CAR_W)
}

/// Two senders' claims at 10 Hz for 3 s, through one suite with sizes; returns the
/// verdicts on `second`'s messages.
fn pair(
    first: (f64, f64, f64),
    second: (f64, f64, f64),
    own: Option<&OwnFootprint>,
) -> Vec<TsVerdict> {
    let mut suite = Ts103759Suite::cited_defaults();
    let mut ctx = CollectingCtx::new(3);
    let f = boxes();
    let mut out = Vec::new();
    for i in 0..30u64 {
        let t = i * secs_to_ns(0.1);
        let dt = i as f64 * 0.1;
        for (who, (x, y, v)) in [(A, first), (G, second)] {
            let m = msg(who, t, x + v * dt, y, v, 0.0, 2.0);
            ctx.set_now(t);
            let mut x_in = CrossCheckInputs::none().and_frame(&f);
            x_in.own = own;
            // The receiver sits 40 m north of the road unless its own footprint is used.
            let me = if own.is_some() {
                me_at(t, 0.0, 0.0)
            } else {
                me_at(t, 0.0, 40.0)
            };
            let v = suite.check(&mut ctx, &me, &m, &NoMap, &x_in);
            if who == G {
                out.push(v);
            }
        }
    }
    out
}

#[test]
fn boxes_side_by_side_and_in_a_tight_queue_do_not_fire_and_ghosts_do() {
    // Side by side in adjacent 3.5 m lanes at 15 m/s: 1.7 m of air between the bodies.
    let v = pair((0.0, 0.0, 15.0), (0.0, 3.5, 15.0), None);
    assert_eq!(fired(&v, Ts103759Check::BoxIntersection), 0);
    assert!(
        v.iter()
            .all(|v| v.score(Ts103759Check::BoxIntersection) == Some(0.0))
    );

    // A stopped queue with a 1.5 m gap between bumpers.
    let v = pair((0.0, 0.0, 0.0), (-(CAR_L + 1.5), 0.0, 0.0), None);
    assert_eq!(fired(&v, Ts103759Check::BoxIntersection), 0);
    assert!(
        v.iter()
            .all(|v| v.score(Ts103759Check::BoxIntersection) == Some(0.0))
    );

    // A ghost 2.4 m behind a car's centre along the lane: the bodies overlap by 52 %, but
    // the centres are further apart than the point check's 2 m.
    let v = pair((0.0, 0.0, 15.0), (-2.4, 0.0, 15.0), None);
    assert!(fired(&v, Ts103759Check::BoxIntersection) > 0);
    assert_eq!(fired(&v, Ts103759Check::Intersection), 0);

    // A ghost on top of the receiver itself, which nobody else can see.
    let own = OwnFootprint {
        heading_rad: 0.0,
        length_m: CAR_L,
        width_m: CAR_W,
        reference: ReferencePoint::Centre,
    };
    let v = pair((0.0, 200.0, 0.0), (0.5, 0.3, 0.0), Some(&own));
    assert!(fired(&v, Ts103759Check::BoxIntersection) > 0);
    // Without its own footprint the receiver has nothing to compare it with.
    let v = pair((0.0, 200.0, 0.0), (0.5, 0.3, 0.0), None);
    assert_eq!(fired(&v, Ts103759Check::BoxIntersection), 0);

    // No size on the frame: unchecked, and counted.
    let mut s = Ts103759Suite::cited_defaults();
    let vs = run(&mut s, &[msg(A, 0, 0.0, 0.0, 0.0, 0.0, 2.0)], None);
    assert_eq!(vs[0].score(Ts103759Check::BoxIntersection), None);
    assert_eq!(s.skipped_no_size(), 1);
}

/// A 10 Hz track braking at `decel` from `v0` to a stop, then standing, with the engine's
/// GNSS white noise; `keep_every` keeps one message in that many (packet loss).
fn stop(v0: f64, decel: f64, keep_every: u64, seed: u64) -> Vec<ObservedMessage> {
    let mut n = Noise(seed);
    let t_stop = v0 / decel;
    (0..80u64)
        .filter(|i| i % keep_every == 0)
        .map(|i| {
            let t = i as f64 * 0.1;
            let tb = t.min(t_stop);
            let x = v0 * tb - 0.5 * decel * tb * tb;
            let v = (v0 - decel * t).max(0.0);
            msg(
                A,
                secs_to_ns(t),
                x + 0.3 * n.normal(),
                0.3 * n.normal(),
                (v + 0.05 * n.normal()).max(0.0),
                0.0,
                1.0,
            )
        })
        .collect()
}

#[test]
fn an_honest_emergency_stop_is_not_flagged() {
    // 8 m/s² from 22 m/s, at 10 Hz and under heavy loss (one message in five and one in
    // fifteen arriving). Nothing in the suite may fire.
    for keep in [1u64, 5, 15] {
        let mut s = Ts103759Suite::cited_defaults();
        let vs = run(&mut s, &stop(22.0, 8.0, keep, 5), None);
        let all: Vec<String> = vs
            .iter()
            .flat_map(|v| v.fired.iter().map(|o| o.check.as_str().to_string()))
            .collect();
        assert!(all.is_empty(), "keep 1/{keep}: {all:?}");
        assert!(
            vs.iter()
                .any(|v| v.evaluated(Ts103759Check::PositionSpeedEnvelope)),
            "the envelope never ran"
        );
    }
    // The default limits are the vehicle classes': the largest acceleration and
    // emergency deceleration any class in v2xw_mobility has.
    let a = VehicleClass::ALL
        .iter()
        .map(|c| c.spec().accel_mps2)
        .fold(0.0, f64::max);
    let b = VehicleClass::ALL
        .iter()
        .map(|c| c.spec().emergency_decel_mps2)
        .fold(0.0, f64::max);
    let p = Ts103759Params::default();
    assert_eq!(p.max_plausible_accel_mps2, a);
    assert_eq!(p.max_plausible_decel_mps2, b);
}

#[test]
fn f2md_braking_limits_flag_the_same_honest_stop() {
    // The comparison that motivates the class limits: F2MD's 4.5 m/s².
    let mut s = Ts103759Suite::new(Ts103759Params::default().f2md_kinematics());
    let vs = run(&mut s, &stop(22.0, 8.0, 1, 5), None);
    assert!(
        vs.iter().any(|v| v.fired()),
        "F2MD's limits should flag an 8 m/s² stop"
    );
}

#[test]
fn a_claim_that_stands_still_at_speed_fires_the_envelope_and_the_mean_speed_check() {
    // F2MD's constant-position attack: 30 m/s claimed, 2 m moved per second, 1 Hz.
    let msgs: Vec<ObservedMessage> = (0..4u64)
        .map(|i| msg(A, i * NS_PER_S, 2.0 * i as f64, 0.0, 30.0, 0.0, 2.0))
        .collect();
    let mut s = Ts103759Suite::cited_defaults();
    let vs = run(&mut s, &msgs, None);
    assert!(fired(&vs, Ts103759Check::PositionSpeedEnvelope) > 0);
    assert!(fired(&vs, Ts103759Check::PositionSpeedMaxConsistency) > 0);
    // At an honest 2 m/s the same track is quiet.
    let honest: Vec<ObservedMessage> = (0..4u64)
        .map(|i| msg(A, i * NS_PER_S, 2.0 * i as f64, 0.0, 2.0, 0.0, 2.0))
        .collect();
    let mut s = Ts103759Suite::cited_defaults();
    let vs = run(&mut s, &honest, None);
    assert!(vs.iter().all(|v| !v.fired()));
}

fn leaky() -> Ts103759Suite {
    let mut p = Ts103759Params::default();
    assert!(p.apply_label("fusion", "leaky-bucket"));
    Ts103759Suite::new(p)
}

/// A claim at `x` with speed `v`. On a first sighting, or after a gap longer than the
/// comparison window, no interval check runs, so the bucket's f_min is the speed check's
/// factor whenever that is the lowest.
fn at_speed(signer: [u8; 8], t: SimTime, x: f64, v: f64) -> ObservedMessage {
    msg(signer, t, x, 0.0, v, 0.0, 2.0)
}

#[test]
fn the_leaky_bucket_keeps_each_level_on_its_own_pseudonym() {
    let mut s = leaky();
    let mut ctx = CollectingCtx::new(1);
    let me = me_at(0, 0.0, 0.0);
    let x = CrossCheckInputs::none();
    let mut go = |s: &mut Ts103759Suite, m: &ObservedMessage| {
        ctx.set_now(m.received_at);
        s.check(&mut ctx, &me, m, &NoMap, &x)
    };
    // f = 1 − (v/40)·(1 − θ): A at 60 m/s is a hard fail (f = 0), B at 51.9 m/s is
    // f = 0.1, C at 46.3 m/s is f = 0.2.
    let theta = 0.28125;
    let v_for = |f: f64| 40.0 * (1.0 - f) / (1.0 - theta);
    let va = go(&mut s, &at_speed(A, 0, 100.0, 60.0));
    let vb = go(&mut s, &at_speed(B, 0, 200.0, v_for(0.1)));
    let vc = go(&mut s, &at_speed(C, 0, 300.0, v_for(0.2)));
    let pen = |f: f64| 0.0005 * (10.0 * (1.0 - f)).exp();
    let (la, lb, lc) = (pen(0.0) - 1.0, pen(0.1) - 1.0, pen(0.2) - 1.0);
    // An honest 16 m/s claim: its lowest factor is the speed check's, 1 − 0.4·(1 − θ).
    let honest = pen(1.0 - 0.4 * (1.0 - theta));
    assert!((s.bucket(&A).unwrap() - la).abs() < 1e-9);
    assert!((s.bucket(&B).unwrap() - lb).abs() < 1e-9);
    assert!((s.bucket(&C).unwrap() - lc).abs() < 1e-9);
    // A's bucket overflowed on one hard fail; B's failing speed check is damped.
    assert!(
        va.fired
            .iter()
            .any(|o| o.check == Ts103759Check::SpeedPlausibility)
    );
    assert!(!vb.fired() && !vc.fired());

    // B is removed (say, revoked). A and C keep their own levels (F2MD D6 moved them).
    assert!(s.remove_bucket(&B));
    assert_eq!(s.bucket(&B), None);
    assert!((s.bucket(&A).unwrap() - la).abs() < 1e-9);
    assert!((s.bucket(&C).unwrap() - lc).abs() < 1e-9);
    assert_eq!(s.bucket_pseudonyms(), vec![A, C]);
    // An update after the removal (past the 1.86 s comparison window) lands on its own
    // pseudonym only.
    go(&mut s, &at_speed(A, 2 * NS_PER_S, 100.0, 16.0));
    let a2 = la + honest - 1.0;
    assert!(
        (s.bucket(&A).unwrap() - a2).abs() < 1e-9,
        "{:?}",
        s.bucket(&A)
    );
    assert!((s.bucket(&C).unwrap() - lc).abs() < 1e-9);

    // Expiry: C, silent for longer than bucket_expiry_s, is dropped when A is next heard;
    // A's level is A's.
    go(&mut s, &at_speed(A, 11 * NS_PER_S, 100.0, 16.0));
    assert_eq!(s.bucket(&C), None);
    assert!((s.bucket(&A).unwrap() - (a2 + honest - 1.0)).abs() < 1e-9);
}

#[test]
fn the_windowed_mean_damps_a_single_spike_and_fires_on_a_sustained_fault() {
    let mean = || {
        let mut p = Ts103759Params::default();
        assert!(p.apply_label("fusion", "windowed-mean"));
        Ts103759Suite::new(p)
    };
    // Three honest messages, then a two-message spike of 100 m/s, then honest again; a
    // separate run with a sustained 60 m/s.
    let speeds_spike = [16.0, 16.0, 16.0, 100.0, 100.0, 16.0, 16.0, 16.0];
    let speeds_sustained = [16.0, 16.0, 16.0, 60.0, 60.0, 60.0, 60.0];
    let msgs = |sp: &[f64]| -> Vec<ObservedMessage> {
        sp.iter()
            .enumerate()
            .map(|(i, v)| msg(A, i as u64 * NS_PER_S, 16.0 * i as f64, 0.0, *v, 0.0, 2.0))
            .collect()
    };
    let mut s = mean();
    let spike = run(&mut s, &msgs(&speeds_spike), None);
    // (0.4 + 0.4 + 2.5 + 2.5)/4 = 1.45 on the second spike message: fires …
    assert!(fired(&spike, Ts103759Check::SpeedPlausibility) > 0);
    // … but a one-message spike does not ((0.4·3 + 2.5)/4 = 0.925).
    let mut s = mean();
    let one = run(&mut s, &msgs(&[16.0, 16.0, 16.0, 100.0, 16.0, 16.0]), None);
    assert_eq!(fired(&one, Ts103759Check::SpeedPlausibility), 0);
    let mut s = mean();
    let sustained = run(&mut s, &msgs(&speeds_sustained), None);
    let first = sustained
        .iter()
        .position(|v| {
            v.fired
                .iter()
                .any(|o| o.check == Ts103759Check::SpeedPlausibility)
        })
        .expect("a sustained 60 m/s never fired");
    // (0.4 + 1.5·3)/4 = 1.225 on the third 60 m/s message (index 5).
    assert_eq!(first, 5);
    // Scores older than the 10 s window do not count.
    let mut late: Vec<ObservedMessage> = msgs(&[100.0, 100.0, 100.0]);
    late.push(msg(A, 20 * NS_PER_S, 320.0, 0.0, 16.0, 0.0, 2.0));
    let mut s = mean();
    let vs = run(&mut s, &late, None);
    assert!(
        !vs[3]
            .fired
            .iter()
            .any(|o| o.check == Ts103759Check::SpeedPlausibility)
    );
}

#[test]
fn catch_scoring_scores_the_confidence_overlap() {
    let mut p = Ts103759Params::default();
    assert!(p.apply_label("scoring", "catch"));
    assert_eq!(p.scoring, Scoring::Catch);
    assert!(!p.apply_label("scoring", "binary"));
    // A claim at exactly 40 m/s with the 1 m/s floor: half its interval is plausible,
    // f = 0.5, score (1 − 0.5)/(1 − 0.28125).
    let mut s = Ts103759Suite::new(p.clone());
    let vs = run(&mut s, &[msg(A, 0, 0.0, 0.0, 40.0, 0.0, 2.0)], None);
    let sc = vs[0].score(Ts103759Check::SpeedPlausibility).unwrap();
    assert!((sc - 0.5 / 0.71875).abs() < 1e-9, "{sc}");
    // With a broadcast speed confidence of 2 m/s, 40.5 m/s: f = 1.5/4.
    let f = FrameExtras {
        speed_confidence_mps: Some(2.0),
        ..FrameExtras::default()
    };
    let mut s = Ts103759Suite::new(p.clone());
    let vs = run(&mut s, &[msg(A, 0, 0.0, 0.0, 40.5, 0.0, 2.0)], Some(&f));
    let sc = vs[0].score(Ts103759Check::SpeedPlausibility).unwrap();
    assert!((sc - (1.0 - 1.5 / 4.0) / 0.71875).abs() < 1e-9, "{sc}");
    // Range: a 2 m disc centred on the acceptance range is half inside.
    let art = p.art_m();
    let mut s = Ts103759Suite::new(p.clone());
    let vs = run(&mut s, &[msg(A, 0, art, 0.0, 10.0, 0.0, 2.0)], None);
    let sc = vs[0].score(Ts103759Check::RangePlausibility).unwrap();
    assert!((sc - 0.5 / 0.71875).abs() < 1e-3, "{sc}");
    // Fully inside: zero.
    let mut s = Ts103759Suite::new(p);
    let vs = run(&mut s, &[msg(A, 0, 100.0, 0.0, 10.0, 0.0, 2.0)], None);
    assert_eq!(vs[0].score(Ts103759Check::RangePlausibility), Some(0.0));
    assert_eq!(vs[0].score(Ts103759Check::SpeedPlausibility), Some(0.0));
}

#[test]
fn fusion_and_scoring_labels_round_trip() {
    for f in [
        Fusion::Consecutive,
        Fusion::WindowedMean,
        Fusion::LeakyBucket,
    ] {
        assert_eq!(Fusion::parse(f.label()), Some(f));
    }
    assert_eq!(Ts103759Params::default().fusion, Fusion::Consecutive);
    assert_eq!(Ts103759Params::default().scoring, Scoring::Threshold);
    let _ = BeaconProfile::BSM_J2945_1;
}

/// The new checks' honest false-positive bound, per message the check evaluated.
const FP_BOUND: f64 = 1.0e-3;

#[test]
fn honest_gauss_markov_traffic_stays_under_the_stated_bounds() {
    // The attack-in-the-loop harness with no attackers and no faulty sensors: the
    // engine's Gauss–Markov GNSS, 10 Hz BSMs, a two-lane-per-direction signalised grid
    // (side-by-side cars, queues at red lights), modelled propagation and loss.
    let opts = SimOptions {
        duration_s: 40.0,
        max_vehicles: 40,
        attacker_fraction: 0.0,
        faulty_fraction: 0.0,
        tap: true,
        ..SimOptions::new(7, 14_400.0, 0.0)
    };
    let out = sim::run(&opts);
    assert!(out.tap.len() > 20_000, "only {} deliveries", out.tap.len());
    let digest = |tap: &[(SelfBelief, f64, ObservedMessage)], conf: Option<f64>| {
        let mut suites: BTreeMap<NodeId, Ts103759Suite> = BTreeMap::new();
        let mut ctx = CollectingCtx::new(9);
        let frame = FrameExtras::with_size(CAR_L, CAR_W);
        let mut eval: BTreeMap<Ts103759Check, u64> = BTreeMap::new();
        let mut fire: BTreeMap<Ts103759Check, u64> = BTreeMap::new();
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for (me, heading, m) in tap {
            let own = OwnFootprint {
                heading_rad: *heading,
                length_m: CAR_L,
                width_m: CAR_W,
                reference: ReferencePoint::Centre,
            };
            let x = CrossCheckInputs::none().and_frame(&frame).and_own(&own);
            ctx.set_now(m.received_at);
            let s = suites
                .entry(me.node)
                .or_insert_with(Ts103759Suite::cited_defaults);
            let mut m = m.clone();
            if let Some(c) = conf {
                m.claimed_pos_confidence_m = c;
            }
            let v = s.check(&mut ctx, me, &m, &NoMap, &x);
            for (c, sc) in &v.scores {
                *eval.entry(*c).or_default() += 1;
                h = (h ^ sc.to_bits()).wrapping_mul(0x100_0000_01b3);
            }
            for o in &v.fired {
                *fire.entry(o.check).or_default() += 1;
            }
        }
        (eval, fire, h)
    };
    let (eval, fire, h1) = digest(&out.tap, None);
    let mut report = String::new();
    for (c, n) in &eval {
        let f = fire.get(c).copied().unwrap_or(0);
        report.push_str(&format!("{c}: {f}/{n}\n"));
    }
    let mut confs: Vec<f64> = out
        .tap
        .iter()
        .map(|(_, _, m)| m.claimed_pos_confidence_m)
        .collect();
    confs.sort_by(f64::total_cmp);
    eprintln!(
        "deliveries {} broadcast confidence p5/p50/p95 {:.2}/{:.2}/{:.2} m\n{report}",
        out.tap.len(),
        confs[confs.len() / 20],
        confs[confs.len() / 2],
        confs[confs.len() * 19 / 20]
    );
    for c in [
        Ts103759Check::KalmanConsistency,
        Ts103759Check::PositionSpeedEnvelope,
        Ts103759Check::PositionSpeedMaxConsistency,
        Ts103759Check::BoxIntersection,
    ] {
        let n = eval.get(&c).copied().unwrap_or(0);
        assert!(n > 10_000, "{c} evaluated only {n} times");
        let f = fire.get(&c).copied().unwrap_or(0) as f64 / n as f64;
        assert!(
            f < FP_BOUND,
            "{c}: honest false-positive rate {f} over {FP_BOUND}\n{report}"
        );
    }
    // Determinism: the same deliveries give byte-identical scores.
    let (_, _, h2) = digest(&out.tap, None);
    assert_eq!(h1, h2);
    // Diagnostic, not a bound: the same deliveries from a receiver that under-reports its
    // confidence as 1 m (the Kalman R then sits on its 4 m floor).
    let (e1, f1, _) = digest(&out.tap, Some(1.0));
    for c in [
        Ts103759Check::KalmanConsistency,
        Ts103759Check::PositionSpeedEnvelope,
        Ts103759Check::PositionSpeedMaxConsistency,
        Ts103759Check::BoxIntersection,
    ] {
        eprintln!(
            "conf 1 m: {c}: {}/{}",
            f1.get(&c).copied().unwrap_or(0),
            e1.get(&c).copied().unwrap_or(0)
        );
    }
}
