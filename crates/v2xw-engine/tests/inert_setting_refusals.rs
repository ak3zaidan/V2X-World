//! Settings that change nothing in the context they are set in. A setting that cannot act is refused at
//! load with a message that names it, as the loader does for the radio channel's
//! "would change nothing" rules; each test also shows the refusal lifts once the setting
//! can act, so the rule is discriminating and not merely loud.

use v2xw_engine::scenario::{Scenario, validate};

fn fields(text: &str) -> Vec<String> {
    let s: Scenario = serde_yml::from_str(text).expect("the document deserialises");
    validate(&s)
        .iter()
        .filter_map(|e| e.field().map(str::to_string))
        .collect()
}

const HEAD: &str = r#"
schema: v2xw/scenario/1
world:
  source: {kind: procedural, generator: world/source/procedural-grid, params: {}}
"#;

/// `traffic.impatience_gap_floor` 0.3 on a run without impatience gave the
/// base's record digest bit for bit, because impatience is 0 unless
/// `traffic.time_to_impatience_s` is set.
#[test]
fn an_impatience_floor_without_impatience_is_refused() {
    let off = fields(&format!("{HEAD}traffic: {{impatience_gap_floor: 0.3}}\n"));
    assert!(
        off.iter().any(|f| f == "traffic.impatience_gap_floor"),
        "the floor loads although no driver can grow impatient: {off:?}"
    );
    let on = fields(&format!(
        "{HEAD}traffic: {{impatience_gap_floor: 0.3, time_to_impatience_s: 180}}\n"
    ));
    assert!(
        !on.iter().any(|f| f == "traffic.impatience_gap_floor"),
        "the floor is refused although impatience is on: {on:?}"
    );
}

/// `messages.codec_tier: size-model` on BSMs gave the base's record digest,
/// because the size model stands in only for the sets with no real encoder.
#[test]
fn a_size_model_tier_with_nothing_to_size_is_refused() {
    let bsm = fields(&format!(
        "{HEAD}messages: {{sets: [bsm], codec_tier: size-model}}\n"
    ));
    assert!(
        bsm.iter().any(|f| f == "messages.codec_tier"),
        "size-model loads on BSMs alone, where it sizes nothing: {bsm:?}"
    );
    let srm = fields(&format!(
        "{HEAD}messages: {{sets: [bsm, srm], codec_tier: size-model}}\n"
    ));
    assert!(
        !srm.iter().any(|f| f == "messages.codec_tier"),
        "size-model is refused although an SRM needs it: {srm:?}"
    );
    let uper = fields(&format!(
        "{HEAD}messages: {{sets: [bsm], codec_tier: uper}}\n"
    ));
    assert!(!uper.iter().any(|f| f == "messages.codec_tier"));
}

/// The PSM has a real UPER encoder (`v2xw_msg::j2735::psm`, oracle-validated) and a VRU
/// device always sends its bytes, so a `psm` listed as a set is refused for what it is (a
/// device's message, not a set) and not also for a missing encoder; and since the size
/// model never stands in for it, `size-model` beside it sizes nothing and is refused as on
/// BSMs alone.
#[test]
fn a_listed_psm_is_refused_as_a_device_message_not_for_a_missing_encoder() {
    let messages = |text: &str| -> Vec<String> {
        let s: Scenario = serde_yml::from_str(text).expect("the document deserialises");
        validate(&s).iter().map(|e| e.to_string()).collect()
    };
    let uper = messages(&format!(
        "{HEAD}messages: {{sets: [bsm, psm], codec_tier: uper}}\n"
    ));
    let on_psm: Vec<&String> = uper
        .iter()
        .filter(|m| m.starts_with("messages.sets[1]"))
        .collect();
    assert_eq!(
        on_psm.len(),
        1,
        "the listed psm should get one refusal, the device one: {on_psm:#?}"
    );
    assert!(
        on_psm[0].contains("vulnerable road users' devices"),
        "{on_psm:#?}"
    );
    assert!(
        !uper.iter().any(|m| m.contains("no real UPER encoder")),
        "the PSM is said to lack the encoder it has: {uper:#?}"
    );
    let srm = messages(&format!(
        "{HEAD}messages: {{sets: [bsm, srm], codec_tier: uper}}\n"
    ));
    assert!(
        srm.iter()
            .any(|m| m.starts_with("messages.sets[1]") && m.contains("no real UPER encoder")),
        "an SRM under 'uper' is no longer refused: {srm:#?}"
    );
    let sized = fields(&format!(
        "{HEAD}messages: {{sets: [bsm, psm], codec_tier: size-model}}\n"
    ));
    assert!(
        sized.iter().any(|f| f == "messages.codec_tier"),
        "size-model loads beside a psm, which it never sizes: {sized:?}"
    );
}

/// The same for `traffic.impatience_foe_braking: true`.
#[test]
fn foe_braking_without_impatience_is_refused() {
    let off = fields(&format!(
        "{HEAD}traffic: {{impatience_foe_braking: true}}\n"
    ));
    assert!(
        off.iter().any(|f| f == "traffic.impatience_foe_braking"),
        "foe braking loads although no driver can grow impatient: {off:?}"
    );
    let on = fields(&format!(
        "{HEAD}traffic: {{impatience_foe_braking: true, time_to_impatience_s: 60}}\n"
    ));
    assert!(
        !on.iter().any(|f| f == "traffic.impatience_foe_braking"),
        "foe braking is refused although impatience is on: {on:?}"
    );
    // writing the default changes nothing either way and stays accepted
    let default = fields(&format!(
        "{HEAD}traffic: {{impatience_foe_braking: false}}\n"
    ));
    assert!(
        !default
            .iter()
            .any(|f| f == "traffic.impatience_foe_braking")
    );
}

/// `radio.phy.energy_detect_dbm: -95` on one channel with no jammer gave the
/// base's record digest: nothing on that air is energy from elsewhere.
#[test]
fn energy_detect_with_no_foreign_energy_is_refused() {
    let off = fields(&format!(
        "{HEAD}radio: {{phy: {{energy_detect_dbm: -70.0}}}}\n"
    ));
    assert!(
        off.iter().any(|f| f == "radio.phy.energy_detect_dbm"),
        "energy detect loads although nothing on the air can reach it: {off:?}"
    );
    let on = fields(&format!(
        "{HEAD}radio: {{phy: {{energy_detect_dbm: -70.0}}}}\nthreats: {{jammers: [{{id: threat/jammer/constant, params: {{power_dbm: 20.0, position: [0.0, 0.0, 2.0]}}}}]}}\n"
    ));
    assert!(
        !on.iter().any(|f| f == "radio.phy.energy_detect_dbm"),
        "energy detect is refused although a jammer is on the air: {on:?}"
    );
}

/// `scale.radio.follows` turns the outside into abstract propagation; under a high PHY the
/// document it writes does not load, so the run's own tools could not read it.
#[test]
fn a_high_phy_under_a_following_radio_is_refused() {
    const SCALE: &str = "scale:\n  traffic:\n    model: mobility/tiered/meso-micro\n    region: {min_lat_deg: 34.7, min_lon_deg: -86.6, max_lat_deg: 34.8, max_lon_deg: -86.5}\n  radio: {follows: true}\n";
    let high = fields(&format!(
        "{HEAD}{SCALE}radio: {{tiers: {{phy: high, mac: high, propagation: high}}}}\n"
    ));
    assert!(
        high.iter().any(|f| f == "radio.tiers.phy"),
        "a high PHY under a following radio loads: {high:?}"
    );
    let medium = fields(&format!("{HEAD}{SCALE}"));
    assert!(
        !medium.iter().any(|f| f == "radio.tiers.phy"),
        "the medium PHY is refused too: {medium:?}"
    );
}

/// `radio.devices.obu.radios: 2` on one channel gave the base's record digest.
#[test]
fn a_second_radio_with_one_channel_is_refused() {
    let off = fields(&format!(
        "{HEAD}radio: {{devices: {{obu: {{radios: 2}}}}}}\n"
    ));
    assert!(
        off.iter().any(|f| f == "radio.devices.obu.radios"),
        "two radios load although the run has one channel: {off:?}"
    );
    let on = fields(&format!(
        "{HEAD}radio: {{devices: {{obu: {{radios: 2}}}}, channels: [{{channel: 174, messages: [spat]}}]}}\nmessages: {{sets: [bsm, spat]}}\n"
    ));
    assert!(
        !on.iter().any(|f| f == "radio.devices.obu.radios"),
        "two radios are refused although the run has a service channel: {on:?}"
    );
}

/// `net.geonet.range_m: 100` on the air gave the base's record digest.
#[test]
fn a_geonet_overlay_range_on_the_air_is_refused() {
    const GN: &str = "net: {layer: gn-btp, geonet: {forwarding: on-air, range_m: 100.0}}\n";
    let air = fields(&format!("{HEAD}{GN}"));
    assert!(
        air.iter().any(|f| f == "net.geonet.range_m"),
        "the overlay's range loads on the air: {air:?}"
    );
    let overlay = fields(&format!("{HEAD}{}", GN.replace("on-air", "overlay")));
    assert!(
        !overlay.iter().any(|f| f == "net.geonet.range_m"),
        "the range is refused under the overlay: {overlay:?}"
    );
}

/// `net.geonet.non_area_forwarding: cbf` under the default `cbf_variant` gave a
/// bit-identical run, because only the standard's variant forwards a GBC broadcast received
/// outside its area (`out_of_area_broadcast_forward`).
#[test]
fn non_area_cbf_under_a_variant_that_drops_the_packet_is_refused() {
    let off = fields(&format!(
        "{HEAD}net: {{geonet: {{non_area_forwarding: cbf}}}}\n"
    ));
    assert!(
        off.iter().any(|f| f == "net.geonet.non_area_forwarding"),
        "the setting loads although no copy can take its path: {off:?}"
    );
    let on = fields(&format!(
        "{HEAD}net: {{geonet: {{non_area_forwarding: cbf, cbf_variant: etsi}}}}\n"
    ));
    assert!(
        !on.iter().any(|f| f == "net.geonet.non_area_forwarding"),
        "the setting is refused although the variant takes its path: {on:?}"
    );
}
