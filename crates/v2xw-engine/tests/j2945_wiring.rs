//! SAE J2945/1 wiring: each J2945/1 setting reaches the model it names.

use v2xw_engine::Scenario;
use v2xw_engine::scenario::schema::{GnssQualitySpec, GnssSettings};
use v2xw_mobility::gnss::GnssQuality;

/// `actors.vehicles.gnss.quality: j2945-1` builds the J2945/1 open-sky receiver (the
/// class, its speed and heading noise and the heading latch), unset keeps the measured
/// receiver, and a set `velocity_noise_mps` still wins over the class's speed noise.
#[test]
fn the_j2945_gnss_quality_reaches_the_model() {
    let mut s = Scenario::minimal();
    let reid = v2xw_engine::wiring::gnss_params(&s);
    assert_eq!(
        reid,
        GnssQuality::Reid2019.params(),
        "unset is the measured receiver"
    );

    s.actors.vehicles.gnss = Some(GnssSettings {
        quality: Some(GnssQualitySpec::J2945_1),
        ..GnssSettings::default()
    });
    let j = v2xw_engine::wiring::gnss_params(&s);
    assert_eq!(
        j,
        GnssQuality::J2945_1.params(),
        "j2945-1 is the J2945/1 class"
    );
    assert!(j.is_j2945() && j.heading_latch_mps > 0.0);

    s.actors.vehicles.gnss = Some(GnssSettings {
        quality: Some(GnssQualitySpec::J2945_1),
        velocity_noise_mps: Some(0.05),
        ..GnssSettings::default()
    });
    let v = v2xw_engine::wiring::gnss_params(&s);
    assert_eq!(v.sigma_velocity_mps, 0.05, "a set key wins");
    assert!(v.is_j2945(), "and the class stays");

    s.actors.vehicles.gnss = Some(GnssSettings {
        quality: Some(GnssQualitySpec::Rtk),
        ..GnssSettings::default()
    });
    assert!(v2xw_engine::wiring::gnss_params(&s).is_rtk());
}

/// `messages.bsm.path_history_on_change` reaches each node's configuration; unset
/// leaves the node its own standard's (a BSM sender keeps the history, an ETSI CAM station
/// clears it; `v2xw_node::NodeConfig::path_history_on_change`).
#[test]
fn the_path_history_rule_at_a_change_reaches_the_node() {
    use v2xw_engine::scenario::schema::{BsmSettings, PathHistoryOnChangeSpec};
    use v2xw_msg::j2945::PathHistoryOnChange;
    let mut s = Scenario::minimal();
    assert_eq!(v2xw_engine::wiring::path_history_on_change(&s), None);
    for (spec, want) in [
        (PathHistoryOnChangeSpec::Keep, PathHistoryOnChange::Keep),
        (PathHistoryOnChangeSpec::Clear, PathHistoryOnChange::Clear),
    ] {
        s.messages.bsm = Some(BsmSettings {
            path_history_on_change: Some(spec),
            ..Default::default()
        });
        assert_eq!(v2xw_engine::wiring::path_history_on_change(&s), Some(want));
    }
    s.messages.bsm = Some(BsmSettings::default());
    assert_eq!(v2xw_engine::wiring::path_history_on_change(&s), None);
}
