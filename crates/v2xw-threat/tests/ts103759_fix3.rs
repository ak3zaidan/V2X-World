//! detect-fix3 defects in the TS 103 759 suite found by the first live run with the suite
//! hosted (crates/v2xw-engine/tests/expectations/detect-fix3.yaml, run T0).
//!
//! * The class-5 intersection check compared a claim with where another station stood up
//!   to 2 s earlier: a follower driving over its leader's old spot "intersected" it. The
//!   other claim is now moved on at its own claimed velocity (as the box check moves it).
//!   Shown red by the old comparison: the follower case scored 2 / 0.
//! * Under CaTCh scoring the map check scores the stated confidence disc, so an honest
//!   claim 4 m off the lane with a 5 m confidence is not off the road, while one 20 m off is.

use v2xw_core::ids::NodeId;
use v2xw_core::time::{SimTime, secs_to_ns};
use v2xw_threat::ctx::CollectingCtx;
use v2xw_threat::obs::{
    LocalEnvironment, ObservedKind, ObservedMessage, SelfBelief, StationType, VerificationState,
};
use v2xw_threat::ts103759::{
    CrossCheckInputs, Scoring, Ts103759Check, Ts103759Params, Ts103759Suite, TsVerdict,
};

const A: [u8; 8] = [0xAA; 8];
const B: [u8; 8] = [0xBB; 8];

/// Every claim is a fixed distance off the road.
struct OffRoad(f64);

impl LocalEnvironment for OffRoad {
    fn distance_to_road_m(&self, _x_m: f64, _y_m: f64) -> f64 {
        self.0
    }
}

fn beacon(signer: [u8; 8], x_m: f64, speed_mps: f64, t: SimTime, conf: f64) -> ObservedMessage {
    ObservedMessage {
        bsm: Default::default(),
        signer,
        kind: ObservedKind::Beacon,
        received_at: t,
        claimed_generation_time: t,
        claimed_x_m: x_m,
        claimed_y_m: 0.0,
        claimed_speed_mps: speed_mps,
        claimed_heading_rad: 0.0,
        claimed_pos_confidence_m: conf,
        repetitions: 1,
        cert_valid_from: 0,
        cert_valid_to: u64::MAX,
        station_type: StationType::Vehicle,
        verification: VerificationState::Valid,
    }
}

fn one(
    suite: &mut Ts103759Suite,
    ctx: &mut CollectingCtx,
    m: &ObservedMessage,
    env: &dyn LocalEnvironment,
) -> TsVerdict {
    ctx.set_now(m.received_at);
    let me = SelfBelief {
        node: NodeId::new(1),
        believed_time: m.received_at,
        x_m: 0.0,
        y_m: 0.0,
        radio_range_m: 500.0,
    };
    suite.check(ctx, &me, m, env, &CrossCheckInputs::none())
}

#[test]
fn a_follower_on_its_leaders_old_spot_does_not_intersect_it() {
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let road = OffRoad(0.0);
    // The leader claims x = 100 at 15 m/s; half a second later its follower claims x = 100.
    one(&mut suite, &mut ctx, &beacon(A, 100.0, 15.0, 0, 5.0), &road);
    let v = one(
        &mut suite,
        &mut ctx,
        &beacon(B, 100.0, 15.0, secs_to_ns(0.5), 5.0),
        &road,
    );
    let s = v.score(Ts103759Check::Intersection).expect("ran");
    // The leader is now at 107.5 m: 2 m / 7.5 m.
    assert!((s - 2.0 / 7.5).abs() < 1e-9, "score {s}");
    // A stationary overlap still scores as before (the published check's case).
    let mut suite = Ts103759Suite::cited_defaults();
    one(&mut suite, &mut ctx, &beacon(A, 100.0, 0.0, 0, 5.0), &road);
    let v = one(
        &mut suite,
        &mut ctx,
        &beacon(B, 100.5, 0.0, secs_to_ns(0.5), 5.0),
        &road,
    );
    assert_eq!(v.score(Ts103759Check::Intersection), Some(2.0 / 0.5));
}

#[test]
fn catch_scores_the_map_check_on_the_stated_confidence() {
    let catch = Ts103759Params {
        scoring: Scoring::Catch,
        ..Ts103759Params::default()
    };
    let score = |p: &Ts103759Params, off: f64, conf: f64| {
        let mut ctx = CollectingCtx::new(1);
        let mut suite = Ts103759Suite::new(p.clone());
        one(
            &mut suite,
            &mut ctx,
            &beacon(A, 0.0, 10.0, 0, conf),
            &OffRoad(off),
        )
        .score(Ts103759Check::PositionPlausibility)
        .expect("ran")
    };
    // Threshold: 4 m off against 2 m is a score of 2, whatever the confidence.
    assert!((score(&Ts103759Params::default(), 4.0, 5.0) - 2.0).abs() < 1e-9);
    // CaTCh: inside the stated 5 m disc it is plausible; 20 m off is not.
    assert!(
        score(&catch, 4.0, 5.0) < 1.0,
        "an honest 4 m offset fired under CaTCh"
    );
    assert!(
        score(&catch, 20.0, 5.0) >= 1.0,
        "a claim 20 m off the road passed"
    );
}
