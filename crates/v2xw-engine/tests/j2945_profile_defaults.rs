//! The US profile's defaults from SAE J2945/1: the receiver's `generationTime` relevance
//! window per profile (Table 11, 30 s each
//! way for a BSM), and the region's pseudonym-change rule when `strategy` is
//! left at `by-region` (§6.5.3 `CERTCHG`).

use v2xw_core::time::{NS_PER_S, WallClock};
use v2xw_engine::Scenario;
use v2xw_engine::credentials::{CredentialFactory, ReceiverLimits};
use v2xw_msg::MsgType;
use v2xw_node::rxval::{GenerationTimeWindow, RxReject};
use v2xw_node::stores::RotationPolicy;

const ETSI: &str = "etsi103097";

/// The window a device of `s` ends up with: the scenario's limits through the credential
/// factory into the trust store, the path every vehicle of a run takes.
fn device_window(s: &Scenario) -> GenerationTimeWindow {
    let mut sec =
        v2xw_node::NodeSecurity::new(WallClock::new(0), v2xw_node::CryptoMode::Modeled, 0x20);
    CredentialFactory::for_run(1, WallClock::new(0), None)
        .with_receiver_limits(ReceiverLimits::of(s))
        .install_trust(&mut sec);
    sec.trust().generation_time_window()
}

fn etsi() -> Scenario {
    let mut s = Scenario::minimal();
    s.security.envelope = ETSI.to_string();
    s
}

/// Under the US IEEE 1609.2 profile a BSM 20 s old or 20 s ahead is relevant and one 31 s
/// either way is not (J2945/1 Table 11: validity period and acceptable future data period,
/// 30 s); other messages keep 600 s. An ETSI CAM is still limited to 2 s and 40 ms
/// (RS_BSP_168), and a key that is set still overrides the profile.
///
/// Fault: the C2C-CC window for the US profile -> red.
#[test]
fn the_us_bsm_window_is_thirty_seconds_each_way() {
    let now = 10_000 * NS_PER_S;
    let us = device_window(&Scenario::minimal());
    assert_eq!(us, GenerationTimeWindow::j2945_1());
    assert_eq!(us.admits(MsgType::Bsm, now - 20 * NS_PER_S, now), Ok(()));
    assert_eq!(us.admits(MsgType::Bsm, now + 20 * NS_PER_S, now), Ok(()));
    assert_eq!(
        us.admits(MsgType::Bsm, now - 31 * NS_PER_S, now),
        Err(RxReject::GenerationTimeTooOld)
    );
    assert_eq!(
        us.admits(MsgType::Bsm, now + 31 * NS_PER_S, now),
        Err(RxReject::GenerationTimeInFuture)
    );
    assert_eq!(us.admits(MsgType::Spat, now - 599 * NS_PER_S, now), Ok(()));
    assert_eq!(
        us.admits(MsgType::Spat, now - 601 * NS_PER_S, now),
        Err(RxReject::GenerationTimeTooOld)
    );

    let eu = device_window(&etsi());
    assert_eq!(eu, GenerationTimeWindow::c2c_cc());
    assert_eq!(
        eu,
        GenerationTimeWindow::default(),
        "ETSI is the node's own default"
    );
    assert_eq!(
        eu.admits(MsgType::Cam, now - 3 * NS_PER_S, now),
        Err(RxReject::GenerationTimeTooOld)
    );
    assert_eq!(
        eu.admits(MsgType::Cam, now + 41_000_000, now),
        Err(RxReject::GenerationTimeInFuture)
    );
    assert_eq!(eu.admits(MsgType::Cam, now + 39_000_000, now), Ok(()));

    // The three keys still override the profile's values.
    let mut set = Scenario::minimal();
    let mut scms = set.security.scms.clone().unwrap_or_default();
    scms.rx_beacon_max_age_s = Some(2.0);
    scms.rx_max_future_s = Some(0.5);
    set.security.scms = Some(scms);
    assert_eq!(
        device_window(&set),
        GenerationTimeWindow::from_secs(2.0, 600.0, 0.5)
    );
}

/// A replay 10 s old passes the US receiver's relevance check, so it is verified and
/// reaches the detectors (J2945/1 leaves duplicates inside the validity period to the
/// application, Table 11 "Relevance: Replay False"); under ETSI the same replay is
/// discarded unverified.
///
/// Fault: the 2 s window for the US profile -> red.
#[test]
fn a_replay_ten_seconds_old_passes_the_us_relevance_check() {
    let now = 600 * NS_PER_S;
    let replayed = now - 10 * NS_PER_S;
    assert_eq!(
        device_window(&Scenario::minimal()).admits(MsgType::Bsm, replayed, now),
        Ok(()),
        "a 10 s replay was discarded before the misbehaviour detectors under the US profile"
    );
    assert_eq!(
        device_window(&etsi()).admits(MsgType::Cam, replayed, now),
        Err(RxReject::GenerationTimeTooOld)
    );
}

/// With `strategy` left at its default (`by-region`), a US scenario changes
/// pseudonym under SAE J2945/1 `CERTCHG` (5 min and 2,100 m straight-line), whether built
/// in code or loaded from a document that says nothing; an ETSI one, or one that sets
/// `period_s`, keeps the time rule.
///
/// Fault: `effective_strategy` returns `time` for `by-region` -> red.
#[test]
fn a_us_scms_scenario_defaults_to_the_j2945_rule() {
    use v2xw_engine::wiring::rotation_policy;
    let s = Scenario::minimal();
    assert_eq!(s.security.pseudonym_change.strategy, "by-region");
    assert_eq!(rotation_policy(&s), RotationPolicy::J2945_1);
    assert_eq!(RotationPolicy::J2945_1.min_distance_m, 2100.0);
    let errors: Vec<String> = v2xw_engine::scenario::validate::validate(&s)
        .iter()
        .map(|e| format!("{e:?}"))
        .filter(|e| e.contains("pseudonym_change"))
        .collect();
    assert!(errors.is_empty(), "{errors:?}");

    // A document with no pseudonym_change block at all.
    let mut doc = serde_json::to_value(Scenario::minimal()).expect("serialises");
    doc["security"]
        .as_object_mut()
        .expect("security")
        .remove("pseudonym_change");
    let loaded = Scenario::from_document(doc).expect("loads");
    assert_eq!(rotation_policy(&loaded), RotationPolicy::J2945_1);

    let five_min = v2xw_core::time::Duration::from_secs(300);
    let eu = rotation_policy(&etsi());
    assert_eq!(eu.min_age, five_min);
    assert!(eu.is_due(five_min, 0.0), "ETSI keeps the time rule");

    let mut timed = Scenario::minimal();
    timed.security.pseudonym_change.period_s = Some(60.0);
    let t = rotation_policy(&timed);
    assert_eq!(t.min_age, v2xw_core::time::Duration::from_secs(60));
    assert!(t.is_due(t.min_age, 0.0), "a period_s means the time rule");
}
