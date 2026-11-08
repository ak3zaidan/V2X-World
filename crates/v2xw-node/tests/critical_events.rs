//! The critical event flags of SAE
//! J2945/1 §3.1.2 (hard braking, ABS, traction control, stability control), the ABS flag
//! from the vehicle bus, and the full certificate a critical BSM carries (§6.5.2
//! `BSMSIGN-004`). The no-change rule (`CERTCHG-002`) is tested beside the store,
//! `stores::tests::no_change_while_a_critical_flag_is_set`.

use v2xw_core::time::{NS_PER_MS, SimTime, WallClock};
use v2xw_msg::MsgType;
use v2xw_msg::j2735::bsm::VehicleEventFlags;
use v2xw_node::secure::{self, CryptoMode, NodeSecurity, PSID_SAFETY};
use v2xw_node::vehicle::{OwnVehicle, VehicleBus};
use v2xw_sec::envelope::SignerIdChoice;

fn bus(t_s: f64, a_long_mps2: f64, abs_active: bool) -> VehicleBus {
    VehicleBus {
        t: (t_s * 1e9) as SimTime,
        speed_mps: 10.0,
        a_long_mps2,
        a_lat_mps2: 0.0,
        yaw_rate_rad_s: 0.0,
        hazard_lights: false,
        low_beam: false,
        intent: None,
        heavy: false,
        speed_limit_mps: 13.4,
        non_urban: None,
        abs_active,
    }
}

/// The ABS regulating on a slippery road sets `eventABSactivated`, a critical flag, at a
/// deceleration under the hard-braking threshold.
///
/// Fault: `events()` omits the ABS line -> red.
#[test]
fn abs_activity_sets_the_abs_flag() {
    let mut v = OwnVehicle::default();
    v.set_bus(bus(1.0, -2.5, true));
    let e = v.events(1_000_000_000).expect("the ABS sets an event flag");
    assert!(e.contains(VehicleEventFlags::ABS_ACTIVATED), "{e:?}");
    assert!(!e.contains(VehicleEventFlags::HARD_BRAKING), "{e:?}");
    assert!(secure::is_critical_event(Some(e)));
    v.set_bus(bus(2.0, -2.5, false));
    assert!(!secure::is_critical_event(v.events(2_000_000_000)));
}

/// The critical set is the four flags of §3.1.2 and no other.
#[test]
fn the_critical_flags_are_the_four_of_j2945_1() {
    for f in [
        VehicleEventFlags::HARD_BRAKING,
        VehicleEventFlags::ABS_ACTIVATED,
        VehicleEventFlags::TRACTION_CONTROL_LOSS,
        VehicleEventFlags::STABILITY_CONTROL_ACTIVATED,
    ] {
        assert!(secure::is_critical_event(Some(f)), "{f:?}");
    }
    for f in [
        VehicleEventFlags::HAZARD_LIGHTS,
        VehicleEventFlags::LIGHTS_CHANGED,
        VehicleEventFlags::DISABLED_VEHICLE,
    ] {
        assert!(!secure::is_critical_event(Some(f)), "{f:?}");
    }
    assert!(!secure::is_critical_event(None));
}

/// A critical BSM 100 ms after the last full certificate carries the certificate again
/// (`BSMSIGN-004`), where the 450 ms cadence alone gives a digest. It does not restart the
/// cadence: the next certificate is still due 450 ms after the last scheduled one. A CAM
/// is not a BSM and keeps its cadence.
///
/// Fault: `signer_id_for_event` ignores `critical` -> a digest, red.
#[test]
fn a_critical_bsm_carries_the_full_certificate() {
    let mut s = NodeSecurity::new(
        WallClock::new(1_767_225_600),
        CryptoMode::Modeled,
        PSID_SAFETY,
    );
    let t0: SimTime = 10_000 * NS_PER_MS;
    assert_eq!(
        s.signer_id_for_event(MsgType::Bsm, t0, false),
        SignerIdChoice::Certificate,
        "the first BSM carries its certificate"
    );
    s.note_certificate_attached_for_event(MsgType::Bsm, t0, false);

    let t1 = t0 + 100 * NS_PER_MS;
    assert_eq!(
        s.signer_id_for_event(MsgType::Bsm, t1, false),
        SignerIdChoice::Digest,
        "the 450 ms cadence alone gives a digest at 100 ms"
    );
    assert_eq!(
        s.signer_id_for_event(MsgType::Bsm, t1, true),
        SignerIdChoice::Certificate,
        "a critical BSM must carry the full certificate"
    );
    s.note_certificate_attached_for_event(MsgType::Bsm, t1, true);
    // The cadence still counts from t0, not from the critical attachment.
    let t2 = t0 + 450 * NS_PER_MS;
    assert_eq!(
        s.signer_id_for_event(MsgType::Bsm, t2, false),
        SignerIdChoice::Certificate,
        "the critical certificate restarted the 450 ms cadence"
    );
    // A CAM is outside the BSM rule.
    s.note_certificate_attached_for_event(MsgType::Cam, t0, false);
    assert_eq!(
        s.signer_id_for_event(MsgType::Cam, t1, true),
        SignerIdChoice::Digest
    );
}

/// A BSM that J2945/1's congestion control sends for a critical event carries the critical
/// event flag, and so the entire certificate: the transmission decision's critical event
/// condition (§6.3.8.5) is the event a critical event flag stands for (§3.1.3), the BSM
/// sent for it sets that flag (§6.3.6.15, `BSMTX-DATAACC-034`) and a BSM with the flag
/// set attaches the certificate, not its digest (§6.5.2, `BSMSIGN-004`). For a light
/// vehicle braking at every deceleration the bus reads (to the mm/s²) from 3.800 to
/// 4.100 m/s², the congestion control raises a critical event exactly where the bus sets
/// the hard-braking flag, and each such BSM, 100 ms after a full certificate (a digest by
/// the 450 ms cadence), is signed with the certificate.
///
/// Fault: `J2945Params::J2945_1.critical_decel_mps2` back at 0.4 × 9.8 -> red at 3.921
/// m/s², a critical-event BSM with no flag and a digest signer.
#[test]
fn a_bsm_sent_for_a_critical_event_carries_its_flag_and_its_certificate() {
    use v2xw_core::ids::NodeId;
    use v2xw_radio::{HostState, J2945Trigger, SaeJ2945Dcc};
    let t0: SimTime = 10_000 * NS_PER_MS;
    let t1 = t0 + 100 * NS_PER_MS;
    let mut dcc = SaeJ2945Dcc::new();
    let mut critical_seen = 0;
    for k in 0..=300u32 {
        let decel = f64::from(3_800 + k) / 1_000.0;
        // A unit that has sent no BSM yet: no tracking error, so only the critical event
        // can raise anything.
        let host = HostState {
            x: 0.0,
            y: 0.0,
            speed_mps: 10.0,
            heading_rad: 0.0,
            at: t1,
        };
        let raised = dcc.on_tracking(NodeId::new(k), host, decel, 0.999);
        let mut v = OwnVehicle::default();
        v.set_bus(bus(10.1, -decel, false));
        let flagged = secure::is_critical_event(v.events(t1));
        assert_eq!(
            raised == Some(J2945Trigger::CriticalEvent),
            flagged,
            "at {decel:.3} m/s² the congestion control raised {raised:?} and the bus flags \
             {:?}",
            v.events(t1)
        );
        let mut s = NodeSecurity::new(
            WallClock::new(1_767_225_600),
            CryptoMode::Modeled,
            PSID_SAFETY,
        );
        s.note_certificate_attached_for_event(MsgType::Bsm, t0, false);
        let signer = s.signer_id_for_event(MsgType::Bsm, t1, flagged);
        if raised == Some(J2945Trigger::CriticalEvent) {
            critical_seen += 1;
            assert_eq!(
                signer,
                SignerIdChoice::Certificate,
                "the critical-event BSM at {decel:.3} m/s² went with a digest"
            );
        } else {
            assert_eq!(signer, SignerIdChoice::Digest, "at {decel:.3} m/s²");
        }
    }
    // 3.923 to 4.100 m/s²: above 0.4 g in standard gravity, 3.922 66 m/s².
    assert_eq!(critical_seen, 178);
}
