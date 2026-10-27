//! The TS 103 759 observation suite: every class can go red, benign traffic stays quiet,
//! a check whose input is missing is *unchecked* rather than passed, and the four
//! verification states produce four different outcomes.
//!
//! The last one is a regression test for a real defect: a suite that treated "this node's
//! policy has not verified this message yet" as a cryptographic failure scored 98 % of
//! honest messages as misbehaviour. It is asserted here from both sides — nothing fires,
//! and nothing is even scored.

use std::collections::BTreeSet;

use v2xw_core::ids::NodeId;
use v2xw_core::time::{NS_PER_S, SimTime, secs_to_ns};
use v2xw_threat::ctx::CollectingCtx;
use v2xw_threat::obs::{
    DiscSensor, EnvelopeExtras, LocalEnvironment, NoPerception, ObservedKind, ObservedMessage,
    PerceivedObject, RegionId, SelfBelief, SensedObject, StationType, VerificationState,
};
use v2xw_threat::ts103759::{
    ART_ANTENNA_GAIN_DBI, ART_CARRIER_HZ, ART_DERIVATION, ART_MARGIN_SIGMAS, ART_SENSITIVITY_DBM,
    ART_TX_POWER_DBM, AcceptanceRanges, ArtRow, BeaconProfile, CrossCheckInputs, ObservationClass,
    RadioEnvironment, Ts103759Check, Ts103759Params, Ts103759Suite, TsVerdict,
};

const RX: NodeId = NodeId::new(1);
const A: [u8; 8] = [0xAA; 8];
const B: [u8; 8] = [0xBB; 8];

/// The world's only road: the line `y = 0`.
struct OnRoad;

impl LocalEnvironment for OnRoad {
    fn distance_to_road_m(&self, _x_m: f64, y_m: f64) -> f64 {
        y_m.abs()
    }
}

/// A map that says every claim is a fixed distance off the road.
struct OffRoad(f64);

impl LocalEnvironment for OffRoad {
    fn distance_to_road_m(&self, _x_m: f64, _y_m: f64) -> f64 {
        self.0
    }
}

fn me(t: SimTime) -> SelfBelief {
    SelfBelief {
        node: RX,
        believed_time: t,
        x_m: 0.0,
        y_m: 0.0,
        radio_range_m: 500.0,
    }
}

fn beacon(signer: [u8; 8], x_m: f64, speed_mps: f64, t: SimTime) -> ObservedMessage {
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
        claimed_pos_confidence_m: 2.0,
        repetitions: 1,
        cert_valid_from: 0,
        cert_valid_to: u64::MAX,
        station_type: StationType::Vehicle,
        verification: VerificationState::Valid,
    }
}

/// Runs a sequence of messages through a fresh suite and returns every check that fired.
fn fired_over(
    params: Ts103759Params,
    msgs: &[ObservedMessage],
    env: &dyn LocalEnvironment,
) -> (BTreeSet<Ts103759Check>, CollectingCtx, Ts103759Suite) {
    let mut ctx = CollectingCtx::new(7);
    let mut suite = Ts103759Suite::new(params);
    let mut fired = BTreeSet::new();
    for m in msgs {
        ctx.set_now(m.received_at);
        let x = CrossCheckInputs::none();
        let v = suite.check(&mut ctx, &me(m.received_at), m, env, &x);
        for o in &v.fired {
            fired.insert(o.check);
        }
    }
    (fired, ctx, suite)
}

fn one(
    suite: &mut Ts103759Suite,
    ctx: &mut CollectingCtx,
    m: &ObservedMessage,
    env: &dyn LocalEnvironment,
    x: &CrossCheckInputs<'_>,
) -> TsVerdict {
    ctx.set_now(m.received_at);
    suite.check(ctx, &me(m.received_at), m, env, x)
}

// ---------------------------------------------------------------------------------------
// The four verification states
// ---------------------------------------------------------------------------------------

#[test]
fn a_deferred_verification_scores_nothing_at_all() {
    // The 98 %-false-positive defect, asserted from both sides.
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    for i in 0..20u64 {
        let mut m = beacon(A, 16.0 * i as f64, 16.0, i * NS_PER_S);
        m.verification = VerificationState::Unverified;
        let v = one(&mut suite, &mut ctx, &m, &OnRoad, &CrossCheckInputs::none());
        assert!(
            v.scores.is_empty(),
            "an unverified message was scored: {:?}",
            v.scores
        );
        assert!(!v.fired(), "an unverified message produced a finding");
        assert!(
            !v.evaluated(Ts103759Check::SignatureInvalid),
            "NOT CHECKED was reported as a signature failure"
        );
    }
    assert_eq!(suite.deferred(), 20);
    assert!(
        ctx.on_channel("det.observation").is_empty(),
        "a deferred verification wrote an observation"
    );
}

#[test]
fn the_four_verification_states_produce_four_outcomes() {
    let env = OnRoad;
    let x = CrossCheckInputs::none();

    // Valid: the content is evidence, so the content checks run.
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let v = one(&mut suite, &mut ctx, &beacon(A, 10.0, 16.0, 0), &env, &x);
    assert!(v.evaluated(Ts103759Check::RangePlausibility));
    assert!(!v.evaluated(Ts103759Check::SignatureInvalid));
    assert!(!v.evaluated(Ts103759Check::CertificateUnknown));

    // Bad signature: tested and failed. One finding, and no content check.
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let mut bad = beacon(A, 10.0, 16.0, 0);
    bad.verification = VerificationState::BadSignature;
    let v = one(&mut suite, &mut ctx, &bad, &env, &x);
    assert_eq!(v.score(Ts103759Check::SignatureInvalid), Some(1.5));
    assert!(!v.evaluated(Ts103759Check::RangePlausibility));
    assert!(!v.evaluated(Ts103759Check::CertificateUnknown));

    // Unknown certificate: a statement about this receiver's trust store, and a
    // DIFFERENT finding from a forged signature. A report citing one names a different
    // misbehaviour from a report citing the other.
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let mut unknown = beacon(A, 10.0, 16.0, 0);
    unknown.verification = VerificationState::UnknownCertificate;
    let v = one(&mut suite, &mut ctx, &unknown, &env, &x);
    assert_eq!(v.score(Ts103759Check::CertificateUnknown), Some(1.5));
    assert!(!v.evaluated(Ts103759Check::SignatureInvalid));
    assert!(!v.evaluated(Ts103759Check::RangePlausibility));

    // Deferred: nothing at all, and it is counted.
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let mut deferred = beacon(A, 10.0, 16.0, 0);
    deferred.verification = VerificationState::Unverified;
    let v = one(&mut suite, &mut ctx, &deferred, &env, &x);
    assert!(v.scores.is_empty());
    assert_eq!(suite.deferred(), 1);
}

#[test]
fn two_consecutive_signature_failures_fire_and_one_does_not() {
    // The legacy streak gate, kept: one failure is one sample.
    let mut msgs = Vec::new();
    for i in 0..2u64 {
        let mut m = beacon(A, 10.0, 16.0, i * NS_PER_S);
        m.verification = VerificationState::BadSignature;
        msgs.push(m);
    }
    let (fired_once, _, _) = fired_over(Ts103759Params::default(), &msgs[..1], &OnRoad);
    assert!(fired_once.is_empty(), "one failure is one sample");
    let (fired_twice, ctx, _) = fired_over(Ts103759Params::default(), &msgs, &OnRoad);
    assert!(fired_twice.contains(&Ts103759Check::SignatureInvalid));
    assert_eq!(ctx.on_channel("det.observation").len(), 1);
}

// ---------------------------------------------------------------------------------------
// Benign traffic
// ---------------------------------------------------------------------------------------

#[test]
fn benign_traffic_stays_quiet() {
    // One vehicle driving straight at 16 m/s at 1 Hz, on the road, inside range. Every
    // check that runs must stay under its threshold, or every number this suite produces
    // is noise.
    let msgs: Vec<ObservedMessage> = (0..20u64)
        .map(|i| beacon(A, 16.0 * i as f64, 16.0, i * NS_PER_S))
        .collect();
    let (fired, ctx, suite) = fired_over(Ts103759Params::default(), &msgs, &OnRoad);
    assert!(
        fired.is_empty(),
        "benign traffic fired: {:?}",
        fired.iter().map(|c| c.as_str()).collect::<Vec<_>>()
    );
    assert!(ctx.on_channel("det.observation").is_empty());
    assert_eq!(suite.deferred(), 0);
    // With no sensors the class-4 check did not run, and the count says so rather than
    // the run reporting a class-4 recall it never measured.
    assert_eq!(suite.class4_skipped_no_perception(), 20);
    assert_eq!(suite.class4_evaluated(), 0);
    assert_eq!(suite.skipped_no_envelope(), 20);
    assert_eq!(suite.skipped_no_region(), 20);
}

// ---------------------------------------------------------------------------------------
// One firing case per class
// ---------------------------------------------------------------------------------------

#[test]
fn class_one_fires_on_an_implausible_speed() {
    let msgs: Vec<ObservedMessage> = (0..3u64)
        .map(|i| beacon(A, 10.0 * i as f64, 60.0, i * NS_PER_S))
        .collect();
    let (fired, _, _) = fired_over(Ts103759Params::default(), &msgs, &OnRoad);
    assert!(fired.contains(&Ts103759Check::SpeedPlausibility));
    assert!(
        fired
            .iter()
            .any(|c| c.class() == ObservationClass::ImplausibleValues)
    );
}

#[test]
fn class_one_fires_on_an_oversized_message_and_is_unchecked_without_the_size() {
    let extras = EnvelopeExtras::with_payload_bytes(5_000);
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let x = CrossCheckInputs {
        perception: &NoPerception,
        envelope: Some(&extras),
        frame: None,
        own: None,
    };
    let mut fired = BTreeSet::new();
    for i in 0..2u64 {
        let m = beacon(A, 16.0 * i as f64, 16.0, i * NS_PER_S);
        let v = one(&mut suite, &mut ctx, &m, &OnRoad, &x);
        assert_eq!(
            v.score(Ts103759Check::OversizedMessage),
            Some(5_000.0 / 2_304.0)
        );
        for o in &v.fired {
            fired.insert(o.check);
        }
    }
    assert!(fired.contains(&Ts103759Check::OversizedMessage));
    assert_eq!(suite.skipped_no_envelope(), 0);

    // No measured size: unchecked, not passed.
    let (fired, _, suite) = fired_over(
        Ts103759Params::default(),
        &[beacon(A, 0.0, 16.0, 0)],
        &OnRoad,
    );
    assert!(fired.is_empty());
    assert_eq!(suite.skipped_no_envelope(), 1);
}

#[test]
fn class_two_fires_on_a_position_that_moved_further_than_any_speed_allows() {
    let msgs: Vec<ObservedMessage> = (0..3u64)
        .map(|i| beacon(A, 120.0 * i as f64, 16.0, i * NS_PER_S))
        .collect();
    let (fired, _, _) = fired_over(Ts103759Params::default(), &msgs, &OnRoad);
    assert!(fired.contains(&Ts103759Check::PositionConsistency));
    assert!(
        fired
            .iter()
            .any(|c| c.class() == ObservationClass::PreviousMessages)
    );
}

#[test]
fn class_two_fires_on_a_generation_time_that_did_not_advance() {
    let mut msgs = Vec::new();
    for i in 0..3u64 {
        let mut m = beacon(A, 16.0 * i as f64, 16.0, i * NS_PER_S);
        m.claimed_generation_time = 0;
        msgs.push(m);
    }
    let (fired, _, _) = fired_over(Ts103759Params::default(), &msgs, &OnRoad);
    assert!(fired.contains(&Ts103759Check::GenerationTimeOrder));
}

// ---------------------------------------------------------------------------------------
// Beacon timing from the message profile (f2md.md §5 P1). This replaces the test that
// asserted 0.1 s gaps fire: 0.1 s is an honest BSM sender, and F2MD's 0.9 s minimum is a
// 1 Hz value that would have flagged every one of them.
// ---------------------------------------------------------------------------------------

/// A deterministic uniform draw in `[0, 1)` (SplitMix64), so the jitter is reproducible.
fn unit(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// An honest J2945/1 sender at 10 Hz for `secs`, driving at 16 m/s, with the engine's
/// own hand-off jitter (`U[0, 10 ms]`, `v2xw_msg::GenerationTiming`) and up to 2 ms of
/// channel access on every message; every tenth pair is the worst case, a 10 ms jitter
/// followed by none.
fn honest_bsm_sender(secs: u64) -> Vec<ObservedMessage> {
    let jitter_ns = v2xw_msg::GenerationTiming::NS3_TX_MAX_DELAY.as_nanos() as f64;
    let mut state = 0x5EED_u64;
    (0..secs * 10)
        .map(|i| {
            let j = match i % 10 {
                4 => jitter_ns,
                5 => 0.0,
                _ => unit(&mut state) * jitter_ns,
            };
            let access = unit(&mut state) * 2.0e6;
            let t = i * secs_to_ns(0.1) + (j + access) as u64;
            beacon(A, 16.0 * t as f64 / NS_PER_S as f64, 16.0, t)
        })
        .collect()
}

#[test]
fn an_honest_10_hz_bsm_sender_raises_no_beacon_frequency_event() {
    let msgs = honest_bsm_sender(60);
    assert_eq!(msgs.len(), 600);
    let (fired, ctx, _) = fired_over(Ts103759Params::default(), &msgs, &OnRoad);
    assert!(
        fired.is_empty(),
        "an honest 10 Hz sender fired: {:?}",
        fired.iter().map(|c| c.as_str()).collect::<Vec<_>>()
    );
    assert!(ctx.on_channel("det.observation").is_empty());
    // F2MD's 0.9 s on the same traffic flags it on its second gap.
    let f2md = Ts103759Params {
        min_beacon_interval_s: Some(0.9),
        ..Ts103759Params::default()
    };
    let (fired, _, _) = fired_over(f2md, &msgs, &OnRoad);
    assert!(fired.contains(&Ts103759Check::BeaconFrequency));
}

#[test]
fn a_20_hz_sender_raises_beacon_frequency() {
    let msgs: Vec<ObservedMessage> = (0..40u64)
        .map(|i| beacon(A, 0.8 * i as f64, 16.0, i * secs_to_ns(0.05)))
        .collect();
    let (fired, ctx, _) = fired_over(Ts103759Params::default(), &msgs, &OnRoad);
    assert!(fired.contains(&Ts103759Check::BeaconFrequency));
    assert!(!ctx.on_channel("det.observation").is_empty());
}

#[test]
fn the_1_hz_profile_reproduces_f2md() {
    let p = Ts103759Params::default().with_beacon_profile(BeaconProfile::F2MD_VEREMI_1HZ);
    assert_eq!(p.min_interval_s(), 0.9);
    assert_eq!(p.time_delta_s(), 3.1);
    assert_eq!(p.sa_time_s(), 2.1);
    // The BSM and CAM profiles are the generators' own numbers.
    let bsm = v2xw_msg::BsmGenParams::j2945_1();
    let b = BeaconProfile::BSM_J2945_1;
    assert_eq!(b.min_interval_s, bsm.min_itt.as_secs_f64());
    assert_eq!(b.max_interval_s, bsm.max_itt.as_secs_f64());
    let cam = v2xw_msg::CamGenParams::default();
    let c = BeaconProfile::CAM_EN302637_2;
    assert_eq!(c.min_interval_s, cam.t_gen_cam_min.as_secs_f64());
    assert_eq!(c.max_interval_s, cam.t_gen_cam_max.as_secs_f64());
    // The tolerance covers the generator's whole hand-off jitter, and a 20 Hz gap is
    // still under the minimum it leaves.
    let jitter = v2xw_msg::GenerationTiming::NS3_TX_MAX_DELAY.as_secs_f64();
    assert!(b.jitter_tolerance_s > jitter);
    assert!(b.min_beacon_interval_s() > 0.05);
    // A 1 Hz sender is quiet under the 1 Hz profile, and 0.5 s gaps fire there.
    let slow: Vec<ObservedMessage> = (0..20u64)
        .map(|i| beacon(A, 16.0 * i as f64, 16.0, i * NS_PER_S))
        .collect();
    let (fired, _, _) = fired_over(p.clone(), &slow, &OnRoad);
    assert!(fired.is_empty());
    let fast: Vec<ObservedMessage> = (0..6u64)
        .map(|i| beacon(A, 8.0 * i as f64, 16.0, i * secs_to_ns(0.5)))
        .collect();
    let (fired, _, _) = fired_over(p, &fast, &OnRoad);
    assert!(fired.contains(&Ts103759Check::BeaconFrequency));
}

// ---------------------------------------------------------------------------------------
// The acceptance range from this engine's own link budget (f2md.md §5 P9)
// ---------------------------------------------------------------------------------------

/// The furthest distance at which `loss` is within `budget_db`, by bisection (every law
/// here is monotone in distance).
fn reach_m(loss: impl Fn(f64) -> f64, budget_db: f64) -> f64 {
    let (mut lo, mut hi) = (1.0f64, 1.0e6f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if loss(mid) <= budget_db {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// One derivation row recomputed from `v2xw-radio`: the law's line-of-sight σ and reach.
fn radio_reach(row: &ArtRow) -> (f64, f64) {
    use v2xw_world::model::EnvClass;
    let env = [
        EnvClass::Urban,
        EnvClass::Suburban,
        EnvClass::Highway,
        EnvClass::Rural,
    ]
    .into_iter()
    .find(|e| e.label() == row.environment.label())
    .expect("every environment is an EnvClass");
    let f_hz = v2xw_radio::types::ChannelId::CCH.centre_hz();
    let budget = ART_TX_POWER_DBM + 2.0 * ART_ANTENNA_GAIN_DBI - ART_SENSITIVITY_DBM;
    match row.law {
        "propagation/free-space" => (
            0.0,
            reach_m(|d| v2xw_radio::prop::friis_loss_db(d, f_hz), budget),
        ),
        "propagation/log-distance-shadowing" => {
            let ds = v2xw_radio::LogDistancePreset::for_environment(env, false).params();
            let margin = ART_MARGIN_SIGMAS * ds.sigma_db;
            (
                ds.sigma_db,
                reach_m(|d| ds.path_loss_db(d), budget + margin),
            )
        }
        "propagation/tr37885" => {
            // The engine's own LOS dispatch (`v2xw_engine::wiring::los_mean_loss_db`):
            // urban and suburban take the urban law.
            let urban = matches!(env, EnvClass::Urban | EnvClass::Suburban);
            let sigma = v2xw_radio::prop::tr37885_sigma_db(v2xw_radio::Tr37885State::Los);
            let law = |d: f64| {
                if urban {
                    v2xw_radio::prop::tr37885_urban_los_db(d, f_hz / 1e9)
                } else {
                    v2xw_radio::prop::tr37885_highway_los_db(d, f_hz / 1e9)
                }
            };
            (sigma, reach_m(law, budget + ART_MARGIN_SIGMAS * sigma))
        }
        other => panic!("no derivation for {other}"),
    }
}

#[test]
fn the_acceptance_range_is_the_engine_link_budget() {
    use v2xw_radio::types::{ActorClass, FrameDescriptor, Mcs, SduRef};
    // The inputs are the engine's radio defaults.
    let frame = FrameDescriptor::broadcast(
        300,
        Mcs::R6Qpsk12,
        SduRef::new(
            v2xw_core::ids::SduId::new(1),
            v2xw_core::ids::FrameSeq::new(1),
        ),
    );
    assert_eq!(ART_TX_POWER_DBM, frame.tx_power_dbm);
    assert_eq!(ART_ANTENNA_GAIN_DBI, ActorClass::Car.default_gain_dbi());
    assert_eq!(
        ART_SENSITIVITY_DBM,
        v2xw_radio::SensitivityPreset::MeasuredObu.sensitivity_dbm(Mcs::R6Qpsk12)
    );
    assert_eq!(
        ART_CARRIER_HZ,
        v2xw_radio::types::ChannelId::CCH.centre_hz()
    );
    // Every row, recomputed.
    for row in &ART_DERIVATION {
        let (sigma, reach) = radio_reach(row);
        assert_eq!(
            row.sigma_db,
            sigma,
            "{} {}",
            row.law,
            row.environment.label()
        );
        assert!(
            (row.reach_m - reach).abs() <= 1.0,
            "{} {}: the card says {} m, v2xw-radio gives {reach:.1} m",
            row.law,
            row.environment.label(),
            row.reach_m
        );
    }
    // And the card records it.
    let c = v2xw_threat::ts103759::card(&Ts103759Params::default());
    let art = c
        .parameters
        .iter()
        .find(|p| p.name == "max_plausible_range_m")
        .expect("the card declares the ART");
    assert_eq!(
        art.default,
        serde_json::json!(AcceptanceRanges::DERIVED.highway_m)
    );
    let note = art.source.note.as_deref().unwrap_or_default();
    assert!(
        note.contains("propagation/tr37885 (high) 12857 m"),
        "{note}"
    );
    for env in RadioEnvironment::ALL {
        let row = c
            .parameters
            .iter()
            .find(|p| p.name == format!("art_{}_m", env.label()))
            .expect("one row per environment");
        assert_eq!(
            row.default,
            serde_json::json!(AcceptanceRanges::DERIVED.get(env))
        );
    }
}

#[test]
fn an_honest_los_highway_link_never_fires_and_twice_the_range_does() {
    let p = Ts103759Params::default().with_environment(RadioEnvironment::Highway);
    let art = p.art_m();
    // A 23 dBm sender driving away at 30 m/s and 10 Hz through the last 900 m of the
    // range, every message at its true position and inside the range.
    let far: Vec<ObservedMessage> = (0..300u64)
        .map(|i| beacon(A, art - 901.0 + 3.0 * i as f64, 30.0, i * secs_to_ns(0.1)))
        .collect();
    assert!(far.iter().all(|m| m.claimed_x_m < art));
    let (fired, _, _) = fired_over(p.clone(), &far, &OnRoad);
    assert!(
        !fired.contains(&Ts103759Check::RangePlausibility),
        "an honest link inside the ART fired"
    );
    // Veins' 420 m on the same link flags it.
    let veins = Ts103759Params {
        max_plausible_range_m: Some(420.0),
        ..p.clone()
    };
    let (fired, _, _) = fired_over(veins, &far, &OnRoad);
    assert!(fired.contains(&Ts103759Check::RangePlausibility));
    // A claim at twice the range fires.
    let ghost: Vec<ObservedMessage> = (0..5u64)
        .map(|i| beacon(B, 2.0 * art + 3.0 * i as f64, 30.0, i * secs_to_ns(0.1)))
        .collect();
    let (fired, _, _) = fired_over(p, &ghost, &OnRoad);
    assert!(fired.contains(&Ts103759Check::RangePlausibility));
}

#[test]
fn class_three_fires_on_a_claim_off_every_road_the_node_knows() {
    let msgs: Vec<ObservedMessage> = (0..3u64)
        .map(|i| beacon(A, 16.0 * i as f64, 16.0, i * NS_PER_S))
        .collect();
    let (fired, _, _) = fired_over(Ts103759Params::default(), &msgs, &OffRoad(10.0));
    assert!(fired.contains(&Ts103759Check::PositionPlausibility));
    assert!(
        fired
            .iter()
            .any(|c| c.class() == ObservationClass::LocalEnvironment)
    );
}

#[test]
fn class_three_fires_on_a_certificate_from_another_region_and_is_unchecked_without_ours() {
    let extras = EnvelopeExtras::with_region(RegionId(840));
    let params = Ts103759Params {
        own_region: Some(RegionId(276)),
        ..Ts103759Params::default()
    };
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::new(params);
    let x = CrossCheckInputs {
        perception: &NoPerception,
        envelope: Some(&extras),
        frame: None,
        own: None,
    };
    let mut fired = BTreeSet::new();
    for i in 0..2u64 {
        let m = beacon(A, 16.0 * i as f64, 16.0, i * NS_PER_S);
        let v = one(&mut suite, &mut ctx, &m, &OnRoad, &x);
        assert_eq!(v.score(Ts103759Check::ForeignRegion), Some(1.5));
        for o in &v.fired {
            fired.insert(o.check);
        }
    }
    assert!(fired.contains(&Ts103759Check::ForeignRegion));
    assert_eq!(suite.skipped_no_region(), 0);

    // The same certificate, with the receiver's own region undeclared: unchecked.
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let v = one(&mut suite, &mut ctx, &beacon(A, 0.0, 16.0, 0), &OnRoad, &x);
    assert!(!v.evaluated(Ts103759Check::ForeignRegion));
    assert_eq!(suite.skipped_no_region(), 1);

    // The receiver's own region, matching: a check that ran and passed.
    let same = EnvelopeExtras::with_region(RegionId(276));
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::new(Ts103759Params {
        own_region: Some(RegionId(276)),
        ..Ts103759Params::default()
    });
    let x_same = CrossCheckInputs {
        perception: &NoPerception,
        envelope: Some(&same),
        frame: None,
        own: None,
    };
    let v = one(
        &mut suite,
        &mut ctx,
        &beacon(A, 0.0, 16.0, 0),
        &OnRoad,
        &x_same,
    );
    assert_eq!(v.score(Ts103759Check::ForeignRegion), Some(0.0));
}

#[test]
fn the_perception_cross_check_catches_a_ghost_inside_the_field_of_view() {
    // A claim 100 m straight ahead, inside a VSC-A-class radar's 150 m / ±7.5° envelope,
    // and the radar holds nothing there.
    let sensor = DiscSensor::vsc_a_flr(0.0, Vec::new());
    let x = CrossCheckInputs::with_perception(&sensor);
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let mut fired = BTreeSet::new();
    for i in 0..2u64 {
        let m = beacon(A, 100.0, 0.0, i * NS_PER_S);
        let v = one(&mut suite, &mut ctx, &m, &OnRoad, &x);
        assert_eq!(v.score(Ts103759Check::PerceptionCrossCheck), Some(1.5));
        for o in &v.fired {
            fired.insert(o.check);
        }
    }
    assert!(fired.contains(&Ts103759Check::PerceptionCrossCheck));
    assert_eq!(suite.class4_evaluated(), 2);
    assert_eq!(suite.class4_skipped_no_perception(), 0);
    assert_eq!(suite.class4_skipped_outside_coverage(), 0);
}

#[test]
fn a_corroborated_claim_does_not_fire_the_cross_check() {
    let sensor = DiscSensor::vsc_a_flr(
        0.0,
        vec![SensedObject {
            object_id: 1,
            x_m: 100.0,
            y_m: 0.0,
            speed_mps: 0.0,
            confidence: 0.9,
            at: 0,
        }],
    );
    let x = CrossCheckInputs::with_perception(&sensor);
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let v = one(&mut suite, &mut ctx, &beacon(A, 100.0, 0.0, 0), &OnRoad, &x);
    assert_eq!(v.score(Ts103759Check::PerceptionCrossCheck), Some(0.0));
    assert!(!v.fired());
}

#[test]
fn a_claim_outside_the_sensor_coverage_is_unchecked_not_passed() {
    // 200 m straight ahead is beyond a 150 m radar; 100 m abeam is outside ±7.5°. In both
    // cases the absence of a sensed object is the blind spot, not evidence — and the
    // counter is how a reader knows the check did not run.
    let sensor = DiscSensor::vsc_a_flr(0.0, Vec::new());
    let x = CrossCheckInputs::with_perception(&sensor);
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();

    let far = beacon(A, 200.0, 0.0, 0);
    let v = one(&mut suite, &mut ctx, &far, &OnRoad, &x);
    assert!(!v.evaluated(Ts103759Check::PerceptionCrossCheck));

    let mut abeam = beacon(B, 0.0, 0.0, NS_PER_S);
    abeam.claimed_y_m = 100.0;
    let v = one(&mut suite, &mut ctx, &abeam, &OnRoad, &x);
    assert!(!v.evaluated(Ts103759Check::PerceptionCrossCheck));

    assert_eq!(suite.class4_skipped_outside_coverage(), 2);
    assert_eq!(suite.class4_evaluated(), 0);
    assert_eq!(suite.class4_skipped_no_perception(), 0);
}

#[test]
fn class_five_fires_when_two_stations_claim_the_same_space() {
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let x = CrossCheckInputs::none();
    // A stakes out a point; B claims half a metre away, twice.
    one(&mut suite, &mut ctx, &beacon(A, 100.0, 0.0, 0), &OnRoad, &x);
    let mut fired = BTreeSet::new();
    for i in 1..3u64 {
        let m = beacon(B, 100.5, 0.0, i * secs_to_ns(0.5));
        let v = one(&mut suite, &mut ctx, &m, &OnRoad, &x);
        assert_eq!(v.score(Ts103759Check::Intersection), Some(2.0 / 0.5));
        for o in &v.fired {
            fired.insert(o.check);
        }
    }
    assert!(fired.contains(&Ts103759Check::Intersection));
    assert!(
        fired
            .iter()
            .any(|c| c.class() == ObservationClass::OtherStations)
    );
}

#[test]
fn a_collective_perception_message_nobody_can_corroborate_fires_class_five() {
    let sensor = DiscSensor::vsc_a_flr(0.0, Vec::new());
    let x = CrossCheckInputs::with_perception(&sensor);
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let mut fired = BTreeSet::new();
    for i in 0..2u64 {
        let mut m = beacon(A, 20.0, 0.0, i * NS_PER_S);
        m.kind = ObservedKind::Cpm(vec![
            PerceivedObject::from_metres(1, 60.0, 0.0, 10.0, 15),
            PerceivedObject::from_metres(2, 90.0, 0.0, 10.0, 15),
        ]);
        let v = one(&mut suite, &mut ctx, &m, &OnRoad, &x);
        assert_eq!(v.score(Ts103759Check::CpmConsistency), Some(1.0));
        for o in &v.fired {
            fired.insert(o.check);
        }
    }
    assert!(fired.contains(&Ts103759Check::CpmConsistency));
}

#[test]
fn a_collective_perception_message_about_what_we_cannot_see_is_not_evidence() {
    // Objects beyond our own radar are exactly what collective perception is for.
    let sensor = DiscSensor::vsc_a_flr(0.0, Vec::new());
    let x = CrossCheckInputs::with_perception(&sensor);
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let mut m = beacon(A, 20.0, 0.0, 0);
    m.kind = ObservedKind::Cpm(vec![PerceivedObject::from_metres(1, 400.0, 0.0, 10.0, 15)]);
    let v = one(&mut suite, &mut ctx, &m, &OnRoad, &x);
    assert!(
        !v.evaluated(Ts103759Check::CpmConsistency),
        "an object outside our coverage was counted as uncorroborated"
    );
}

#[test]
fn a_collective_perception_message_we_corroborate_does_not_fire() {
    let sensor = DiscSensor::vsc_a_flr(
        0.0,
        vec![SensedObject {
            object_id: 1,
            x_m: 60.0,
            y_m: 0.0,
            speed_mps: 10.0,
            confidence: 0.9,
            at: 0,
        }],
    );
    let x = CrossCheckInputs::with_perception(&sensor);
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let mut m = beacon(A, 20.0, 0.0, 0);
    m.kind = ObservedKind::Cpm(vec![PerceivedObject::from_metres(1, 60.0, 0.0, 10.0, 15)]);
    let v = one(&mut suite, &mut ctx, &m, &OnRoad, &x);
    assert_eq!(v.score(Ts103759Check::CpmConsistency), Some(0.0));
}

// ---------------------------------------------------------------------------------------
// Across the suite
// ---------------------------------------------------------------------------------------

#[test]
fn every_class_has_been_observed_to_go_red() {
    // The discipline: a check nobody has shown can fire is a check nobody has tested.
    // Each of the five classes is exercised by one of the cases above; this collects them.
    let mut covered: BTreeSet<ObservationClass> = BTreeSet::new();

    let speeding: Vec<ObservedMessage> = (0..3u64)
        .map(|i| beacon(A, 10.0 * i as f64, 60.0, i * NS_PER_S))
        .collect();
    let (f, _, _) = fired_over(Ts103759Params::default(), &speeding, &OnRoad);
    covered.extend(f.iter().map(|c| c.class()));

    let jumping: Vec<ObservedMessage> = (0..3u64)
        .map(|i| beacon(A, 120.0 * i as f64, 16.0, i * NS_PER_S))
        .collect();
    let (f, _, _) = fired_over(Ts103759Params::default(), &jumping, &OnRoad);
    covered.extend(f.iter().map(|c| c.class()));

    let (f, _, _) = fired_over(Ts103759Params::default(), &jumping, &OffRoad(10.0));
    covered.extend(f.iter().map(|c| c.class()));

    // Class 4, with a radar that sees nothing where the claim says a vehicle is.
    let sensor = DiscSensor::vsc_a_flr(0.0, Vec::new());
    let x = CrossCheckInputs::with_perception(&sensor);
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    for i in 0..2u64 {
        let v = one(
            &mut suite,
            &mut ctx,
            &beacon(A, 100.0, 0.0, i * NS_PER_S),
            &OnRoad,
            &x,
        );
        covered.extend(v.fired.iter().map(|o| o.check.class()));
    }

    // Class 5, two stations in one place.
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let none = CrossCheckInputs::none();
    one(
        &mut suite,
        &mut ctx,
        &beacon(A, 100.0, 0.0, 0),
        &OnRoad,
        &none,
    );
    for i in 1..3u64 {
        let v = one(
            &mut suite,
            &mut ctx,
            &beacon(B, 100.5, 0.0, i * secs_to_ns(0.5)),
            &OnRoad,
            &none,
        );
        covered.extend(v.fired.iter().map(|o| o.check.class()));
    }

    for class in [
        ObservationClass::ImplausibleValues,
        ObservationClass::PreviousMessages,
        ObservationClass::LocalEnvironment,
        ObservationClass::OnBoardSensors,
        ObservationClass::OtherStations,
    ] {
        assert!(
            covered.contains(&class),
            "class {} was never observed to fire",
            class.number()
        );
    }
}

#[test]
fn a_fired_check_reaches_the_channel_the_metrics_crate_reads() {
    let msgs: Vec<ObservedMessage> = (0..3u64)
        .map(|i| beacon(A, 120.0 * i as f64, 16.0, i * NS_PER_S))
        .collect();
    let (_, ctx, _) = fired_over(Ts103759Params::default(), &msgs, &OnRoad);
    let records = ctx.on_channel("det.observation");
    assert!(!records.is_empty());
    assert!(ctx.on_channel("gt.attack.action").is_empty(), "I-T2");
}

#[test]
fn a_report_can_be_built_from_a_verdict_of_this_suite() {
    // The `Detector` trait's Verdict is typed on the legacy DetectorId enum, so this suite
    // reports through `from_named_checks` — same wire shape, its own check ids.
    let msgs: Vec<ObservedMessage> = (0..3u64)
        .map(|i| beacon(A, 120.0 * i as f64, 16.0, i * NS_PER_S))
        .collect();
    let mut ctx = CollectingCtx::new(1);
    let mut suite = Ts103759Suite::cited_defaults();
    let mut last = TsVerdict::default();
    for m in &msgs {
        last = one(&mut suite, &mut ctx, m, &OnRoad, &CrossCheckInputs::none());
    }
    assert!(last.fired());
    let reasons: Vec<(String, f64)> = last
        .fired
        .iter()
        .map(|o| (o.check.as_str().to_string(), o.score))
        .collect();
    let report = v2xw_threat::MisbehaviourReport::from_named_checks(
        "rpt_ts_00001",
        RX,
        "ffee",
        last.subject.as_str(),
        &reasons,
        &last.columns(),
        &v2xw_threat::Evidence::at(2 * NS_PER_S, 2 * NS_PER_S, 2.0),
    )
    .expect("something fired");
    assert_eq!(report.leading_reason(), Some(reasons[0].0.as_str()));
    assert!(report.detector_score >= 1.0);
    assert_eq!(report.subject_cert_digest, last.subject);
    assert!(!report.detnorm.is_empty());
    // Only the checks that RAN are columns: a suite that padded the row with zeros would
    // be reporting checks it never performed.
    assert!(report.detnorm.len() <= Ts103759Check::ALL.len());
    assert!(
        !report
            .detnorm
            .iter()
            .any(|(id, _)| id == Ts103759Check::PerceptionCrossCheck.as_str()),
        "a check that could not run must not appear as a column"
    );
}
